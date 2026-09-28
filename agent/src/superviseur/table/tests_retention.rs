//! Sub-block D3's tests: the virtual output is RETAINED between a child's death
//! and its restart, instead of being destroyed then recreated.
//!
//! File distinct from `tests_relance.rs`: that one is at 211 lines and the
//! project's ceiling is at 500, but the real reason is readability — these
//! tests are about retention, those about capacity safeguards.

use super::*;

/// Opens a window and takes it to `Vivante`, returning the session.
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    live_session_of_size(t, fenetre, titre, sortie, nom, (1280, 720))
}

/// Same bootstrap, but the output is born at an imposed size — the case of a VM
/// whose registry was polluted (D9 §9).
fn live_session_of_size(
    t: &mut Table,
    fenetre: u64,
    titre: &str,
    sortie: u32,
    nom: &str,
    size: (u32, u32),
) -> IdSession {
    let effets = t.fenetre_apparue(IdFenetre(fenetre), titre.into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, sortie, nom.into(), size);
    session
}

#[test]
fn the_table_keeps_the_real_output_size() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 713);
    // The driver quantises: 1280×713 requested, 1280×720 returned. It is the RETURNED
    // size that a later viewport will have to match.
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    assert_eq!(t.output_size_of(&session), Some((1280, 720)));
}

#[test]
fn a_session_without_output_has_no_size() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    assert_eq!(t.output_size_of(session), None);
}

#[test]
fn the_size_survives_the_child_death() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    assert_eq!(
        t.output_size_of(&session),
        Some((1280, 720)),
        "the size goes with the retained output"
    );
}

/// D3's fix §7.1. Before it, `enfant_mort` handed back the output to the
/// driver and the restart recreated one — and it is this RECREATION that
/// abandons the mutex of all neighbouring duplications (D2, 44 access
/// losses absorbed; a single doomed window took the
/// reopening counter from 6 to 38).
#[test]
fn the_child_death_no_longer_gives_the_output_back_to_the_driver() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.enfant_mort(&session);

    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "the output is retained for the relaunch, got {effets:?}"
    );
    assert!(effets.contains(&Effet::AnnoncerFermeture {
        session: session.clone()
    }));
}

#[test]
fn the_relaunched_entry_still_carries_its_output() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);

    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();

    assert_ne!(
        neuve, session,
        "a reused identifier would pair a late message"
    );
    assert_eq!(
        t.nom_sortie_de(&neuve),
        Some("\\\\.\\DISPLAY7"),
        "the output follows the window into its new session"
    );
    assert_eq!(t.output_size_of(&neuve), Some((1280, 720)));
}

#[test]
fn without_a_retained_output_the_viewport_requests_one() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();

    let effets = t.viewport_recu(&session, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::CreateOutput {
            session: session.clone(),
            titre: "Bloc-notes".into(),
            largeur: 1280,
            hauteur: 720
        }]
    );
}

/// **The path that removes the spurious reopenings.** The window keeps its
/// output, the announced viewport matches it: nothing left to create, hence no
/// more abandoned mutex at the neighbours.
#[test]
fn a_compatible_retained_output_is_reused_without_creating_anything() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: neuve.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY7".into(),
            size: (1280, 720),
        }],
        "neither DetruireSortie nor CreerSortie: that is the whole point of the fix"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante));
}

/// A retained output LARGER than the viewport will serve again: destroying
/// and recreating it would make the neighbouring duplications abandon the
/// mutex at each restart — exactly the recreation sub-block
/// D3 exists to remove, and the cause of its 32 spurious reopenings.
#[test]
fn a_larger_retained_output_is_reused() {
    let mut t = Table::new(4);
    let session =
        live_session_of_size(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY8", (3840, 2160));
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: neuve.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY8".into(),
            size: (1280, 720),
        }],
        "a retained output that is big enough must be neither destroyed nor recreated"
    );
}

/// The tolerance is the pairing one — four pixels — and no more.
#[test]
fn a_retained_output_within_four_pixels_is_reused() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1278, 718);

    assert!(
        matches!(effets.first(), Some(Effet::LancerEnfant { .. })),
        "got {effets:?}"
    );
}

