//! Le fil d'écriture : il lit le fichier local, le découpe, pousse les trames,
//! attend le `Fait`, tient le journal, et annonce les dues.
//!
//! # 🔵 Pourquoi ce module est PUR, alors qu'il lit un fichier « de ProjFS »
//!
//! Après `FILE_HANDLE_CLOSED_FILE_MODIFIED`, le fichier est **complet** dans la
//! racine : ProjFS n'appelle `GetFileData` que sur un **substitut**. Le lire est
//! donc un `std::fs::File::open` **ordinaire**, portable, testable sur Linux
//! avec un répertoire temporaire réel.
//!
//! ⚠️ **C'est une INFÉRENCE du modèle de ProjFS, pas une mesure.** Si elle est
//! fausse, la lecture ré-entre dans nos propres rappels. **Elle ne provoquerait
//! pas d'interblocage** — les commandes ainsi créées sont complétées par le
//! **fil du pont**, un fil distinct —, mais le pont relirait ses propres octets
//! à travers le navigateur, ce qui serait **visible au journal** : des `Lire`
//! sur un chemin en cours d'écriture. Le critère ④ de la recette existe pour
//! trancher.
//!
//! # 🔴 LE FIL EST DÉDIÉ, ET JAMAIS CELUI DU PONT
//!
//! `pont/service.rs` l'écrit déjà pour le relevé d'hydratation : « un `read_dir`
//! sur la racine traverserait ProjFS, donc déclencherait nos propres rappels
//! d'énumération, qui inscrivent une commande que **ce fil-ci** doit compléter :
//! **il s'attendrait lui-même** ». **La même phrase vaut ici, et c'est la
//! raison d'être de ce fil.**
//!
//! # L'ordre de la séquence n'est PAS négociable
//!
//! 1. l'événement arrive ;
//! 2. **le journal est écrit ET VIDÉ (`sync_all`) AVANT la première trame** —
//!    une entrée poussée avant d'être journalisée est une entrée qu'un arrêt
//!    brutal perd ;
//! 3. `TYPE_DUES` est annoncé ;
//! 4. le fichier local est lu et découpé ;
//! 5. **un morceau en vol à la fois** — *(ces lignes ajoutaient « le contrôle
//!    de flux par `bufferedAmount` / `SEUIL_TAMPON` est un livrable de F3, et
//!    l'implémenter à moitié ici serait pire ». **F3 est arrivé, et il ne
//!    change RIEN ici.**)*
//!
//!    ⚠️ **La fenêtre de F3 est celle de la LECTURE, pas de l'écriture, et la
//!    distinction n'est pas un détail** : en lecture, c'est le NAVIGATEUR qui
//!    émet les gros messages, et la fenêtre du pont sert à ne pas le laisser
//!    inactif entre deux morceaux. En écriture, c'est le PONT qui les émet —
//!    demander plusieurs morceaux d'avance n'aurait aucun sens, et en pousser
//!    plusieurs inonderait la file SCTP, ce que F1 a déjà décidé d'éviter. La
//!    contre-pression `bufferedAmount`, elle, vit côté navigateur
//!    (`client/src/fichiers/flux.ts`) et ne couvre donc pas ce sens-ci ;
//! 6. sur le `Fait` du **dernier** morceau : `journal.retirer`, **puis**
//!    `TYPE_DUES` réannoncé ;
//! 7. sur un `Echec` ou une expiration : **l'entrée RESTE au journal**, un
//!    `warn!` nomme le chemin et le code.

mod disque;
mod mutations;
mod reprise;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::{Evenement, File};
use crate::pont::decoupe::{decouper, Morceau};
use crate::pont::journal::Journal;
use crate::pont::mutation::FileMutations;
use crate::pont::table::{Attendue, Table, DELAI_ECRIRE};
use crate::pont::transport::VersNavigateur;
use proto::fichiers::{entetes, CodeEchec};

/// La corrélation portée par une **annonce**.
///
/// ⚠️ **Elle n'identifie RIEN** : une annonce n'attend aucune réponse, et le
/// navigateur ne s'en sert pas. Elle n'est là que pour le journal du transport,
/// qui trace `correlation` sur chaque émission.
///
/// ⚠️ **Elle n'est PAS réservée dans [`Table`]** : la lui faire enjamber
/// coûterait un cas particulier dans la distribution des corrélations pour un
/// gain de lisibilité de journal. Une collision exigerait 2^32 inscriptions
/// dans une même exécution du pont.
const CORRELATION_ANNONCE: u32 = u32::MAX;

