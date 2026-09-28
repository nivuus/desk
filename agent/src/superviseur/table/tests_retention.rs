//! Sub-block D3's tests: the virtual output is RETAINED between a child's death
//! and its restart, instead of being destroyed then recreated.
//!
//! File distinct from `tests_relance.rs`: that one is at 211 lines and the
//! project's ceiling is at 500, but the real reason is readability — these
//! tests are about retention, those about capacity safeguards.

use super::*;

/// Opens a window and takes it to `Vivante`, returning the session.
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    session_vivante_de_taille(t, fenetre, titre, sortie, nom, (1280, 720))
}

/// Same bootstrap, but the output is born at an imposed size — the case of a VM
/// whose registry was polluted (D9 §9).
fn session_vivante_de_taille(
    t: &mut Table,
    fenetre: u64,
    titre: &str,
    sortie: u32,
    nom: &str,
    taille: (u32, u32),
) -> IdSession {
    let effets = t.fenetre_apparue(IdFenetre(fenetre), titre.into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, sortie, nom.into(), taille);
    session
}

#[test]
fn la_table_retient_la_taille_reelle_de_la_sortie() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 713);
    // The driver quantises: 1280×713 requested, 1280×720 returned. It is the RETURNED
    // size that a later viewport will have to match.
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    assert_eq!(t.taille_sortie_de(&session), Some((1280, 720)));
}

#[test]
fn une_session_sans_sortie_n_a_pas_de_taille() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    assert_eq!(t.taille_sortie_de(session), None);
}

#[test]
fn la_taille_survit_a_la_mort_de_l_enfant() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    assert_eq!(
        t.taille_sortie_de(&session),
        Some((1280, 720)),
        "la taille accompagne la sortie retenue"
    );
}

/// D3's fix §7.1. Before it, `enfant_mort` handed back the output to the
/// driver and the restart recreated one — and it is this RECREATION that
/// abandons the mutex of all neighbouring duplications (D2, 44 access
/// losses absorbed; a single doomed window took the
/// reopening counter from 6 to 38).
#[test]
fn la_mort_de_l_enfant_ne_rend_plus_la_sortie_au_pilote() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.enfant_mort(&session);

    assert!(
        !effets
            .iter()
            .any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "la sortie est retenue pour la relance, reçu {effets:?}"
    );
    assert!(effets.contains(&Effet::AnnoncerFermeture {
        session: session.clone()
    }));
}

#[test]
fn l_entree_relancee_porte_encore_sa_sortie() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);

    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    assert_ne!(
        neuve, session,
        "un identifiant réutilisé apparierait un message tardif"
    );
    assert_eq!(
        t.nom_sortie_de(&neuve),
        Some("\\\\.\\DISPLAY7"),
        "la sortie suit la fenêtre dans sa nouvelle session"
    );
    assert_eq!(t.taille_sortie_de(&neuve), Some((1280, 720)));
}

#[test]
fn sans_sortie_retenue_le_viewport_en_demande_une() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();

    let effets = t.viewport_recu(&session, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::CreerSortie {
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
fn une_sortie_retenue_compatible_est_reutilisee_sans_rien_creer() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: neuve.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY7".into(),
            taille: (1280, 720),
        }],
        "ni DetruireSortie ni CreerSortie : c'est tout l'objet du correctif"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante));
}

/// A retained output LARGER than the viewport will serve again: destroying
/// and recreating it would make the neighbouring duplications abandon the
/// mutex at each restart — exactly the recreation sub-block
/// D3 exists to remove, and the cause of its 32 spurious reopenings.
#[test]
fn une_sortie_retenue_plus_grande_est_reutilisee() {
    let mut t = Table::nouvelle(4);
    let session =
        session_vivante_de_taille(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY8", (3840, 2160));
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1280, 720);

    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: neuve.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY8".into(),
            taille: (1280, 720),
        }],
        "une sortie retenue assez grande ne doit être ni détruite ni recréée"
    );
}

/// The tolerance is the pairing one — four pixels — and no more.
#[test]
fn une_sortie_retenue_a_quatre_pixels_pres_est_reutilisee() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();

    let effets = t.viewport_recu(&neuve, 1278, 718);

    assert!(
        matches!(effets.first(), Some(Effet::LancerEnfant { .. })),
        "reçu {effets:?}"
    );
}

/// The browser resized its window meanwhile: the retained output
/// no longer fits, it must be handed back BEFORE requesting another — otherwise
/// it would stay captive from the pool of ten.
#[test]
fn une_sortie_retenue_incompatible_est_rendue_puis_remplacee() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
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
            Effet::CreerSortie {
                session: neuve.clone(),
                titre: "Bloc-notes".into(),
                largeur: 1920,
                hauteur: 1080
            },
        ],
        "la destruction précède la demande, et dans cet ordre"
    );
    assert_eq!(
        t.nom_sortie_de(&neuve),
        None,
        "l'entrée ne retient plus rien"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::AttendLaSortie));
}

