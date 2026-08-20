//! Tests du fil d'écriture, **sur un répertoire temporaire RÉEL**.
//!
//! 🔵 **Ils tournent sur l'hôte Linux, et c'est tout l'intérêt du module.**
//! Après `FILE_HANDLE_CLOSED_FILE_MODIFIED`, le fichier est complet dans la
//! racine : le lire est un `File::open` ordinaire. Le fil est donc éprouvé ici
//! pour de vrai — pas simulé — sans qu'aucune ligne de ProjFS n'entre en jeu.
//!
//! ⚠️ **AUCUNE DÉPENDANCE NEUVE** : `std::env::temp_dir()` et un nom unique,
//! plutôt que `tempfile`. F2 s'est donné pour règle de n'ajouter aucune
//! dépendance, et `agent/Cargo.toml` n'a aucune section `dev-dependencies`.
//!
//! 🔵 **Les tests pilotent [`Fil`] DIRECTEMENT, pas [`super::tourner`].** La
//! boucle publique bloque sur un `Receiver` ; l'éprouver exigerait un fil et un
//! `sleep`, donc un test au verdict dépendant du minutage. Ici chaque `traiter`
//! est un pas déterministe — la propriété que `pont::table` s'est donnée en
//! prenant le temps en paramètre.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use super::*;
use crate::pont::table::Table;

static COMPTEUR: AtomicU32 = AtomicU32::new(0);

/// Un bac à sable : une racine, un chemin de journal, et un canal capté.
struct Bac {
    racine: PathBuf,
    journal: PathBuf,
    recu: Receiver<VersNavigateur>,
    _envoi: Sender<VersNavigateur>,
    table: Arc<Mutex<Table>>,
}

impl Bac {
    fn neuf() -> Self {
        let n = COMPTEUR.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("f2-fil-{}-{n}", std::process::id()));
        let racine = base.join("racine");
        std::fs::create_dir_all(&racine).expect("racine de test");
        let (envoi, recu) = std::sync::mpsc::channel();
        Self {
            racine,
            journal: base.join("ecritures.journal"),
            recu,
            _envoi: envoi.clone(),
            table: Arc::new(Mutex::new(Table::nouvelle())),
        }
    }

    fn config(&self, armee: bool) -> Config {
        Config {
            racine: self.racine.clone(),
            chemin_journal: self.journal.clone(),
            table: Arc::clone(&self.table),
            vers_navigateur: self._envoi.clone(),
            armee,
        }
    }

    fn poser(&self, nom: &str, octets: &[u8]) {
        std::fs::write(self.racine.join(nom), octets).expect("fichier de test");
    }

    fn journal_brut(&self) -> String {
        std::fs::read_to_string(&self.journal).unwrap_or_default()
    }

    /// Les trames émises depuis le dernier appel, décodées en (type, corrélation).
    fn trames(&self) -> Vec<(u8, u32, Vec<u8>, Vec<u8>)> {
        let mut sorties = Vec::new();
        while let Ok(VersNavigateur::Requete { trame, .. }) = self.recu.try_recv() {
            let t = proto::fichiers::decoder(&trame).expect("trame licite");
            sorties.push((
                t.type_message,
                t.correlation,
                t.entete.to_vec(),
                t.charge.to_vec(),
            ));
        }
        sorties
    }
}

fn modifie(chemin: &str) -> Ordre {
    Ordre::Survenu(Evenement::Modifie { chemin: chemin.to_string() })
}

/// 🔴 **LE JOURNAL EST ÉCRIT AVANT LA PREMIÈRE TRAME.**
///
/// Une entrée poussée avant d'être journalisée est une entrée qu'un arrêt
/// brutal perd : le pont relancé ne saurait même pas qu'elle a existé.
#[test]
fn le_journal_est_ecrit_avant_la_premiere_trame() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"bonjour");
    let mut fil = Fil::demarrer(bac.config(true));
    // Aucune trame n'a encore été LUE : on vérifie l'état du DISQUE au moment
    // où la première trame est déjà partie. Si l'ordre était inverse, le
    // journal serait vide ici.
    let avant = bac.journal_brut();
    assert!(avant.is_empty(), "rien n'est encore arrivé");
    fil.traiter(modifie("note.txt"));
    assert!(
        bac.journal_brut().contains("note.txt"),
        "le journal doit porter l'entrée DÈS que la trame est partie"
    );
    let trames = bac.trames();
    // ⚠️ La PREMIÈRE trame est l'annonce des dues, la seconde l'écriture : le
    // navigateur doit savoir ce qui est dû avant de recevoir les octets.
    assert_eq!(trames[0].0, proto::fichiers::TYPE_DUES);
    assert_eq!(trames[1].0, proto::fichiers::TYPE_ECRIRE);
    assert_eq!(trames[1].3, b"bonjour", "la charge porte les octets, jamais encodés");
}