/// Au-delà de cette taille, une écriture due est journalisée en `warn!` et
/// **nommée** à la page-shell.
///
/// 🔴 **CE N'EST PAS UN PLAFOND DE REFUS, et la distinction est de fond.** Le
/// seul endroit où un refus de taille serait **visible par l'application** est
/// `PRE_CONVERT_TO_FULL` — mais on n'y connaît que la taille **d'AVANT**
/// l'écriture, qui ne borne pas celle d'après. Un plafond appliqué au
/// write-back, lui, serait **invisible** : le handle est refermé depuis
/// longtemps. La spec §3.5.2 prescrivait `TAILLE_MAX_FICHIER` avec
/// `ERROR_DISK_FULL` ; **ce code d'erreur n'atteindrait personne.**
///
/// ⚠️ **NON CALIBRÉE.**
pub const TAILLE_ECRITURE_SIGNALEE: u64 = 64 * 1024 * 1024;

/// Ce que le fil d'écriture reçoit.
///
/// ⚠️ **UN SEUL CANAL, et c'est une divergence déclarée avec le plan de F2**,
/// dont la signature prend **deux** `Receiver` (les événements, les faits).
/// Deux récepteurs sur un fil bloquant imposeraient un sondage alterné, donc
/// une latence bornée par un délai arbitraire de plus — et un test qui dépend
/// d'un `sleep`. Un canal unique rend la boucle déterministe, donc testable
/// sans dormir : la propriété que `pont::table` s'est donnée pour l'expiration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// Une notification ProjFS a désigné un chemin.
    Survenu(Evenement),
    /// Le navigateur a acquitté un morceau.
    Fait { correlation: u32 },
    /// Le navigateur a refusé, ou la commande a expiré.
    Echec { correlation: u32, code: CodeEchec },
}

/// Ce dont le fil a besoin pour tourner.
pub struct Config {
    /// La racine de virtualisation, où vivent les fichiers hydratés.
    pub racine: PathBuf,
    /// Le journal de reprise, **hors de la racine**.
    pub chemin_journal: PathBuf,
    /// **La MÊME table que les lectures** : deux sources de corrélations sur un
    /// canal unique se collisionneraient en silence.
    pub table: Arc<Mutex<Table>>,
    pub vers_navigateur: Sender<VersNavigateur>,
    /// `PONT_ECRITURE` : `false` = le bras désarmé de l'A/B.
    pub armee: bool,
}

/// La boucle du fil. Rend quand le canal des ordres se ferme.
pub fn tourner(config: Config, ordres: Receiver<Ordre>) {
    let mut fil = Fil::demarrer(config);
    while let Ok(ordre) = ordres.recv() {
        fil.traiter(ordre);
    }
    tracing::info!("fil d'ecriture du pont arrete");
}

// ⚠️ `pub(super)` PARCE QUE LE CHAMP QUI LA PORTE L'EST, et pas l'inverse.
// L'extraction de `fil/mutations.rs` a rendu `Fil::en_cours` visible au module
// parent sans hisser son TYPE avec lui : `rustc` le dit par
// `private_interfaces` — **un avertissement d'une autre famille que
// `dead_code`**, et ce dépôt vérifie ses avertissements par leur NATURE à
// chaque clôture. Une extraction déplace la visibilité autant que le code.
pub(super) struct EnCours {
    chemin: String,
    restants: VecDeque<Morceau>,
    correlation: u32,
    dernier_envoye: bool,
    octets: u64,
    debut: Instant,
}

pub(super) struct Fil {
    pub(super) config: Config,
    pub(super) journal: Journal,
    pub(super) file: File,
    pub(super) en_cours: Option<EnCours>,
    /// Les mutations en vol et en attente (F3).
    ///
    /// ⚠️ **DISTINCTE de la file d'écriture, et le rester est le point.** Une
    /// mutation ne porte aucun octet, ne s'inscrit pas au journal des dues, et
    /// **ne se coalesce pas** : `a`→`b` puis `b`→`c` sont deux gestes dont
    /// l'ordre est le sens.
    pub(super) mutations: FileMutations,
    /// La corrélation de la mutation en vol, s'il y en a une.
    pub(super) mutation_en_vol: Option<u32>,
}

