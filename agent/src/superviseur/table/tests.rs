use super::*;

fn table() -> Table {
    Table::new(10)
}

/// Gets the session identifier assigned to the window, by reading
/// the announcement effect — it is the only public output that carries it.
fn session_annoncee(effets: &[Effet]) -> IdSession {
    effets
        .iter()
        .find_map(|e| match e {
            Effet::AnnoncerOuverture { session, .. } => Some(session.clone()),
            _ => None,
        })
        .expect("an opening must be announced")
}

#[test]
fn an_appearing_window_is_announced_and_nothing_more() {
    // Nothing can be created before knowing the viewport: it is what
    // gives the output's size.
    let mut t = table();
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    assert_eq!(effets.len(), 1);
    let session = session_annoncee(&effets);
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
}

#[test]
fn the_viewport_triggers_the_output_creation() {
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    let effets = t.viewport_recu(&session, 1600, 900);
    assert_eq!(
        effets,
        vec![Effet::CreateOutput {
            session: session.clone(),
            // The title follows the request: an output refusal is shown to a
            // human, and "w-1" designates nothing to them.
            titre: "Bloc-notes".into(),
            largeur: 1600,
            hauteur: 900,
        }]
    );
    assert_eq!(t.etat(&session), Some(&Etat::AttendLaSortie));
}

#[test]
fn the_created_output_triggers_the_child_launch() {
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    t.viewport_recu(&session, 1600, 900);
    // Two distinct identifiers, and it is the heart of the matter: `7` is
    // the identifier the DRIVER returned, `"\\.\DISPLAY4"` the name of the
    // same output in the DXGI enumeration. The driver only destroys through
    // the first; the child only knows how to capture through the second.
    let effets = t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));
    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY4".into(),
            size: (1280, 720),
        }]
    );
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));
    assert_eq!(t.nom_sortie_de(&session), Some("\\\\.\\DISPLAY4"));
}

/// Two windows in flight at the same time must never be assigned
/// each other's identifier or output: each `Effet::LancerEnfant`
/// must carry the `IdFenetre` and `nom_sortie` of ITS OWN window.
#[test]
fn two_windows_in_flight_each_keep_their_window_and_output() {
    let mut t = table();
    let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    t.viewport_recu(&a, 1600, 900);
    t.sortie_creee(&a, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));

    let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
    t.viewport_recu(&b, 1280, 720);
    let effets = t.sortie_creee(&b, 8, "\\\\.\\DISPLAY5".into(), (1280, 720));
    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: b,
            fenetre: IdFenetre(2),
            nom_sortie: "\\\\.\\DISPLAY5".into(),
            size: (1280, 720),
        }]
    );
}

#[test]
fn a_vanishing_window_kills_the_child_destroys_the_output_and_announces_it() {
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    t.viewport_recu(&session, 1600, 900);
    t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));

    let effets = t.fenetre_disparue(IdFenetre(1));
    assert_eq!(
        effets,
        vec![
            Effet::TuerEnfant {
                session: session.clone()
            },
            // The DRIVER's identifier, the only one it knows how to remove with.
            Effet::DetruireSortie {
                sortie_pilote: 7,
                nom_sortie: "\\\\.\\DISPLAY4".into()
            },
            Effet::AnnoncerFermeture {
                session: session.clone()
            },
        ]
    );
    assert_eq!(
        t.etat(&session),
        None,
        "the window must have left the table"
    );
}

#[test]
fn a_child_dying_alone_retains_the_output_and_announces_it_without_killing_it() {
    // It is the benefit multi-process was chosen for: the
    // death of a child must take nothing else with it. Since fix
    // §7.1 of sub-block D3, it no longer hands back the output to the driver either —
    // it is precisely its RECREATION at restart that made the neighbouring DXGI
    // duplications abandon the mutex (D2, 6 → 38 reopenings for a single
    // doomed window).
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    t.viewport_recu(&session, 1600, 900);
    t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));

    let effets = t.enfant_mort(&session);
    assert_eq!(
        effets,
        vec![Effet::AnnoncerFermeture {
            session: session.clone()
        }]
    );
    // The window stays in the table, orphaned: it is the periodic
    // check (`relancer_les_orphelines`) that will offer it again, rather
    // than a chance `SHOW` from Windows — see task 10.
    assert_eq!(t.etat(&session), Some(&Etat::SansSession));
    assert_eq!(
        t.output_size_of(&session),
        Some((1280, 720)),
        "the output is retained, not given back"
    );
}

