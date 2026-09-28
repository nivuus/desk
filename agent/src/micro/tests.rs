//! The tests of `micro.rs`, moved to their own file under the
//! 500-line rule: the module crossed the ceiling when gaining the
//! drift correction (work stream E, block E1, task 5), and the repository's
//! doctrine requires EXTRACTING, never compressing a comment to get back
//! under the line.
//!
//! Declared in the parent through `#[path]` — a usage explicitly OUTSIDE the
//! "Child module convention" of `CLAUDE.md`, which only targets modules
//! taken out of a `#[cfg(windows)]` parent to compile them on the host. Here
//! the parent is already pure; the only motive is size, and the precedent is
//! `superviseur/table.rs`.

use super::*;
use std::time::Duration;

/// Builds a frame of `ms` milliseconds starting at `rtp_48k`.
fn trame(rtp_48k: u64, ms: u64) -> TrameMicro {
    let echantillons = (48_000 * ms / 1000) as usize;
    TrameMicro {
        // The content matters little here: THESE tests decode nothing, they
        // test ordering. (The module, for its part, decodes — see `tests_lecteur`.)
        // A byte derived from the timestamp is enough to identify the frame.
        opus: vec![(rtp_48k % 251) as u8, 0x11],
        rtp_48k,
        echantillons,
    }
}

fn tampon() -> TamponGigue {
    TamponGigue::new(CIBLE, PLAFOND)
}

/// Spec §11: "an out-of-order arrival is restored in order".
///
/// ⚠️ str0m ALREADY reorders (`packet/buffer_rx.rs`), and decision 2 of this
/// plan brings its depth down to 2: its guarantee is therefore DELIBERATELY
/// weakened, and it is here that ordering is made up for. This test is not
/// redundant with str0m, it is the safety net for what decision 2 takes
/// away from it.
#[test]
fn an_out_of_order_arrival_is_restored_in_order() {
    let mut t = tampon();
    t.deposer(trame(1920, 20));
    t.deposer(trame(0, 20));
    t.deposer(trame(960, 20));

    let mut vus = Vec::new();
    for _ in 0..3 {
        match t.retirer() {
            Retrait::Trame(tr) => vus.push(tr.rtp_48k),
            autre => panic!("retrait inattendu : {autre:?}"),
        }
    }
    assert_eq!(vus, vec![0, 960, 1920]);
    assert_eq!(t.compteurs().hors_ordre, 2, "the two late arrivals");
}

#[test]
fn a_duplicate_is_counted_and_dropped() {
    let mut t = tampon();
    t.deposer(trame(960, 20));
    t.deposer(trame(960, 20));

    assert_eq!(t.compteurs().deposees, 2);
    assert_eq!(t.compteurs().doublons, 1);
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 960));
    // …and no second copy of it remains.
    assert!(matches!(t.retirer(), Retrait::Manquante));
}

/// Spec §11: "at saturation, it is the OLDEST frame that goes".
/// It is the reverse of emission (work stream A throws away the old to keep
/// the fresh) — here we undergo a remote timeline.
#[test]
fn at_saturation_the_oldest_one_leaves() {
    let mut t = tampon();
    // PLAFOND = 200 ms, that is 10 frames of 20 ms. Drop off 12 in order.
    for i in 0..12u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert!(
        t.occupation() <= PLAFOND,
        "occupancy {:?} above the ceiling {PLAFOND:?}",
        t.occupation()
    );
    assert!(t.compteurs().jetees_saturation >= 2);

    // The FIRST frame returned is no longer 0: it is the oldest
    // that went, not the most recent.
    let premiere = match t.retirer() {
        Retrait::Trame(tr) => tr.rtp_48k,
        autre => panic!("retrait inattendu : {autre:?}"),
    };
    assert!(
        premiere > 0,
        "the oldest frame (rtp 0) is still there: it is the most RECENT \
         that was dropped"
    );

    // …and the most recent, for its part, survived.
    let mut derniere = premiere;
    while let Retrait::Trame(tr) = t.retirer() {
        derniere = tr.rtp_48k;
    }
    assert_eq!(derniere, 11 * 960, "the most recent frame was dropped");
}

/// "under starvation, the sink returns silence without ever blocking" (spec §11).
#[test]
fn when_starving_the_removal_returns_missing_without_blocking() {
    let mut t = tampon();
    for _ in 0..5 {
        assert!(matches!(t.retirer(), Retrait::Manquante));
    }
    assert_eq!(t.compteurs().famines, 5);
    assert_eq!(t.occupation(), Duration::ZERO);
}

/// FEC is ONLY useful if the next one is already there (spec §8): the
/// reconstruction is done from the frame that FOLLOWS the missing one.
#[test]
fn a_missing_frame_whose_next_is_there_gives_rebuild() {
    let mut t = tampon();
    t.deposer(trame(0, 20));
    t.deposer(trame(1920, 20)); // la trame 960 manque

    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    match t.retirer() {
        Retrait::Reconstruire { suivante } => {
            assert_eq!(suivante, trame(1920, 20).opus, "it is not the NEXT one");
        }
        autre => panic!("expected Reconstruire, got {autre:?}"),
    }
    assert_eq!(t.compteurs().fec, 1);
    // The next one was not consumed: it plays at the round after.
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 1920));
}

