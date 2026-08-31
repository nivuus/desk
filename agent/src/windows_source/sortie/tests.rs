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
/// relâcher la duplication d'une sortie virtuelle pour lui substituer
/// celle du bureau physique — la fuite du contenu d'un moniteur vers la
/// session d'autrui.
#[test]
fn seul_le_mode_recadre_recapture_le_bureau() {
    assert!(ModeCapture::FenetreRecadree.recapture_le_bureau());
    assert!(!ModeCapture::SortieEntiere.recapture_le_bureau());
}

/// Le pendant du test ci-dessus, et **les deux ensemble sont le contrat du
/// lot 33** : les deux modes ne se partagent pas seulement un booléen, ils
/// prennent deux chemins EXCLUSIFS. Sans cette seconde assertion, faire
/// rendre `false` aux deux à `suit_le_viewport` ramènerait le `no-op`
/// d'hier sans qu'aucun test ne bronche.
#[test]
fn seul_le_mode_sortie_entiere_suit_le_viewport() {
    assert!(ModeCapture::SortieEntiere.suit_le_viewport());
    assert!(!ModeCapture::FenetreRecadree.suit_le_viewport());
    // Exclusifs, et exhaustifs : tout mode prend exactement un chemin.
    for mode in [ModeCapture::FenetreRecadree, ModeCapture::SortieEntiere] {
        assert!(
            mode.recapture_le_bureau() ^ mode.suit_le_viewport(),
            "{mode:?} doit prendre exactement un des deux chemins"
        );
    }
}

/// 🔴 **L'ATTENDU DE CE TEST VIENT DU JOURNAL DU PRODUIT EN PRODUCTION, PAS
/// D'UN CALCUL SUR CE QU'IL JUGE** — c'est la règle que le lot 32R a payée
/// (« un attendu dérivé de la mesure ne peut pas la réfuter »).
///
/// Les nombres sont relevés le 31 août 2026 sur l'agent qui tournait :
///   - `778x491` : la demande la PLUS FRÉQUENTE des 34 que le garde d'hier a
///     jetées (15 occurrences sur 34, `C:\nivuus\agent.log`, lignes
///     « redimensionnement ignoré … ») ;
///   - `1428x1032` : la borne réelle de la sortie servie — `mon=1428x1080`,
///     `work=1428x1032`, relevé en SESSION 1 par
///     `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md` ;
///   - `1428x1080` : ce que le produit servait, **quoi qu'on lui demande**.
///
/// 🔴 **ET VOICI CE QUE LE PREMIER JET DE CE TEST AVAIT FAUX, ATTRAPÉ PAR LE
/// TEST LUI-MÊME.** Il attendait `(1723, 1080)` pour une demande `1723x1303`,
/// en croyant `borner_a_la_taille_max` un ÉCRÊTAGE axe par axe. **C'en est un
/// de MISE À L'ÉCHELLE, à rapport d'aspect PRÉSERVÉ** : `1723x1303` en sort
/// `1428x1080`, c'est-à-dire exactement la taille que le produit servait déjà.
/// Conséquence qui change le diagnostic et qui est dite ici plutôt qu'oubliée :
/// **les bandes noires ne viennent PAS du plafond**, qui respecte l'aspect
/// demandé, mais du fait que la taille retenue est **FIGÉE à l'ouverture** et
/// que toute demande ultérieure est jetée. C'est ce gel-là que ce lot lève.
#[test]
fn la_demande_la_plus_frequente_est_desormais_honoree_a_l_aspect_pres() {
    // Sous la borne sur les deux axes : elle passe telle quelle (au pair
    // près), donc l'image épouse EXACTEMENT le rapport demandé.
    assert_eq!(taille_pour_viewport((778, 491), (1428, 1032)), (778, 490));
    // …et ce n'est plus 1428×1080, la taille figée d'hier. Sans cette seconde
    // assertion, une règle qui ignorerait sa demande resterait verte si la
    // borne valait 778×490.
    assert_ne!(taille_pour_viewport((778, 491), (1428, 1032)), (1428, 1080));
}

