//! La racine de virtualisation ProjFS : la marquer, la démarrer, l'arrêter.
//!
//! **`#[cfg(windows)]`, et appelé par le SYSTÈME : aucun test d'hôte n'est
//! possible ici, et c'est déclaré, pas contourné** (spec §4.4). La seule
//! compensation est que ce module soit **mince** — il traduit, il ne décide
//! pas. Toute décision qui peut vivre dans un module pur y vit :
//! `pont::chemins` (normalisation), `pont::erreurs` (les `HRESULT`),
//! `pont::decoupe` (les plages), `pont::table` (les commandes en vol),
//! `pont::resolution` (les treize entrées).
//!
//! # LA DISCIPLINE DE FIL, et elle est la décision centrale de ce module
//!
//! **Trois catégories de fils, et la frontière entre elles est stricte.**
//!
//! ⚠️ **QUATRE FILS DEPUIS F2, POUR TROIS CATÉGORIES.** Le **fil d'écriture**
//! (`pont::ecriture::fil`) rejoint la catégorie 3 : il ne complète aucune
//! commande, mais il partage sa propriété essentielle — **il ne court sur
//! aucun fil du système**. Il lit des fichiers de la racine, ce que le fil du
//! pont ne doit JAMAIS faire (voir `pont/service.rs` : « il s'attendrait
//! lui-même »), et c'est précisément pourquoi il lui est distinct.
//!
//! 1. **Les fils de RAPPEL, que le SYSTÈME possède.** ProjFS en tient un vivier
//!    dimensionné par `PRJ_STARTVIRTUALIZING_OPTIONS.PoolThreadCount` /
//!    `ConcurrentThreadCount` (windows-rs, `ProjectedFileSystem/mod.rs:518-523`).
//!    Un rappel qui s'y exécute :
//!      - enveloppe TOUT son corps dans `std::panic::catch_unwind` et rend
//!        `E_UNEXPECTED` — une panique Rust qui traverserait une frontière
//!        `extern "system"` est un **ABANDON DE PROCESSUS** ;
//!      - n'écrit QUE dans les états sous verrou de [`Etat`], et pousse sur un
//!        `mpsc::Sender` ;
//!      - **n'appelle JAMAIS `PrjCompleteCommand`**, ne touche JAMAIS le
//!        socket, ne tient JAMAIS un verrou pendant une E/S ;
//!      - rend `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` et rend la main
//!        IMMÉDIATEMENT.
//!
//!    Y attendre un aller-retour navigateur figerait l'APPLICATION qui lit le
//!    fichier — **pas la vidéo** : le flux continue de couler et la fenêtre
//!    montre une application gelée (spec §5.2). C'est la seule raison d'être
//!    de cette discipline.
//!
//! 2. **Le fil de TRANSPORT**, qui possède le `Rtc` et le socket UDP
//!    (`pont::transport::tourner`).
//!
//! 3. **Le fil du PONT**, qui possède la table, complète les commandes par
//!    `PrjCompleteCommand`, et balaie les expirations
//!    (`pont::service`, tâche 14).
//!
//! ⚠️ **DIVERGENCE DÉLIBÉRÉE D'AVEC LE PLAN DE F1** (tâche 13, step 2), qui
//! écrit « **LE** fil du pont, unique. Il possède le `Rtc`, lit le socket,
//! complète les commandes par `PrjCompleteCommand`, et balaie les
//! expirations ». **Ces deux rôles ne peuvent pas tenir dans un seul fil** :
//! `pont::transport::tourner` — dont la signature est fixée par le §
//! « Interfaces partagées » du même plan, et qui est livrée depuis la tâche 11 —
//! possède le `Rtc` dans une boucle bloquante et **ne connaît ni ProjFS ni
//! Windows**, ce qui est précisément ce qui la rend testable sur l'hôte. Y
//! loger `PrjCompleteCommand` détruirait cette propriété. D'où deux fils, 2 et
//! 3, reliés par les deux `mpsc` que le plan définit lui-même. **L'invariant
//! qui compte est préservé** : aucun rappel ne complète, aucun rappel ne fait
//! d'E/S, et `pont::table` reste pur.
//!
//! Le `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT` est un pointeur brut partagé entre
//! les trois : il est enveloppé dans [`Contexte`], dont l'`unsafe impl Send +
//! Sync` porte sa justification.
//!
//! # Ce qui se passe quand une commande dépasse son délai
//!
//! **Un rappel ne dépasse jamais son délai : il rend en microsecondes.** Ce qui
//! dépasse, c'est la **commande** qu'il a inscrite. Le balayage du fil du pont
//! retire les échues de la table et les complète par
//! `PrjCompleteCommand(command_id, HRESULT_FROM_WIN32(ERROR_SEM_TIMEOUT))`.
//!
//! ⚠️ **CE CHEMIN N'A JAMAIS ÉTÉ OBSERVÉ.** `commande expirée` vaut **0** aux
//! cinq exécutions nominales de la recette F1, et la seule occurrence de tout
//! le corpus (`agent-dbg-plat.log`, une ligne) tombe **après** que le pilote a
//! fermé le navigateur. Or la même recette montre des lectures qui **calent
//! sans expirer** — la mesure VM d'`exec1` ne rend pas la main en 540 s. Ce que
//! l'absence de trace établit est que le balayage n'a rien retiré ; elle ne dit
//! **pas où** le blocage se produit, et un blocage EN AMONT de l'inscription en
//! table laisserait ce paragraphe littéralement vrai tout en décrivant un
//! chemin que rien n'atteint. **Trancher demanderait une trace à
//! l'inscription, qui n'existe pas.**
//!
//! L'application reçoit une E/S expirée ; **rien n'est rejoué**, jamais — une
//! commande expirée dont on rejouerait la requête produirait une seconde
//! réponse sans destinataire (spec §5.3). Si la réponse du navigateur arrive
//! **après**, `Table::resoudre` rend `None` et la réponse est **jetée**.