#[test]
fn a_window_vanishing_before_its_output_requests_no_destruction() {
    // Closed while we waited for its viewport: no output
    // exists, and asking to destroy one would make the driver fail.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let effets = t.fenetre_disparue(IdFenetre(1));
    assert_eq!(
        effets,
        vec![
            Effet::TuerEnfant {
                session: session.clone()
            },
            Effet::AnnoncerFermeture { session },
        ]
    );
}

#[test]
fn the_full_pool_refuses_the_next_window_without_breaking_anything() {
    let mut t = Table::new(2);
    for n in 1..=2u64 {
        let s = session_annoncee(&t.fenetre_apparue(IdFenetre(n), format!("F{n}")));
        t.viewport_recu(&s, 1280, 720);
        t.sortie_creee(&s, n as u32, format!("\\\\.\\DISPLAY{n}"), (1280, 720));
    }
    let effets = t.fenetre_apparue(IdFenetre(3), "F3".into());
    assert_eq!(
        effets,
        vec![Effet::AnnoncerRefus {
            titre: "F3".into(),
            motif: "no virtual output available any more".into()
        }]
    );
}

#[test]
fn a_freed_output_reopens_the_slot() {
    let mut t = Table::new(1);
    let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    t.viewport_recu(&a, 1280, 720);
    t.sortie_creee(&a, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));
    assert!(matches!(
        t.fenetre_apparue(IdFenetre(2), "B".into()).as_slice(),
        [Effet::AnnoncerRefus { .. }]
    ));

    t.fenetre_disparue(IdFenetre(1));
    let effets = t.fenetre_apparue(IdFenetre(3), "C".into());
    assert!(matches!(
        effets.as_slice(),
        [Effet::AnnoncerOuverture { .. }]
    ));
}

#[test]
fn a_viewport_for_an_unknown_session_is_ignored() {
    // The browser is an external source: a late or replayed message
    // must produce no effect.
    let mut t = table();
    let effets = t.viewport_recu(&IdSession("w-inconnue".into()), 800, 600);
    assert!(effets.is_empty());
}

#[test]
fn a_second_viewport_for_the_same_session_is_ignored() {
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    t.viewport_recu(&session, 1600, 900);
    let effets = t.viewport_recu(&session, 800, 600);
    assert!(
        effets.is_empty(),
        "the output is already requested at the first size"
    );
}

#[test]
fn session_identifiers_are_unique() {
    let mut t = table();
    let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
    assert_ne!(a, b);
}

#[test]
fn a_session_window_can_be_found() {
    // The periodic placement check knows the output per session
    // and must get back to the window to place it again.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(42), "A".into()));
    assert_eq!(t.fenetre_de(&session), Some(IdFenetre(42)));
    assert_eq!(t.fenetre_de(&IdSession("w-inconnue".into())), None);
}

#[test]
fn a_re_announced_window_creates_no_second_entry() {
    // The fixed leak case: the startup enumeration and the
    // `EVENT_OBJECT_SHOW` hook can both announce the same HWND. Without a
    // guard, the second announcement would create a second entry, unreachable
    // at closing (Windows only emits one closing event per HWND)
    // — and its driver output would leak indefinitely.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    let effets_reannonce = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    assert!(
        effets_reannonce.is_empty(),
        "no second effect, in particular no second AnnoncerOuverture"
    );
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));

    t.viewport_recu(&session, 1600, 900);
    t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));

    let effets = t.fenetre_disparue(IdFenetre(1));
    assert_eq!(
        effets,
        vec![
            Effet::TuerEnfant {
                session: session.clone()
            },
            // A single DetruireSortie: the leak would be a second
            // output never destroyed because never found.
            Effet::DetruireSortie {
                sortie_pilote: 7,
                nom_sortie: "\\\\.\\DISPLAY4".into()
            },
            Effet::AnnoncerFermeture { session },
        ],
        "a single output to destroy, not two"
    );
}

