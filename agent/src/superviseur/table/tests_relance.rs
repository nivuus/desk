//! Tests de la tâche 10 : relance d'une fenêtre dont l'enfant est mort, et
//! les deux garde-fous de capacité qui l'accompagnent (`RELANCES_MAX`,
//! `DELAI_ATTENTE_VIEWPORT_MAX`).
//!
//! Extrait de `table/tests.rs` : l'ajout de ces tests l'a fait franchir le
//! plafond de 500 lignes du projet. Ces tests lisent `AnnoncerOuverture`
//! directement (le pattern `let Some(Effet::AnnoncerOuverture { .. }) = ...`)
//! plutôt que via le helper `session_annoncee` de `tests.rs` — celui-ci
//! n'aurait servi à rien ici, la session étant systématiquement reprise pour
//! des assertions ultérieures dans le même effet.

use super::*;

/// Une base d'instants qui ne lit pas l'horloge du système : `Table` n'en lit
/// aucune, c'est tout l'intérêt (même montage que `agent/src/capture/reprise.rs`).
fn instant(base: std::time::Instant, ms: u64) -> std::time::Instant {
    base + std::time::Duration::from_millis(ms)
}

/// Le second défaut de conception du §3.3 de D1 : `enfant_mort` retirait
/// l'entrée, et plus rien ne rappelait la fenêtre — sauf un `SHOW` fortuit de
/// Windows. Une fenêtre bien vivante disparaissait de la shell pour toujours,
/// et c'est ce qui laissait la page-shell vide alors que les quatre
/// applications tournaient encore.
#[test]
fn une_fenetre_dont_l_enfant_meurt_est_reproposee() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let effets = t.enfant_mort(&session);
    assert!(
        !effets.iter().any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "depuis D3 §7.1 la sortie est retenue pour la relance, reçu {effets:?}"
    );

    // La fenêtre, elle, n'est pas oubliée : le contrôle périodique la
    // repropose sous une session NEUVE.
    let effets = t.relancer_les_orphelines(std::time::Instant::now());
    let Some(Effet::AnnoncerOuverture { session: neuve, titre }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    assert_ne!(*neuve, session, "un identifiant réutilisé apparierait un message tardif");
    assert_eq!(titre, "Bloc-notes");
}

/// Le garde-fou que l'emballement de D1 rend obligatoire : sans lui, une
/// fenêtre dont l'enfant meurt systématiquement produit la boucle
/// `w-5, w-6, w-7, w-8…` observée en recette.
#[test]
fn une_fenetre_qui_echoue_sans_fin_finit_par_etre_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let mut effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    // `RELANCES_MAX` relances TOLÉRÉES (w-1→w-2→w-3→w-4, trois relances
    // réussies) et c'est la relance suivante, la quatrième, qui essuie le
    // refus : il faut donc `RELANCES_MAX + 1` cycles mort+relance pour
    // observer ce refus, pas `RELANCES_MAX`. Une brique du plan qui bornait
    // la boucle à `0..RELANCES_MAX` s'arrêtait sur la dernière relance
    // réussie (w-4) sans jamais la faire mourir à son tour — l'assertion de
    // refus ne pouvait alors jamais être atteinte.
    //
    // Même instant `instant(base, 0)` à chaque tour : ce test épingle le plafond de
    // RELANCES, pas le délai de staleness (`DELAI_ATTENTE_VIEWPORT_MAX`, très
    // au-dessus de la durée de ce test), les deux garde-fous sont
    // indépendants.
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

/// Une fenêtre qui se ferme pour de bon quitte la table, orpheline ou non :
/// sans quoi `relancer_les_orphelines` la ressusciterait indéfiniment.
#[test]
fn une_fenetre_orpheline_qui_se_ferme_quitte_la_table() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());
    t.fenetre_disparue(IdFenetre(1));
    assert!(t.relancer_les_orphelines(std::time::Instant::now()).is_empty());
}

/// Deuxième moitié d'`enfant_mort`, jusqu'ici non affirmée. Avant le
/// correctif §7.1 de D3, un `.take()` sur `sortie_pilote`/`nom_sortie`
/// évitait qu'un second appel ne redemande au pilote de détruire une sortie
/// déjà rendue. Depuis §7.1, `enfant_mort` ne détruit plus jamais rien : ce
/// risque précis a disparu avec le `.take()` qui le prévenait. Ce qui reste à
/// garantir, c'est qu'une seconde mort ne fait pas fuir la sortie retenue.
#[test]
fn un_second_enfant_mort_ne_fait_pas_fuir_la_sortie_retenue() {
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    let session = session.clone();
    t.viewport_recu(&session, 1280, 720);
    t.sortie_creee(&session, 42, "\\\\.\\DISPLAY7".into(), (1280, 720));

    let premier = t.enfant_mort(&session);
    assert!(
        !premier.iter().any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "depuis D3 §7.1 la première mort ne rend déjà plus la sortie, reçu {premier:?}"
    );

    let second = t.enfant_mort(&session);
    assert!(
        !second.iter().any(|e| matches!(e, Effet::DetruireSortie { .. })),
        "une seconde mort ne doit pas non plus la rendre, reçu {second:?}"
    );
    assert_eq!(
        t.nom_sortie_de(&session),
        Some("\\\\.\\DISPLAY7"),
        "la sortie doit toujours être retenue après deux morts"
    );
}

