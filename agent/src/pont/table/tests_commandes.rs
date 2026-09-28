//! Table tests: writes, ProjFS commands, age and budgets, apart from
//! `tests.rs` to stay under 500 lines.

use super::tests::{attributs, maintenant};
use super::*;

/// 🔴 **A WRITE AND A READ NEVER SHARE A CORRELATION.**
///
/// It is the reason `inscrire_sans_commande` goes through THIS
/// table, and not through a second counter: two independent counters on the
/// same channel would collide, and the collision would be **silent** —
/// a response applied to the wrong command.
#[test]
fn a_write_and_a_read_never_share_a_correlation() {
    let e = maintenant() + DELAI_LIRE;
    let mut t = Table::new();
    let mut vues = std::collections::HashSet::new();
    for i in 0..64 {
        // The two kinds interleave, as on the real path: a write
        // thread pushes while an application reads.
        let lecture = t.inscrire(i, attributs(&format!("l{i}")), e);
        let ecriture = t.inscrire_sans_commande(
            Attendue::Write {
                chemin: format!("e{i}"),
                last: false,
            },
            e,
        );
        assert!(
            vues.insert(lecture),
            "correlation {lecture} handed out twice"
        );
        assert!(
            vues.insert(ecriture),
            "correlation {ecriture} handed out twice"
        );
    }
    assert_eq!(t.en_vol(), 128);
}

/// A registration without a command receives a correlation **and no
/// `command_id`**: `verbes::completer` must be able to tell it apart.
#[test]
fn a_registration_without_a_command_has_no_command_id() {
    let e = maintenant() + WRITE_TIMEOUT;
    let mut t = Table::new();
    let c = t.inscrire_sans_commande(
        Attendue::Create {
            chemin: "neuf.txt".into(),
        },
        e,
    );
    let (commande, quoi, _) = t.resoudre(c, Instant::now()).expect("registered just now");
    assert_eq!(commande, None, "a write completes NO ProjFS callback");
    assert_eq!(
        quoi,
        Attendue::Create {
            chemin: "neuf.txt".into()
        }
    );
}

/// 🔴 **THE AGE RETURNED BY `resoudre` IS A REAL SUBTRACTION, NOT A ZERO.**
///
/// It is the only thing the bridge can measure of a traversal (F4, §0.5),
/// and a constant `Duration::ZERO` would make the whole histogram of
/// `pont::latence` mute **without a single census line missing**:
/// `n:` would rise, `moy_us:` would stay at 0. Time being a parameter, the
/// test exercises it **without sleeping**.
#[test]
fn resolving_returns_the_command_age_not_zero() {
    let depart = maintenant();
    let mut t = Table::new();
    let c = t.inscrire(
        7,
        Attendue::Attributs {
            chemin: "a.txt".into(),
        },
        depart + DELAI_ATTRIBUTS,
    );

    // `inscrire` reads `Instant::now()` for the registration; we therefore measure a
    // FLOOR age by taking a "now" shifted by 250 ms.
    let (_, _, age) = t
        .resoudre(c, Instant::now() + Duration::from_millis(250))
        .expect("registered just now");
    assert!(age >= Duration::from_millis(250), "returned age: {age:?}");
    assert!(
        age < Duration::from_millis(2_000),
        "the age is not the budget: {age:?}"
    );
}

/// 🔴 **`drain` RETURNS WRITES WITH AN ABSENT `command_id`.**
///
/// Returning `Some(0)` would make `PrjCompleteCommand(0)` be called at bridge shutdown,
/// that is, complete a command belonging to someone else.
#[test]
fn drain_returns_writes_with_a_missing_command_id() {
    let e = maintenant() + WRITE_TIMEOUT;
    let mut t = Table::new();
    let lecture = t.inscrire(42, attributs("a"), e);
    let ecriture = t.inscrire_sans_commande(
        Attendue::Write {
            chemin: "b".into(),
            last: true,
        },
        e,
    );
    let tout = t.drain();
    assert_eq!(tout.len(), 2);
    assert!(tout.contains(&(Some(42), lecture)));
    assert!(
        tout.contains(&(None, ecriture)),
        "the write must come out WITHOUT a command_id"
    );
}

/// An expired write is removed like the others.
///
/// Excluding it from the sweep would leave it in the table **forever**: nothing
/// else removes it, since no ProjFS callback registered it and
/// no cancellation can target it.
#[test]
fn an_expired_write_is_removed_like_the_others() {
    let debut = maintenant();
    let mut t = Table::new();
    let c = t.inscrire_sans_commande(
        Attendue::Write {
            chemin: "gros.bin".into(),
            last: false,
        },
        debut + WRITE_TIMEOUT,
    );
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.expirees(debut + WRITE_TIMEOUT), vec![(None, c)]);
    assert_eq!(t.en_vol(), 0);
}