/// 🔴 **CE QUE LE LOT NE RÉSOUT PAS, ÉCRIT EN TEST POUR QUE PERSONNE NE CROIE
/// LE CAS FERMÉ.** `taille_retenue` borne par un `min` axe par axe, qui NE
/// préserve pas le rapport d'aspect — on recadre une texture, on ne la met pas
/// à l'échelle. Un viewport plus large que la borne reste donc servi à la
/// borne, et les bandes noires demeurent sur l'axe débordé.
///
/// Les deux entrées sont mesurées : `5118x1438` est la plus large des 34
/// demandes du journal, `1428x1032` la borne réelle de la sortie servie.
/// `borner_a_la_taille_max` la ramène d'abord à `1919x539` (aspect préservé,
/// 3,559), puis le `min` la borne à `1428x539` — aspect 2,65 contre 3,559
/// demandé.
///
/// **Sans changement de mode d'affichage — que D9 a mesuré et retiré — il n'y
/// a pas d'autre issue**, et créer la sortie plus grande n'est pas établi en
/// donner davantage (D8 : une sortie ne naît pas forcément à la taille
/// demandée).
#[test]
fn un_viewport_plus_large_que_la_borne_garde_ses_bandes_noires() {
    assert_eq!(taille_pour_viewport((5118, 1438), (1428, 1032)), (1428, 538));
}

/// La borne est la ZONE DE TRAVAIL, et c'est ce qui sort la barre des tâches
/// du recadrage. Mesure du 31 août 2026 : `mon=1428x1080`, `work=1428x1032`,
/// `Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080)` — 48 rangées.
#[test]
fn la_zone_de_travail_retire_les_quarante_huit_rangees_de_la_barre() {
    assert_eq!(borne_de_la_sortie((1428, 1080), Some((1428, 1032))), (1428, 1032));
    // Et le recadrage d'une fenêtre plein cadre les perd donc aussi : c'est le
    // même 1032, et non 1080, qui part à `region_de_sortie`.
    assert_eq!(taille_pour_viewport((1428, 1080), (1428, 1032)), (1428, 1032));
}

/// 🔴 **LE REPLI EST LE COMPORTEMENT D'AVANT LE LOT, ET IL DOIT L'ÊTRE
/// EXACTEMENT.** `GetMonitorInfoW` peut refuser ; une zone de travail
/// dégénérée (Windows en rend une le temps d'une transition) doit être
/// refusée de la même façon. Dans les deux cas la borne redevient le
/// rectangle du moniteur — donc le cadrage d'hier, barre des tâches comprise,
/// plutôt qu'une fenêtre de deux pixels.
#[test]
fn une_zone_de_travail_absente_ou_degeneree_rend_le_rectangle_du_moniteur() {
    assert_eq!(borne_de_la_sortie((1428, 1080), None), (1428, 1080));
    assert_eq!(borne_de_la_sortie((1428, 1080), Some((0, 0))), (1428, 1080));
    assert_eq!(borne_de_la_sortie((1428, 1080), Some((1428, 1))), (1428, 1080));
}

/// Une zone de travail que Windows annoncerait PLUS GRANDE que son moniteur
/// ne doit pas faire sortir la région de la texture : le `min` est un filet,
/// et rien d'autre ne le tient.
#[test]
fn une_zone_de_travail_plus_grande_que_le_moniteur_est_ramenee_a_lui() {
    assert_eq!(borne_de_la_sortie((1428, 1080), Some((4096, 4096))), (1428, 1080));
}

/// Le court-circuit du capteur compare la valeur rendue à la taille
/// courante : elle doit donc être **stable**, sinon chaque tour
/// reconstruirait l'encodeur. Un point fixe, éprouvé.
#[test]
fn la_regle_est_stable_sur_son_propre_resultat() {
    let sortie = (1860, 1080);
    let une = taille_pour_viewport((1723, 1303), sortie);
    assert_eq!(taille_pour_viewport(une, sortie), une);
}

/// Une boîte vidéo repliée émet `(0, 0)` (cas réel relevé en D8) : la
/// règle ne doit jamais rendre une dimension nulle, que l'encodeur NV12
/// refuserait.
#[test]
fn une_boite_video_repliee_ne_rend_jamais_une_dimension_nulle() {
    assert_eq!(taille_pour_viewport((0, 0), (1860, 1080)), (2, 2));
}