/// The browser resized its window meanwhile: the retained output
/// no longer fits, it must be handed back BEFORE requesting another — otherwise
/// it would stay captive from the pool of ten.
#[test]
fn an_incompatible_retained_output_is_given_back_then_replaced() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1920, 1080);

    assert_eq!(
        effets,
        vec![
            Effet::DetruireSortie {
                sortie_pilote: 42,
                nom_sortie: "\\\\.\\DISPLAY7".into()
            },
            Effet::CreateOutput {
                session: neuve.clone(),
                titre: "Bloc-notes".into(),
                largeur: 1920,
                hauteur: 1080
            },
        ],
        "the destruction precedes the request, and in that order"
    );
    assert_eq!(
        t.nom_sortie_de(&neuve),
        None,
        "the entry no longer retains anything"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::AttendLaSortie));
}

/// 🔴 **THE TEST THAT SEES THE DEFECT, AND WITHOUT IT ITS NEIGHBOUR IS VACUOUS.**
/// `a_replayed_viewport_advances_no_state_machine` asserted
/// `rejeu.iter().all(matches!(SuivreLeViewport))` — and **an EMPTY vector
/// satisfies `all()`**. Returning `Vec::new()` on a live session, that is,
/// yesterday's behaviour, therefore left it GREEN. It is the pattern
/// `CLAUDE.md` names "a check never seen red is not a
/// check", and it was caught by playing the mutation, not by rereading.
///
/// This test asserts that the effect IS produced, and with the requested size:
/// it is the one that turns red if the `Vivante` branch disappears.
#[test]
fn a_viewport_on_a_live_session_asks_to_follow() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.etat(&session), Some(&Etat::Vivante), "precondition");

    let effets = t.viewport_recu(&session, 1600, 900);

    match effets.as_slice() {
        [Effet::SuivreLeViewport {
            session: s,
            largeur,
            hauteur,
        }] => {
            assert_eq!(s, &session);
            assert_eq!((*largeur, *hauteur), (1600, 900));
        }
        autre => panic!("a single viewport follow-up expected, got {autre:?}"),
    }
    // The table advanced nothing: it is the loop that measures and applies.
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));
}

/// Ceiling ① runs ALSO on this path: without it, a client at
/// `devicePixelRatio = 2` would have a 2560×1440 window put on an output
/// that cannot carry it. Same bounding as at the two other entry points
/// of the viewport (`create_output`, reuse path).
#[test]
fn a_hidpi_viewport_on_a_live_session_is_clamped_before_leaving() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.viewport_recu(&session, 3840, 2160);

    let [Effet::SuivreLeViewport {
        largeur, hauteur, ..
    }] = effets.as_slice()
    else {
        panic!("a viewport follow-up expected, got {effets:?}");
    };
    assert_eq!(
        (*largeur, *hauteur),
        crate::windows_source_sortie::MAX_OUTPUT_SIZE,
        "the ceiling must be applied BEFORE the effect leaves"
    );
}

/// A browser message is an external source: replayed, it must not
/// launch a second child on the same output.
///
/// ❌ **THIS TEST WAS CALLED `un_viewport_rejoue_apres_reutilisation_ne_fait_rien`
/// AND ASSERTED `is_empty()`. BATCH 33 MAKES IT WRONG, DELIBERATELY**: a
/// viewport received on a LIVE session now returns
/// `Effet::SuivreLeViewport`, because it is exactly the message by which
/// the browser says it was resized. The old assertion was the
/// **defect**, not the protection.
///
/// 🔴 **WHAT THE PROPERTY BECOMES, AND WHY IT STILL PROTECTS.** What
/// this test really guarded — "a replayed, late or invented message must
/// not ADVANCE THE MACHINE twice" — stays true and is now
/// asserted explicitly: the replay creates no output, destroys
/// none, launches no child, and does not change the state. `SuivreLeViewport`
/// is moreover idempotent at its executor
/// (`placement_periodique::suivre_le_viewport` short-circuits when the retained
/// size already equals the one it computes), so an identical replay does not
/// even issue a second `SetWindowPos`.
#[test]
fn a_replayed_viewport_advances_no_state_machine() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("reopening expected, got {effets:?}");
    };
    let neuve = neuve.clone();
    t.viewport_recu(&neuve, 1280, 720);
    let before = t.output_size_of(&neuve);

    let rejeu = t.viewport_recu(&neuve, 1280, 720);

    // The ONLY tolerated effect, and nothing else: no `CreateOutput`, no
    // `DetruireSortie`, no `LancerEnfant`.
    assert!(
        rejeu
            .iter()
            .all(|e| matches!(e, Effet::SuivreLeViewport { .. })),
        "a replay must only produce a viewport follow-up, got {rejeu:?}"
    );
    assert_eq!(
        t.etat(&neuve),
        Some(&Etat::Vivante),
        "the state must not advance"
    );
    assert_eq!(
        t.output_size_of(&neuve),
        before,
        "the TABLE does not bound by itself: it is the loop that writes the retained size, \
         after reading the work area — a replay must change nothing here"
    );
}

