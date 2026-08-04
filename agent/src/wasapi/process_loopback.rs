//! Le *process loopback* : capter l'audio d'un seul processus, et de son arbre.
//!
//! **Extrait de `wasapi.rs` et non ajouté dedans.** Ce fichier-là est à
//! 543 lignes, `#[cfg(windows)]`, sans aucun test : c'est de la dette gelée au
//! sens de `CLAUDE.md`, et la règle du dépôt veut qu'une addition
//! substantielle s'y accompagne d'une extraction. Toute la machinerie COM
//! asynchrone d'`ActivateAudioInterfaceAsync` vit donc ici, où le code du
//! sous-bloc D7 a sa place.
//!
//! **Ce que cette API a de particulier** : elle n'est pas synchrone. Le
//! résultat n'arrive pas en retour d'appel mais par
//! `IActivateAudioInterfaceCompletionHandler::ActivateCompleted`, invoqué
//! depuis un fil du pool COM — d'où l'état partagé et la `Condvar` ci-dessous.

#![cfg(windows)]

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX, WAVE_FORMAT_PCM,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, BLOB, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_BLOB;

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};

// ---------------------------------------------------------------------------
// Sonde n°4 de la spec du chantier A (§11) : le *process loopback*.
//
// Répond à une question du chantier D, pas de celui-ci : rien n'est construit
// sur cette sonde ici, elle observe seulement si l'activation réussit sur
// cette VM, et le résultat est consigné pour le chantier D (modèle
// multi-fenêtres, qui a besoin d'isoler l'audio par fenêtre/processus).
// ---------------------------------------------------------------------------

/// État partagé entre le fil appelant de `probe_process_loopback` et le
/// rappel COM de `ActivateAudioInterfaceAsync`, qui s'exécute sur un fil du
/// pool de threads COM — pas forcément celui qui a lancé l'appel.
struct EtatActivation {
    resultat: Mutex<Option<ResultatActivation>>,
    signal: Condvar,
}

/// Résultat de l'activation, tel que déposé par le rappel.
///
/// SÉCURITÉ : `IAudioClient` n'est pas `Send` par défaut (même motif que
/// `LoopbackCapture`, dans le module PARENT `agent/src/wasapi.rs` — s'y
/// référer pour le raisonnement complet, ce fichier-ci n'étant qu'une
/// extraction de celui-là). Ce n'est pas un problème ici : `activer_pour_processus`
/// (appelée par `probe_process_loopback` ET par `CaptureProcessus::ouvrir`)
/// rejoint la MTA avant d'appeler `ActivateAudioInterfaceAsync`, et son
/// rappel de complétion s'exécute nécessairement sur un fil qui est lui-même
/// membre de cette MTA (c'est ce que documente Microsoft pour cette API).
/// Faire transiter cette valeur vers le fil appelant, une fois le rappel
/// signalé via la `Condvar` ci-dessous, ne viole donc aucune contrainte
/// d'appartement COM.
struct ResultatActivation(Result<IAudioClient>);
unsafe impl Send for ResultatActivation {}

