//! Tests du fil d'écriture : la poussée des morceaux — un en vol à la fois, les
//! écritures pendant une poussée, les acquittements tardifs — à part de
//! `tests.rs` pour tenir sous 500 lignes.

use super::tests::{modifie, Bac};
use super::*;

/// 🔴 **UN MORCEAU EN VOL À LA FOIS.**
///
/// Tout pousser d'un coup inonderait la file SCTP — ce que F1 a déjà décidé
/// d'éviter.
///
/// ⚠️ **F3 N'A PAS CHANGÉ CELA, et sa fenêtre ne s'applique pas ici.** *(Cette
/// phrase ajoutait « et le contrôle de flux par `bufferedAmount` est un
/// livrable de F3 ».)* `pont::lecture::Fenetre` gouverne le sens LECTURE, où
/// c'est le NAVIGATEUR qui émet ; en écriture, c'est le pont, et pousser
/// plusieurs morceaux d'avance inonderait précisément ce qu'on évite.
#[test]
fn un_morceau_en_vol_a_la_fois() {
    let bac = Bac::neuf();
    bac.poser(
        "gros.bin",
        &vec![1u8; 3 * proto::fichiers::TAILLE_TRAME_MAX],
    );
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("gros.bin"));
    let ecritures = |b: &Bac| -> Vec<(u8, u32, Vec<u8>, Vec<u8>)> {
        b.trames()
            .into_iter()
            .filter(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE)
            .collect()
    };
    let mut correlation = {
        let lot = ecritures(&bac);
        assert_eq!(
            lot.len(),
            1,
            "UN SEUL morceau part avant le premier acquittement"
        );
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
        .rfind(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE)
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
        !trames
            .iter()
            .any(|(t, ..)| *t == proto::fichiers::TYPE_ECRIRE),
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
    fil.traiter(Ordre::Fait {
        correlation: c.wrapping_add(1),
    });
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
    let (commande, _, _) = bac
        .table
        .lock()
        .expect("verrou")
        .resoudre(c, Instant::now())
        .expect("inscrite");
    assert_eq!(
        commande, None,
        "une écriture ne complète AUCUN rappel ProjFS"
    );
}
