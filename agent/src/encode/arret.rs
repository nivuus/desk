//! Mise au repos des MFT d'un encodeur avant leur relâchement.
//!
//! Séparé d'`encode.rs` (déjà en dette de taille, voir `CLAUDE.md`) plutôt
//! qu'ajouté dedans.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncCallback_Impl, IMFAsyncResult, IMFRealTimeClientEx, IMFShutdown,
    IMFTransform, MFAllocateSerialWorkQueue, MFPutWorkItem, MFUnlockWorkQueue,
    MFASYNC_CALLBACK_QUEUE_MULTITHREADED, MFSHUTDOWN_COMPLETED, MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    MFT_MESSAGE_NOTIFY_END_STREAMING, MFT_MESSAGE_TYPE,
};

/// Garde-fou de l'attente d'arrêt d'une MFT.
///
/// L'attente porte sur une CONDITION OBSERVABLE
/// (`IMFShutdown::GetShutdownStatus`), pas sur une durée : ce délai n'est là
/// que pour ne pas bloquer indéfiniment la fermeture d'une fenêtre si une MFT
/// ne confirmait jamais son arrêt. Le franchir est journalisé — ce n'est pas
/// un chemin silencieux.
const DELAI_ARRET_MFT: Duration = Duration::from_secs(2);

/// Met au repos les deux MFT d'un encodeur, dans l'ordre, avant que leurs
/// références COM ne soient relâchées.
///
/// **Pourquoi cette séquence existe** — relevé de la tâche 2bis (deux
/// plantages, piles identiques, symbolisées) : la faute est levée par
/// `RtlEnterCriticalSection` dans du code de `nvEncMFTH264x.dll` exécuté sous
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem`, c'est-à-dire **sur un fil de
/// la file de travail Media Foundation**, sur une section critique dont le
/// `DebugInfo` vaut `NULL` — pendant que le fil principal est dans
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown`. La MFT a donc
/// encore un élément de travail en vol quand nous détruisons l'encodeur.
///
/// La position du fil principal, elle, s'est révélée fortuite : retirer
/// `MFShutdown` du chemin n'a pas empêché la faute (voir
/// `super::demarrer_media_foundation`). Ce qui la traite est la barrière de
/// `FileEncodeur`, plus bas.
///
/// L'amont (le convertisseur) est mis au repos avant l'aval (l'encodeur) : il
/// ne doit plus rien produire pendant qu'on arrête celui qui consomme.
///
/// Chaque étape se journalise en `debug` (donc muette en exploitation, où
/// `RUST_LOG=info`) : ces appels peuvent bloquer sans rendre la main, et la
/// dernière ligne écrite est alors le seul moyen de savoir lequel. Elles ne
/// s'exécutent qu'à la destruction d'un encodeur, jamais par trame.
pub(super) fn mettre_au_repos(
    convertisseur: &IMFTransform,
    encodeur: &IMFTransform,
    file: &FileEncodeur,
) {
    // Fin de flux : inchangé, c'est ce que faisait déjà `Drop`.
    message(convertisseur, "convertisseur", "END_OF_STREAM", MFT_MESSAGE_NOTIFY_END_OF_STREAM);
    message(convertisseur, "convertisseur", "END_STREAMING", MFT_MESSAGE_NOTIFY_END_STREAMING);
    message(encodeur, "encodeur", "END_OF_STREAM", MFT_MESSAGE_NOTIFY_END_OF_STREAM);
    message(encodeur, "encodeur", "END_STREAMING", MFT_MESSAGE_NOTIFY_END_STREAMING);

    // DEUX MESSAGES DÉLIBÉRÉMENT ABSENTS, et ce n'est pas un oubli.
    //
    // `MFT_MESSAGE_COMMAND_FLUSH` et `MFT_MESSAGE_SET_D3D_MANAGER` à zéro
    // (deux pistes du brief) ont été posés ici, puis retirés sur relevé : avec
    // eux, à N = 4 encodeurs, **une exécution sur quatre s'est bloquée sans
    // retour** dans ce bloc, juste après que l'encodeur n°0 a confirmé son
    // arrêt et pendant la destruction du n°1 (journal arrêté sur « libération
    // d'un encodeur : avant id=1 », processus encore vivant treize minutes
    // plus tard). Le blocage est **borné à ce bloc** : la trace suivante n'a
    // jamais été écrite. Les deux fins de flux ci-dessus, elles, précèdent ce
    // chantier et n'ont jamais bloqué. Restaient donc ces deux messages-là.
    //
    // Sans eux : 22 exécutions à N = 4, aucun blocage.
    //
    // Ce qui n'est PAS établi : lequel des deux bloquait, ni pourquoi. On sait
    // seulement que le blocage est dans ce bloc et qu'il a disparu avec eux —
    // un blocage à la fermeture d'une fenêtre serait pire que le plantage
    // qu'on corrige.

    // Barrière : plus rien de ce qui était déjà en file ne court encore.
    file.barriere("après END_STREAMING");

    // Arrêt explicite, avec attente BORNÉE sur une condition OBSERVABLE.
    arreter(convertisseur, "convertisseur");
    arreter(encodeur, "encodeur");

    // Seconde barrière : l'arrêt lui-même peut avoir déposé du travail.
    file.barriere("après IMFShutdown");
}

