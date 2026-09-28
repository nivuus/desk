use super::*;

pub(super) fn maintenant() -> Instant {
    Instant::now()
}

pub(super) fn attributs(chemin: &str) -> Attendue {
    Attendue::Attributs {
        chemin: chemin.to_string(),
    }
}

#[test]
fn an_answer_arriving_after_cancellation_is_dropped() {
    // ⚠️ The test spec §4.4 names. ProjFS cancels a command when
    // the calling application gives up; the browser's response, for its part, is
    // already in flight. Applying it would write into a buffer the system has taken back.
    let mut t = Table::new();
    let c = t.inscrire(42, attributs("a.txt"), maintenant() + DELAI_ATTRIBUTS);
    assert_eq!(t.en_vol(), 1);

    assert_eq!(t.annuler(42), vec![c]);
    assert_eq!(t.en_vol(), 0, "the cancellation must remove the entry");
    assert_eq!(
        t.resoudre(c, Instant::now()),
        None,
        "the late answer must be DROPPED"
    );
}

#[test]
fn an_expired_command_does_not_stay_in_the_table() {
    let debut = maintenant();
    let mut t = Table::new();
    let c = t.inscrire(7, attributs("a.txt"), debut + DELAI_ATTRIBUTS);

    // Just before the deadline: nothing expires.
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.en_vol(), 1);

    // At the EXACT deadline: it expires. Making expiry depend on a
    // strict overrun would make it depend on the clock's granularity.
    assert_eq!(t.expirees(debut + DELAI_ATTRIBUTS), vec![(Some(7), c)]);
    assert_eq!(t.en_vol(), 0, "en_vol() must be decremented");
    // …and a second pass does not return it twice.
    assert!(t.expirees(debut + DELAI_LISTER).is_empty());
}

#[test]
fn an_answer_arriving_after_expiry_is_dropped() {
    let debut = maintenant();
    let mut t = Table::new();
    let c = t.inscrire(9, attributs("a.txt"), debut + DELAI_ATTRIBUTS);
    t.expirees(debut + DELAI_ATTRIBUTS);
    assert_eq!(t.resoudre(c, Instant::now()), None);
}

#[test]
fn correlations_are_monotonic_and_never_reused() {
    let mut t = Table::new();
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e);
    let b = t.inscrire(2, attributs("b"), e);
    assert_ne!(a, b);
    // And a resolved correlation is NOT recycled: a duplicated response from the
    // browser would otherwise be applied to the next command.
    t.resoudre(a, Instant::now());
    let c = t.inscrire(3, attributs("c"), e);
    assert_ne!(c, a);
    assert_ne!(c, b);
}

#[test]
fn drain_returns_everything_and_leaves_the_table_empty() {
    // It is what precedes `PrjStopVirtualizing`: a command left in flight
    // would wait there for a response nothing can deliver any more, and ProjFS
    // would wait for its completion indefinitely.
    let mut t = Table::new();
    let e = maintenant() + DELAI_LISTER;
    let c1 = t.inscrire(11, attributs("a"), e);
    let c2 = t.inscrire(22, attributs("b"), e);
    let c3 = t.inscrire(33, attributs("c"), e);

    let tout = t.drain();
    assert_eq!(tout.len(), 3, "drain must return EVERYTHING");
    assert_eq!(tout, vec![(Some(11), c1), (Some(22), c2), (Some(33), c3)]);
    assert_eq!(t.en_vol(), 0);
    assert!(t.drain().is_empty());
    // …and no response is applied afterwards any more.
    assert_eq!(t.resoudre(c1, Instant::now()), None);
}

#[test]
fn an_unknown_correlation_returns_none_without_panicking() {
    let mut t = Table::new();
    assert_eq!(t.resoudre(12_345, Instant::now()), None);
    assert!(t.annuler(999).is_empty());
    assert!(t.expirees(maintenant()).is_empty());
}

#[test]
fn two_enumerations_of_the_same_path_coexist() {
    // ⚠️ The enumeration session is indexed by the callback's enumeration GUID,
    // NOT by the path (spec §7.2): two applications listing the
    // same directory at the same time open two distinct sessions. Indexing
    // by path would make the second overwrite the first, and one of the
    // two would receive an empty directory — without any error being raised.
    let mut t = Table::new();
    let e = maintenant() + DELAI_LISTER;
    let g1 = [1u8; 16];
    let g2 = [2u8; 16];
    let c1 = t.inscrire(
        100,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g1,
        },
        e,
    );
    let c2 = t.inscrire(
        200,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g2,
        },
        e,
    );

    assert_ne!(c1, c2);
    assert_eq!(t.en_vol(), 2, "the two sessions must COEXIST");
    // …and each resolves on ITS session, not on the other's.
    let (id1, quoi1, _) = t
        .resoudre(c1, Instant::now())
        .expect("the first session exists");
    assert_eq!(id1, Some(100));
    assert_eq!(
        quoi1,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g1
        }
    );
    let (id2, quoi2, _) = t
        .resoudre(c2, Instant::now())
        .expect("la seconde session existe");
    assert_eq!(id2, Some(200));
    assert_eq!(
        quoi2,
        Attendue::Lister {
            chemin: "dossier".into(),
            enumeration: g2
        }
    );
}

#[test]
fn a_correlation_counter_overflow_does_not_reuse_a_correlation_in_flight() {
    // ⚠️ This test is the only one that cannot be written naively: wrapping
    // a `u32` would require four billion registrations. It goes
    // through the `new_from` seam, which makes it reachable in three
    // calls. **Without it, it would be vacuous** — it would pass without exercising anything.
    let mut t = Table::new_from(u32::MAX - 1);
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(1, attributs("a"), e); // u32::MAX - 1
    let b = t.inscrire(2, attributs("b"), e); // u32::MAX
    let c = t.inscrire(3, attributs("c"), e); // wraps to 0
    let d = t.inscrire(4, attributs("d"), e); // 1

    assert_eq!(a, u32::MAX - 1);
    assert_eq!(b, u32::MAX);
    assert_eq!(c, 0, "the counter must wrap around, not panic");
    assert_eq!(d, 1);
    assert_eq!(t.en_vol(), 4, "none of the four must overwrite another");

    // …and the case that really bites: a correlation STILL IN FLIGHT is
    // stepped over, not overwritten. We start again from 0 while 0 and 1 are taken.
    let mut t = Table::new_from(0);
    let e = maintenant() + DELAI_LIRE;
    let zero = t.inscrire(10, attributs("z"), e);
    let un = t.inscrire(11, attributs("u"), e);
    assert_eq!((zero, un), (0, 1));
    t.prochaine = 0; // the counter wrapped onto entries still in flight
    let apres = t.inscrire(12, attributs("v"), e);
    assert!(
        apres != zero && apres != un,
        "the wrapped correlation {apres} overwrites a command in flight"
    );
    assert_eq!(t.en_vol(), 3);
    // The original command still answers for ITSELF.
    assert_eq!(
        t.resoudre(zero, Instant::now()).map(|(id, _, _)| id),
        Some(Some(10))
    );
}
