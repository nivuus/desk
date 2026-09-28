//! Task 10's tests: restart of a window whose child is dead, and
//! the two capacity safeguards that come with it (`RELANCES_MAX`,
//! `DELAI_ATTENTE_VIEWPORT_MAX`).
//!
//! Extracted from `table/tests.rs`: adding these tests made it cross the
//! project's 500-line ceiling. These tests read `AnnoncerOuverture`
//! directly (the pattern `let Some(Effet::AnnoncerOuverture { .. }) = ...`)
//! rather than through the `session_annoncee` helper of `tests.rs` — that one
//! would have been of no use here, the session being systematically reused for
//! later assertions in the same effect.

use super::*;

/// A base of instants that does not read the system clock: `Table` reads
/// none, that is the whole point (same setup as `agent/src/capture/reprise.rs`).
fn instant(base: std::time::Instant, ms: u64) -> std::time::Instant {
    base + std::time::Duration::from_millis(ms)
}

/// The second design defect of D1 §3.3: `enfant_mort` removed
/// the entry, and nothing recalled the window any more — except a chance `SHOW` from
/// Windows. A very much alive window disappeared from the shell forever,
/// and that is what left the shell page empty while the four
/// applications were still running.
#[test]
fn a_window_whose_child_dies_is_offered_again() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let effets = t.enfant_mort(&session);
    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "since D3 §7.1 the output is retained for the relaunch, got {effets:?}"
    );

    // The window, for its part, is not forgotten: the periodic check
    // offers it again under a NEW session.
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture {
        session: neuve,
        titre,
    }) = effets.first()
    else {
        panic!("reopening expected, got {effets:?}");
    };
    assert_ne!(
        *neuve, session,
        "a reused identifier would pair a late message"
    );
    assert_eq!(titre, "Bloc-notes");
}

/// The safeguard D1's runaway makes mandatory: without it, a
/// window whose child systematically dies produces the loop
/// `w-5, w-6, w-7, w-8…` observed during acceptance.
#[test]
fn a_window_failing_endlessly_ends_up_abandoned() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let mut effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    // `RELANCES_MAX` TOLERATED restarts (w-1→w-2→w-3→w-4, three successful
    // restarts) and it is the next restart, the fourth, that meets the
    // refusal: `RELANCES_MAX + 1` death+restart cycles are therefore needed to
    // observe this refusal, not `RELANCES_MAX`. A plan brick that bounded
    // the loop to `0..RELANCES_MAX` stopped at the last successful restart
    // (w-4) without ever making it die in turn — the refusal
    // assertion could then never be reached.
    //
    // Same instant `instant(base, 0)` at each round: this test pins the ceiling of
    // RESTARTS, not the staleness delay (`DELAI_ATTENTE_VIEWPORT_MAX`, far
    // above this test's duration), the two safeguards are
    // independent.
    for _ in 0..=RELANCES_MAX {
        let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
            panic!("opening expected, got {effets:?}");
        };
        let session = session.clone();
        t.enfant_mort(&session);
        effets = t.relancer_les_orphelines(instant(base, 0));
    }
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "beyond the ceiling, an announced refusal and not one more relaunch, got {effets:?}"
    );
    assert!(
        t.relancer_les_orphelines(instant(base, 0)).is_empty(),
        "an abandoned window must no longer produce anything"
    );
}

/// A window that closes for good leaves the table, orphaned or not:
/// otherwise `relancer_les_orphelines` would resurrect it indefinitely.
#[test]
fn an_orphan_window_that_closes_leaves_the_table() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    t.enfant_mort(&session.clone());
    t.fenetre_disparue(IdFenetre(1));
    assert!(t
        .relancer_les_orphelines(std::time::Instant::now())
        .is_empty());
}

/// Second half of `enfant_mort`, not asserted until now. Before
/// D3's fix §7.1, a `.take()` on `sortie_pilote`/`nom_sortie`
/// prevented a second call from asking the driver again to destroy an output
/// already handed back. Since §7.1, `enfant_mort` never destroys anything: that
/// precise risk disappeared with the `.take()` that prevented it. What remains to
/// guarantee is that a second death does not leak the retained output.
#[test]
fn a_second_dead_child_does_not_leak_the_retained_output() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let premier = t.enfant_mort(&session);
    assert!(
        !premier
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "since D3 §7.1 the first death no longer gives the output back, got {premier:?}"
    );

    let second = t.enfant_mort(&session);
    assert!(
        !second
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "a second death must not give it back either, got {second:?}"
    );
    assert_eq!(
        t.nom_sortie_de(&session),
        Some("\\\\.\\DISPLAY7"),
        "the output must still be retained after two deaths"
    );
}

