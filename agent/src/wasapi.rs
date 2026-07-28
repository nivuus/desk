//! Capture du son que joue la machine, par WASAPI en mode loopback.
//!
//! Le périphérique visé est le **rendu par défaut** de la session : on capte ce
//! qui sortirait des haut-parleurs, quelle que soit l'application qui le
//! produit.
//!
//! **Sondage, pas événement.** `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` n'est pas
//! supporté en combinaison avec `AUDCLNT_STREAMFLAGS_LOOPBACK` : Microsoft
//! documente la capture loopback comme devant être pilotée par minuterie. Un
//! flux de rendu inactif ne signalerait d'ailleurs aucun événement, ce qui est
//! le cas fréquent ici — rien ne joue la plupart du temps. Le sondage est donc
//! la seule forme correcte, et il sert directement le complément de silence de
//! `frames.rs`.

#![cfg(windows)]

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::{implement, Interface, Ref};
use windows::Win32::Media::Audio::{
    eConsole, eRender, ActivateAudioInterfaceAsync, IAudioCaptureClient, IAudioClient,
    IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, BLOB,
};
use windows::Win32::System::Variant::VT_BLOB;

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};

/// Durée du tampon demandé à WASAPI, en unités de 100 ns. 200 ms : large
/// marge pour absorber un tour de boucle en retard sans jamais perdre
/// d'échantillon.
const BUFFER_DURATION_100NS: i64 = 2_000_000;

/// Étiquette `WAVE_FORMAT_EXTENSIBLE` du champ `wFormatTag`.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Étiquette `WAVE_FORMAT_IEEE_FLOAT`.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// Garde RAII pour le pointeur rendu par `IAudioClient::GetMixFormat`.
///
/// Ce pointeur est alloué par COM via `CoTaskMemAlloc` (documentation de
/// `GetMixFormat`) : l'appelant doit le libérer par `CoTaskMemFree`, ce que
/// ce garde fait dans son `Drop`. `IAudioClient::Initialize` copie le format
/// en interne, donc libérer *après* son appel est toujours correct — y
/// compris à la sortie normale de `open()`, où ce garde n'est libéré qu'en
/// toute fin de fonction.
///
/// L'intérêt d'un garde plutôt qu'une libération explicite : entre
/// `GetMixFormat` et `Initialize`, `open()` peut sortir en erreur par deux
/// `bail!` (fréquence ou largeur de format refusée) — exactement les
/// chemins qu'on emprunte le jour où la VM change de configuration audio.
/// Une libération posée seulement en fin de fonction heureuse les
/// manquerait ; un garde RAII les couvre par construction, quel que soit le
/// chemin de sortie (`return`, `?`, `bail!`).
struct FormatMixage(*mut WAVEFORMATEX);

impl std::ops::Deref for FormatMixage {
    type Target = WAVEFORMATEX;
    fn deref(&self) -> &WAVEFORMATEX {
        // SAFETY : le pointeur vient d'un `GetMixFormat` réussi et n'est
        // libéré que dans `Drop`, donc valide pour toute la durée de vie du
        // garde.
        unsafe { &*self.0 }
    }
}

impl Drop for FormatMixage {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

pub struct LoopbackCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    canaux: usize,
    flottant: bool,
    description: String,
}