impl Fil {
    fn demarrer(config: Config) -> Self {
        if !config.armee {
            tracing::warn!(
                "poussee d'ecriture DESARMEE (PONT_ECRITURE=0) : bras de banc, jamais une \
                 configuration livree"
            );
        }
        let contenu = std::fs::read_to_string(&config.chemin_journal).unwrap_or_default();
        let (journal, ignorees) = Journal::relire(&contenu);
        if ignorees > 0 {
            // Une ligne partielle est le seul dommage qu'un arrêt brutal puisse
            // causer à un fichier en ajout. La compter la rend visible ; la
            // taire ferait croire à un journal intact.
            tracing::warn!(ignorees, "lignes illisibles jetees au rechargement du journal");
        }
        let mut fil = Self {
            config,
            journal,
            file: File::nouvelle(),
            en_cours: None,
            mutations: FileMutations::nouvelle(),
            mutation_en_vol: None,
        };
        fil.reprendre();
        fil
    }

    fn traiter(&mut self, ordre: Ordre) {
        match ordre {
            Ordre::Survenu(evenement) if evenement.est_mutation() => {
                // 🔴 **UNE MUTATION N'EST PAS UNE ÉCRITURE, ET LA CONFONDRE
                // DÉTRUIRAIT.** Sans ce bras, `commencer` tomberait sur le
                // chemin de CONTENU : `disque::taille_de` rendrait 0 sur une
                // source qui n'existe plus — renommée, ou effacée —, `decouper`
                // rendrait zéro morceau, et le morceau vide de secours
                // **TRONQUERAIT LE FICHIER LOCAL À ZÉRO** ou **RECRÉERAIT
                // VIDE** ce que l'utilisateur vient d'effacer.
                //
                // ⚠️ **C'est le bras catch-all silencieux que ce dépôt a payé
                // CINQ fois sur `capteur/pont_media.rs`** (D5 `Sommeil`, D6
                // `Part`, D7 `Audio`, D8 `PleinEcran`, presse-papier P1), sous
                // une forme pire : là-bas le message était perdu, ici il aurait
                // été appliqué au mauvais verbe.
                //
                // ⚠️ **Une mutation ne passe donc NI par le journal des
                // écritures dues, NI par la file de contenu** : elle ne porte
                // aucun octet, et l'inscrire ferait monter le compteur de la
                // page-shell pour un geste qui n'a rien à transférer.
                self.mutation(evenement);
            }
            Ordre::Survenu(evenement) => {
                let chemin = evenement.chemin().to_string();
                let octets = disque::taille_de(&self.config.racine, &chemin);
                // ÉTAPE 2 : le journal AVANT la première trame.
                let ligne = self.journal.inscrire(&chemin, octets);
                self.ecrire_journal(&ligne);
                if octets > TAILLE_ECRITURE_SIGNALEE {
                    tracing::warn!(
                        chemin, octets, seuil = TAILLE_ECRITURE_SIGNALEE,
                        "ecriture due volumineuse : elle restera longtemps dans la fenetre de perte"
                    );
                }
                // ÉTAPE 3.
                self.annoncer_les_dues();
                if let Some(a_pousser) = self.file.signaler(evenement) {
                    self.commencer(a_pousser);
                }
            }
            Ordre::Fait { correlation } => self.acquitte(correlation),
            Ordre::Echec { correlation, code } => self.refuse(correlation, code),
        }
    }

