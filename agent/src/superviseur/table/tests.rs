use super::*;

fn table() -> Table {
    Table::nouvelle(10)
}

/// Récupère l'identifiant de session attribué à la fenêtre, en lisant
/// l'effet d'annonce — c'est la seule sortie publique qui le porte.
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
    // Rien ne peut être créé avant de connaître le viewport : c'est lui
    // qui donne la taille de la sortie.
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
        vec![Effet::CreerSortie {
            session: session.clone(),
            // Le titre suit la demande : un refus de sortie s'affiche à un
            // humain, et « w-1 » ne lui désigne rien.
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
    // Deux identifiants distincts, et c'est le fond du sujet : `7` est
    // l'identifiant que le PILOTE a rendu, `"\\.\DISPLAY4"` le nom de la
    // même sortie dans l'énumération DXGI. Le pilote ne détruit que par
    // le premier ; l'enfant ne sait capturer que par le second.
    let effets = t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));
    assert_eq!(
        effets,
        vec![Effet::LancerEnfant {
            session: session.clone(),
            fenetre: IdFenetre(1),
            nom_sortie: "\\\\.\\DISPLAY4".into(),
        }]
    );
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));
    assert_eq!(t.nom_sortie_de(&session), Some("\\\\.\\DISPLAY4"));
}

/// Deux fenêtres en vol en même temps ne doivent jamais se faire attribuer
/// l'identifiant ou la sortie l'une de l'autre : chaque `Effet::LancerEnfant`
/// doit porter le `IdFenetre` et le `nom_sortie` de SA PROPRE fenêtre.
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
            Effet::TuerEnfant { session: session.clone() },
            // L'identifiant du PILOTE, seul avec lequel il sait retirer.
            Effet::DetruireSortie { sortie_pilote: 7, nom_sortie: "\\\\.\\DISPLAY4".into() },
            Effet::AnnoncerFermeture { session: session.clone() },
        ]
    );
    assert_eq!(t.etat(&session), None, "la fenêtre doit avoir quitté la table");
}

#[test]
fn un_enfant_qui_meurt_seul_retient_la_sortie_et_l_annonce_sans_le_tuer() {
    // C'est le bénéfice pour lequel le multi-processus a été choisi : la
    // mort d'un enfant ne doit rien emporter d'autre. Depuis le correctif
    // §7.1 du sous-bloc D3, elle ne rend plus non plus la sortie au pilote —
    // c'est justement sa RECRÉATION à la relance qui abandonnait le mutex des
    // duplications DXGI voisines (D2, 6 → 38 réouvertures pour une seule
    // fenêtre condamnée).
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into()));
    t.viewport_recu(&session, 1600, 900);
    t.sortie_creee(&session, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));

    let effets = t.enfant_mort(&session);
    assert_eq!(effets, vec![Effet::AnnoncerFermeture { session: session.clone() }]);
    // La fenêtre reste dans la table, orpheline : c'est le contrôle
    // périodique (`relancer_les_orphelines`) qui la reproposera, plutôt
    // qu'un `SHOW` fortuit de Windows — voir la tâche 10.
    assert_eq!(t.etat(&session), Some(&Etat::SansSession));
    assert_eq!(t.taille_sortie_de(&session), Some((1280, 720)), "la sortie est retenue, pas rendue");
}

#[test]
fn une_fenetre_qui_disparait_avant_sa_sortie_ne_demande_aucune_destruction() {
    // Fermée pendant qu'on attendait son viewport : aucune sortie
    // n'existe, et demander d'en détruire une ferait échouer le pilote.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let effets = t.fenetre_disparue(IdFenetre(1));
    assert_eq!(
        effets,
        vec![
            Effet::TuerEnfant { session: session.clone() },
            Effet::AnnoncerFermeture { session },
        ]
    );
}