/// Envoie un message à une MFT en encadrant l'appel de deux traces : un appel
/// qui ne rend pas la main se lit alors dans le journal.
fn message(mft: &IMFTransform, quoi: &'static str, nom: &'static str, message: MFT_MESSAGE_TYPE) {
    tracing::debug!(mft = quoi, message = nom, "mise au repos : avant");
    let issue = unsafe { mft.ProcessMessage(message, 0) };
    tracing::debug!(mft = quoi, message = nom, refuse = issue.is_err(), "mise au repos : après");
}

/// Demande à une MFT d'arrêter ses files de travail, et attend qu'elle le
/// confirme.
///
/// Relâcher nos références COM ne suffit pas à faire disparaître les fils
/// d'une MFT matérielle asynchrone. `IMFShutdown::Shutdown` est le mécanisme
/// documenté par lequel un client de MFT obtient cet arrêt — c'est ce que fait
/// le pipeline Media Foundation lui-même, via `MFShutdownObject`, quand il
/// démonte un nœud de topologie. On l'appelle ici directement plutôt que par
/// `MFShutdownObject` pour deux raisons : savoir, et pouvoir journaliser, si
/// la MFT expose seulement cette interface (`MFShutdownObject` rend `S_OK`
/// sans rien dire quand elle l'ignore), et pouvoir attendre la confirmation
/// par `GetShutdownStatus`.
///
/// Ce que cette fonction n'établit pas : rien ici ne prouve QUI détruit la
/// section critique de la trace ci-dessus, ni quand. Voir le rapport 2ter.
fn arreter(mft: &IMFTransform, quoi: &'static str) {
    let arret: IMFShutdown = match mft.cast() {
        Ok(arret) => arret,
        Err(err) => {
            // Relevé, pas supposé : si l'interface manque, le journal le dit,
            // et l'on sait que ce chemin n'a rien arrêté du tout.
            tracing::debug!(
                mft = quoi,
                erreur = %err,
                "MFT sans IMFShutdown : aucun arrêt explicite possible"
            );
            return;
        }
    };

    tracing::debug!(mft = quoi, "IMFShutdown::Shutdown : avant");
    if let Err(err) = unsafe { arret.Shutdown() } {
        tracing::warn!(mft = quoi, erreur = %err, "IMFShutdown::Shutdown refusé");
        return;
    }

    let debut = Instant::now();
    loop {
        match unsafe { arret.GetShutdownStatus() } {
            Ok(statut) if statut == MFSHUTDOWN_COMPLETED => {
                tracing::info!(
                    mft = quoi,
                    attente_ms = debut.elapsed().as_millis() as u64,
                    "arrêt de la MFT confirmé"
                );
                return;
            }
            // `MFSHUTDOWN_INITIATED` : l'arrêt court encore, on repasse.
            Ok(_) => {}
            Err(err) => {
                // La méthode est facultative en pratique ; on ne peut alors
                // pas confirmer, et on le dit au lieu de le taire.
                tracing::info!(
                    mft = quoi,
                    erreur = %err,
                    "GetShutdownStatus indisponible : arrêt demandé mais non confirmable"
                );
                return;
            }
        }
        if debut.elapsed() >= DELAI_ARRET_MFT {
            tracing::warn!(
                mft = quoi,
                delai_ms = DELAI_ARRET_MFT.as_millis() as u64,
                "arrêt de la MFT non confirmé dans le délai"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Délai maximal d'attente d'une barrière de file de travail.
///
/// La barrière porte sur une CONDITION OBSERVABLE (notre propre élément de
/// travail a-t-il été exécuté), pas sur une durée. Ce délai n'est qu'un
/// garde-fou, et le franchir est journalisé.
const DELAI_BARRIERE: Duration = Duration::from_secs(2);

/// File de travail Media Foundation **sérialisée** dédiée à un encodeur, et
/// imposée à sa MFT.
///
/// **Pourquoi.** La faute relevée par la tâche 2bis s'exécute sous
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem` : un élément de travail de
/// la MFT NVIDIA court encore quand nous relâchons l'encodeur. Deux tentatives
/// mesurées ici n'y ont rien changé — `IMFShutdown::Shutdown` a rendu
/// `MFSHUTDOWN_COMPLETED` et la faute est survenue quand même (1 récidive sur
/// 10 exécutions), et retirer `MFShutdown` du chemin ne l'a pas empêchée non
/// plus (1 récidive sur 5, même pile, désormais APRÈS la libération complète
/// de l'encodeur). Aucun des deux mécanismes n'attend donc ces éléments.
///
/// `IMFRealTimeClientEx::SetWorkQueueEx` permet au client de DICTER à la MFT
/// la file sur laquelle elle déposera son travail asynchrone — relevé : cette
/// MFT expose bien l'interface. En lui imposant une file **sérialisée**, dont
/// la sémantique est d'exécuter un élément à la fois dans l'ordre de dépôt, on
/// obtient une barrière réelle : déposer notre propre élément et attendre
/// qu'il s'exécute prouve que tout ce qui était déposé avant a fini.
///
/// C'est une attente **bornée sur une condition observable**, pas un délai.
///
/// Ce que cela ne garantit pas : que la MFT n'utilise QUE cette file. Si elle
/// en garde une autre pour son compte, la barrière ne la couvre pas — et la
/// mesure est le seul juge.
pub(super) struct FileEncodeur {
    /// `None` si l'allocation ou l'imposition à la MFT a échoué : on continue
    /// alors sans barrière plutôt que de refuser de construire l'encodeur.
    id: Option<u32>,
}

impl FileEncodeur {
    /// Alloue une file sérialisée et l'impose à la MFT. À appeler **avant**
    /// tout démarrage de flux.
    pub(super) fn imposer(encodeur: &IMFTransform) -> Self {
        let client = match encodeur.cast::<IMFRealTimeClientEx>() {
            Ok(client) => client,
            Err(err) => {
                tracing::warn!(erreur = %err, "MFT sans IMFRealTimeClientEx : pas de barrière de file");
                return Self { id: None };
            }
        };
        let id = match unsafe { MFAllocateSerialWorkQueue(MFASYNC_CALLBACK_QUEUE_MULTITHREADED) } {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(erreur = %err, "allocation de file sérialisée refusée");
                return Self { id: None };
            }
        };
        // Priorité 0 : la priorité de base des éléments, pas un réglage de
        // temps réel — on ne demande aucun privilège d'ordonnancement.
        if let Err(err) = unsafe { client.SetWorkQueueEx(id, 0) } {
            tracing::warn!(erreur = %err, file = id, "la MFT refuse la file imposée");
            let _ = unsafe { MFUnlockWorkQueue(id) };
            return Self { id: None };
        }
        tracing::info!(file = id, "file de travail sérialisée imposée à l'encodeur");
        Self { id: Some(id) }
    }

    /// Attend que tout ce qui était déposé sur la file avant cet appel ait fini
    /// de s'exécuter.
    fn barriere(&self, quand: &'static str) {
        let Some(id) = self.id else { return };
        let fait = Arc::new((Mutex::new(false), Condvar::new()));
        let rappel: IMFAsyncCallback = Sentinelle { fait: fait.clone() }.into();
        if let Err(err) = unsafe { MFPutWorkItem(id, &rappel, None) } {
            tracing::warn!(erreur = %err, quand, "dépôt de la sentinelle refusé");
            return;
        }
        let debut = Instant::now();
        let (verrou, signal) = &*fait;
        let mut pose = verrou.lock().unwrap_or_else(|e| e.into_inner());
        while !*pose {
            let (garde, issue) = signal
                .wait_timeout(pose, DELAI_BARRIERE)
                .unwrap_or_else(|e| e.into_inner());
            pose = garde;
            if !*pose && issue.timed_out() {
                tracing::warn!(
                    quand,
                    delai_ms = DELAI_BARRIERE.as_millis() as u64,
                    "barrière de file non franchie dans le délai"
                );
                return;
            }
        }
        tracing::debug!(quand, attente_ms = debut.elapsed().as_millis() as u64, "barrière franchie");
    }
}

impl Drop for FileEncodeur {
    fn drop(&mut self) {
        // Déclaré DERNIER champ de `H264Encoder` : la file n'est rendue
        // qu'après le relâchement de la MFT qui s'en sert.
        if let Some(id) = self.id {
            let _ = unsafe { MFUnlockWorkQueue(id) };
        }
    }
}

/// Élément de travail sans effet, dont la seule raison d'être est de signaler
/// son propre passage : c'est lui qui rend la barrière observable.
#[implement(IMFAsyncCallback)]
struct Sentinelle {
    fait: Arc<(Mutex<bool>, Condvar)>,
}

impl IMFAsyncCallback_Impl for Sentinelle_Impl {
    fn GetParameters(&self, _drapeaux: *mut u32, _file: *mut u32) -> windows::core::Result<()> {
        // Réponse normale d'un rappel qui n'impose ni file ni drapeau : Media
        // Foundation l'attend et retombe sur ses valeurs par défaut.
        Err(E_NOTIMPL.into())
    }

    fn Invoke(&self, _resultat: Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        let (verrou, signal) = &*self.fait;
        *verrou.lock().unwrap_or_else(|e| e.into_inner()) = true;
        signal.notify_one();
        Ok(())
    }
}