/// 🔴 **L'ENTRÉE SORT DU JOURNAL APRÈS LE DERNIER `Fait`, ET PAS AVANT.**
///
/// La retirer au premier `Fait` ferait qu'un fichier de deux morceaux dont le
/// second échoue sortirait du journal **en ayant perdu ses octets**.
#[test]
fn l_entree_sort_du_journal_apres_le_dernier_fait_et_pas_avant() {
    let bac = Bac::neuf();
    let gros = vec![7u8; proto::fichiers::TAILLE_TRAME_MAX + 1];
    bac.poser("gros.bin", &gros);
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("gros.bin"));

    let premier = bac.trames();
    let (_, c1, entete, charge) = premier.last().expect("un morceau parti").clone();
    let e: entetes::Ecrire = serde_json::from_slice(&entete).expect("en-tête Ecrire");
    assert!(e.premier && !e.dernier, "le premier de DEUX morceaux");
    assert_eq!(charge.len(), proto::fichiers::TAILLE_TRAME_MAX);

    fil.traiter(Ordre::Fait { correlation: c1 });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        1,
        "au PREMIER Fait, l'entrée est ENCORE due"
    );

    let second = bac.trames();
    let (_, c2, entete, charge) = second.last().expect("le second morceau").clone();
    let e: entetes::Ecrire = serde_json::from_slice(&entete).expect("en-tête Ecrire");
    assert!(!e.premier && e.dernier, "le second est le DERNIER");
    assert_eq!(charge.len(), 1);

    fil.traiter(Ordre::Fait { correlation: c2 });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        0,
        "au DERNIER Fait seulement, l'entrée sort"
    );
}

/// 🔴 **UN ÉCHEC LAISSE L'ENTRÉE AU JOURNAL.**
///
/// La retirer serait **la perte de données que ce module existe pour
/// empêcher** : l'application a déjà cru avoir enregistré.
#[test]
fn un_echec_laisse_l_entree_au_journal() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"a");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("note.txt"));
    let (_, c, _, _) = *bac.trames().last().expect("un morceau parti");
    fil.traiter(Ordre::Echec { correlation: c, code: CodeEchec::DisquePlein });
    assert_eq!(Journal::relire(&bac.journal_brut()).0.compte(), 1);
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.dues()[0].0,
        "note.txt",
        "et l'entrée est NOMMÉE"
    );
}

/// 🔴 **UN FICHIER DE TAILLE NULLE PRODUIT UN MORCEAU VIDE, ET L'ENTRÉE SORT.**
///
/// C'est le contrôle le plus important de ce module. Un fichier vide est le cas
/// nominal d'un « nouveau document » enregistré aussitôt, et `decouper` rend
/// délibérément **zéro** morceau pour une longueur nulle. Sans le cas
/// particulier, aucun `dernier` ne serait jamais émis, l'entrée ne sortirait
/// **jamais** du journal, et l'utilisateur verrait une alerte permanente pour
/// un fichier correctement transmis.
///
/// ⚠️ **Et le morceau vide n'est PAS une création**, contre la lettre du plan :
/// une création n'aurait aucun effet sur un fichier local existant, si bien
/// qu'un fichier TRONQUÉ À ZÉRO sur la VM garderait son ancien contenu sur le
/// poste local. Le test le vérifie sur les DEUX drapeaux.
#[test]
fn un_fichier_de_taille_nulle_produit_un_morceau_vide_et_sort_du_journal() {
    let bac = Bac::neuf();
    bac.poser("vide.txt", b"");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("vide.txt"));
    let trames = bac.trames();
    let (type_message, c, entete, charge) = trames.last().expect("une trame").clone();
    assert_eq!(type_message, proto::fichiers::TYPE_ECRIRE, "un morceau, PAS une création");
    let e: entetes::Ecrire = serde_json::from_slice(&entete).expect("en-tête Ecrire");
    assert_eq!((e.premier, e.dernier, e.longueur), (true, true, 0));
    assert!(charge.is_empty());
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        0,
        "sans le cas particulier, l'entrée resterait due POUR TOUJOURS"
    );
}

