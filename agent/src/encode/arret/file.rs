//! La file de travail Media Foundation sérialisée imposée à la MFT de
//! l'encodeur, et la barrière qui attend qu'elle se vide — la seconde moitié
//! du couple arrêt + barrière décrit dans `arret.rs`.
//!
//! Extrait de `arret.rs` quand ce fichier a franchi les 500 lignes au passage
//! de `cargo fmt`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncCallback_Impl, IMFAsyncResult, IMFRealTimeClientEx, IMFTransform,
    MFAllocateSerialWorkQueue, MFPutWorkItem, MFUnlockWorkQueue,
    MFASYNC_CALLBACK_QUEUE_MULTITHREADED,
};

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
pub(in crate::encode) struct FileMft {
    /// `None` si l'allocation ou l'imposition à la MFT a échoué : on continue
    /// alors sans barrière plutôt que de refuser de construire l'encodeur.
    ///
    /// **Ce que vaut alors l'encodeur** : la configuration « arrêt seul »,
    /// **mesurée à 1 récidive sur 10** — le défaut d'origine, atténué mais
    /// présent. Pas silencieux pour autant : les trois chemins qui mènent ici
    /// journalisent un `warn!` (`allouer`, puis les deux échecs de `confier`)
    /// et le cas nominal un `info!` — un journal dit donc, encodeur par
    /// encodeur, lequel est protégé. Le relâchement, lui, ne le redit pas :
    /// `barriere` rend la main sans un mot si la file manque.
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
    pub(in crate::encode) fn allouer() -> Self {
        match unsafe { MFAllocateSerialWorkQueue(MFASYNC_CALLBACK_QUEUE_MULTITHREADED) } {
            Ok(id) => Self {
                id: Some(id),
                compromise: AtomicBool::new(false),
            },
            Err(err) => {
                tracing::warn!(erreur = %err, "allocation de file sérialisée refusée");
                Self {
                    id: None,
                    compromise: AtomicBool::new(false),
                }
            }
        }
    }

    /// Impose la file à une MFT. À appeler **avant** tout démarrage de flux.
    pub(in crate::encode) fn confier(&mut self, mft: &IMFTransform, quoi: &'static str) {
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
    pub(super) fn barriere(&self, quoi: &'static str, quand: &'static str) {
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
    pub(in crate::encode) fn bloquer(&self, duree: Duration) {
        let Some(id) = self.id else {
            tracing::warn!("épreuve de file demandée mais aucune file imposée");
            return;
        };
        let rappel: IMFAsyncCallback = Bouchon { duree }.into();
        match unsafe { MFPutWorkItem(id, &rappel, None) } {
            Ok(()) => tracing::info!(
                file = id,
                duree_ms = duree.as_millis() as u64,
                "épreuve : file bouchée"
            ),
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
            // garde : un déverrouillage à éléments pendants coûte un plantage.
            //
            // CE QUE LA FUITE COÛTE, sans le minimiser : pas « quelques
            // octets » mais un objet de plateforme adossé au pool de fils de
            // RTWorkQ, enregistré pour la vie du processus. Chaque expiration
            // en fuite une définitivement, et rien ne les compte ni ne les
            // plafonne : arbitrage à rouvrir si ces `error!` cessaient d'être
            // exceptionnels — aucune expiration observée à ce jour.
            tracing::error!(
                file = id,
                "file compromise : NON rendue, délibérément fuitée"
            );
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
