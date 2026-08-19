//! L'état que les rappels ProjFS partagent avec le fil du pont, et les trois
//! enveloppes de handles qui traversent cette frontière.
//!
//! Extrait de [`super`] **avant** que l'addition de la tâche 14 ne le fasse
//! franchir le plafond, et non après : `projfs.rs` était à 537 lignes. Ce dépôt
//! a payé quatre fois la leçon « la marge regagnée par une extraction se reperd
//! à la ronde suivante si on la traite comme acquise », et les deux fichiers
//! que le sous-bloc D9 a traités APRÈS coup ont d'abord été **compressés**,
//! geste que `CLAUDE.md` interdit nommément.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use windows::core::PCWSTR;

use super::{chargement, Contexte};
use crate::pont::decoupe::Morceau;
use crate::pont::enumeration::Session;
use crate::pont::erreurs::{hresult, Erreur};
use crate::pont::table::{Attendue, Table};
use crate::pont::transport::VersNavigateur;

/// Le handle du tampon d'entrées d'une énumération.
///
/// # Sûreté
///
/// `PRJ_DIR_ENTRY_BUFFER_HANDLE` est un `*mut c_void` opaque. Il est
/// `Send + Sync` **parce que l'énumération ASYNCHRONE de ProjFS l'exige** : le
/// rappel rend `ERROR_IO_PENDING` et la complétion — qui reprend ce même
/// handle dans ses paramètres étendus — se fait nécessairement depuis un autre
/// fil. Nous ne le déréférençons jamais ; nous ne faisons que le rendre à
/// `PrjFillDirEntryBuffer` et à `PrjCompleteCommand`.
#[derive(Clone, Copy)]
pub struct TamponEntrees(pub windows::Win32::Storage::ProjectedFileSystem::PRJ_DIR_ENTRY_BUFFER_HANDLE);
// SÛRETÉ : voir ci-dessus.
unsafe impl Send for TamponEntrees {}
unsafe impl Sync for TamponEntrees {}

/// Le GUID d'un flux de données, pour `PrjWriteFileData`.
///
/// Enveloppé pour la même raison que [`TamponEntrees`] : il traverse le fil, et
/// un `windows::core::GUID` nu est `Send`, mais le nommer ici rend l'intention
/// lisible au site d'usage.
#[derive(Clone, Copy)]
pub struct FluxDonnees(pub windows::core::GUID);

/// Ce que [`crate::pont::table`] ne peut pas porter **parce qu'elle est PURE** :
/// les handles ProjFS d'une commande en vol.
///
/// ⚠️ **C'est délibérément deux structures et non une.** Fusionner ferait
/// entrer `windows` dans `pont::table`, qui est le module dont la concurrence
/// est éprouvée sur l'hôte — et l'éprouver est tout son intérêt.
pub enum ContexteProjFs {
    Attributs {
        /// Le chemin **tel que ProjFS l'a livré** (UTF-16, terminé par un nul).
        ///
        /// Le chemin logique de la table est normalisé en `/` pour la File
        /// System Access API ; `PrjWritePlaceholderInfo` veut celui de ProjFS.
        /// Le conserver tel quel évite une reconversion, donc un aller-retour
        /// où une casse ou un séparateur pourrait se perdre.
        chemin_projfs: Vec<u16>,
    },
    /// `QueryFileName` — « ce nom existe-t-il ? », et **rien de plus**.
    ///
    /// ⚠️ **Distinct d'`Attributs`, et ce n'est pas une subtilité.** Les deux
    /// posent la même question au navigateur (`TYPE_ATTRIBUTS`), mais ProjFS
    /// n'attend pas la même chose en retour : `GetPlaceholderInfo` veut un
    /// marqueur écrit par `PrjWritePlaceholderInfo`, `QueryFileName` veut
    /// **seulement** `S_OK` ou `ERROR_FILE_NOT_FOUND`. Écrire un marqueur
    /// depuis `QueryFileName` créerait un objet projeté pour un fichier que
    /// personne n'ouvre — et le rappel n'est appelé, précisément, que pour
    /// alimenter le cache négatif.
    Existence,
    Lecture {
        flux: FluxDonnees,
        /// Les morceaux qui restent à demander. **Un seul en vol à la fois**
        /// en F1 ; le contrôle de flux par `bufferedAmount` est un livrable de
        /// F3.
        restants: VecDeque<Morceau>,
    },
    Enumeration {
        tampon: TamponEntrees,
        expression: Option<String>,
    },
}