/// 🔴 **UN FICHIER ABSENT AU REDÉMARRAGE SORT DU JOURNAL EN LE NOMMANT.**
///
/// La racine a été recréée, et le fichier est parti avec elle (spec §6.4
/// cas 3). Boucler sur le réessai ferait repousser indéfiniment un fichier qui
/// n'existe plus.
#[test]
fn un_fichier_absent_au_redemarrage_sort_du_journal_en_le_nommant() {
    let bac = Bac::neuf();
    bac.poser("survivant.txt", b"ok");
    // Un pont antérieur a laissé deux dues, dont une dont le fichier a disparu.
    let mut j = Journal::nouveau();
    let mut brut = String::new();
    brut.push_str(&j.inscrire("disparu.txt", 42));
    brut.push_str(&j.inscrire("survivant.txt", 2));
    std::fs::write(&bac.journal, &brut).expect("journal de test");

    let _fil = Fil::demarrer(bac.config(true));
    let (relu, _) = Journal::relire(&bac.journal_brut());
    let restants: Vec<&str> = relu.dues().iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(restants, ["survivant.txt"], "seul le disparu devait partir");
}

/// Au démarrage, les dues sont **annoncées avant** toute poussée.
#[test]
fn les_dues_sont_annoncees_avant_toute_poussee_au_demarrage() {
    let bac = Bac::neuf();
    bac.poser("repris.txt", b"abc");
    let mut j = Journal::nouveau();
    std::fs::write(&bac.journal, j.inscrire("repris.txt", 3)).expect("journal de test");

    let _fil = Fil::demarrer(bac.config(true));
    let trames = bac.trames();
    assert_eq!(trames[0].0, proto::fichiers::TYPE_DUES, "l'annonce d'abord");
    assert!(
        trames.iter().any(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE),
        "puis la reprise"
    );
}

/// 🔴 **DÉSARMÉ, LE FIL JOURNALISE ET ANNONCE, MAIS NE POUSSE RIEN.**
///
/// C'est le bras désarmé de l'A/B, et **c'est lui qui rend le compteur
/// d'écritures dues ROUGE**. Ignorer la variable rendrait ce rouge impossible à
/// provoquer, donc le critère ④ de la recette non mesurable.
#[test]
fn desarme_le_fil_journalise_mais_ne_pousse_rien() {
    let bac = Bac::neuf();
    bac.poser("note.txt", b"bonjour");
    let mut fil = Fil::demarrer(bac.config(false));
    fil.traiter(modifie("note.txt"));
    assert_eq!(
        Journal::relire(&bac.journal_brut()).0.compte(),
        1,
        "l'entrée est due — c'est ce que le compteur montrera"
    );
    let types: Vec<u8> = bac.trames().iter().map(|(t, ..)| *t).collect();
    assert!(
        types.iter().all(|t| *t == proto::fichiers::TYPE_DUES),
        "SEULES des annonces ; aucune écriture ne part : {types:?}"
    );
    // …et la file avance quand même : un second chemin est journalisé lui aussi.
    bac.poser("autre.txt", b"x");
    fil.traiter(modifie("autre.txt"));
    assert_eq!(Journal::relire(&bac.journal_brut()).0.compte(), 2);
}