pub mod chargement;
mod etat;
mod racine;
mod rappels;

pub use etat::{ContexteProjFs, Etat, FluxDonnees, TamponEntrees};
pub use racine::dossier_etat;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use windows::core::PCWSTR;
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_FLAG_USE_NEGATIVE_PATH_CACHE, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    PRJ_NOTIFICATION_MAPPING, PRJ_STARTVIRTUALIZING_OPTIONS,
};

use crate::pont::erreurs::Erreur;
use crate::pont::table::Table;
use crate::pont::transport::VersNavigateur;

/// Période du relevé d'hydratation.
///
/// ⚠️ **Le cas 4 de la spec §6.4 — « pas une perte, mais qui mord » —
/// S'APPLIQUE PLEINEMENT à F1, et F1 est le sous-bloc qui le crée** : chaque
/// fichier LU est écrit en entier sur le disque de la VM, et **il y reste**.
/// **Aucune politique d'éviction en F1** : `PrjDeleteFile` est chargée
/// (tâche 12) pour que la politique, quand elle viendra, n'ait pas à rouvrir la
/// couche. Poser une politique sans mesure serait exactement le geste que ce
/// dépôt reproche à ses constantes non calibrées. **La mesure appartient à F5,
/// l'instrument est ici.**
pub const PERIODE_HYDRATATION: std::time::Duration = std::time::Duration::from_secs(60);