    pub(super) fn commencer(&mut self, evenement: Evenement) {
        let chemin = evenement.chemin().to_string();
        if !self.config.armee {
            // Le bras DÉSARMÉ : on journalise et on annonce, on ne pousse
            // JAMAIS. L'entrée reste donc due, et le compteur de la page-shell
            // monte sans jamais redescendre — c'est ce qui le rend rouge.
            if let Some(suivant) = self.file.terminee(&chemin) {
                self.commencer(suivant);
            }
            return;
        }
        if evenement.est_repertoire() {
            // Règle 4 : un répertoire ne porte AUCUN contenu.
            self.pousser_creation(&chemin, true);
            return;
        }
        if matches!(evenement, Evenement::Cree { .. }) {
            self.pousser_creation(&chemin, false);
            return;
        }
        let octets = disque::taille_de(&self.config.racine, &chemin);
        let mut morceaux: VecDeque<Morceau> =
            decouper(0, octets, proto::fichiers::TAILLE_TRAME_MAX).into();
        if morceaux.is_empty() {
            // 🔴 **UN FICHIER VIDE EST LE CAS NOMINAL D'UN « NOUVEAU DOCUMENT »
            // ENREGISTRÉ AUSSITÔT**, et `decouper` rend délibérément ZÉRO
            // morceau pour une longueur nulle. Sans ce cas particulier, aucun
            // `dernier` ne serait jamais émis, l'entrée ne sortirait JAMAIS du
            // journal, et l'utilisateur verrait une alerte permanente pour un
            // fichier correctement transmis. *Un compteur qui ne redescend
            // jamais est aussi faux qu'un compteur qui ne monte jamais.*
            //
            // ⚠️ **ET C'EST UN MORCEAU VIDE, PAS UNE CRÉATION**, contre la
            // lettre du plan de F2 (« un fichier de taille nulle produit une
            // création et zéro morceau »). Une création n'a aucun effet sur un
            // fichier local qui existe déjà : un fichier TRONQUÉ À ZÉRO sur la
            // VM garderait son ancien contenu sur le poste local, ce qui est
            // une corruption silencieuse. Le morceau vide, lui, ouvre le flux
            // sans `keepExistingData` et le referme : le fichier local devient
            // vide, ce qu'il doit être.
            morceaux.push_back(Morceau { position: 0, longueur: 0 });
        }
        self.en_cours = Some(EnCours {
            chemin,
            restants: morceaux,
            correlation: 0,
            dernier_envoye: false,
            octets,
            debut: Instant::now(),
        });
        self.pousser_morceau(true);
    }

    fn pousser_creation(&mut self, chemin: &str, repertoire: bool) {
        let entete = serde_json::to_string(&entetes::Creer {
            chemin: chemin.to_string(),
            repertoire,
        })
        .expect("un en-tete Creer se serialise toujours");
        let correlation = self.inscrire(Attendue::Creer { chemin: chemin.to_string() });
        self.en_cours = Some(EnCours {
            chemin: chemin.to_string(),
            restants: VecDeque::new(),
            correlation,
            dernier_envoye: true,
            octets: 0,
            debut: Instant::now(),
        });
        self.emettre(proto::fichiers::TYPE_CREER, correlation, &entete, &[]);
        tracing::debug!(chemin, repertoire, correlation, "creation poussee");
    }

    /// Pousse le morceau suivant. `premier` n'est vrai qu'au tout premier.
    fn pousser_morceau(&mut self, premier: bool) {
        let Some(en_cours) = self.en_cours.as_mut() else { return };
        let Some(morceau) = en_cours.restants.pop_front() else { return };
        let dernier = en_cours.restants.is_empty();
        let chemin = en_cours.chemin.clone();
        let entete = serde_json::to_string(&entetes::Ecrire {
            chemin: chemin.clone(),
            position: morceau.position,
            longueur: morceau.longueur,
            premier,
            dernier,
        })
        .expect("un en-tete Ecrire se serialise toujours");

        let octets = match disque::lire(&self.config.racine, &chemin, morceau) {
            Ok(octets) => octets,
            Err(erreur) => {
                tracing::warn!(chemin, %erreur, "lecture du fichier local echouee : ecriture due RETENUE");
                self.terminer(&chemin, false);
                return;
            }
        };
        let correlation = self.inscrire(Attendue::Ecrire { chemin: chemin.clone(), dernier });
        if let Some(en_cours) = self.en_cours.as_mut() {
            en_cours.correlation = correlation;
            en_cours.dernier_envoye = dernier;
        }
        self.emettre(proto::fichiers::TYPE_ECRIRE, correlation, &entete, &octets);
        tracing::debug!(
            chemin, correlation,
            position = morceau.position, longueur = morceau.longueur, premier, dernier,
            "ecriture poussee"
        );
    }

    fn acquitte(&mut self, correlation: u32) {
        if self.mutation_en_vol == Some(correlation) {
            tracing::info!(correlation, "mutation acquittee : le poste local a suivi");
            self.terminer_mutation(true);
            return;
        }
        let Some(en_cours) = self.en_cours.as_ref() else { return };
        if en_cours.correlation != correlation {
            // Un `Fait` tardif, arrivé après une expiration. Le jeter est
            // l'invariant de `Table::resoudre`, transposé.
            tracing::debug!(correlation, "acquittement tardif ou inconnu : jete");
            return;
        }
        if !en_cours.dernier_envoye {
            self.pousser_morceau(false);
            return;
        }
        let chemin = en_cours.chemin.clone();
        let octets = en_cours.octets;
        let duree_ms = en_cours.debut.elapsed().as_millis();
        // ÉTAPE 6 : le journal, PUIS l'annonce.
        tracing::info!(
            chemin, octets, duree_ms,
            "ecriture acquittee : les octets sont sur le poste local"
        );
        self.terminer(&chemin, true);
    }

