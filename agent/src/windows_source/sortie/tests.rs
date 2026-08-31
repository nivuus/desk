//! Les tests d'hôte de `windows_source_sortie`.
//!
//! **Extrait de `sortie.rs` VERBATIM**, dans une tâche DÉDIÉE et **AVANT**
//! l'addition qui l'aurait porté au-delà du plafond de 500 lignes du projet —
//! c'est la forme forte que `CLAUDE.md` prescrit (« extraire, jamais
//! comprimer », et « la forme forte est l'extraction jouée dans une tâche
//! DÉDIÉE, AVANT celle qui ajoute »). Aucune assertion, aucun commentaire n'a
//! été réécrit au déplacement.
//!
//! Déclaré par `#[path = "sortie/tests.rs"] mod tests;` **à l'intérieur** de
//! `sortie.rs`, et non à la racine du crate : c'est le mécanisme employé pour
//! scinder un module de TESTS trop long dans un fichier par ailleurs portable
//! (précédent : `superviseur/table.rs`, qui déclare ainsi `table/tests.rs` et
//! `table/tests_relance.rs`). ⚠️ **Ce n'est PAS la convention de module enfant
//! de `CLAUDE.md`** — celle-ci ne régit que les modules extraits d'un parent
//! `#[cfg(windows)]` pour compiler sur l'hôte, et ce fichier-ci n'en est pas
//! un.

use super::*;

#[test]
fn une_taille_sous_le_plafond_passe_telle_quelle() {
    assert_eq!(borner_a_la_taille_max((1280, 720)), (1280, 720));
}

#[test]
fn une_taille_4k_est_ramenee_au_plafond() {
    // D6 a mesuré le décodeur du navigateur saturé dès huit fenêtres de
    // 720p : 9× les pixels d'une seule est exactement ce qu'il encaisse le
    // plus mal.
    assert_eq!(borner_a_la_taille_max((3840, 2160)), TAILLE_MAX_SORTIE);
}

#[test]
fn le_bornage_preserve_le_rapport_d_aspect() {
    // Un 21:9 borné indépendamment sur chaque axe déformerait l'image.
    let (l, h) = borner_a_la_taille_max((3440, 1440));
    assert!(l <= TAILLE_MAX_SORTIE.0 && h <= TAILLE_MAX_SORTIE.1, "{l}x{h}");
    let ecart = (l as f64 / h as f64) - (3440.0 / 1440.0);
    assert!(ecart.abs() < 0.01, "rapport {l}/{h} contre 3440/1440");
}

#[test]
fn le_bornage_rend_des_dimensions_paires() {
    // Une fenêtre Windows impose des dimensions paires, et un encodeur
    // NV12 aussi.
    let (l, h) = borner_a_la_taille_max((3441, 1441));
    assert_eq!(l % 2, 0, "largeur {l}");
    assert_eq!(h % 2, 0, "hauteur {h}");
}

/// IMPORTANT 2 de la revue de la tâche 9 : **le test ci-dessus ne peut pas
/// échouer sur la propriété qu'il nomme.**
///
/// Sur `(3441, 1441)`, `facteur ≈ 0,557977` donne `round(3441×f) = 1920` et
/// `round(1441×f) = 804` — **déjà pairs avant tout masquage**. Retirer les
/// deux `& !1` de `borner_a_la_taille_max` le laisse VERT. Et les deux
/// autres entrées paires des tests voisins (`(1280, 720)`, `(0, 0)`) ne
/// l'exercent pas davantage : avant ce cas-ci, **aucun des cinq tests ne
/// couvrait l'alignement pair**, alors que l'un porte son nom.
///
/// `(1281, 721)` passe par la **branche rapide** (sous le plafond, donc
/// aucun facteur d'échelle) : l'alignement y est le seul mécanisme en jeu,
/// et le retrait des `& !1` rend `(1281, 721)` au lieu de `(1280, 720)`.
/// C'est la doctrine du dépôt appliquée à un test : **un contrôle qu'on n'a
/// jamais vu rouge n'est pas un contrôle** (D7, F1).
#[test]
fn l_alignement_pair_est_reellement_exerce_par_une_entree_impaire() {
    assert_eq!(borner_a_la_taille_max((1281, 721)), (1280, 720));
}

/// IMPORTANT 4 (revue de la tâche 9) : la branche rapide ne bornait pas
/// vers le bas, contrairement à la branche d'échelle qui appliquait déjà
/// `.max(2)`. `(0, 0)` en est le cas dégénéré réel : une boîte vidéo
/// réduite à rien (fenêtre repliée, transition de plein écran) l'émet.
#[test]
fn le_bornage_ne_rend_jamais_une_dimension_nulle() {
    assert_eq!(borner_a_la_taille_max((0, 0)), (2, 2));
}

#[test]
fn la_region_part_de_l_origine_de_la_sortie() {
    assert_eq!(
        region_de_sortie(1600, 900),
        Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
    );
}

#[test]
fn les_dimensions_impaires_sont_alignees_vers_le_bas() {
    assert_eq!(
        region_de_sortie(1601, 901),
        Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
    );
}

#[test]
fn une_sortie_degeneree_ne_donne_aucune_region() {
    assert_eq!(region_de_sortie(1, 900), None);
    assert_eq!(region_de_sortie(0, 0), None);
}

/// Le défaut C1 en une ligne : c'est ce booléen qui empêche `resize` de
/// retailler une fenêtre qui a sa propre sortie, donc de relâcher la
/// duplication de cette sortie et de lui substituer celle du bureau
/// physique.
#[test]
fn seul_le_mode_recadre_redimensionne_la_fenetre() {
    assert!(ModeCapture::FenetreRecadree.redimensionne_la_fenetre());
    assert!(!ModeCapture::SortieEntiere.redimensionne_la_fenetre());
}