/// Le contexte de virtualisation, partagé entre les fils de rappel, le fil du
/// pont et le fil principal.
///
/// # Sûreté
///
/// `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT` est un `*mut c_void` opaque. Il est
/// `Send + Sync` **parce que ProjFS le documente comme tel** : c'est ce même
/// contexte que le système passe simultanément à tous les fils de son propre
/// vivier de rappels, et toutes les entrées qui le prennent
/// (`PrjCompleteCommand`, `PrjWriteFileData`, `PrjAllocateAlignedBuffer`…) sont
/// appelables depuis n'importe lequel d'entre eux. Nous n'en faisons jamais
/// rien d'autre que de le passer à ces entrées ; **nous ne le déréférençons
/// jamais**.
#[derive(Clone, Copy)]
pub struct Contexte(pub PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT);
// SÛRETÉ : voir la justification ci-dessus. Elle est écrite plutôt que
// supposée — c'est la règle que ce dépôt s'impose pour tout `unsafe impl`.
unsafe impl Send for Contexte {}
unsafe impl Sync for Contexte {}

/// Une racine de virtualisation vivante. **Son `Drop` arrête la
/// virtualisation** — c'est le seul chemin d'arrêt, et il n'est pas facultatif.
pub struct Virtualisation {
    etat: Arc<Etat>,
    /// L'exemplaire d'`Arc` confié à ProjFS, à reprendre après l'arrêt.
    confie: *const Etat,
    racine: PathBuf,
}

// SÛRETÉ : `confie` est un `*const Etat` issu d'`Arc::into_raw`, et `Etat` est
// `Send + Sync` (tous ses champs le sont, `Contexte` par l'`unsafe impl`
// ci-dessus). Le pointeur n'est jamais déréférencé par ce type ; il n'est que
// rendu à `Arc::from_raw` dans `drop`.
unsafe impl Send for Virtualisation {}

