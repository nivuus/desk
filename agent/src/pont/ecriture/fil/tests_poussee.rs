//! Write thread tests for pushing chunks - one in flight at a time, writes
//! during a push, late acknowledgements - apart from `tests.rs` to stay under
//! 500 lines.

use super::tests::{modified, Bac};
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
fn one_chunk_in_flight_at_a_time() {
    let bac = Bac::neuf();
    bac.poser("gros.bin", &vec![1u8; 3 * proto::files::MAX_FRAME_SIZE]);
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("gros.bin"));
    let ecritures = |b: &Bac| -> Vec<(u8, u32, Vec<u8>, Vec<u8>)> {
        b.trames()
            .into_iter()
            .filter(|(t, ..)| *t == proto::files::TYPE_WRITE)
            .collect()
    };
    let mut correlation = {
        let lot = ecritures(&bac);
        assert_eq!(
            lot.len(),
            1,
            "ONE SINGLE chunk leaves before the first acknowledgement"
        );
        lot[0].1
    };
    for tour in 0..2 {
        fil.traiter(Ordre::Fait { correlation });
        let lot = ecritures(&bac);
        assert_eq!(lot.len(), 1, "round {tour}: only one more chunk");
        correlation = lot[0].1;
    }
    fil.traiter(Ordre::Fait { correlation });
    assert!(ecritures(&bac).is_empty(), "three chunks, and that is all");
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// A write arriving DURING a push is replayed afterwards — and the
/// file is reread **from the start**.
#[test]
fn a_write_during_a_push_is_replayed_afterwards() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"premier");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("a.txt"));
    let (_, c1, _, charge) = bac.trames().last().expect("morceau").clone();
    assert_eq!(charge, b"premier");

    // The user saves again while the push is in flight.
    bac.poser("a.txt", b"SECOND CONTENU PLUS LONG");
    fil.traiter(modified("a.txt"));
    fil.traiter(Ordre::Fait { correlation: c1 });

    let (_, c2, _, charge) = bac
        .trames()
        .into_iter()
        .rfind(|(t, ..)| *t == proto::files::TYPE_WRITE)
        .expect("the replay must restart");
    assert_eq!(
        charge, b"SECOND CONTENU PLUS LONG",
        "read again FROM THE START"
    );
    fil.traiter(Ordre::Fait { correlation: c2 });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// A DIRECTORY creation produces no chunk.
#[test]
fn a_created_directory_produces_no_chunk() {
    let bac = Bac::neuf();
    std::fs::create_dir(bac.racine.join("dossier")).expect("test folder");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(Ordre::Survenu(Evenement::Cree {
        chemin: "dossier".to_string(),
        repertoire: true,
    }));
    let trames = bac.trames();
    let (type_message, c, entete, _) = trames.last().expect("one frame").clone();
    assert_eq!(type_message, proto::files::TYPE_CREATE);
    let create: entetes::Create = serde_json::from_slice(&entete).expect("Creer header");
    assert!(create.repertoire);
    assert!(
        !trames.iter().any(|(t, ..)| *t == proto::files::TYPE_WRITE),
        "no chunk: a directory has nothing to read"
    );
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// A late acknowledgement — arrived after an expiry — is **thrown away**, never
/// applied to the next push.
#[test]
fn a_late_acknowledgement_is_dropped() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"a");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("a.txt"));
    let (_, c, _, _) = *bac.trames().last().expect("morceau");
    // A correlation that is not the one in flight.
    fil.traiter(Ordre::Fait {
        correlation: c.wrapping_add(1),
    });
    assert_eq!(
        Journal::compte_du_brut(&bac.journal_brut()),
        1,
        "a foreign Fait must acknowledge NOTHING"
    );
    fil.traiter(Ordre::Fait { correlation: c });
    assert_eq!(Journal::compte_du_brut(&bac.journal_brut()), 0);
}

/// Writes take their correlations from **the same table** as
/// reads: two sources on a single channel would collide silently.
#[test]
fn writes_take_their_correlations_from_the_shared_table() {
    let bac = Bac::neuf();
    bac.poser("a.txt", b"a");
    let mut fil = Fil::start(bac.config(true));
    fil.traiter(modified("a.txt"));
    assert_eq!(
        bac.table.lock().expect("lock").en_vol(),
        1,
        "the write must be REGISTERED in the bridge table"
    );
    let (_, c, _, _) = *bac.trames().last().expect("morceau");
    let (commande, _, _) = bac
        .table
        .lock()
        .expect("lock")
        .resoudre(c, Instant::now())
        .expect("inscrite");
    assert_eq!(commande, None, "a write completes NO ProjFS callback");
}
