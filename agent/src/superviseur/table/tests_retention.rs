//! Tests du sous-bloc D3 : la sortie virtuelle est RETENUE entre la mort d'un
//! enfant et sa relance, au lieu d'être détruite puis recréée.
//!
//! Fichier distinct de `tests_relance.rs` : celui-ci est à 211 lignes et le
//! plafond du projet est à 500, mais la vraie raison est de lisibilité — ces
//! tests portent sur la rétention, ceux-là sur les garde-fous de capacité.

use super::*;

/// Ouvre une fenêtre et la mène jusqu'à `Vivante`, en rendant la session.
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    let effets = t.fenetre_apparue(IdFenetre(fenetre), titre.into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, sortie, nom.into(), (1280, 720));
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
    // Le pilote quantifie : demandé 1280×713, rendu 1280×720. C'est la taille
    // RENDUE qu'un viewport ultérieur devra égaler.
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

/// Le correctif §7.1 de D3. Avant lui, `enfant_mort` rendait la sortie au
/// pilote et la relance en recréait une — et c'est cette RECRÉATION qui
/// abandonne le mutex de toutes les duplications voisines (D2, 44 pertes
/// d'accès encaissées ; une seule fenêtre condamnée faisait passer le
/// compteur de réouvertures de 6 à 38).
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
    assert!(effets.contains(&Effet::AnnoncerFermeture { session: session.clone() }));
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

    assert_ne!(neuve, session, "un identifiant réutilisé apparierait un message tardif");
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

/// **Le chemin qui supprime les réouvertures parasites.** La fenêtre garde sa
/// sortie, le viewport annoncé lui correspond : plus rien à créer, donc plus
/// aucun mutex abandonné chez les voisines.
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
            audio: true
        }],
        "ni DetruireSortie ni CreerSortie : c'est tout l'objet du correctif"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante));
}

/// La tolérance est celle de l'appariement — quatre pixels — et pas davantage.
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

    assert!(matches!(effets.first(), Some(Effet::LancerEnfant { .. })), "reçu {effets:?}");
}

/// Le navigateur a redimensionné sa fenêtre entre-temps : la sortie retenue
/// ne convient plus, il faut la rendre AVANT d'en demander une autre — sans
/// quoi elle resterait captive du vivier de dix.
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
    assert_eq!(t.nom_sortie_de(&neuve), None, "l'entrée ne retient plus rien");
    assert_eq!(t.etat(&neuve), Some(&Etat::AttendLaSortie));
}

/// Un message du navigateur est une source externe : rejoué, il ne doit pas
/// relancer un second enfant sur la même sortie.
#[test]
fn un_viewport_rejoue_apres_reutilisation_ne_fait_rien() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let neuve = neuve.clone();
    t.viewport_recu(&neuve, 1280, 720);

    assert!(t.viewport_recu(&neuve, 1280, 720).is_empty());
}

/// Contrepartie du §7.1 : une entrée abandonnée porte désormais une sortie,
/// ce qui n'arrivait jamais avant D3. L'oublier viderait le vivier de dix du
/// pilote, silencieusement, jusqu'à l'arrêt du superviseur.
#[test]
fn l_abandon_apres_relances_max_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let mut session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    // RELANCES_MAX relances, puis l'abandon au tour suivant.
    for tour in 0..=RELANCES_MAX {
        t.enfant_mort(&session);
        let effets = t.relancer_les_orphelines(base + std::time::Duration::from_secs(tour as u64));
        if let Some(Effet::AnnoncerOuverture { session: neuve, .. }) = effets.first() {
            session = neuve.clone();
            // La sortie suit ; on ne la recrée pas.
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

/// Second chemin d'abandon : la page-shell ne répond jamais après la relance.
/// L'entrée porte encore sa sortie retenue — même exigence.
#[test]
fn l_abandon_d_une_entree_figee_rend_la_sortie() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    t.enfant_mort(&session);
    // Relance : l'entrée repasse en AttendLeViewport, tamponnée à `base`.
    t.relancer_les_orphelines(base);

    // La page-shell ne répond jamais : au-delà du délai, abandon.
    let effets = t.relancer_les_orphelines(base + DELAI_ATTENTE_VIEWPORT_MAX + std::time::Duration::from_secs(1));

    assert!(
        effets.contains(&Effet::DetruireSortie {
            sortie_pilote: 42,
            nom_sortie: "\\\\.\\DISPLAY7".into()
        }),
        "la sortie retenue doit être rendue, reçu {effets:?}"
    );
}