impl Virtualisation {
    /// Prépare la racine, la marque si besoin, et démarre la virtualisation.
    pub fn demarrer(
        projfs: chargement::ProjFs,
        sortant: std::sync::mpsc::Sender<VersNavigateur>,
        vers_ecriture: std::sync::mpsc::Sender<crate::pont::ecriture::fil::Ordre>,
        inscriptible: bool,
        mutations_armees: bool,
        cache_arme: bool,
    ) -> Result<Self> {
        let racine = racine::racine()?;
        let etat = Arc::new(Etat {
            projfs,
            contexte: Mutex::new(None),
            table: Arc::new(Mutex::new(Table::nouvelle())),
            sessions: Mutex::new(HashMap::new()),
            en_attente: Mutex::new(HashMap::new()),
            sortant,
            vers_ecriture,
            inscriptible,
            mutations_armees,
            // ⚠️ **`false` AU DÉPART, et ce n'est pas une précaution de style** :
            // ProjFS peut appeler un rappel PENDANT `PrjStartVirtualizing`,
            // c'est-à-dire bien avant que le navigateur n'ait ouvert son canal.
            // Partir de `true` autoriserait une écriture qui n'aurait personne
            // à qui être poussée.
            canal_ouvert: std::sync::atomic::AtomicBool::new(false),
            compteurs: crate::pont::compteurs::Compteurs::nouveaux(),
            latences: crate::pont::latence::Histogramme::nouveau(),
            cache: Mutex::new(crate::pont::cache::CacheEnumeration::nouveau()),
            cache_arme,
            octets_hydrates: AtomicU64::new(0),
            entrees_hydratees: AtomicU64::new(0),
        });

        racine::preparer(&etat.projfs, &racine)?;

        // Le bloc de rappels et les options doivent rester valides pendant tout
        // l'appel. `Box::leak` plutôt qu'une variable locale : la documentation
        // de ProjFS ne dit pas si le service RETIENT le pointeur des
        // `NotificationMappings` au-delà de l'appel, et **une supposition qui
        // se révélerait fausse produirait une lecture de mémoire libérée dans
        // un service du système**. Une allocation de quelques dizaines
        // d'octets, une fois par processus, achète la certitude — et le pont
        // est un processus dédié qui n'en démarre qu'une.
        let rappels = Box::leak(Box::new(rappels::bloc()));
        // Un masque vide comme `NotificationRoot` désigne la racine elle-même :
        // le chemin est RELATIF à la racine de virtualisation.
        let racine_relative = Box::leak(Box::new([0u16; 1]));
        let mappings = Box::leak(Box::new([PRJ_NOTIFICATION_MAPPING {
            // Le masque vit dans le module PUR, où il est épinglé : SEPT bits
            // exactement depuis F2 — cinq en F1 —, et aucun d'eux ne retombe
            // dans le bras fourre-tout de `notifications::decider`.
            NotificationBitMask: windows::Win32::Storage::ProjectedFileSystem::PRJ_NOTIFY_TYPES(
                crate::pont::notifications::MASQUE,
            ),
            NotificationRoot: PCWSTR(racine_relative.as_ptr()),
        }]));
        let options = Box::leak(Box::new(PRJ_STARTVIRTUALIZING_OPTIONS {
            Flags: PRJ_FLAG_USE_NEGATIVE_PATH_CACHE,
            // Zéro = le dimensionnement par défaut de ProjFS. Poser une valeur
            // sans l'avoir mesurée rejoindrait la liste des constantes non
            // calibrées de ce dépôt, pour un gain inconnu.
            PoolThreadCount: 0,
            ConcurrentThreadCount: 0,
            NotificationMappings: mappings.as_mut_ptr(),
            NotificationMappingsCount: 1,
        }));

        // L'exemplaire confié à ProjFS. Créé AVANT `PrjStartVirtualizing` :
        // un rappel peut survenir pendant l'appel, et il doit déjà pouvoir
        // retrouver l'état.
        let confie = Arc::into_raw(Arc::clone(&etat));
        let chemin = racine::utf16(&racine);
        let mut contexte = PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT::default();
        // SÛRETÉ : `chemin`, `rappels`, `options` et `mappings` sont valides ;
        // `contexte` est un emplacement de sortie initialisé. La signature
        // appelée est la transcription du `link!` de `mod.rs:101`, à CINQ
        // paramètres — le cinquième est ce paramètre de sortie, que l'enveloppe
        // de windows-rs cache derrière son `Result`.
        let issue = unsafe {
            (etat.projfs.demarrer_virtualisation)(
                PCWSTR(chemin.as_ptr()),
                rappels,
                confie as *const core::ffi::c_void,
                options,
                &mut contexte,
            )
        };
        if issue.is_err() {
            // Reprendre l'exemplaire confié : la virtualisation n'a pas
            // démarré, donc plus aucun rappel ne peut l'atteindre.
            // SÛRETÉ : `confie` vient d'`Arc::into_raw` juste au-dessus et n'a
            // été rendu à personne d'autre.
            drop(unsafe { Arc::from_raw(confie) });
            bail!("PrjStartVirtualizing sur « {} » : {issue}", racine.display());
        }
        *etat.contexte.lock().expect("verrou du contexte") = Some(Contexte(contexte));
        tracing::info!(racine = %racine.display(), "racine de virtualisation ProjFS démarrée");
        Ok(Self { etat, confie, racine })
    }

    /// L'état partagé, pour le fil du pont.
    pub fn etat(&self) -> Arc<Etat> {
        Arc::clone(&self.etat)
    }

    /// La racine, pour le relevé d'hydratation.
    pub fn racine(&self) -> &Path {
        &self.racine
    }
}