/// L'état que les rappels partagent avec le fil du pont.
///
/// Il vit dans un `Arc` dont **un exemplaire est confié à ProjFS** comme
/// `instancecontext` de `PrjStartVirtualizing`, et repris par
/// [`Virtualisation::drop`] **après** `PrjStopVirtualizing`.
pub struct Etat {
    /// Les treize entrées. `pub` : le fil du pont les appelle aussi.
    pub projfs: chargement::ProjFs,
    /// Posé juste après `PrjStartVirtualizing`.
    ///
    /// ⚠️ **Il ne peut pas être posé avant** : c'est ce même appel qui le rend.
    /// Or ProjFS peut appeler un rappel **pendant** cet appel — d'où le
    /// `Mutex<Option<…>>` plutôt qu'un champ nu, et d'où le fait qu'un rappel
    /// doive tolérer de ne pas encore le voir.
    pub contexte: Mutex<Option<Contexte>>,
    /// Les commandes en vol. **PURE**, sous verrou.
    pub table: Mutex<Table>,
    /// Les sessions d'énumération ouvertes, par GUID d'énumération.
    pub sessions: Mutex<HashMap<[u8; 16], Session>>,
    /// Les handles ProjFS des commandes en vol, par corrélation.
    pub en_attente: Mutex<HashMap<u32, ContexteProjFs>>,
    /// Par où les rappels poussent leurs requêtes vers le transport.
    pub sortant: Sender<VersNavigateur>,
    /// Ce que CE processus a hydraté depuis son démarrage — voir
    /// [`PERIODE_HYDRATATION`] et [`Etat::tracer_hydratation`].
    pub octets_hydrates: AtomicU64,
    pub entrees_hydratees: AtomicU64,
}

impl Etat {
    /// Le contexte de virtualisation, s'il est déjà posé.
    ///
    /// ⚠️ **Un rappel peut survenir PENDANT `PrjStartVirtualizing`**, donc avant
    /// que le contexte ne soit connu : rendre `Option` plutôt que de supposer
    /// est ce qui empêche une panique dans un rappel, c'est-à-dire un abandon
    /// de processus.
    pub fn contexte(&self) -> Option<Contexte> {
        self.contexte.lock().ok().and_then(|c| *c)
    }

    /// `PrjFileNameCompare` — **l'ordre que ProjFS impose** à une énumération.
    /// Ni l'ordre lexicographique d'`OsStr`, ni `Ordering::cmp`.
    pub fn comparer(&self, a: &str, b: &str) -> std::cmp::Ordering {
        let (a, b) = (utf16_nul(a), utf16_nul(b));
        // SÛRETÉ : les deux chaînes sont terminées par un nul et vivent
        // jusqu'à la fin de la fonction. Transcription du `link!` de
        // `mod.rs:38` — retour `i32`.
        let rang = unsafe { (self.projfs.comparer_noms)(PCWSTR(a.as_ptr()), PCWSTR(b.as_ptr())) };
        rang.cmp(&0)
    }

    /// `PrjFileNameMatch` — le filtre `searchExpression`, **facultatif mais
    /// fourni**. L'ignorer ferait qu'un `dir /b *.txt` rendrait tout.
    pub fn apparier(&self, nom: &str, motif: &str) -> bool {
        let (nom, motif) = (utf16_nul(nom), utf16_nul(motif));
        // SÛRETÉ : idem. Transcription du `link!` de `mod.rs:47` — retour
        // `bool`, un octet, comme le `BOOLEAN` de Win32.
        unsafe { (self.projfs.apparier_nom)(PCWSTR(nom.as_ptr()), PCWSTR(motif.as_ptr())) }
    }