/// Le second risque de capacité de la tâche 10 : une entrée relancée reste
/// `AttendLeViewport`, et si la page-shell ne répond jamais (pop-up bloqué,
/// shell déconnectée — `CLAUDE.md` documente ce cas nommément), rien ne la
/// relève : elle n'est plus `SansSession` (le premier filtre de
/// `relancer_les_orphelines` ne la voit plus) et elle n'atteindra jamais
/// `Vivante`. Sans ce garde-fou, sa place resterait perdue jusqu'à l'arrêt du
/// superviseur.
#[test]
fn une_relance_qui_stagne_sans_viewport_finit_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());

    // La relance a lieu à instant(base, 0) : l'entrée neuve porte cet instant.
    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture { session: relancee, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let relancee = relancee.clone();
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLeViewport));

    // Aucun viewport ne vient jamais. Bien avant le délai, rien ne se passe.
    assert!(
        t.relancer_les_orphelines(instant(base, 100)).is_empty(),
        "une entrée qui n'a pas encore stagné ne doit rien produire"
    );

    // Passé le délai, l'entrée est abandonnée — et retirée de la table.
    let apres_delai = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 + 1;
    let effets = t.relancer_les_orphelines(instant(base, apres_delai));
    assert!(
        matches!(effets.first(), Some(Effet::AnnoncerRefus { titre, .. }) if titre == "Bloc-notes"),
        "au-delà du délai, un refus plutôt qu'un silence indéfini, reçu {effets:?}"
    );
    assert_eq!(t.etat(&relancee), None, "l'entrée figée doit avoir quitté la table");
}

/// Le pendant du test précédent : une entrée relancée qui reçoit son viewport
/// À TEMPS ne doit jamais être abandonnée, même longtemps après.
#[test]
fn une_relance_qui_repond_a_temps_n_est_pas_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    let effets = t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());
    let Some(Effet::AnnoncerOuverture { session, .. }) = effets.first() else {
        panic!("ouverture attendue, reçu {effets:?}");
    };
    t.enfant_mort(&session.clone());

    let effets = t.relancer_les_orphelines(instant(base, 0));
    let Some(Effet::AnnoncerOuverture { session: relancee, .. }) = effets.first() else {
        panic!("réouverture attendue, reçu {effets:?}");
    };
    let relancee = relancee.clone();

    // Le viewport arrive avant le délai.
    let effets = t.viewport_recu(&relancee, 1280, 720);
    assert!(!effets.is_empty(), "le viewport doit déclencher la création de sortie");
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));

    // Longtemps après, largement au-delà du délai : l'entrée n'est plus
    // `AttendLeViewport`, le garde-fou ne la concerne plus.
    let bien_plus_tard = DELAI_ATTENTE_VIEWPORT_MAX.as_millis() as u64 * 10;
    assert!(
        t.relancer_les_orphelines(instant(base, bien_plus_tard)).is_empty(),
        "une entrée qui a répondu à temps ne doit jamais être abandonnée"
    );
    assert_eq!(t.etat(&relancee), Some(&Etat::AttendLaSortie));
}

/// §7.3 du sous-bloc D2, corrigé en D3. Une fenêtre NEUVE dont la page-shell
/// ne répond jamais restait `AttendLeViewport` sans être ni relancée ni
/// abandonnée : ni `SansSession`, ni `Vivante`. Sa place était perdue jusqu'à
/// l'arrêt du superviseur.
#[test]
fn une_fenetre_neuve_dont_la_shell_ne_repond_jamais_finit_par_etre_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
    t.fenetre_apparue(IdFenetre(1), "Bloc-notes".into());

    // Premier passage : le tampon est posé, rien n'est abandonné.
    assert!(t.relancer_les_orphelines(base).is_empty());

    // Le délai court à partir du premier passage, pas du démarrage.
    let effets = t.relancer_les_orphelines(instant(base, 30_001));

    assert!(
        effets.iter().any(|e| matches!(e, Effet::AnnoncerRefus { .. })),
        "la place doit être libérée, reçu {effets:?}"
    );
    assert_eq!(t.fenetre_apparue(IdFenetre(2), "Autre".into()).len(), 1);
}

/// Le tampon ne doit pas abandonner une fenêtre qui répond dans le délai.
#[test]
fn une_fenetre_neuve_qui_repond_dans_le_delai_n_est_pas_abandonnee() {
    let base = std::time::Instant::now();
    let mut t = Table::nouvelle(4);
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
