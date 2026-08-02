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