/// Task 10's second capacity risk: a restarted entry stays
/// `AttendLeViewport`, and if the shell page never answers (pop-up blocked,
/// shell disconnected — `CLAUDE.md` documents this case by name), nothing picks it
/// up: it is no longer `SansSession` (the first filter of
/// `relancer_les_orphelines` no longer sees it) and it will never reach
/// `Vivante`. Without this safeguard, its place would stay lost until the
/// supervisor's shutdown.
#[test]
fn a_relaunch_stalling_without_a_viewport_ends_up_abandoned() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    t.enfant_mort(&session.clone());

    // The restart happens at instant(base, 0): the new entry carries that instant.
    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture {
        session: relancee, ..
    }) = effets.first()
    else {
        panic!("reopening expected, got {effets:?}");
    };
    let relancee = relancee.clone();
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLeViewport));

    // No viewport ever comes. Well before the delay, nothing happens.
    assert!(
        t.relancer_les_orphelines(instant(base, 100)).is_empty(),
        "an entry that has not stalled yet must produce nothing"
    );

    // Past the delay, the entry is abandoned — and removed from the table.
    let apres_delai = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 + 1;
    let effets = t.relancer_les_orphelines(instant(base, apres_delai));
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "beyond the delay, a refusal rather than an indefinite silence, got {effets:?}"
    );
    assert_eq!(
        t.etat(&relancee),
        None,
        "the frozen entry must have left the table"
    );
}

/// The counterpart of the previous test: a restarted entry that receives its viewport
/// IN TIME must never be abandoned, even long after.
#[test]
fn a_relaunch_answering_in_time_is_not_abandoned() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    t.enfant_mort(&session.clone());

    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture {
        session: relancee, ..
    }) = effets.first()
    else {
        panic!("reopening expected, got {effets:?}");
    };
    let relancee = relancee.clone();

    // The viewport arrives before the delay.
    let effets = t.viewport_recu(&relancee, 1280, 720);
    assert!(
        !effets.is_empty(),
        "the viewport must trigger the output creation"
    );
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));

    // Long after, well beyond the delay: the entry is no longer
    // `AttendLeViewport`, the safeguard no longer concerns it.
    let bien_plus_tard = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 * 10;
    assert!(
        t.relancer_les_orphelines(instant(base, bien_plus_tard))
            .is_empty(),
        "an entry that answered in time must never be abandoned"
    );
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));
}

/// §7.3 of sub-block D2, fixed in D3. A NEW window whose shell page
/// never answers stayed `AttendLeViewport` without being either restarted or
/// abandoned: neither `SansSession`, nor `Vivante`. Its place was lost until
/// the supervisor's shutdown.
#[test]
fn a_new_window_whose_shell_never_answers_ends_up_abandoned() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());

    // First pass: the timestamp is set, nothing is abandoned.
    assert!(t.relancer_les_orphelines(base).is_empty());

    // The delay runs from the first pass, not from startup.
    let effets = t.relancer_les_orphelines(instant(base, 30_001));

    assert!(
        effets
            .iter()
            .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
        "the slot must be freed, got {effets:?}"
    );
    assert_eq!(t.fenetre_apparue(IdFenetre(2), "Autre".into()).len(), 1);
}

/// The timestamp must not abandon a window that answers within the delay.
#[test]
fn a_new_window_answering_in_time_is_not_abandoned() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();

    t.relancer_les_orphelines(base);
    t.viewport_recu(&session, 1280, 720);

    let effets = t.relancer_les_orphelines(instant(base, 30_001));
    assert!(effets.is_empty(), "got {effets:?}");
}

/// 🔴 THE PRODUCTION DEFECT OF AUGUST 30TH, 2026, PLAYED ON THE PURE TABLE.
///
/// The supervisor announces its windows to a relay where no one listens;
/// the announcement is lost, and thirty seconds later the window is
/// abandoned. When the shell page finally arrives, it must NOT find an
/// empty desktop.
///
/// ⚠️ **This test exercises the "tell again" half (`reannoncer_les_attentes`); the
/// "catching up abandoned ones" half lives in `boucle.rs`, which replays
/// the Windows enumeration — hence out of reach of a host test.** The
/// next test exercises that this catching up is indeed possible: an
/// abandoned window can COME BACK into the table.
#[test]
fn a_shell_page_arriving_late_receives_the_pending_windows() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    // This announcement is LOST: no `client` peer is connected.
    t.relancer_les_orphelines(base);

    // The shell arrives 20 s later, before abandonment.
    let effets = t.reannoncer_les_attentes(instant(base, 20_000));
    let Some(Effet::AnnoncerOuverture {
        session: redite,
        titre,
    }) = effets.first()
    else {
        panic!("re-announcement expected, got {effets:?}");
    };
    assert_eq!(
        *redite, session,
        "the session does not change: neither does the window"
    );
    assert_eq!(titre, "Bloc-notes");
    assert_eq!(
        effets.len(),
        1,
        "a single window, a single announcement: {effets:?}"
    );
}