/// Counterpart of §7.1: an abandoned entry now carries an output,
/// which never happened before D3. Forgetting it would empty the driver's
/// pool of ten, silently, until supervisor shutdown.
#[test]
fn abandonment_after_max_relaunches_gives_the_output_back() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let mut session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    // RELANCES_MAX restarts, then abandonment at the next round.
    for tour in 0..=RELANCES_MAX {
        t.enfant_mort(&session);
        let effets = t.relancer_les_orphelines(base + std::time::Duration::from_secs(tour as u64));
        if let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() {
            session = neuve.clone();
            // The output follows; it is not recreated.
            t.viewport_recu(&session, 1280, 720);
            continue;
        }
        // Tour d'abandon.
        assert!(
            effets.contains(&Effet::DetruireSortie {
                sortie_pilote: 42,
                nom_sortie: "\\\\.\\DISPLAY7".into()
            }),
            "the retained output must be given back on abandonment, got {effets:?}"
        );
        assert!(
            effets
                .iter()
                .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
            "got {effets:?}"
        );
        return;
    }
    panic!("the abandonment never happened");
}

/// Second abandonment path: the shell page never answers after the restart.
/// The entry still carries its retained output — same requirement.
#[test]
fn abandoning_a_frozen_entry_gives_the_output_back() {
    let base = std::time::Instant::now();
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    // Restart: the entry goes back to AttendLeViewport, timestamped at `base`.
    t.relancer_les_orphelines(base);

    // The shell page never answers: beyond the delay, abandonment.
    let effets = t.relancer_les_orphelines(
        base + DELAI_ATTENTE_VIEWPORT_MAX + std::time::Duration::from_secs(1),
    );

    assert!(
        effets.contains(&Effet::DetruireSortie {
            sortie_pilote: 42,
            nom_sortie: "\\\\.\\DISPLAY7".into()
        }),
        "the retained output must be given back, got {effets:?}"
    );
}

/// Former IMPORTANT 5 (review of task 9): `changer_mode_de_sortie` (D8)
/// resized an output outside this table, and `refresh_output_size`
/// was the catch-up. **That path was removed in sub-block D9**, with measurement
/// to back it (see the finding at the head of `capteur/plein_ecran.rs`). **And
/// `refresh_output_size` has had no caller since sub-block
/// D10**: the periodic placement check (`placement_periodique.rs`)
/// called it on each fresh DXGI read, but `output_size` now carries
/// the RETAINED size, with no more reason to equal the raw DXGI
/// size — this refresh would therefore have overwritten it, and the call was
/// removed. This test covers the method itself, general and still
/// exposed: the repercussion on what `viewport_recu` will compare at the
/// next restart.
#[test]
fn refresh_size_updates_an_already_retained_output() {
    let mut t = Table::new(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.output_size_of(&session), Some((1280, 720)));

    // An output resized by some mechanism (none exists any more
    // in production since D9, and since D10 no caller even invokes
    // this method — see the doc above). A future mechanism of that
    // kind would have to pass it the RETAINED size, not reread the output's raw
    // DXGI size.
    t.refresh_output_size(&session, (1920, 1080));

    assert_eq!(t.output_size_of(&session), Some((1920, 1080)));
}

/// Must invent NOTHING: a session without a retained output (still waiting
/// for creation, or unknown) stays without a size after the call — only
/// `sortie_creee` has the right to set the very first value.
#[test]
fn refresh_size_invents_nothing_without_a_retained_output() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    // Neither `viewport_recu` nor `sortie_creee` has run yet: no output
    // is retained.
    assert_eq!(t.output_size_of(&session), None);

    t.refresh_output_size(&session, (1920, 1080));
    assert_eq!(
        t.output_size_of(&session),
        None,
        "nothing to refresh, nothing should have appeared"
    );

    // A totally unknown session must not panic nor create
    // a ghost entry either.
    t.refresh_output_size(&IdSession("w-inconnue".into()), (1920, 1080));
}