#[test]
fn le_vivier_plein_refuse_la_fenetre_suivante_sans_rien_casser() {
    let mut t = Table::nouvelle(2);
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
    let mut t = Table::nouvelle(1);
    let a = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    t.viewport_recu(&a, 1280, 720);
    t.sortie_creee(&a, 7, "\\\\.\\DISPLAY4".into(), (1280, 720));
    assert!(matches!(
        t.fenetre_apparue(IdFenetre(2), "B".into()).as_slice(),
        [Effet::AnnoncerRefus { .. }]
    ));

    t.fenetre_disparue(IdFenetre(1));
    let effets = t.fenetre_apparue(IdFenetre(3), "C".into());
    assert!(matches!(effets.as_slice(), [Effet::AnnoncerOuverture { .. }]));
}

#[test]
fn un_viewport_pour_une_session_inconnue_est_ignore() {
    // Le navigateur est une source externe : un message tardif ou rejoué
    // ne doit produire aucun effet.
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
    assert!(effets.is_empty(), "la sortie est déjà demandée à la première taille");
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
    // Le contrôle périodique de placement connaît la sortie par session
    // et doit remonter à la fenêtre pour la replacer.
    let mut t = table();
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(42), "A".into()));
    assert_eq!(t.fenetre_de(&session), Some(IdFenetre(42)));
    assert_eq!(t.fenetre_de(&IdSession("w-inconnue".into())), None);
}

#[test]
fn une_fenetre_reannoncee_ne_cree_pas_de_seconde_entree() {
    // Le cas de fuite corrigé : l'énumération de démarrage et le hook
    // `EVENT_OBJECT_SHOW` peuvent tous deux annoncer le même HWND. Sans
    // garde, la seconde annonce créerait une seconde entrée, inatteignable
    // à la fermeture (Windows n'émet qu'un événement de fermeture par HWND)
    // — et sa sortie pilote fuirait indéfiniment.
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
            Effet::TuerEnfant { session: session.clone() },
            // Une seule DetruireSortie : la fuite serait une seconde
            // sortie jamais détruite parce que jamais retrouvée.
            Effet::DetruireSortie { sortie_pilote: 7, nom_sortie: "\\\\.\\DISPLAY4".into() },
            Effet::AnnoncerFermeture { session },
        ],
        "une seule sortie à détruire, pas deux"
    );
}

#[test]
fn une_reannonce_ne_declenche_pas_le_refus_meme_table_pleine() {
    // Précédence à ne pas inverser : la garde d'idempotence doit être
    // évaluée AVANT le contrôle de capacité. Si l'ordre était inversé, la
    // réannonce d'une fenêtre déjà ouverte, sur une table pleine,
    // produirait à tort un `AnnoncerRefus` pour une fenêtre qui est
    // pourtant déjà ouverte.
    let mut t = Table::nouvelle(1);
    let session = session_annoncee(&t.fenetre_apparue(IdFenetre(1), "A".into()));
    let effets = t.fenetre_apparue(IdFenetre(1), "A".into());
    assert!(effets.is_empty(), "pas de refus pour une fenêtre déjà ouverte");
    assert_eq!(t.etat(&session), Some(&Etat::AttendLeViewport));
}

/// Le défaut §3.3 bis de D1 : l'enfant recevait `(adaptateur, sortie)`, un
/// couple POSITIONNEL qu'il résolvait plus tard — après que d'autres sorties
/// avaient pu apparaître ou disparaître. D'où les `aucune sortie DXGI à
/// l'index adaptateur 0, sortie 5` relevés en recette.
#[test]
fn la_sortie_est_transmise_a_l_enfant_par_son_nom() {
    let mut t = Table::nouvelle(4);
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
        }]
    );
    assert_eq!(t.nom_sortie_de(&session), Some("\\\\.\\DISPLAY7"));
}

/// La destruction porte les DEUX identifiants — celui du pilote pour retirer,
/// le nom DXGI pour libérer la place — et ils n'ont aucune relation calculable.
#[test]
fn la_destruction_porte_l_identifiant_pilote_et_le_nom_dxgi() {
    let mut t = Table::nouvelle(4);
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

// Les tests de la tâche 10 (relance après mort d'enfant, garde-fous de
// RELANCES_MAX et de staleness du viewport) sont extraits dans un fichier
// voisin : leur ajout a fait franchir à ce fichier le plafond de 500 lignes
// du projet. Voir `table/tests_relance.rs`.