/// 🔴 THE CLOCK RESTARTS FROM THE SHELL'S ARRIVAL, AND NOT FROM STARTUP — it is
/// the difference between fixing the defect and lengthening the delay, which the
/// instruction forbade by name.
///
/// Without the reset, a shell arriving at 20 s would only have
/// 10 s to open its pop-up and send back the viewport; here it has a full
/// thirty.
#[test]
fn the_re_announcement_resets_the_countdown() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    t.relancer_les_orphelines(base);

    t.reannoncer_les_attentes(instant(base, 20_000));

    // 20 s + 25 s = 45 s after startup: the old count would have
    // abandoned long ago.
    let effets = t.relancer_les_orphelines(instant(base, 45_000));
    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
        "the window has 25 s of waiting since the shell arrived, got {effets:?}"
    );
}

/// 🔴 WHAT THE BOUND PROTECTED STAYS PROTECTED: the re-announcement does not
/// remove it, it makes it run from an instant that makes sense. A
/// PRESENT but silent shell page still loses its window after
/// thirty seconds, and the place is handed back.
///
/// **Without this test, the fix would be indistinguishable from removing
/// the bound** — the pattern "a check never seen red".
#[test]
fn a_present_but_silent_shell_always_loses_its_window() {
    let base = std::time::Instant::now();
    let mut t = Table::new(1);
    t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    t.relancer_les_orphelines(base);
    t.reannoncer_les_attentes(instant(base, 20_000));

    let effets = t.relancer_les_orphelines(instant(base, 50_001));
    assert!(
        effets
            .iter()
            .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
        "30 s after the shell arrived, the abandonment must take place: {effets:?}"
    );
    // The place is really handed back: the table offered only ONE.
    assert_eq!(t.fenetre_apparue(IdFenetre(2), "Autre".into()).len(), 1);
}

/// 🔴 THE RETAINED VIRTUAL OUTPUT — the COSTLY resource the bound
/// protects — is always handed back on abandonment, re-announcement or not.
#[test]
fn the_retained_output_is_always_released_on_abandon_after_a_reannounce() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));
    // The child dies: the entry retains its output (§7.1 of D3) and goes back to
    // waiting for a viewport at restart.
    t.enfant_mort(&session);
    t.relancer_les_orphelines(base);

    t.reannoncer_les_attentes(instant(base, 5_000));

    let effets = t.relancer_les_orphelines(instant(base, 35_001));
    assert!(
        effets.iter().any(|e| matches!(
            e,
            Effet::DetruireSortie {
                sortie_pilote: 42,
                ..
            }
        )),
        "the retained output must go back to the driver, got {effets:?}"
    );
}

/// 🔴 A **LIVE** WINDOW IS NOT TOLD AGAIN, AND IT IS DELIBERATE: its
/// child consumes ONE offer and never renegotiates
/// (`agent/src/demarrage.rs`), so the reopened page would send an offer
/// no one would take. Telling a live session again would open a
/// permanently silent window — worse than saying nothing.
///
/// **It is a named legacy**: a reload of the shell page does not recover
/// already live windows.
#[test]
fn a_live_window_is_not_said_again() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    // `sortie_creee` moves the entry to `Vivante`: it is the only
    // path, and the assertion below checks it rather than trusting it.
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));

    let effets = t.reannoncer_les_attentes(instant(base, 1_000));
    assert!(
        effets.is_empty(),
        "a live session is not said again, got {effets:?}"
    );
}

/// The re-announcement invents nothing: on an empty table it returns nothing.
/// **The negative witness of the test above** — without it, `is_empty()` would be
/// true of a method that NEVER returns anything.
#[test]
fn the_re_announcement_on_an_empty_table_returns_nothing() {
    let mut t = Table::new(4);
    assert!(t
        .reannoncer_les_attentes(std::time::Instant::now())
        .is_empty());
}