#[test]
fn a_re_announcement_does_not_trigger_the_refusal_even_with_a_full_table() {
    // Precedence not to invert: the idempotence guard must be
    // evaluated BEFORE the capacity check. If the order were inverted, the
    // re-announcement of an already open window, on a full table,
    // would wrongly produce an `AnnoncerRefus` for a window that is
    // nevertheless already open.
    let mut t = Table::new(1);
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let effets = t.fenetre_apparue(IdFenetre(1), "A".into());
    assert!(effets.is_empty(), "no refusal for an already open window");
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
}

/// Defect §3.3 bis of D1: the child received `(adaptateur, sortie)`, a
/// POSITIONAL pair it resolved later — after other outputs
/// could have appeared or disappeared. Hence the `no DXGI output at
/// adapter index 0, output 5` noted during acceptance.
#[test]
fn the_output_is_passed_to_the_child_by_its_name() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);

    let effets = t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY7".into(),
            size: (1280, 720),
        }]
    );
    assert_eq!(t.nom_sortie_de(&session), Some("\\\\.\\DISPLAY7"));
}

/// Destruction carries BOTH identifiers — the driver's to remove,
/// the DXGI name to free the slot — and they have no computable relation.
#[test]
fn the_destruction_carries_the_driver_identifier_and_the_dxgi_name() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("opening expected, got {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let effets = t.fenetre_disparue(IdFenetre(1));

    assert!(effets.contains(&Effet::DetruireSortie {
        sortie_pilote: 42,
        nom_sortie: "\\\\.\\DISPLAY7".into(),
    }));
}

// Task 10's tests (restart after a child's death, safeguards of
// RELANCES_MAX and of viewport staleness) are extracted into a neighbouring
// file: adding them made this file cross the project's 500-line
// ceiling. See `table/tests_relance.rs`.

// ---- The VM prefix (sub-block P3, task 19) ------------------------------

/// 🔴 **NO test froze the name `w-1` before this one**: all go through
/// `session_annoncee`, which returns the identifier whatever it is. Setting the
/// separator unconditionally would therefore have given `":w-1"` without anything
/// turning red — and `":w-1"` is the name of no existing session.
#[test]
fn without_prefix_a_session_keeps_exactly_its_current_name() {
    let mut t = Table::new(10);
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    assert_eq!(session, IdSession("w-1".into()));
}

#[test]
fn with_a_prefix_the_session_carries_it_before_its_name() {
    let mut t = Table::with_prefix(10, "Zm9vYmFy".into());
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    assert_eq!(session, IdSession("Zm9vYmFy:w-1".into()));
}

/// 🔴 The ACQUIRED property P3 must not lose along the way: the counter
/// grows without ever going back, "a reused identifier would pair a
/// late browser message with the wrong window" (`table.rs`). The
/// prefix makes the namespace global without changing anything in the mechanism
/// carrying it — mutation that turns red: deriving the counter from `entrees.len()`.
#[test]
fn the_counter_never_goes_back_even_under_a_prefix() {
    let mut t = Table::with_prefix(10, "Zm9vYmFy".into());
    let premiere = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    assert_eq!(premiere, IdSession("Zm9vYmFy:w-1".into()));
    t.fenetre_disparue(IdFenetre(1));
    let seconde = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "Explorateur".into()));
    assert_eq!(
        seconde,
        IdSession("Zm9vYmFy:w-2".into()),
        "the table REUSED an identifier: a late message from the browser \
         would pair with the wrong window"
    );
}