/// Gestionnaire de complétion COM pour `ActivateAudioInterfaceAsync`.
///
/// Cette API est **asynchrone à rappel** : le résultat n'arrive pas en retour
/// d'appel mais via `ActivateCompleted`, invoqué depuis un fil du pool COM.
/// On dépose le résultat dans l'état partagé et on réveille le fil appelant.
#[implement(IActivateAudioInterfaceCompletionHandler)]
struct GestionnaireCompletion {
    etat: Arc<EtatActivation>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for GestionnaireCompletion_Impl {
    fn ActivateCompleted(
        &self,
        activateoperation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let resultat: Result<IAudioClient> = (|| {
            let operation = activateoperation
                .ok()
                .context("le rappel d'activation n'a rendu aucune opération")?;
            let mut hr = windows::core::HRESULT::default();
            let mut interface: Option<windows::core::IUnknown> = None;
            unsafe { operation.GetActivateResult(&mut hr, &mut interface) }
                .context("GetActivateResult")?;
            hr.ok().context("activation du process loopback refusée")?;
            interface
                .context("GetActivateResult a réussi sans rendre d'interface")?
                .cast::<IAudioClient>()
                .context("l'interface activée n'est pas un IAudioClient")
        })();

        let mut verrou = self
            .etat
            .resultat
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *verrou = Some(ResultatActivation(resultat));
        self.etat.signal.notify_one();
        Ok(())
    }
}

/// Délai maximal d'attente du rappel d'activation.
const DELAI_RAPPEL_ACTIVATION: Duration = Duration::from_secs(10);

/// Active un `IAudioClient` de *process loopback* pour `pid` et l'attend.
///
/// `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` (Windows 10 build 19041+)
/// isole l'audio d'un seul processus (et de ses enfants) — exactement ce
/// qu'exige le modèle multi-fenêtres du chantier D (une fenêtre Windows = une
/// fenêtre navigateur, donc potentiellement une piste audio par fenêtre
/// plutôt qu'un unique loopback global).
///
/// **Extrait de `probe_process_loopback`, qui l'appelle désormais** (tâche 2
/// du sous-bloc D7) : la sonde et `CaptureProcessus::ouvrir` doivent activer
/// **exactement de la même façon**, sans quoi la sonde ne mesurerait pas ce
/// que le produit fait.
fn activer_pour_processus(pid: u32) -> Result<IAudioClient> {
    unsafe {
        // Même garde-fou que `LoopbackCapture::open` (module PARENT
        // `agent/src/wasapi.rs`) : voir son commentaire pour le raisonnement
        // complet sur `RPC_E_CHANGED_MODE`.
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr == RPC_E_CHANGED_MODE {
            bail!(
                "activation du process loopback refusée : le fil appelant appartient déjà à une \
                 STA, pas à la MTA qu'exige cette API (voir `LoopbackCapture::open` dans \
                 `agent/src/wasapi.rs` pour le même garde-fou)"
            );
        }

        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    // INCLUDE et non EXCLUDE : c'est bien l'audio DE ce
                    // processus (et de ses enfants) qu'on veut isoler, pas
                    // celui de tout le reste de la machine.
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };

        // `ActivateAudioInterfaceAsync` attend ses paramètres sous la forme
        // d'un PROPVARIANT de type VT_BLOB portant un pointeur brut vers
        // `params` — le pendant Rust du `PropVariantInit` puis affectation
        // manuelle des champs `vt`/`blob` en C++. `params` doit rester vivant
        // jusqu'à la fin de l'appel synchrone (seule l'ACTIVATION elle-même
        // est asynchrone, la lecture des paramètres ne l'est pas) : les deux
        // valeurs restent dans cette même portée `unsafe`.
        // `Anonymous` (le premier niveau) est un champ `ManuallyDrop<...>`
        // d'union COM : l'auto-déréférencement implicite de `ManuallyDrop`
        // n'est pas appliqué sur un champ d'union par le compilateur (il
        // faudrait sinon appeler le destructeur de l'ancienne valeur active,
        // indéterminée) — d'où le `*` explicite.
        //
        // CORRECTIF (revue) : `PROPVARIANT` implémente `Drop`
        // (`windows-0.62.2/src/extensions/Win32/System/StructuredStorage.rs`)
        // et appelle `PropVariantClear` — qui, pour `VT_BLOB`, relâche
        // `blob.pBlobData` via `CoTaskMemFree`. Or `pBlobData` pointe ici sur
        // `params`, une variable de PILE, pas une allocation `CoTaskMemAlloc` :
        // laisser ce `Drop` s'exécuter (sur TOUT chemin de sortie, y compris le
        // `?` d'`ActivateAudioInterfaceAsync` juste en dessous) appelle
        // `CoTaskMemFree` sur une adresse de pile — un comportement indéfini
        // franc, seule cause plausible de la corruption qui rendait la sonde
        // silencieuse (aucune ligne de log, aucun rapport de plantage
        // cohérent avec le point d'échec). `ManuallyDrop` empêche ce `Drop` :
        // rien n'a besoin d'être libéré, `blob` ne référence aucune mémoire
        // dont ce PROPVARIANT est propriétaire.
        let mut propriete = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        (*propriete.Anonymous.Anonymous).vt = VT_BLOB;
        (*propriete.Anonymous.Anonymous).Anonymous.blob = BLOB {
            cbSize: std::mem::size_of_val(&params) as u32,
            pBlobData: &mut params as *mut AUDIOCLIENT_ACTIVATION_PARAMS as *mut u8,
        };

        let etat = Arc::new(EtatActivation {
            resultat: Mutex::new(None),
            signal: Condvar::new(),
        });
        let gestionnaire: IActivateAudioInterfaceCompletionHandler = GestionnaireCompletion {
            etat: etat.clone(),
        }
        .into();

        // L'opération rendue doit rester en vie jusqu'à la fin de l'attente :
        // la laisser tomber prématurément peut annuler l'activation en cours.
        let _operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*propriete),
            &gestionnaire,
        )
        .context("appel à ActivateAudioInterfaceAsync")?;

        let verrou = etat
            .resultat
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (mut verrou, _attente) = etat
            .signal
            .wait_timeout_while(verrou, DELAI_RAPPEL_ACTIVATION, |r| r.is_none())
            .unwrap_or_else(|e| e.into_inner());

        match verrou.take() {
            Some(ResultatActivation(Ok(client))) => Ok(client),
            Some(ResultatActivation(Err(e))) => Err(e).context("activation refusée"),
            None => bail!(
                "aucun rappel d'activation reçu en {DELAI_RAPPEL_ACTIVATION:?} pour le PID {pid}"
            ),
        }
    }
}