#[test]
fn a_missing_frame_without_a_next_gives_missing() {
    let mut t = tampon();
    t.deposer(trame(0, 20));
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    assert!(matches!(t.retirer(), Retrait::Manquante));
    assert_eq!(t.compteurs().fec, 0, "no next one: nothing to rebuild");
    assert_eq!(t.compteurs().famines, 1);
}

/// A frame that arrives AFTER its place has passed is not played out
/// of turn: it is stale, counted, and thrown away.
#[test]
fn a_stale_frame_is_counted_and_dropped() {
    let mut t = tampon();
    t.deposer(trame(960, 20));
    assert!(matches!(t.retirer(), Retrait::Trame(_)));
    t.deposer(trame(0, 20)); // arrives after its turn
    assert_eq!(t.compteurs().jetees_perimees, 1);
    assert!(matches!(t.retirer(), Retrait::Manquante));
}

/// The module's invariant, and it is STRUCTURAL: `deposer` returns nothing and
/// therefore cannot make the transport loop wait.
///
/// ⚠️ **No mutation can make this test fail**, and it is noted
/// rather than dressed up with a token check: the property is carried by
/// the SIGNATURE (`fn deposer(&mut self, trame: TrameMicro)`, without a return
/// value and without `Result`), and the compiler holds it. This test only checks
/// that 10,000 drop-offs in a row do not diverge and leave the
/// buffer bounded.
#[test]
fn depositing_never_blocks_even_at_saturation() {
    let mut t = tampon();
    for i in 0..10_000u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert_eq!(t.compteurs().deposees, 10_000);
    assert!(
        t.occupation() <= PLAFOND,
        "the buffer swelled without bound: {:?}",
        t.occupation()
    );
}

/// Spec §8: above 120 ms of occupancy we skip a frame,
/// below 20 ms we insert one. "Crude, audible once every
/// several minutes" — and assumed.
#[test]
fn beyond_the_high_threshold_a_frame_is_skipped_and_counted() {
    let mut t = tampon();
    // 7 frames of 20 ms = 140 ms, above SEUIL_SAUT (120 ms) and under
    // PLAFOND (200 ms): it is DRIFT we exercise, not saturation.
    for i in 0..7u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert_eq!(
        t.compteurs().jetees_saturation,
        0,
        "this is drift, not saturation"
    );

    match t.retirer() {
        // Frame 0 was skipped: it is 960 that comes out.
        Retrait::Trame(tr) => assert_eq!(tr.rtp_48k, 960, "no frame was skipped"),
        autre => panic!("retrait inattendu : {autre:?}"),
    }
    assert_eq!(t.compteurs().sauts, 1);
    assert_eq!(t.compteurs().insertions, 0);
    // And the skip is not made up for by an FEC reconstruction of the hole
    // it has just dug — that would be a disguised no-op.
    assert_eq!(t.compteurs().fec, 0);
}

#[test]
fn below_the_low_threshold_a_frame_is_inserted_and_counted() {
    let mut t = tampon();
    // A single 10 ms frame: 10 ms of occupancy, under SEUIL_INSERTION.
    t.deposer(trame(0, 10));

    assert!(matches!(t.retirer(), Retrait::Manquante));
    assert_eq!(t.compteurs().insertions, 1);
    assert_eq!(t.compteurs().sauts, 0);
    // ⚠️ The insertion does NOT CONSUME: the occupancy has not moved, and it is
    // what lets it grow up to the dead band.
    assert_eq!(t.occupation(), Duration::from_millis(10));
    // …and it is not a starvation: the frame is there, it is we who
    // are waiting.
    assert_eq!(t.compteurs().famines, 0);
}

/// Between the two thresholds, NOTHING moves: it is the hysteresis, and its
/// absence would make the buffer oscillate at each frame.
#[test]
fn between_the_two_thresholds_no_correction_is_applied() {
    let mut t = tampon();
    // 4 frames of 20 ms = 80 ms, squarely between 20 and 120.
    for i in 0..4u64 {
        t.deposer(trame(i * 960, 20));
    }
    assert!(matches!(t.retirer(), Retrait::Trame(tr) if tr.rtp_48k == 0));
    assert_eq!(t.compteurs().sauts, 0, "a skip in the dead band");
    assert_eq!(t.compteurs().insertions, 0, "an insertion in the dead band");
}

/// "None is silent" (spec §8). Each correction increments its
/// counter, and this test checks it on BOTH at once, within a single
/// buffer lifetime — a counter shared by both would pass the two
/// previous tests taken separately.
#[test]
fn each_correction_has_its_counter() {
    let mut t = tampon();
    for i in 0..7u64 {
        t.deposer(trame(i * 960, 20));
    }
    // Too late: we skip.
    assert!(matches!(t.retirer(), Retrait::Trame(_)));
    assert_eq!((t.compteurs().sauts, t.compteurs().insertions), (1, 0));

    // We empty until going under the low threshold.
    while t.occupation() >= SEUIL_INSERTION {
        t.retirer();
    }
    let skips_before = t.compteurs().sauts;
    // Enough remains not to be starving, but not enough to play.
    t.deposer(trame(100_000, 10));
    assert!(t.occupation() < SEUIL_INSERTION);
    assert!(matches!(t.retirer(), Retrait::Manquante));

    assert_eq!(
        t.compteurs().insertions,
        1,
        "the insertion does not have its counter"
    );
    assert_eq!(
        t.compteurs().sauts,
        skips_before,
        "the insertion incremented the SKIPS counter: the two corrections \
         share a counter"
    );
}
