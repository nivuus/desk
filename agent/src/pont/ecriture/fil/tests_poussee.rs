//! Write thread tests for pushing chunks - one in flight at a time, writes
//! during a push, late acknowledgements - apart from `tests.rs` to stay under
//! 500 lines.

use super::tests::{modifie, Bac};
use super::*;

/// 🔴 **ONE CHUNK IN FLIGHT AT A TIME.**
///
/// Pushing everything at once would flood the SCTP queue — which F1 already decided
/// to avoid.
///
/// ⚠️ **F3 DID NOT CHANGE THIS, and its window does not apply here.** *(This
/// sentence added "and flow control through `bufferedAmount` is a
/// deliverable of F3".)* `pont::lecture::Fenetre` governs the READ direction, where
/// it is the BROWSER that emits; in writing, it is the bridge, and pushing
/// several chunks in advance would flood precisely what we avoid.
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

/// A write arriving DURING a push is replayed afterwards — and the
/// file is reread **from the start**.
#[test]
fn une_ecriture_pendant_une_poussee_est_rejouee_apres() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"premier");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("a.txt"));
    let (_, c1, _, charge) = bac.trames().last().expect("morceau").clone();
    assert_eq!(charge, b"premier");

    // The user saves again while the push is in flight.
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

/// A DIRECTORY creation produces no chunk.
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

/// A late acknowledgement — arrived after an expiry — is **thrown away**, never
/// applied to the next push.
#[test]
fn un_acquittement_tardif_est_jete() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"a");
    let mut fil = Fil::demarrer(bac.config(true));
    fil.traiter(modifie("a.txt"));
    let (_, c, _, _) = *bac.trames().last().expect("morceau");
    // A correlation that is not the one in flight.
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

/// Writes take their correlations from **the same table** as
/// reads: two sources on a single channel would collide silently.
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