    fn refuse(&mut self, correlation: u32, code: CodeEchec) {
        if self.mutation_en_vol == Some(correlation) {
            // 🔴 **UNE MUTATION EN ÉCHEC NE SERA JAMAIS REJOUÉE**, et c'est ce
            // qui la distingue d'une écriture : ProjFS ne renvoie pas de
            // notification pour un geste déjà accompli dans la VM. Les deux
            // côtés ont DIVERGÉ, définitivement, et le seul remède est humain —
            // d'où le `warn!` et la ligne de la page-shell.
            tracing::warn!(
                correlation, ?code,
                "MUTATION REFUSEE : le poste local n'a PAS suivi, et rien ne le rejouera"
            );
            self.terminer_mutation(false);
            return;
        }
        let Some(en_cours) = self.en_cours.as_ref() else { return };
        if en_cours.correlation != correlation {
            return;
        }
        let chemin = en_cours.chemin.clone();
        // 🔴 **L'ENTRÉE RESTE AU JOURNAL.** La retirer serait la perte de
        // données que ce module existe pour empêcher.
        tracing::warn!(
            chemin, ?code,
            "ecriture due retenue : le navigateur a refuse, l'entree reste au journal"
        );
        self.terminer(&chemin, false);
    }

    /// Clôt la poussée en cours. `acquittee` décide si l'entrée sort du journal.
    fn terminer(&mut self, chemin: &str, acquittee: bool) {
        self.en_cours = None;
        if acquittee {
            let ligne = self.journal.retirer(chemin);
            self.ecrire_journal(&ligne);
        }
        self.annoncer_les_dues();
        if let Some(suivant) = self.file.terminee(chemin) {
            self.commencer(suivant);
        }
    }

    pub(super) fn annoncer_les_dues(&self) {
        let dues: Vec<entetes::Due> = self
            .journal
            .dues()
            .iter()
            .map(|(chemin, octets)| entetes::Due { chemin: chemin.clone(), octets: *octets })
            .collect();
        let entete = serde_json::to_string(&entetes::Dues { dues })
            .expect("un en-tete Dues se serialise toujours");
        self.emettre(proto::fichiers::TYPE_DUES, CORRELATION_ANNONCE, &entete, &[]);
    }

    pub(super) fn emettre(&self, type_message: u8, correlation: u32, entete: &str, charge: &[u8]) {
        let trame = proto::fichiers::encoder(type_message, correlation, entete, charge);
        if self
            .config
            .vers_navigateur
            .send(VersNavigateur::Requete { correlation, trame })
            .is_err()
        {
            tracing::warn!(correlation, "transport du pont parti : trame d'ecriture non emise");
        }
    }

    fn inscrire(&self, quoi: Attendue) -> u32 {
        let echeance = Instant::now() + DELAI_ECRIRE;
        match self.config.table.lock() {
            Ok(mut table) => table.inscrire_sans_commande(quoi, echeance),
            Err(empoisonne) => empoisonne.into_inner().inscrire_sans_commande(quoi, echeance),
        }
    }

    /// Ajoute une ligne au journal, et le compacte si c'est possible.
    ///
    /// ⚠️ **UN ÉCHEC D'ÉCRITURE DU JOURNAL N'EMPÊCHE PAS LA POUSSÉE.** Ne pas
    /// pousser perdrait la donnée tout autant, et **sans même la nommer**. Le
    /// `warn!` est tout ce qu'on peut faire — et il dit exactement ce qui est
    /// perdu : la capacité de REPRENDRE cette entrée après un arrêt brutal.
    pub(super) fn ecrire_journal(&mut self, ligne: &str) {
        if let Err(erreur) = disque::ajouter(&self.config.chemin_journal, ligne) {
            tracing::warn!(
                %erreur, chemin = %self.config.chemin_journal.display(),
                "journal des ecritures dues non ecrit : une reprise apres arret brutal perdrait \
                 cette entree"
            );
            return;
        }
        disque::compacter_si_possible(&self.config.chemin_journal, &self.journal);
    }
}

#[cfg(test)]
mod tests;