/// Sonde d'ACTIVATION seule — conservée telle quelle pour que le relevé du
/// chantier A (28 juillet 2026) reste reproductible à l'identique. Elle
/// n'initialise rien et ne lit aucun octet : c'est `CaptureProcessus::ouvrir`
/// qui va plus loin, et c'est `PROCESS_LOOPBACK_CAPTURE` qui le mesure.
pub fn probe_process_loopback(pid: u32) -> Result<String> {
    let _client = activer_pour_processus(pid)?;
    Ok(format!(
        "activation réussie : IAudioClient obtenu pour le PID {pid} \
         (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
    ))
}

/// Durée du tampon demandé, en unités de 100 ns. 200 ms, comme
/// `LoopbackCapture::open` : large marge pour absorber un tour de boucle en
/// retard sans perdre d'échantillon.
const DUREE_TAMPON_100NS: i64 = 2_000_000;

/// Capture loopback d'un seul processus et de son arbre.
///
/// **Le format n'est pas demandé, il est IMPOSÉ.** Un client de *process
/// loopback* n'est lié à aucun point de terminaison : `GetMixFormat` n'y a pas
/// de sens évident, et l'échantillon officiel de Microsoft pose lui aussi un
/// format explicite. On pose donc exactement celui qu'`opus.rs` attend
/// (48 kHz, 2 canaux, 16 bits entiers), ce qui supprime du même coup toute
/// conversion : `read` rend des `i16` entrelacés directement exploitables par
/// `FrameAssembler`.
///
/// ⚠️ **Que Windows accepte ce format n'est pas établi avant la mesure de la
/// tâche 3.** S'il refuse, le `HRESULT` exact est le relevé qui compte — ne pas
/// deviner un repli.
pub struct CaptureProcessus {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    description: String,
    /// Vrai quand `Start()` a été appelé sans `Stop()` depuis. **Nécessaire** :
    /// `IAudioClient::Start` sur un flux déjà démarré rend
    /// `AUDCLNT_E_NOT_STOPPED`, et l'arbitrage peut réémettre un ordre
    /// identique après un rattachement de canal.
    demarre: bool,
}

