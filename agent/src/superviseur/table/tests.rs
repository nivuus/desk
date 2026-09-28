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
        .expect("une ouverture doit être annoncée")
}

#[test]
fn une_fenetre_qui_apparait_est_annoncee_et_rien_de_plus() {
    // Nothing can be created before knowing the viewport: it is what
    // gives the output's size.
    let mut t = table();
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    assert_eq!(effets.len(), 1);
    let session = session_annoncee(&effets);
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
}

#[test]
fn le_viewport_declenche_la_creation_de_la_sortie() {
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
fn la_sortie_creee_declenche_le_lancement_de_l_enfant() {
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
fn deux_fenetres_en_vol_gardent_chacune_leur_fenetre_et_leur_sortie() {
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
fn une_fenetre_qui_disparait_tue_l_enfant_detruit_la_sortie_et_l_annonce() {
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
        "la fenêtre doit avoir quitté la table"
    );
}

#[test]
fn un_enfant_qui_meurt_seul_retient_la_sortie_et_l_annonce_sans_le_tuer() {
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
        "la sortie est retenue, pas rendue"
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
fn le_vivier_plein_refuse_la_fenetre_suivante_sans_rien_casser() {
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
            motif: "plus aucune sortie virtuelle disponible".into()
        }]
    );
}

#[test]
fn une_sortie_liberee_rouvre_la_place() {
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
fn un_viewport_pour_une_session_inconnue_est_ignore() {
    // The browser is an external source: a late or replayed message
    // must produce no effect.
    let mut t = table();
    let effets = t.viewport_recu(&IdSession("w-inconnue".into()), 800, 600);
    assert!(effets.is_empty());
}

#[test]
fn un_second_viewport_pour_la_meme_session_est_ignore() {
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    t.viewport_recu(&session, 1600, 900);
    let effets = t.viewport_recu(&session, 800, 600);
    assert!(
        effets.is_empty(),
        "la sortie est déjà demandée à la première taille"
    );
}

#[test]
fn les_identifiants_de_session_sont_uniques() {
    let mut t = table();
    let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let b = session_annoncee(&t.fenetre_apparue(IdFenetre(2), "B".into()));
    assert_ne!(a, b);
}

#[test]
fn la_fenetre_d_une_session_est_retrouvable() {
    // The periodic placement check knows the output per session
    // and must get back to the window to place it again.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(42), "A".into()));
    assert_eq!(t.fenetre_de(&session), Some(IdFenetre(42)));
    assert_eq!(t.fenetre_de(&IdSession("w-inconnue".into())), None);
}

#[test]
fn une_fenetre_reannoncee_ne_cree_pas_de_seconde_entree() {
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
        "aucun second effet, en particulier aucune seconde AnnoncerOuverture"
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
        "une seule sortie à détruire, pas deux"
    );
}

#[test]
fn une_reannonce_ne_declenche_pas_le_refus_meme_table_pleine() {
    // Precedence not to invert: the idempotence guard must be
    // evaluated BEFORE the capacity check. If the order were inverted, the
    // re-announcement of an already open window, on a full table,
    // would wrongly produce an `AnnoncerRefus` for a window that is
    // nevertheless already open.
    let mut t = Table::new(1);
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let effets = t.fenetre_apparue(IdFenetre(1), "A".into());
    assert!(
        effets.is_empty(),
        "pas de refus pour une fenêtre déjà ouverte"
    );
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
}

/// Defect §3.3 bis of D1: the child received `(adaptateur, sortie)`, a
/// POSITIONAL pair it resolved later — after other outputs
/// could have appeared or disappeared. Hence the `no DXGI output at
/// adapter index 0, output 5` noted during acceptance.
#[test]
fn la_sortie_est_transmise_a_l_enfant_par_son_nom() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
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
fn la_destruction_porte_l_identifiant_pilote_et_le_nom_dxgi() {
    let mut t = Table::new(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
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
fn sans_prefixe_une_session_garde_exactement_son_nom_d_aujourd_hui() {
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
        "la table a REUTILISÉ un identifiant : un message tardif du navigateur \
         s'apparierait à la mauvaise fenêtre"
    );
}