// SÉCURITÉ : `LoopbackCapture` enveloppe des interfaces COM (`IAudioClient`,
// `IAudioCaptureClient`) que `windows-core` ne marque pas `Send` par défaut —
// un objet COM générique peut être lié à un appartement mono-thread (STA), et
// le déplacer vers un autre fil serait alors un comportement indéfini. Ce
// n'est pas le cas ici : `open()` (ci-dessous) *vérifie*, plutôt que de
// supposer, que le fil appelant rejoint l'appartement multi-thread (MTA) via
// `CoInitializeEx(None, COINIT_MULTITHREADED)`, et refuse d'ouvrir si ce fil
// appartient déjà à une autre apartement (`RPC_E_CHANGED_MODE`).
// `WindowsAudioSource::new` (agent/src/windows_audio.rs) déplace ensuite cet
// objet, par `move`, vers un fil de capture dédié qui rejoint à son tour
// cette même MTA avant tout appel COM (voir son commentaire). Un objet créé
// dans une MTA est par construction appelable depuis n'importe quel fil qui
// en est membre, sans marshaling — c'est cette propriété, garantie par la
// vérification d'`open()`, qui rend le transfert sûr.
//
// Cette promesse porte sur le **struct entier, champs futurs compris** : si
// un futur champ ajoute un `HANDLE` d'événement, un pointeur brut, ou tout
// autre état lié à un fil précis plutôt qu'à l'apartement, cet `unsafe impl`
// cesserait d'être valide sans que rien ne le signale. Quiconque ajoute un
// champ à `LoopbackCapture` doit vérifier qu'il reste utilisable depuis
// n'importe quel fil membre de la MTA avant de le faire — sans quoi ce
// `Send` doit être retiré ou restreint.
//
// Alternative écartée : `windows_core::AgileReference<T>`, le mécanisme
// officiellement prévu par `windows-core` 0.62 pour transporter un objet COM
// entre fils sans supposer son modèle de threading. Non retenu ici : il exige
// une résolution (`resolve()`, un `QueryInterface` interne) à chaque
// récupération, un coût et une complexité inutiles alors que ce process n'a,
// sous ce plan, aucune STA — la vérification d'`open()` suffit et reste bon
// marché.
unsafe impl Send for LoopbackCapture {}

