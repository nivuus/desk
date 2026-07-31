//! Mise au repos des MFT d'un encodeur avant leur relâchement.
//!
//! Séparé d'`encode.rs` (déjà en dette de taille, voir `CLAUDE.md`) plutôt
//! qu'ajouté dedans.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncCallback_Impl, IMFAsyncResult, IMFRealTimeClientEx, IMFShutdown,
    IMFTransform, MFAllocateSerialWorkQueue, MFPutWorkItem, MFUnlockWorkQueue,
    MFASYNC_CALLBACK_QUEUE_MULTITHREADED, MFSHUTDOWN_COMPLETED,
    MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_END_STREAMING, MFT_MESSAGE_TYPE,
};

/// Met au repos les deux MFT d'un encodeur, dans l'ordre, avant que leurs
/// références COM ne soient relâchées.
///
/// **Pourquoi cette séquence existe** — relevé de la tâche 2bis (deux
/// plantages, piles identiques, symbolisées) : la faute est levée par
/// `RtlEnterCriticalSection` dans du code de `nvEncMFTH264x.dll` exécuté sous
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem`, c'est-à-dire **sur un fil de
/// la file de travail Media Foundation**, sur une section critique dont le
/// `DebugInfo` vaut `NULL`. La MFT a donc encore un élément de travail en vol
/// quand nous détruisons l'encodeur.
///
/// Dans les deux vidages de 2bis, le fil principal était simultanément dans
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown`. Retirer
/// `MFShutdown` du chemin n'a PAS empêché la faute : cet appel n'est donc pas
/// **nécessaire** à la faute (voir `super::demarrer_media_foundation` pour la
/// portée exacte de ce relevé).
///
/// Ce qui la traite est le COUPLE arrêt + barrière, et il a fallu retirer
/// chacun des deux séparément pour l'établir : l'arrêt seul laisse la faute
/// revenir (1 sur 10), la barrière seule aussi (2 sur 5). Ni l'un ni l'autre
/// n'est redondant — voir `arreter` et `FileMft`.
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
    file_encodeur: &FileMft,
) {
    // Fin de flux : inchangé, c'est ce que faisait déjà `Drop`.
    message(convertisseur, "convertisseur", "END_OF_STREAM", MFT_MESSAGE_NOTIFY_END_OF_STREAM);
    message(convertisseur, "convertisseur", "END_STREAMING", MFT_MESSAGE_NOTIFY_END_STREAMING);
    message(encodeur, "encodeur", "END_OF_STREAM", MFT_MESSAGE_NOTIFY_END_OF_STREAM);
    message(encodeur, "encodeur", "END_STREAMING", MFT_MESSAGE_NOTIFY_END_STREAMING);

    // DEUX MESSAGES DÉLIBÉRÉMENT ABSENTS, et ce n'est pas un oubli.
    //
    // `MFT_MESSAGE_COMMAND_FLUSH` et `MFT_MESSAGE_SET_D3D_MANAGER` à zéro
    // (deux pistes du brief 2ter) ont été posés ici, puis retirés sur relevé :
    // avec eux, à N = 4 encodeurs, **une exécution sur quatre s'est bloquée
    // sans retour** dans ce bloc, juste après la mise au repos de l'encodeur
    // n°0 et pendant celle du n°1 (journal arrêté sur « libération d'un
    // encodeur : avant id=1 », processus encore vivant treize minutes plus
    // tard, `Responding: True`). Le blocage est **borné à ce bloc** : la trace
    // suivante n'a jamais été écrite. Les deux fins de flux ci-dessus, elles,
    // précèdent ce chantier et n'ont jamais bloqué. Preuve versée :
    // `docs/superpowers/plans/journaux-duplications-paralleles/2ter-blocage-n4-flush-setd3dmanager.log`.
    // NON établi : lequel des deux bloquait, ni pourquoi.

    // LE CONVERTISSEUR N'A PAS DE FILE IMPOSÉE, et c'est un arbitrage mesuré,
    // pas un oubli. La revue a raison sur le principe : `create_color_converter`
    // tente d'abord `find_hardware_video_processor()`, et sur un hôte où un
    // Video Processor MATÉRIEL est enregistré, le convertisseur serait une MFT
    // matérielle avec son propre travail asynchrone, que rien ici ne couvre.
    // Sur cette VM c'est toujours le repli logiciel qui sort, donc une MFT
    // synchrone.
    //
    // Lui imposer une file et une barrière a été fait, puis retiré : dans cette
    // forme (8 files sérialisées à N = 4 au lieu de 4), une exécution sur six à
    // N = 4 s'est **figée dans `IMFShutdown::Shutdown` de l'encodeur**, trace
    // « Shutdown : avant » écrite, « après » jamais
    // (`2ter-gel-n4-shutdown.log`). La forme sans file au convertisseur avait,
    // elle, passé 16 exécutions à N = 4 sans gel. Couvrir un cas qui n'existe
    // sur aucune machine éprouvée, au prix d'un gel observé sur celle qu'on
    // éprouve, est un mauvais échange.
    //
    // NON établi : que la file du convertisseur soit la CAUSE de ce gel. C'est
    // la seule différence structurelle entre les deux formes, et le gel n'est
    // apparu qu'avec elle — sur six exécutions. `Shutdown()` peut aussi bien
    // porter ce risque en propre (voir `arreter`).

    // Barrière : plus rien de ce qui était déjà en file ne court encore.
    file_encodeur.barriere("encodeur", "après END_STREAMING");

    // Arrêt explicite des MFT, puis SECONDE barrière : l'arrêt lui-même dépose
    // du travail sur la file, et c'est précisément ce travail-là qu'il faut
    // attendre. Voir `arreter` pour ce qui rend ces deux appels nécessaires.
    arreter(convertisseur, "convertisseur");
    arreter(encodeur, "encodeur");

    file_encodeur.barriere("encodeur", "après IMFShutdown");
}