/// 🔴 **THE TEST THAT SEES THE DEFECT, AND WITHOUT IT ITS NEIGHBOUR IS VACUOUS.**
/// `un_viewport_rejoue_ne_fait_avancer_aucune_machine` asserted
/// `rejeu.iter().all(matches!(SuivreLeViewport))` — and **an EMPTY vector
/// satisfies `all()`**. Returning `Vec::new()` on a live session, that is,
/// yesterday's behaviour, therefore left it GREEN. It is the pattern
/// `CLAUDE.md` names "a check never seen red is not a
/// check", and it was caught by playing the mutation, not by rereading.
///
/// This test asserts that the effect IS produced, and with the requested size:
/// it is the one that turns red if the `Vivante` branch disappears.
#[test]
fn un_viewport_sur_une_session_vivante_demande_de_suivre() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.etat(&session), Some(&Etat::Vivante), "précondition");

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
        autre => panic!("un seul suivi de viewport attendu, reçu {autre:?}"),
    }
    // The table advanced nothing: it is the loop that measures and applies.
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));
}

/// Ceiling ① runs ALSO on this path: without it, a client at
/// `devicePixelRatio = 2` would have a 2560×1440 window put on an output
/// that cannot carry it. Same bounding as at the two other entry points
/// of the viewport (`creer_sortie`, reuse path).
#[test]
fn un_viewport_hidpi_sur_une_session_vivante_est_borne_avant_de_partir() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.viewport_recu(&session, 3840, 2160);

    let [Effet::SuivreLeViewport {
        largeur, hauteur, ..
    }] = effets.as_slice()
    else {
        panic!("un suivi de viewport attendu, reçu {effets:?}");
    };
    assert_eq!(
        (*largeur, *hauteur),
        crate::windows_source_sortie::TAILLE_MAX_SORTIE,
        "le plafond doit être appliqué AVANT que l'effet ne parte"
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
fn un_viewport_rejoue_ne_fait_avancer_aucune_machine() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();
    t.viewport_recu(&neuve, 1280, 720);
    let avant = t.taille_sortie_de(&neuve);

    let rejeu = t.viewport_recu(&neuve, 1280, 720);

    // The ONLY tolerated effect, and nothing else: no `CreerSortie`, no
    // `DetruireSortie`, no `LancerEnfant`.
    assert!(
        rejeu
            .iter()
            .all(|e| matches!(e, Effet::SuivreLeViewport { .. })),
        "un rejeu ne doit produire qu'un suivi de viewport, reçu {rejeu:?}"
    );
    assert_eq!(
        t.etat(&neuve),
        Some(&Etat::Vivante),
        "l'état ne doit pas avancer"
    );
    assert_eq!(
        t.taille_sortie_de(&neuve),
        avant,
        "la TABLE ne borne pas elle-même : c'est la boucle qui écrit la taille retenue, \
         après avoir lu la zone de travail — un rejeu ne doit rien changer ici"
    );
}

/// Counterpart of §7.1: an abandoned entry now carries an output,
/// which never happened before D3. Forgetting it would empty the driver's
/// pool of ten, silently, until supervisor shutdown.
#[test]
fn l_abandon_apres_relances_max_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
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
            "la sortie retenue doit être rendue à l'abandon, reçu {effets:?}"
        );
        assert!(
            effets
                .iter()
                .any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
            "reçu {effets:?}"
        );
        return;
    }
    panic!("l'abandon n'est jamais survenu");
}

/// Second abandonment path: the shell page never answers after the restart.
/// The entry still carries its retained output — same requirement.
#[test]
fn l_abandon_d_une_entree_figee_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
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
        "la sortie retenue doit être rendue, reçu {effets:?}"
    );
}

/// Former IMPORTANT 5 (review of task 9): `changer_mode_de_sortie` (D8)
/// resized an output outside this table, and `rafraichir_taille_sortie`
/// was the catch-up. **That path was removed in sub-block D9**, with measurement
/// to back it (see the finding at the head of `capteur/plein_ecran.rs`). **And
/// `rafraichir_taille_sortie` has had no caller since sub-block
/// D10**: the periodic placement check (`placement_periodique.rs`)
/// called it on each fresh DXGI read, but `taille_sortie` now carries
/// the RETAINED size, with no more reason to equal the raw DXGI
/// size — this refresh would therefore have overwritten it, and the call was
/// removed. This test covers the method itself, general and still
/// exposed: the repercussion on what `viewport_recu` will compare at the
/// next restart.
#[test]
fn rafraichir_la_taille_met_a_jour_une_sortie_deja_retenue() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.taille_sortie_de(&session), Some((1280, 720)));

    // An output resized by some mechanism (none exists any more
    // in production since D9, and since D10 no caller even invokes
    // this method — see the doc above). A future mechanism of that
    // kind would have to pass it the RETAINED size, not reread the output's raw
    // DXGI size.
    t.rafraichir_taille_sortie(&session, (1920, 1080));

    assert_eq!(t.taille_sortie_de(&session), Some((1920, 1080)));
}

/// Must invent NOTHING: a session without a retained output (still waiting
/// for creation, or unknown) stays without a size after the call — only
/// `sortie_creee` has the right to set the very first value.
#[test]
fn rafraichir_la_taille_n_invente_rien_sans_sortie_retenue() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    // Neither `viewport_recu` nor `sortie_creee` has run yet: no output
    // is retained.
    assert_eq!(t.taille_sortie_de(&session), None);

    t.rafraichir_taille_sortie(&session, (1920, 1080));
    assert_eq!(
        t.taille_sortie_de(&session),
        None,
        "rien à rafraîchir, rien n'a dû apparaître"
    );

    // A totally unknown session must not panic nor create
    // a ghost entry either.
    t.rafraichir_taille_sortie(&IdSession("w-inconnue".into()), (1920, 1080));
}