impl LoopbackCapture {
    /// Ouvre le loopback sur le périphérique de rendu par défaut et démarre la
    /// capture.
    pub fn open() -> Result<Self> {
        unsafe {
            // `CoInitializeEx` doit être **vérifié**, pas ignoré : c'est la
            // précondition dont dépend `unsafe impl Send for LoopbackCapture`
            // ci-dessus (lire son commentaire d'abord si ce n'est pas fait).
            // `S_OK` (ce fil vient de rejoindre la MTA) et `S_FALSE` (il en
            // était déjà membre) sont tous deux acceptables : dans les deux
            // cas, ce fil est membre de l'appartement multi-thread — le même
            // que rejoindra le fil de capture de `windows_audio.rs`. Seul
            // `RPC_E_CHANGED_MODE` — ce fil appartient déjà à un autre
            // appartement, typiquement une STA liée par un appel antérieur à
            // `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` sur ce même fil
            // — doit faire échouer l'ouverture : sans ce refus,
            // `LoopbackCapture` migrerait d'une STA vers la MTA du fil de
            // capture sans marshaling, un comportement indéfini qu'aucun test
            // ne révèle puisque l'appel par vtable directe « marche » la
            // plupart du temps même quand c'est interdit.
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr == RPC_E_CHANGED_MODE {
                bail!(
                    "ouverture du loopback audio refusée : le fil appelant appartient déjà \
                     à un appartement à thread unique (STA), pas à l'appartement \
                     multi-thread (MTA) qu'exige `LoopbackCapture`. `WindowsAudioSource::new` \
                     (agent/src/windows_audio.rs) déplace cet objet, par `move`, vers un fil \
                     de capture dédié qui rejoint la MTA : migrer un objet COM d'une STA vers \
                     un autre appartement sans marshaling est un comportement indéfini, pas \
                     seulement une erreur de type. Vérifiez qu'aucun \
                     `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` (ni aucune autre \
                     initialisation qui lie ce fil à une STA, par exemple une init WinRT \
                     implicite) n'a précédé cet appel sur ce même fil."
                );
            }

            // Pas de `CoUninitialize` en regard, et c'est délibéré : ce fil
            // n'est pas forcément celui qui utilisera ni celui qui libérera
            // l'objet rendu. La tâche 6 (`windows_audio.rs`) appelle `open()`
            // sur le fil appelant de `WindowsAudioSource::new()`, puis
            // déplace le `LoopbackCapture` obtenu par `move` vers un fil de
            // capture dédié — c'est CE fil-là qui appelle `read()` en boucle
            // et qui exécute `Drop` en sortant. Appeler `CoUninitialize` dans
            // `Drop` s'exécuterait donc sur un fil différent de celui qui a
            // appelé `CoInitializeEx`, ce que COM interdit explicitement.
            // Conséquence acceptée : si le fil appelant de `open()` est
            // recyclé entre sessions (fil d'un pool, par ex. les workers
            // bloquants de tokio), son compte de références COM croît d'une
            // unité par session — jamais celui du fil de capture, qui lui
            // n'est jamais recyclé (créé et détruit une fois par session). Un
            // futur rééquilibrage devra se faire là où l'appel est
            // réellement possédé : autour du fil de `WindowsAudioSource::new`,
            // pas ici.

            let enumerateur: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .context("création de l'énumérateur de périphériques audio")?;
            let peripherique = enumerateur
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .context("aucun périphérique de rendu audio par défaut")?;
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio")?;

            let mix = FormatMixage(
                client
                    .GetMixFormat()
                    .context("lecture du format de mixage")?,
            );
            let canaux = mix.nChannels as usize;
            let frequence = mix.nSamplesPerSec;
            let bits = mix.wBitsPerSample;

            let flottant = if mix.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // WAVEFORMATEXTENSIBLE est repr(packed) : prendre une
                // référence sur SubFormat — ce que fait `==` sur un GUID —
                // est un accès non aligné, donc un comportement indéfini.
                let ext = mix.0 as *const WAVEFORMATEXTENSIBLE;
                let sous_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sous_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                mix.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            };

            let description = format!(
                "{frequence} Hz, {canaux} canaux, {bits} bits, {}",
                if flottant { "flottant" } else { "entier" }
            );

            if frequence != SAMPLE_RATE_HZ {
                bail!(
                    "format de mixage à {frequence} Hz : seul {SAMPLE_RATE_HZ} Hz est supporté \
                     (aucun rééchantillonneur n'est embarqué)"
                );
            }
            if !flottant && bits != 16 {
                bail!("format de mixage entier {bits} bits non supporté ({description})");
            }
            if flottant && bits != 32 {
                bail!("format de mixage flottant {bits} bits non supporté ({description})");
            }

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    BUFFER_DURATION_100NS,
                    0,
                    mix.0,
                    None,
                )
                .context("initialisation du client audio en loopback")?;
            // `mix` (le garde `FormatMixage`) sort de portée en fin de bloc
            // `unsafe` et libère alors le format par `CoTaskMemFree` — après
            // `Initialize`, qui en a fait sa propre copie interne, comme
            // l'exige la documentation de `GetMixFormat`.

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("obtention du service de capture")?;
            client.Start().context("démarrage de la capture")?;

            Ok(Self {
                client,
                capture,
                canaux,
                flottant,
                description,
            })
        }
    }

    /// Format réellement obtenu, pour le journal et la sonde.
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// Lit le paquet disponible suivant, converti en entiers 16 bits
    /// entrelacés stéréo. Rend `None` quand rien n'est disponible — le cas
    /// courant quand aucune application ne joue.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let dispo = self
                .capture
                .GetNextPacketSize()
                .context("interrogation du paquet suivant")?;
            if dispo == 0 {
                return Ok(None);
            }

            let mut donnees: *mut u8 = std::ptr::null_mut();
            let mut images: u32 = 0;
            let mut drapeaux: u32 = 0;
            self.capture
                .GetBuffer(&mut donnees, &mut images, &mut drapeaux, None, None)
                .context("lecture du tampon de capture")?;

            let muet = drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            let sortie = if muet {
                vec![0i16; images as usize * CHANNELS]
            } else {
                let brut = images as usize * self.canaux;
                if self.flottant {
                    let source = std::slice::from_raw_parts(donnees as *const f32, brut);
                    convertir_flottant(source, self.canaux)
                } else {
                    let source = std::slice::from_raw_parts(donnees as *const i16, brut);
                    convertir_entier(source, self.canaux)
                }
            };

            self.capture
                .ReleaseBuffer(images)
                .context("libération du tampon de capture")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

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
/// `LoopbackCapture` plus haut dans ce fichier — s'y référer pour le
/// raisonnement complet). Ce n'est pas un problème ici : `probe_process_loopback`
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