// SÉCURITÉ : même raisonnement que `unsafe impl Send for LoopbackCapture`
// (module PARENT `agent/src/wasapi.rs`, dont il faut lire le commentaire
// d'abord). `activer_pour_processus` vérifie que le fil appelant est membre
// de la MTA et refuse `RPC_E_CHANGED_MODE` ; le fil de capture de
// `windows_audio.rs` rejoint cette même MTA avant tout appel COM.
// **La promesse porte sur le struct entier, champs futurs compris.**
unsafe impl Send for CaptureProcessus {}

impl CaptureProcessus {
    pub fn ouvrir(pid: u32) -> Result<Self> {
        let client = activer_pour_processus(pid)?;
        unsafe {
            let mut format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: SAMPLE_RATE_HZ,
                wBitsPerSample: 16,
                nBlockAlign: (CHANNELS as u16) * 2,
                nAvgBytesPerSec: SAMPLE_RATE_HZ * (CHANNELS as u32) * 2,
                cbSize: 0,
            };
            format.nBlockAlign = format.nChannels * format.wBitsPerSample / 8;
            format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign as u32;

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    DUREE_TAMPON_100NS,
                    0,
                    &format,
                    None,
                )
                .context(
                    "Initialize du client de process loopback (format impose : 48 kHz, \
                     2 canaux, 16 bits)",
                )?;

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("GetService(IAudioCaptureClient) sur le client de process loopback")?;

            // `WAVEFORMATEX` est `repr(packed)` (même motif que
            // `WAVEFORMATEXTENSIBLE` dans le module parent) : y prendre une
            // référence — ce que fait `format!` pour tout argument — est un
            // accès non aligné, donc un comportement indéfini. On copie
            // d'abord les champs vers des variables de pile ordinaires.
            let frequence = format.nSamplesPerSec;
            let canaux = format.nChannels;
            let description = format!(
                "process loopback pid={pid} — {frequence} Hz, {canaux} canaux, \
                 16 bits entiers"
            );

            Ok(Self {
                client,
                capture,
                description,
                demarre: false,
            })
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Démarre le flux. Idempotent : un second appel ne fait rien.
    pub fn demarrer(&mut self) -> Result<()> {
        if self.demarre {
            return Ok(());
        }
        unsafe { self.client.Start() }.context("Start du client de process loopback")?;
        self.demarre = true;
        Ok(())
    }

    /// Arrête le flux. Idempotent.
    ///
    /// **Ce n'est pas une destruction** : le client reste activé, donc aucune
    /// réactivation COM — la seule étape qui puisse refuser — n'a lieu à la
    /// bascule suivante. C'est tout l'intérêt de l'approche retenue au §4.4 de
    /// la spec.
    pub fn arreter(&mut self) -> Result<()> {
        if !self.demarre {
            return Ok(());
        }
        unsafe { self.client.Stop() }.context("Stop du client de process loopback")?;
        self.demarre = false;
        Ok(())
    }

    /// Lit un paquet, ou `None` s'il n'y en a aucun de prêt.
    ///
    /// Rend des `i16` entrelacés, sans conversion : le format est imposé à
    /// l'ouverture.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let disponibles = self
                .capture
                .GetNextPacketSize()
                .context("GetNextPacketSize sur le process loopback")?;
            if disponibles == 0 {
                return Ok(None);
            }

            let mut donnees: *mut u8 = std::ptr::null_mut();
            let mut images = 0u32;
            let mut drapeaux = 0u32;
            self.capture
                .GetBuffer(&mut donnees, &mut images, &mut drapeaux, None, None)
                .context("GetBuffer sur le process loopback")?;

            let echantillons = images as usize * CHANNELS;
            let sortie = if drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                // Le drapeau SILENT autorise le pilote à ne pas remplir le
                // tampon : lire ses octets rendrait n'importe quoi.
                vec![0i16; echantillons]
            } else {
                std::slice::from_raw_parts(donnees as *const i16, echantillons).to_vec()
            };

            self.capture
                .ReleaseBuffer(images)
                .context("ReleaseBuffer sur le process loopback")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for CaptureProcessus {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
