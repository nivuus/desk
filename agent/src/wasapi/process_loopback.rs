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
    IAudioClient, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, BLOB, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_BLOB;

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