/// `annuler` cannot target a write — it has no `command_id`.
///
/// ⚠️ **Without this assertion, `annuler(0)` could pair a write whose
/// `command_id` is `None`** the day the comparison is written
/// backwards. An application giving up its I/O would then carry away a
/// due write, which would never be pushed AND never removed from the journal.
#[test]
fn cancel_never_targets_a_write() {
    let e = maintenant() + WRITE_TIMEOUT;
    let mut t = Table::new();
    let ecriture = t.inscrire_sans_commande(
        Attendue::Write {
            chemin: "a".into(),
            last: true,
        },
        e,
    );
    assert!(t.annuler(0).is_empty(), "no ProjFS command 0 exists");
    assert_eq!(t.en_vol(), 1, "the write is still there");
    assert!(t.resoudre(ecriture, Instant::now()).is_some());
}

/// 🔴 **THE SURVEY THAT MAKES F1'S LEGACY NO. 4 DIAGNOSABLE.**
///
/// F1 measured reads that STALL without ever expiring — the expired-command count
/// stays at 0 for 540 s — and declares that we do not know WHERE the blockage
/// happens, "for lack of a trace at table registration". This is that trace.
///
/// Red: return `None` unconditionally, or return the MINIMUM instead of the
/// maximum. In both cases the legacy stays undiagnosable, and that is
/// exactly today's state.
#[test]
fn oldest_returns_the_duration_of_the_oldest_command_in_flight() {
    let mut t = Table::new();
    let depart = Instant::now();
    // Nothing in flight: `None`, and it is the FIRST line of the reading table —
    // "nothing was ever registered, the blockage is in the callback".
    assert_eq!(t.plus_ancienne(depart), None);

    t.inscrire(
        1,
        Attendue::Attributs { chemin: "a".into() },
        depart + Duration::from_secs(2),
    );
    std::thread::sleep(Duration::from_millis(20));
    t.inscrire(
        2,
        Attendue::Attributs { chemin: "b".into() },
        depart + Duration::from_secs(2),
    );

    let vue = t.plus_ancienne(Instant::now()).expect("deux en vol");
    assert!(
        vue >= Duration::from_millis(20),
        "the OLDEST, not the youngest: {vue:?}"
    );
}

/// Commands **without a ProjFS callback** are counted separately.
///
/// ⚠️ A frozen application with `en vol=3` and `sans_commande=3` waits for NOTHING
/// from the bridge: all three are pushes, and its blockage is elsewhere. Without
/// this distinction, the census would blame the bridge for a blockage that
/// does not concern it.
#[test]
fn without_command_counts_only_what_completes_no_callback() {
    let mut t = Table::new();
    let echeance = Instant::now() + Duration::from_secs(5);
    t.inscrire(1, Attendue::Attributs { chemin: "a".into() }, echeance);
    t.inscrire_sans_commande(
        Attendue::Write {
            chemin: "b".into(),
            last: true,
        },
        echeance,
    );
    t.inscrire_sans_commande(
        Attendue::Muter {
            chemin: "c".into(),
            renommage: true,
            destination: Some("d".into()),
        },
        echeance,
    );
    assert_eq!(t.en_vol(), 3);
    assert_eq!(t.sans_commande(), 2);
}

/// ⚠️ **The five budgets are DISTINCT, and staying so is the point.**
///
/// The old bridge had **a single one**, 10 s, for everything (`src/file.js:89`),
/// hence two symmetric defects: reads of large blocks that expired
/// before completing, and `getattr`s that froze Explorer for ten seconds
/// on a nonexistent path.
#[test]
fn the_five_budgets_are_distinct() {
    let all = [
        DELAI_ATTRIBUTS,
        DELAI_LIRE,
        DELAI_LISTER,
        WRITE_TIMEOUT,
        DELAI_MUTATION,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b, "two budgets share the value {a:?}");
        }
    }
    // A mutation's budget lies between a read's and a
    // write's: a single round trip, but whose copy fallback is
    // O(size) on the browser side.
    assert!(DELAI_MUTATION > DELAI_LIRE);
    assert!(DELAI_MUTATION < WRITE_TIMEOUT);
}

/// 🔴 **`annuler` RETURNS ALL THE CORRELATIONS OF A COMMAND, AND IT IS
/// F3'S READ WINDOW THAT REQUIRES IT.**
///
/// Red: return only one, as until F2. The *N−1* others would stay
/// in flight, would expire at the budget, and `service::balayer` would then call
/// `PrjCompleteCommand` on an **ALREADY COMPLETED** command — a system
/// call on an identifier that now belongs to someone else.
/// *Mute, deferred, and outside our process.*
#[test]
fn cancel_removes_the_n_correlations_of_a_windowed_read() {
    let mut t = Table::new();
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 0,
            length: 4,
        },
        e,
    );
    let b = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 4,
            length: 4,
        },
        e,
    );
    let c = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 8,
            length: 4,
        },
        e,
    );
    // A NEIGHBOURING command must not be carried away.
    let autre = t.inscrire(8, attributs("x"), e);
    assert_eq!(t.en_vol(), 4);

    let annulees = t.annuler(7);
    assert_eq!(
        annulees,
        vec![a, b, c],
        "all THREE, in a deterministic order"
    );
    assert_eq!(t.en_vol(), 1, "only command 8 survives");
    assert!(t.resoudre(autre, Instant::now()).is_some());
}
