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
fn une_fenetre_dont_l_enfant_meurt_est_reproposee() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let effets = t.enfant_mort(&session);
    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "depuis D3 §7.1 la sortie est retenue pour la relance, reçu {effets:?}"
    );

    // The window, for its part, is not forgotten: the periodic check
    // offers it again under a NEW session.
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture {
        session: neuve,
        titre,
    }) = effets.first()
    else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    assert_ne!(
        *neuve, session,
        "un identifiant réutilisé apparierait un message tardif"
    );
    assert_eq!(titre, "Bloc-notes");
}

/// The safeguard D1's runaway makes mandatory: without it, a
/// window whose child systematically dies produces the loop
/// `w-5, w-6, w-7, w-8…` observed during acceptance.
#[test]
fn une_fenetre_qui_echoue_sans_fin_finit_par_etre_abandonnee() {
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
            panic!("ouverture attendue, reçu {effets:?}");
        };
        let session = session.clone();
        t.enfant_mort(&session);
        effets = t.relancer_les_orphelines(instant(base, 0));
    }
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "au-delà du plafond, un refus annoncé et non une relance de plus, reçu {effets:?}"
    );
    assert!(
        t.relancer_les_orphelines(instant(base, 0)).is_empty(),
        "une fenêtre abandonnée ne doit plus rien produire"
    );
}

/// A window that closes for good leaves the table, orphaned or not:
/// otherwise `relancer_les_orphelines` would resurrect it indefinitely.
#[test]
fn une_fenetre_orpheline_qui_se_ferme_quitte_la_table() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
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
fn un_second_enfant_mort_ne_fait_pas_fuir_la_sortie_retenue() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let premier = t.enfant_mort(&session);
    assert!(
        !premier
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "depuis D3 §7.1 la première mort ne rend déjà plus la sortie, reçu {premier:?}"
    );

    let second = t.enfant_mort(&session);
    assert!(
        !second
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "une seconde mort ne doit pas non plus la rendre, reçu {second:?}"
    );
    assert_eq!(
        t.nom_sortie_de(&session),
        Some("\\\\.\\DISPLAY7"),
        "la sortie doit toujours être retenue après deux morts"
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
fn une_relance_qui_stagne_sans_viewport_finit_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());

    // The restart happens at instant(base, 0): the new entry carries that instant.
    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture {
        session: relancee, ..
    }) = effets.first()
    else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let relancee = relancee.clone();
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLeViewport));

    // No viewport ever comes. Well before the delay, nothing happens.
    assert!(
        t.relancer_les_orphelines(instant(base, 100)).is_empty(),
        "une entrée qui n'a pas encore stagné ne doit rien produire"
    );

    // Past the delay, the entry is abandoned — and removed from the table.
    let apres_delai = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 + 1;
    let effets = t.relancer_les_orphelines(instant(base, apres_delai));
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "au-delà du délai, un refus plutôt qu'un silence indéfini, reçu {effets:?}"
    );
    assert_eq!(
        t.etat(&relancee),
        None,
        "l'entrée figée doit avoir quitté la table"
    );
}

/// The counterpart of the previous test: a restarted entry that receives its viewport
/// IN TIME must never be abandoned, even long after.
#[test]
fn une_relance_qui_repond_a_temps_n_est_pas_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());

    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture {
        session: relancee, ..
    }) = effets.first()
    else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let relancee = relancee.clone();

    // The viewport arrives before the delay.
    let effets = t.viewport_recu(&relancee, 1280, 720);
    assert!(
        !effets.is_empty(),
        "le viewport doit déclencher la création de sortie"
    );
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));

    // Long after, well beyond the delay: the entry is no longer
    // `AttendLeViewport`, the safeguard no longer concerns it.
    let bien_plus_tard = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 * 10;
    assert!(
        t.relancer_les_orphelines(instant(base, bien_plus_tard))
            .is_empty(),
        "une entrée qui a répondu à temps ne doit jamais être abandonnée"
    );
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));
}

/// §7.3 of sub-block D2, fixed in D3. A NEW window whose shell page
/// never answers stayed `AttendLeViewport` without being either restarted or
/// abandoned: neither `SansSession`, nor `Vivante`. Its place was lost until
/// the supervisor's shutdown.
#[test]
fn une_fenetre_neuve_dont_la_shell_ne_repond_jamais_finit_par_etre_abandonnee() {
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
        "la place doit être libérée, reçu {effets:?}"
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
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();

    t.relancer_les_orphelines(base);
    t.viewport_recu(&session, 1280, 720);

    let effets = t.relancer_les_orphelines(instant(base, 30_001));
    assert!(effets.is_empty(), "reçu {effets:?}");
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
        panic!("ouverture attendue, reçu {effets:?}");
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
        panic!("réannonce attendue, reçu {effets:?}");
    };
    assert_eq!(
        *redite, session,
        "la session ne change pas : la fenêtre non plus"
    );
    assert_eq!(titre, "Bloc-notes");
    assert_eq!(
        effets.len(),
        1,
        "une seule fenêtre, une seule annonce : {effets:?}"
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
fn la_reannonce_remet_le_compte_a_rebours_a_zero() {
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
        "la fenêtre a 25 s d'attente depuis l'arrivée de la shell, reçu {effets:?}"
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
        "30 s après l'arrivée de la shell, l'abandon doit avoir lieu : {effets:?}"
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
        panic!("ouverture attendue, reçu {effets:?}");
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
        "la sortie retenue doit repartir au pilote, reçu {effets:?}"
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
fn une_fenetre_vivante_n_est_pas_redite() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
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
        "une session vivante ne se redit pas, reçu {effets:?}"
    );
}

/// The re-announcement invents nothing: on an empty table it returns nothing.
/// **The negative witness of the test above** — without it, `is_empty()` would be
/// true of a method that NEVER returns anything.
#[test]
fn la_reannonce_sur_une_table_vide_ne_rend_rien() {
    let mut t = Table::new(4);
    assert!(t
        .reannoncer_les_attentes(std::time::Instant::now())
        .is_empty());
}