    /// Inscrit une commande, retient son contexte ProjFS, et pousse la requête.
    ///
    /// ⚠️ **Le verrou de la table est relâché AVANT l'envoi**, et ce n'est pas
    /// une élégance : la discipline de fil interdit de tenir un verrou pendant
    /// une E/S, et un rappel qui bloquerait ici figerait l'application qui lit
    /// le fichier.
    ///
    /// Rend `false` si le canal du transport est parti — l'appelant rend alors
    /// une erreur plutôt que d'attendre un délai.
    pub fn demander(
        &self,
        commande: i32,
        quoi: Attendue,
        echeance: std::time::Instant,
        contexte: ContexteProjFs,
        type_message: u8,
        entete: &str,
    ) -> bool {
        let Ok(mut table) = self.table.lock() else { return false };
        let correlation = table.inscrire(commande, quoi, echeance);
        drop(table);
        if let Ok(mut attente) = self.en_attente.lock() {
            attente.insert(correlation, contexte);
        }
        let trame = proto::fichiers::encoder(type_message, correlation, entete, &[]);
        if self.sortant.send(VersNavigateur::Requete { correlation, trame }).is_err() {
            // Le transport est parti : retirer ce qu'on vient d'inscrire,
            // sinon la commande attendrait son budget entier pour rien.
            if let Ok(mut table) = self.table.lock() {
                table.resoudre(correlation);
            }
            if let Ok(mut attente) = self.en_attente.lock() {
                attente.remove(&correlation);
            }
            return false;
        }
        true
    }

    /// Demande le morceau suivant d'une lecture déjà entamée.
    ///
    /// Réinscrit la commande **sous une corrélation neuve** : l'ancienne a été
    /// résolue par la réponse qu'on vient d'appliquer, et la réutiliser ferait
    /// qu'une réponse tardive à cette ancienne corrélation serait appliquée au
    /// morceau suivant.
    pub fn demander_lecture(
        &self,
        commande: i32,
        chemin: &str,
        morceau: Morceau,
        flux: windows::core::GUID,
        restants: VecDeque<Morceau>,
    ) {
        let entete = serde_json::to_string(&proto::fichiers::entetes::Lire {
            chemin: chemin.to_string(),
            position: morceau.position,
            longueur: morceau.longueur,
        })
        .expect("un en-tête Lire se sérialise toujours");
        let poursuivie = self.demander(
            commande,
            Attendue::Lire {
                chemin: chemin.to_string(),
                position: morceau.position,
                longueur: morceau.longueur,
            },
            std::time::Instant::now() + crate::pont::table::DELAI_LIRE,
            ContexteProjFs::Lecture { flux: FluxDonnees(flux), restants },
            proto::fichiers::TYPE_LIRE,
            &entete,
        );
        if !poursuivie {
            tracing::warn!(chemin, commande, "lecture interrompue : le canal du pont est parti");
            let contexte = self.contexte();
            if let Some(Contexte(contexte)) = contexte {
                // SÛRETÉ : contexte valide, aucun paramètre étendu.
                let _ = unsafe {
                    (self.projfs.completer_commande)(
                        contexte,
                        commande,
                        windows::core::HRESULT(hresult(Erreur::CanalFerme)),
                        std::ptr::null(),
                    )
                };
            }
        }
    }

    /// Le relevé d'hydratation, à la période [`PERIODE_HYDRATATION`].
    ///
    /// ⚠️ **Il COMPTE ce que le pont a écrit ; il ne MESURE pas le disque, et
    /// c'est une décision, pas une approximation de confort.** Mesurer la
    /// taille occupée par la racine demanderait de la parcourir — donc de
    /// traverser ProjFS, donc de déclencher nos propres rappels
    /// d'énumération, qui inscrivent une commande que **le fil du pont** doit
    /// compléter. Un parcours lancé depuis ce fil-là s'attendrait lui-même ;
    /// lancé depuis un autre, il produirait un aller-retour navigateur réel à
    /// chaque période. **L'instrument détruirait ce qu'il mesure** — la leçon
    /// que ce dépôt a payée deux fois (la trace par paquet du chantier TURN,
    /// la capture d'écran CDP du sous-bloc D1).
    ///
    /// **Ce que le chiffre dit exactement** : les octets et les entrées que
    /// CETTE exécution du pont a hydratés. Pas ce que la racine porte
    /// cumulativement d'exécutions antérieures. La mesure de fond appartient à
    /// F5, avec la politique d'éviction qu'elle servira.
    pub fn tracer_hydratation(&self) {
        tracing::info!(
            octets = self.octets_hydrates.load(Ordering::Relaxed),
            entrees = self.entrees_hydratees.load(Ordering::Relaxed),
            "racine hydratee (par CETTE execution du pont, pas par le disque)"
        );
    }
}


/// Une chaîne UTF-16 terminée par un nul, pour un `PCWSTR`.
fn utf16_nul(texte: &str) -> Vec<u16> {
    texte.encode_utf16().chain(std::iter::once(0)).collect()
}