/// Garde-fou de l'attente de confirmation d'arrêt d'une MFT.
const DELAI_ARRET_MFT: Duration = Duration::from_secs(2);

/// Demande à une MFT d'arrêter ses files de travail, et attend qu'elle le
/// confirme.
///
/// `IMFShutdown::Shutdown` est le mécanisme documenté par lequel un client de
/// MFT obtient cet arrêt — c'est ce que fait le pipeline Media Foundation
/// lui-même, via `MFShutdownObject`, quand il démonte un nœud de topologie. On
/// l'appelle directement plutôt que par `MFShutdownObject` pour savoir, et
/// pouvoir journaliser, si la MFT expose seulement cette interface
/// (`MFShutdownObject` rend `S_OK` sans rien dire quand elle l'ignore), et pour
/// pouvoir attendre la confirmation par `GetShutdownStatus`.
///
/// # Deux relevés qui se contredisent en apparence, et ce qu'ils disent
///
/// 1. **Seul, cet appel ne suffit pas.** Il rend `MFSHUTDOWN_COMPLETED` en 0 ms
///    et la faute survient quand même : 1 récidive sur 10 exécutions, la
///    dernière ligne du journal avant la mort étant justement la confirmation
///    d'arrêt (`2ter-recidive-apres-imfshutdown-pile.log`).
///    **`MFSHUTDOWN_COMPLETED` d'une MFT ne prouve donc pas l'absence
///    d'élément de travail en vol la concernant.**
/// 2. **Mais il est nécessaire.** Retiré du chemin en laissant la barrière
///    seule, la faute est revenue **2 fois sur 5 exécutions**, pile et décalage
///    identiques, alors même que la barrière avait été franchie
///    (`2ter-r1-barriere-seule-*` du rapport). Barrière et arrêt ne sont pas
///    redondants : l'arrêt fait cesser la MFT, la barrière attend ce qu'il
///    laisse derrière lui. C'est pourquoi la seconde barrière suit cet appel.
///
/// # Ce que cet appel coûte comme risque, et pourquoi il reste
///
/// `Shutdown()` n'est borné par RIEN — le garde-fou ci-dessous ne borne que la
/// boucle de confirmation qui suit. Un appel non borné dans un `Drop` est un
/// gel potentiel à la fermeture d'une fenêtre, ce qui serait pire que le
/// plantage qu'on corrige. Il reste malgré tout, faute d'alternative sûre :
/// le déporter sur un autre fil exigerait de faire traverser une interface COM
/// à une frontière d'appartement (le fil principal est dans un STA — cadres
/// `ClassicSTAThreadWaitForHandles` du vidage 2bis), ce qui échangerait un
/// risque contre un défaut certain. Ce qui borne le risque en pratique :
/// l'appel est encadré de deux traces `debug`, le seul blocage jamais observé
/// sur ce chemin venait de deux messages désormais retirés, et `Shutdown()`
/// est rentré en moins d'une milliseconde sur chacune des exécutions relevées.
fn arreter(mft: &IMFTransform, quoi: &'static str) {
    let arret: IMFShutdown = match mft.cast() {
        Ok(arret) => arret,
        Err(err) => {
            // Relevé, pas supposé : si l'interface manque, le journal le dit,
            // et l'on sait que ce chemin n'a rien arrêté du tout. C'est le cas
            // du convertisseur logiciel sur cette VM (`0x80004002`).
            tracing::debug!(mft = quoi, erreur = %err, "MFT sans IMFShutdown : pas d'arrêt explicite");
            return;
        }
    };

    tracing::debug!(mft = quoi, "IMFShutdown::Shutdown : avant");
    if let Err(err) = unsafe { arret.Shutdown() } {
        tracing::warn!(mft = quoi, erreur = %err, "IMFShutdown::Shutdown refusé");
        return;
    }
    tracing::debug!(mft = quoi, "IMFShutdown::Shutdown : après");

    let debut = Instant::now();
    loop {
        match unsafe { arret.GetShutdownStatus() } {
            Ok(statut) if statut == MFSHUTDOWN_COMPLETED => {
                tracing::debug!(
                    mft = quoi,
                    attente_ms = debut.elapsed().as_millis() as u64,
                    "arrêt de la MFT confirmé"
                );
                return;
            }
            // `MFSHUTDOWN_INITIATED` : l'arrêt court encore, on repasse.
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(
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
                "arrêt de la MFT non confirmé dans le délai : on relâche quand même"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Envoie un message à une MFT en encadrant l'appel de deux traces : un appel
/// qui ne rend pas la main se lit alors dans le journal.
fn message(mft: &IMFTransform, quoi: &'static str, nom: &'static str, message: MFT_MESSAGE_TYPE) {
    tracing::debug!(mft = quoi, message = nom, "mise au repos : avant");
    let issue = unsafe { mft.ProcessMessage(message, 0) };
    tracing::debug!(mft = quoi, message = nom, refuse = issue.is_err(), "mise au repos : après");
}

/// Garde-fou de l'attente d'une barrière de file de travail.
///
/// La barrière porte sur une CONDITION OBSERVABLE (notre propre élément de
/// travail a-t-il été exécuté), pas sur une durée. Ce délai n'est qu'un
/// garde-fou contre une file qui ne dépêcherait plus rien ; le franchir est
/// journalisé en `error` et **change le comportement de `Drop`** (voir
/// `Drop for FileMft`).
const DELAI_BARRIERE: Duration = Duration::from_secs(2);

/// File de travail Media Foundation **sérialisée** dédiée à UNE MFT, et
/// imposée à elle.
///
/// **Pourquoi.** La faute relevée par la tâche 2bis s'exécute sous
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem` : un élément de travail de la
/// MFT NVIDIA court encore quand nous relâchons l'encodeur. Deux mécanismes
/// documentés ont été mesurés incapables de l'empêcher — `IMFShutdown::Shutdown`
/// (1 récidive sur 10) et le retrait de `MFShutdown` (1 récidive sur 5).
///
/// `IMFRealTimeClientEx::SetWorkQueueEx` permet au client de DICTER à la MFT la
/// file sur laquelle elle déposera son travail asynchrone — relevé : les deux
/// MFT de l'encodeur exposent l'interface. En lui imposant une file
/// **sérialisée**, dont la sémantique est d'exécuter un élément à la fois dans
/// l'ordre de dépôt, on vise une barrière : déposer notre propre élément et
/// attendre qu'il s'exécute.
///
/// C'est une attente **bornée sur une condition observable**, pas un délai.
///
/// # Mode d'échec résiduel identifié — imbrication de files sérialisées
///
/// **Ce n'est pas une réserve de style, c'est le mécanisme précis par lequel
/// cette barrière pourrait ne rien barrer.** L'implémentation idiomatique de
/// `SetWorkQueueEx` dans un objet Media Foundation est
/// `MFAllocateSerialWorkQueue(file_du_client, &sa_propre_file)` : l'objet
/// empile SA file sérialisée sur la nôtre, ses éléments attendent chez lui et
/// ne sont dépêchés vers nous **qu'un à la fois**. Notre sentinelle, déposée
/// directement sur notre file, serait alors ordonnancée derrière **un seul**
/// de ses éléments, pas derrière tous — et la barrière ne serait qu'une
/// réduction de fenêtre, pas une garantie.
///
/// L'indice qui rend l'hypothèse sérieuse : la pile d'AVANT correctif porte
/// déjà des cadres `CSerialWorkQueue` alors qu'aucune file sérialisée n'était
/// allouée par nous — la MFT en avait donc déjà une à elle.
///
/// Ce qui est mesuré, et qui ne tranche que la moitié de la question :
/// bloquer notre file pendant l'encodage **arrête l'encodeur** (épreuve
/// `MULTIFENETRE_EPREUVE_FILE_MS`, voir le rapport 2ter). Le travail de la MFT
/// transite donc bien par notre file — mais cela reste vrai que l'imbrication
/// existe ou non, puisque bloquer la file cible bloque aussi la file empilée
/// dessus. **Ce qui départagerait** : symboliser une récidive survenant malgré
/// la barrière, ou observer la file interne de la MFT (aucune API ne
/// l'expose).
pub(super) struct FileMft {
    /// `None` si l'allocation ou l'imposition à la MFT a échoué : on continue
    /// alors sans barrière plutôt que de refuser de construire l'encodeur.
    id: Option<u32>,
    /// Vrai si une barrière n'a pas été franchie dans le délai. La file porte
    /// alors, au minimum, notre sentinelle non exécutée : la rendre serait
    /// pire que la garder (voir `Drop`).
    compromise: AtomicBool,
}

impl FileMft {
    /// Alloue une file sérialisée, **sans encore la confier à quiconque**.
    ///
    /// Séparé de `confier` pour une raison de durée de vie, pas de style : les
    /// variables locales sont détruites dans l'ordre **inverse** de leur
    /// déclaration. En allouant ici, avant que la MFT n'existe, la `FileMft`
    /// est la locale la plus ancienne et donc la **dernière** détruite si la
    /// construction de l'encodeur échoue plus loin sur un `?` — la file n'est
    /// alors rendue qu'après le relâchement de la MFT qui la détient. L'ordre
    /// inverse (allouer après la MFT) rendait la file en premier, exactement
    /// l'inversion que l'ordre des champs de `H264Encoder` est conçu pour
    /// éviter. Ce chemin n'est pas théorique : `windows_source.rs` traite
    /// l'échec de construction d'un encodeur et poursuit la session.
    ///
    /// Exige que Media Foundation soit démarré.
    pub(super) fn allouer() -> Self {
        match unsafe { MFAllocateSerialWorkQueue(MFASYNC_CALLBACK_QUEUE_MULTITHREADED) } {
            Ok(id) => Self { id: Some(id), compromise: AtomicBool::new(false) },
            Err(err) => {
                tracing::warn!(erreur = %err, "allocation de file sérialisée refusée");
                Self { id: None, compromise: AtomicBool::new(false) }
            }
        }
    }

    /// Impose la file à une MFT. À appeler **avant** tout démarrage de flux.
    pub(super) fn confier(&mut self, mft: &IMFTransform, quoi: &'static str) {
        let Some(id) = self.id else { return };
        let client = match mft.cast::<IMFRealTimeClientEx>() {
            Ok(client) => client,
            Err(err) => {
                tracing::warn!(mft = quoi, erreur = %err, "MFT sans IMFRealTimeClientEx : pas de barrière");
                self.rendre();
                return;
            }
        };
        // Priorité 0 : la priorité de base des éléments, pas un réglage de
        // temps réel — on ne demande aucun privilège d'ordonnancement.
        if let Err(err) = unsafe { client.SetWorkQueueEx(id, 0) } {
            tracing::warn!(mft = quoi, erreur = %err, file = id, "la MFT refuse la file imposée");
            self.rendre();
            return;
        }
        tracing::info!(mft = quoi, file = id, "file de travail sérialisée imposée");
    }

    /// Rend la file immédiatement, quand personne ne la détient encore.
    fn rendre(&mut self) {
        if let Some(id) = self.id.take() {
            let _ = unsafe { MFUnlockWorkQueue(id) };
        }
    }

    /// Attend que tout ce qui était déposé sur la file avant cet appel ait fini
    /// de s'exécuter.
    fn barriere(&self, quoi: &'static str, quand: &'static str) {
        let Some(id) = self.id else { return };
        let fait = Arc::new((Mutex::new(false), Condvar::new()));
        let rappel: IMFAsyncCallback = Sentinelle { fait: fait.clone() }.into();
        if let Err(err) = unsafe { MFPutWorkItem(id, &rappel, None) } {
            // La file ne dépêche plus : notre sentinelle n'y est pas, mais le
            // travail de la MFT, lui, peut y être resté.
            tracing::error!(mft = quoi, erreur = %err, quand, "dépôt de la sentinelle refusé : file NON barrée");
            self.compromise.store(true, Ordering::SeqCst);
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
                // On ne peut pas refuser de poursuivre : `Drop` doit finir. Ce
                // qu'on peut faire, c'est ne pas AGGRAVER — voir `Drop`.
                tracing::error!(
                    mft = quoi,
                    quand,
                    delai_ms = DELAI_BARRIERE.as_millis() as u64,
                    "barrière non franchie : les références COM vont être relâchées avec du \
                     travail possiblement encore en file — c'est le défaut d'origine, non barré"
                );
                self.compromise.store(true, Ordering::SeqCst);
                return;
            }
        }
        tracing::debug!(
            mft = quoi,
            quand,
            attente_ms = debut.elapsed().as_millis() as u64,
            "barrière franchie"
        );
    }

    /// Occupe la file pendant `duree`, pour éprouver si le travail de la MFT y
    /// transite réellement (voir le mode d'échec résiduel documenté plus haut).
    ///
    /// Sonde de mesure, appelée seulement sous variable d'environnement. Rend
    /// la main immédiatement : c'est la file qui reste occupée.
    pub(super) fn bloquer(&self, duree: Duration) {
        let Some(id) = self.id else {
            tracing::warn!("épreuve de file demandée mais aucune file imposée");
            return;
        };
        let rappel: IMFAsyncCallback = Bouchon { duree }.into();
        match unsafe { MFPutWorkItem(id, &rappel, None) } {
            Ok(()) => tracing::info!(file = id, duree_ms = duree.as_millis() as u64, "épreuve : file bouchée"),
            Err(err) => tracing::warn!(erreur = %err, "épreuve : dépôt du bouchon refusé"),
        }
    }
}

impl Drop for FileMft {
    fn drop(&mut self) {
        let Some(id) = self.id else { return };
        if self.compromise.load(Ordering::SeqCst) {
            // Rendre une file dont des éléments n'ont pas été dépêchés
            // ajouterait un défaut à celui qu'on n'a pas su éviter. On la
            // garde : une file fuitée coûte quelques octets pour la vie du
            // processus, un déverrouillage à éléments pendants coûte un
            // plantage.
            tracing::error!(file = id, "file compromise : NON rendue, délibérément fuitée");
            return;
        }
        // Champ déclaré en dernier dans `H264Encoder`, et locale déclarée en
        // premier dans `H264Encoder::new` : dans les deux cas la file n'est
        // rendue qu'après le relâchement de la MFT qui s'en sert.
        let _ = unsafe { MFUnlockWorkQueue(id) };
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

/// Élément de travail qui occupe la file : instrument de l'épreuve, jamais
/// déposé en exploitation.
#[implement(IMFAsyncCallback)]
struct Bouchon {
    duree: Duration,
}

impl IMFAsyncCallback_Impl for Bouchon_Impl {
    fn GetParameters(&self, _drapeaux: *mut u32, _file: *mut u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Invoke(&self, _resultat: Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        std::thread::sleep(self.duree);
        Ok(())
    }
}
