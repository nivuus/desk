//! Consistency between the captured region and the injection region, and
//! clamping of resizes.

use super::*;

const DESKTOP: Rect = Rect {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};

// --- Consistency between the captured region and the injection region ---
//
// Real case noted on 29/07/2026: client viewport 1187 px high,
// desktop of 1080. The window's client area measured 1550×1178 at
// origin (62, 0), hence 98 px below the screen. The capture encoded
// 1550×1080 (the intersection) while the injection mapped onto 1178:
// the click drifted by `t × 98` px, nil at the top, growing towards the bottom.

#[test]
fn le_milieu_de_l_image_vise_le_milieu_de_ce_qui_est_montre() {
    // Window sticking out 98 px below a 1080 desktop.
    let window = Rect {
        x: 62,
        y: 0,
        width: 1550,
        height: 1178,
    };
    let desktop = Rect {
        x: 0,
        y: 0,
        width: 2400,
        height: 1080,
    };

    let (_, y) =
        to_virtual_desktop_visible(32768, 32768, window, desktop).expect("la fenêtre est visible");

    // The middle of the shown image is screen pixel 540, that is 32768 once
    // normalised on the desktop. Mapping onto the complete client area
    // would give 589 px, that is 35742 — the gap the user saw.
    assert!((y - 32768).abs() <= 40, "y = {y}, attendu ~32768");
}

#[test]
fn le_bas_de_l_image_vise_le_bas_de_ce_qui_est_montre() {
    let window = Rect {
        x: 62,
        y: 0,
        width: 1550,
        height: 1178,
    };
    let desktop = Rect {
        x: 0,
        y: 0,
        width: 2400,
        height: 1080,
    };

    let (_, y) =
        to_virtual_desktop_visible(0, 65535, window, desktop).expect("la fenêtre est visible");

    assert_eq!(y, 65535, "le bas de l'image doit viser le bas du bureau");
}

#[test]
fn l_axe_horizontal_reste_intact_quand_seul_le_bas_deborde() {
    // The width does not stick out: the horizontal mapping must not move.
    let window = Rect {
        x: 62,
        y: 0,
        width: 1550,
        height: 1178,
    };
    let desktop = Rect {
        x: 0,
        y: 0,
        width: 2400,
        height: 1080,
    };

    let (avec, _) = to_virtual_desktop_visible(32768, 0, window, desktop).unwrap();
    let (sans, _) = to_virtual_desktop(32768, 0, window, desktop);
    assert_eq!(avec, sans);
}

#[test]
fn une_fenetre_entierement_visible_est_mappee_a_l_identique() {
    // Without overflow, the fix must change nothing: that is what
    // made the defect invisible until now.
    let window = Rect {
        x: 100,
        y: 50,
        width: 800,
        height: 600,
    };
    assert_eq!(
        to_virtual_desktop_visible(12345, 54321, window, DESKTOP),
        Some(to_virtual_desktop(12345, 54321, window, DESKTOP))
    );
}

#[test]
fn une_fenetre_hors_ecran_ne_produit_aucune_coordonnee() {
    let window = Rect {
        x: 5000,
        y: 0,
        width: 400,
        height: 300,
    };
    assert_eq!(to_virtual_desktop_visible(0, 0, window, DESKTOP), None);
}

// --- Bornage du redimensionnement ---

#[test]
fn borne_une_hauteur_qui_depasserait_le_bas_du_bureau() {
    // The real case: 1187 requested from a viewport taller than the
    // VM's desktop.
    assert_eq!(
        borner_au_bureau(62, 0, 1550, 1187, 2400, 1080),
        (1550, 1080)
    );
}

#[test]
fn tient_compte_de_l_origine_de_la_fenetre() {
    // Window already moved down by 100 px: only 980 remain for it.
    assert_eq!(borner_au_bureau(0, 100, 800, 1187, 2400, 1080), (800, 980));
}

#[test]
fn ne_touche_pas_a_une_taille_qui_tient_deja() {
    assert_eq!(borner_au_bureau(62, 0, 1550, 900, 2400, 1080), (1550, 900));
}