/// Sonde le *process loopback* (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
/// Windows 10 build 19041+), qui isole l'audio d'un seul processus (et de ses
/// enfants) — exactement ce qu'exige le modèle multi-fenêtres du chantier D
/// (une fenêtre Windows = une fenêtre navigateur, donc potentiellement une
/// piste audio par fenêtre plutôt qu'un unique loopback global). Rend une
/// courte description en cas de succès ; l'échec (activation refusée, délai
/// dépassé) est rendu comme erreur, jamais comme panique.
pub fn probe_process_loopback(pid: u32) -> Result<String> {
    unsafe {
        // Même garde-fou que `LoopbackCapture::open` : voir son commentaire
        // pour le raisonnement complet sur `RPC_E_CHANGED_MODE`.
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr == RPC_E_CHANGED_MODE {
            bail!(
                "sonde process loopback refusée : le fil appelant appartient déjà à une \
                 STA, pas à la MTA qu'exige cette API (voir LoopbackCapture::open pour le \
                 même garde-fou)"
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
            Some(ResultatActivation(Ok(_client))) => Ok(format!(
                "activation réussie : IAudioClient obtenu pour le PID {pid} \
                 (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
            )),
            Some(ResultatActivation(Err(e))) => Err(e).context("activation refusée"),
            None => bail!(
                "aucun rappel d'activation reçu en {DELAI_RAPPEL_ACTIVATION:?} pour le PID {pid}"
            ),
        }
    }
}

/// Convertit des échantillons flottants en entiers 16 bits stéréo entrelacés.
///
/// Mono : le canal est dupliqué. Plus de deux canaux : **troncature**, pas
/// sous-mixage — seuls les deux premiers canaux (gauche et droite d'un flux
/// multicanal, par convention WAVE_FORMAT_EXTENSIBLE) sont conservés tels
/// quels ; l'énergie des canaux surround n'est mélangée dans aucun des deux,
/// elle est simplement ignorée. Sans conséquence en pratique : le format
/// réel de cette VM est déjà stéréo.
fn convertir_flottant(source: &[f32], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(vers_i16(g));
        sortie.push(vers_i16(d));
    }
    sortie
}

/// Même conversion, pour une source déjà en entiers 16 bits.
fn convertir_entier(source: &[i16], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(g);
        sortie.push(d);
    }
    sortie
}

/// Flottant normalisé → entier 16 bits, avec écrêtage explicite.
///
/// WASAPI ne garantit pas que les échantillons restent dans [-1, 1] : un
/// mixage de plusieurs flux peut dépasser. Le cast `as` sature déjà
/// nativement depuis Rust 1.45 (une valeur hors bornes est ramenée à
/// `i16::MIN`/`i16::MAX`, jamais enroulée) : le `clamp` explicite ne sert
/// donc pas à éviter un dépassement silencieux, mais à fixer la borne haute
/// exactement sur `i16::MAX` — le cast seul, sans clamp, saturerait vers le
/// bas jusqu'à `i16::MIN`, un LSB plus loin que `-i16::MAX` — et à rendre
/// l'intention explicite plutôt que de reposer sur ce détail de sémantique
/// de `as`.
fn vers_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}
