//! Tests du sous-bloc D3 : la sortie virtuelle est RETENUE entre la mort d'un
//! enfant et sa relance, au lieu d'être détruite puis recréée.
//!
//! Fichier distinct de `tests_relance.rs` : celui-ci est à 211 lignes et le
//! plafond du projet est à 500, mais la vraie raison est de lisibilité — ces
//! tests portent sur la rétention, ceux-là sur les garde-fous de capacité.

use super::*;

/// Ouvre une fenêtre et la mène jusqu'à `Vivante`, en rendant la session.
fn session_vivante(t: &mut Table, fenetre: u64, titre: &str, sortie: u32, nom: &str) -> IdSession {
    session_vivante_de_taille(t, fenetre, titre, sortie, nom, (1280, 720))
}

/// Même amorce, mais la sortie naît à une taille imposée — le cas d'une VM
/// dont le registre a été pollué (D9 §9).
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
            taille: (1280, 720),
        }],
        "ni DetruireSortie ni CreerSortie : c'est tout l'objet du correctif"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante));
}

/// Une sortie retenue plus GRANDE que le viewport resservira : la
/// détruire et la recréer ferait abandonner le mutex des duplications
/// voisines à chaque relance — exactement la recréation que le sous-bloc
/// D3 existe pour supprimer, et la cause de ses 32 réouvertures parasites.
#[test]
fn une_sortie_retenue_plus_grande_est_reutilisee() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante_de_taille(
        &mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY8", (3840, 2160),
    );
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

/// 🔴 **LE TEST QUI VOIT LE DÉFAUT, ET SANS LUI SON VOISIN EST VACUEUX.**
/// `un_viewport_rejoue_ne_fait_avancer_aucune_machine` assertait
/// `rejeu.iter().all(matches!(SuivreLeViewport))` — et **un vecteur VIDE
/// satisfait `all()`**. Rendre `Vec::new()` sur une session vivante, c'est-à-
/// dire le comportement d'hier, le laissait donc VERT. C'est le patron que
/// `CLAUDE.md` nomme « un contrôle qu'on n'a jamais vu rouge n'est pas un
/// contrôle », et il a été attrapé en jouant la mutation, pas en relisant.
///
/// Ce test-ci assère que l'effet EST produit, et avec la taille demandée :
/// c'est lui qui rougit si la branche `Vivante` disparaît.
#[test]
fn un_viewport_sur_une_session_vivante_demande_de_suivre() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.etat(&session), Some(&Etat::Vivante), "précondition");

    let effets = t.viewport_recu(&session, 1600, 900);

    match effets.as_slice() {
        [Effet::SuivreLeViewport { session: s, largeur, hauteur }] => {
            assert_eq!(s, &session);
            assert_eq!((*largeur, *hauteur), (1600, 900));
        }
        autre => panic!("un seul suivi de viewport attendu, reçu {autre:?}"),
    }
    // La table n'a rien avancé : c'est la boucle qui mesure et applique.
    assert_eq!(t.etat(&session), Some(&Etat::Vivante));
}

/// Le plafond ① court AUSSI sur ce chemin : sans lui, un client à
/// `devicePixelRatio = 2` ferait poser une fenêtre de 2560×1440 sur une sortie
/// qui ne peut pas la porter. Même bornage qu'aux deux autres points d'entrée
/// du viewport (`creer_sortie`, chemin de réutilisation).
#[test]
fn un_viewport_hidpi_sur_une_session_vivante_est_borne_avant_de_partir() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");

    let effets = t.viewport_recu(&session, 3840, 2160);

    let [Effet::SuivreLeViewport { largeur, hauteur, .. }] = effets.as_slice() else {
        panic!("un suivi de viewport attendu, reçu {effets:?}");
    };
    assert_eq!(
        (*largeur, *hauteur),
        crate::windows_source_sortie::TAILLE_MAX_SORTIE,
        "le plafond doit être appliqué AVANT que l'effet ne parte"
    );
}