#[test]
fn borne_aussi_la_largeur() {
    assert_eq!(borner_au_bureau(2000, 0, 800, 500, 2400, 1080), (400, 500));
}

#[test]
fn une_origine_negative_ne_produit_pas_une_taille_absurde() {
    // Window whose top-left corner is off screen: the clamping must
    // neither overflow, nor return a zero size that would make the capture fail.
    let (w, h) = borner_au_bureau(-500, -300, 800, 600, 2400, 1080);
    assert!(w > 0 && h > 0, "taille = {w}x{h}");
    assert!(w <= 2400 && h <= 1080, "taille = {w}x{h}");
}

#[test]
fn coin_superieur_gauche_d_une_fenetre_a_l_origine() {
    let window = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    assert_eq!(to_virtual_desktop(0, 0, window, DESKTOP), (0, 0));
}

#[test]
fn coin_inferieur_droit_d_une_fenetre_plein_ecran() {
    let window = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    assert_eq!(
        to_virtual_desktop(65535, 65535, window, DESKTOP),
        (65535, 65535)
    );
}

#[test]
fn centre_d_une_fenetre_decalee() {
    // 960×540 window placed in the centre: its centre is the screen's.
    let window = Rect {
        x: 480,
        y: 270,
        width: 960,
        height: 540,
    };
    let (x, y) = to_virtual_desktop(32768, 32768, window, DESKTOP);
    assert!((x - 32768).abs() <= 40, "x = {x}");
    assert!((y - 32768).abs() <= 40, "y = {y}");
}

#[test]
fn origine_d_une_fenetre_decalee() {
    let window = Rect {
        x: 960,
        y: 540,
        width: 960,
        height: 540,
    };
    let (x, y) = to_virtual_desktop(0, 0, window, DESKTOP);
    assert_eq!((x, y), (32768, 32768));
}

#[test]
fn borne_les_debordements_sur_un_bureau_multi_ecrans() {
    // Virtual desktop starting at negative coordinates (screen on the left).
    let desktop = Rect {
        x: -1920,
        y: 0,
        width: 3840,
        height: 1080,
    };
    let window = Rect {
        x: -1920,
        y: 0,
        width: 1920,
        height: 1080,
    };
    assert_eq!(to_virtual_desktop(0, 0, window, desktop), (0, 0));
    let (x, _) = to_virtual_desktop(65535, 0, window, desktop);
    assert!((x - 32768).abs() <= 40, "x = {x}");
}

#[test]
fn ne_divise_jamais_par_zero() {
    let degenerate = Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    let (x, y) = to_virtual_desktop(32768, 32768, degenerate, degenerate);
    assert!((0..=65535).contains(&x) && (0..=65535).contains(&y));
}

#[test]
fn ne_deborde_ni_ne_panique_sur_des_coordonnees_extremes() {
    // Same spirit as `gere_une_largeur_superieure_a_i32_max` for
    // `crop_region`: a window with extreme coordinates or dimensions
    // (never produced by `client_rect_on_screen` in practice, but not
    // structurally impossible) must neither panic, nor leave
    // `0..=65535`.
    let window = Rect {
        x: i32::MAX - 10,
        y: i32::MIN + 10,
        width: u32::MAX,
        height: u32::MAX,
    };
    let desktop = Rect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let (x, y) = to_virtual_desktop(65535, 0, window, desktop);
    assert!((0..=65535).contains(&x), "x = {x}");
    assert!((0..=65535).contains(&y), "y = {y}");

    // Desktop itself degenerate to the extreme, combined with a normal window.
    let window = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let desktop = Rect {
        x: i32::MIN + 10,
        y: i32::MAX - 10,
        width: u32::MAX,
        height: 0,
    };
    let (x, y) = to_virtual_desktop(0, 65535, window, desktop);
    assert!((0..=65535).contains(&x), "x = {x}");
    assert!((0..=65535).contains(&y), "y = {y}");
}