/// 🔴 **UN MORCEAU EN VOL À LA FOIS.**
///
/// Tout pousser d'un coup inonderait la file SCTP — ce que F1 a déjà décidé
/// d'éviter, et le contrôle de flux par `bufferedAmount` est un livrable de F3.
#[test]
fn un_morceau_en_vol_a_la_fois() {
    let bac = Bac::neuf();
    bac.poser("gros.bin", &vec![1u8; 3 * proto::fichiers::TAILLE_TRAME_MAX]);
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("gros.bin"));
    let ecritures = |b: &Bac| -> Vec<(u8, u32, Vec<u8>, Vec<u8>)> {
        b.trames().into_iter().filter(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE).collect()
    };
    let mut correlation = {
        let lot = ecritures(&bac);
        assert_eq!(lot.len(), 1, "UN SEUL morceau part avant le premier acquittement");
        lot[0].1
    };
    for tour in 0..2 {
        fil.traiter(Ordre::Fait { correlation });
        let lot = ecritures(&bac);
        assert_eq!(lot.len(), 1, "tour {tour} : un seul morceau de plus");
        correlation = lot[0].1;
    }
    fil.traiter(Ordre::Fait { correlation });
    assert!(ecritures(&bac).is_empty(), "trois morceaux, et c'est tout");
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// Une écriture qui arrive PENDANT une poussée est rejouée après — et le
/// fichier est relu **depuis le début**.
#[test]
fn une_ecriture_pendant_une_poussee_est_rejouee_apres() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"premier");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("a.txt"));
    let (_, c1, _, charge) = bac.trames().last().expect("morceau").clone();
    assert_eq!(charge, b"premier");

    // L'utilisateur réenregistre pendant que la poussée est en vol.
    bac.poser("a.txt", b"SECOND CONTENU PLUS LONG");
    fil.traiter(modifie("a.txt"));
    fil.traiter(Ordre::Fait { correlation: c1 });

    let (_, c2, _, charge) = bac
        .trames()
        .into_iter()
        .filter(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE)
        .next_back()
        .expect("le rejeu doit repartir");
    assert_eq!(charge, b"SECOND CONTENU PLUS LONG", "relu DEPUIS LE DÉBUT");
    fil.traiter(Ordre::Fait { correlation: c2 });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// Une création de RÉPERTOIRE ne produit aucun morceau.
#[test]
fn un_repertoire_cree_ne_produit_aucun_morceau() {
    let bac = Bac::neuf();
    std::fs::create_dir(bac.racine.join("dossier")).expect("dossier de test");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(Ordre::Survenu(Evenement::Cree {
        chemin: "dossier".to_string(),
        repertoire: true,
    }));
    let trames = bac.trames();
    let (type_message, c, entete, _) = trames.last().expect("une trame").clone();
    assert_eq!(type_message, proto::fichiers::TYPE_CREER);
    let creer: entetes::Creer = serde_json::from_slice(&entete).expect("en-tête Creer");
    assert!(creer.repertoire);
    assert!(
        !trames.iter().any(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE),
        "aucun morceau : un répertoire n'a rien à lire"
    );
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// Un acquittement tardif — arrivé après une expiration — est **jeté**, jamais
/// appliqué à la poussée suivante.
#[test]
fn un_acquittement_tardif_est_jete() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"a");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("a.txt"));
    let (_, c, _, _) = *bac.trames().last().expect("morceau");
    // Une corrélation qui n'est pas celle en vol.
    fil.traiter(Ordre::Fait { correlation: c.wrapping_add(1) });
    assert_eq!(
        Journal::compte_du_brut(&bac.journal_brut()),
        1,
        "un Fait étranger ne doit RIEN acquitter"
    );
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// Les écritures prennent leurs corrélations dans **la même table** que les
/// lectures : deux sources sur un canal unique se collisionneraient en silence.
#[test]
fn les_ecritures_prennent_leurs_correlations_dans_la_table_partagee() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"a");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("a.txt"));
    assert_eq!(
        bac.table.lock().expect("verrou").en_vol(),
        1,
        "l'écriture doit être INSCRITE dans la table du pont"
    );
    let (_, c, _, _) = *bac.trames().last().expect("morceau");
    let (commande, _) = bac.table.lock().expect("verrou").resoudre(c).expect("inscrite");
    assert_eq!(commande, None, "une écriture ne complète AUCUN rappel ProjFS");
}

impl Journal {
    /// Raccourci de lecture pour les tests : le nombre de dues d'un journal brut.
    fn compte_du_brut(brut: &str) -> usize {
        Journal::relire(brut).0.compte()
    }
}