/// Un message du navigateur est une source externe : rejoué, il ne doit pas
/// relancer un second enfant sur la même sortie.
///
/// ❌ **CE TEST S'APPELAIT `un_viewport_rejoue_apres_reutilisation_ne_fait_rien`
/// ET ASSERTAIT `is_empty()`. LE LOT 33 LE REND FAUX, DÉLIBÉRÉMENT** : un
/// viewport reçu sur une session VIVANTE rend désormais
/// `Effet::SuivreLeViewport`, parce que c'est exactement le message par lequel
/// le navigateur dit qu'il a été retaillé. L'ancienne assertion était le
/// **défaut**, pas la protection.
///
/// 🔴 **CE QUE LA PROPRIÉTÉ DEVIENT, ET POURQUOI ELLE PROTÈGE ENCORE.** Ce que
/// ce test gardait réellement — « un message rejoué, tardif ou inventé ne doit
/// pas faire AVANCER LA MACHINE deux fois » — reste vrai et est désormais
/// asserté explicitement : le rejeu ne crée aucune sortie, n'en détruit
/// aucune, ne lance aucun enfant, et ne change pas l'état. `SuivreLeViewport`
/// est de surcroît idempotent chez son exécutant
/// (`placement_periodique::suivre_le_viewport` court-circuite quand la taille
/// retenue vaut déjà celle qu'il calcule), donc un rejeu à l'identique ne pose
/// même pas un second `SetWindowPos`.
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

    // Le SEUL effet toléré, et rien d'autre : pas de `CreerSortie`, pas de
    // `DetruireSortie`, pas de `LancerEnfant`.
    assert!(
        rejeu.iter().all(|e| matches!(e, Effet::SuivreLeViewport { .. })),
        "un rejeu ne doit produire qu'un suivi de viewport, reçu {rejeu:?}"
    );
    assert_eq!(t.etat(&neuve), Some(&Etat::Vivante), "l'état ne doit pas avancer");
    assert_eq!(
        t.taille_sortie_de(&neuve),
        avant,
        "la TABLE ne borne pas elle-même : c'est la boucle qui écrit la taille retenue, \
         après avoir lu la zone de travail — un rejeu ne doit rien changer ici"
    );
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

/// Ex-IMPORTANT 5 (revue de la tâche 9) : `changer_mode_de_sortie` (D8)
/// retaillait une sortie hors de cette table, et `rafraichir_taille_sortie`
/// était le rattrapage. **Ce chemin a été retiré au sous-bloc D9**, mesure à
/// l'appui (voir le constat en tête de `capteur/plein_ecran.rs`). **Et
/// `rafraichir_taille_sortie` n'a plus aucun appelant depuis le sous-bloc
/// D10** : le contrôle périodique de placement (`placement_periodique.rs`)
/// l'appelait sur chaque lecture DXGI fraîche, mais `taille_sortie` porte
/// désormais la taille RETENUE, sans plus de raison d'égaler la taille DXGI
/// brute — ce rafraîchissement l'aurait donc écrasée, et l'appel a été
/// retiré. Ce test couvre la méthode elle-même, générale et toujours
/// exposée : la répercussion sur ce que `viewport_recu` comparera à la
/// prochaine relance.
#[test]
fn rafraichir_la_taille_met_a_jour_une_sortie_deja_retenue() {
    let mut t = Table::nouvelle(4);
    let session = session_vivante(&mut t, 1, "Bloc-notes", 42, "\\\\.\\DISPLAY7");
    assert_eq!(t.taille_sortie_de(&session), Some((1280, 720)));

    // Une sortie retaillée par un mécanisme quelconque (aucun n'existe plus
    // en production depuis D9, et depuis D10 plus aucun appelant n'invoque
    // même cette méthode — voir la doc ci-dessus). Un futur mécanisme de ce
    // genre devrait lui passer la taille RETENUE, pas relire la taille DXGI
    // brute de la sortie.
    t.rafraichir_taille_sortie(&session, (1920, 1080));

    assert_eq!(t.taille_sortie_de(&session), Some((1920, 1080)));
}

/// Ne doit RIEN inventer : une session sans sortie retenue (encore en attente
/// de création, ou inconnue) reste sans taille après l'appel — seul
/// `sortie_creee` a le droit de poser la toute première valeur.
#[test]
fn rafraichir_la_taille_n_invente_rien_sans_sortie_retenue() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    // Ni `viewport_recu` ni `sortie_creee` n'ont encore couru : aucune sortie
    // n'est retenue.
    assert_eq!(t.taille_sortie_de(&session), None);

    t.rafraichir_taille_sortie(&session, (1920, 1080));
    assert_eq!(t.taille_sortie_de(&session), None, "rien à rafraîchir, rien n'a dû apparaître");

    // Une session totalement inconnue ne doit pas non plus paniquer ni créer
    // d'entrée fantôme.
    t.rafraichir_taille_sortie(&IdSession("w-inconnue".into()), (1920, 1080));
}