impl Drop for Virtualisation {
    /// **L'ordre n'est pas négociable** (plan, tâche 13 step 6) :
    ///
    /// 1. vider la table, et **compléter CHAQUE commande** en `ERROR_IO_DEVICE` ;
    /// 2. `PrjStopVirtualizing` ;
    /// 3. reprendre l'exemplaire d'`Arc` confié à ProjFS.
    ///
    /// ⚠️ **C'est le remède au dernier défaut de l'annexe §13 de la spec** :
    /// l'arrêt forcé de l'ancien pont n'appelait PAS les callbacks FUSE en
    /// attente (`src/file.js:359-365`), et le noyau n'obtenait jamais de
    /// réponse. Ici, `PrjStopVirtualizing` est **précédé** de la complétion en
    /// erreur de tout ce qui reste — une commande laissée en vol y attendrait
    /// une réponse que plus rien ne peut délivrer.
    ///
    /// **`Drop` ne panique jamais** : chaque verrou est pris par
    /// `lock().ok()`, jamais par `expect`, parce qu'un verrou empoisonné par la
    /// panique d'un autre fil ferait ici une double panique, donc un `abort`
    /// — et l'`abort` laisserait la racine montée, c'est-à-dire le cas exact
    /// que ce `Drop` existe pour éviter.
    fn drop(&mut self) {
        let contexte = self.etat.contexte.lock().ok().and_then(|c| *c);
        let Some(Contexte(contexte)) = contexte else {
            tracing::warn!("arrêt du pont : aucun contexte de virtualisation à relâcher");
            return;
        };

        // 1. Les commandes en vol, complétées en erreur d'E/S. La table est
        //    vidée AVANT l'arrêt, jamais après : après, le contexte n'est plus
        //    valide et `PrjCompleteCommand` n'a plus où écrire.
        let restantes = match self.etat.table.lock() {
            Ok(mut table) => table.vider(),
            Err(empoisonne) => {
                tracing::error!("verrou de la table empoisonné à l'arrêt : la table est vidée quand même");
                empoisonne.into_inner().vider()
            }
        };
        let echec = windows::core::HRESULT(self.etat.compteurs.rendre(Erreur::CanalFerme));
        for (commande, correlation) in &restantes {
            // 🔴 **UNE ÉCRITURE EN VOL EST VIDÉE DE LA TABLE COMME LES AUTRES,
            // MAIS N'EST PAS RETIRÉE DU JOURNAL** — c'est exactement le cas que
            // le journal existe pour couvrir. Le pont relancé la repoussera.
            //
            // Il n'y a rien à compléter : `command_id` est `None`, et il
            // n'existe aucun rappel ProjFS derrière une écriture.
            let Some(commande) = commande else {
                tracing::debug!(
                    correlation,
                    "écriture en vol à l'arrêt : rien à compléter, l'entrée RESTE au journal"
                );
                continue;
            };
            // SÛRETÉ : contexte valide (la virtualisation n'est pas encore
            // arrêtée), `commande` vient de la table, et le quatrième paramètre
            // est nul — `PrjCompleteCommand` accepte l'absence de paramètres
            // étendus, ce que l'enveloppe de windows-rs exprime par un
            // `Option::None` transformé en pointeur nul (`mod.rs:14`).
            let issue = unsafe {
                (self.etat.projfs.completer_commande)(
                    contexte,
                    *commande,
                    echec,
                    std::ptr::null(),
                )
            };
            if issue.is_err() {
                tracing::warn!(commande, correlation, %issue, "complétion d'arrêt refusée");
            }
        }
        if !restantes.is_empty() {
            tracing::info!(
                commandes = restantes.len(),
                "commandes en vol complétées en erreur d'E/S avant l'arrêt de la virtualisation"
            );
        }

        // 2. L'arrêt lui-même. ProjFS garantit qu'aucun rappel ne court après
        //    le retour de cet appel : c'est ce qui rend l'étape 3 sûre.
        // SÛRETÉ : `PrjStopVirtualizing` ne rend RIEN (`mod.rs:109`), et la
        // transcription le reflète.
        unsafe { (self.etat.projfs.arreter_virtualisation)(contexte) };
        tracing::info!(racine = %self.racine.display(), "virtualisation ProjFS arrêtée");

        // 3. L'exemplaire confié, repris.
        // SÛRETÉ : `confie` vient d'`Arc::into_raw` dans `demarrer`, n'a été
        // rendu qu'à ProjFS, et plus aucun rappel ne peut courir.
        drop(unsafe { Arc::from_raw(self.confie) });
    }
}


