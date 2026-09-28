use super::*;

#[test]
fn fenetre_entierement_visible() {
    let w = Rect {
        x: 100,
        y: 50,
        width: 800,
        height: 600,
    };
    assert_eq!(crop_region(w, 1920, 1080), Some(w));
}

#[test]
fn aligne_les_dimensions_impaires() {
    let w = Rect {
        x: 0,
        y: 0,
        width: 801,
        height: 601,
    };
    let r = crop_region(w, 1920, 1080).unwrap();
    assert_eq!((r.width, r.height), (800, 600));
}

#[test]
fn borne_une_fenetre_qui_deborde_a_droite() {
    let w = Rect {
        x: 1800,
        y: 0,
        width: 400,
        height: 400,
    };
    let r = crop_region(w, 1920, 1080).unwrap();
    assert_eq!((r.x, r.width), (1800, 120));
}

#[test]
fn borne_une_fenetre_a_coordonnees_negatives() {
    let w = Rect {
        x: -100,
        y: -50,
        width: 400,
        height: 300,
    };
    let r = crop_region(w, 1920, 1080).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 300, 250));
}

#[test]
fn rejette_une_fenetre_hors_ecran() {
    let w = Rect {
        x: 5000,
        y: 0,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
    let w = Rect {
        x: 0,
        y: -5000,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn rejette_une_intersection_trop_petite() {
    let w = Rect {
        x: 1919,
        y: 0,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn fenetre_plus_grande_que_l_ecran_dans_les_deux_dimensions() {
    let w = Rect {
        x: -500,
        y: -300,
        width: 5000,
        height: 4000,
    };
    let r = crop_region(w, 1920, 1080).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 1920, 1080));
}

#[test]
fn fenetre_exactement_a_la_limite_de_l_ecran() {
    // The window's bottom-right corner touches exactly the edge of the
    // screen (1920, 1080): non-empty intersection, nothing to clamp.
    let w = Rect {
        x: 1520,
        y: 780,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), Some(w));

    // One pixel further: the window starts exactly where the screen
    // stops, empty intersection.
    let w = Rect {
        x: 1920,
        y: 0,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn rejette_une_largeur_ou_une_hauteur_nulle() {
    let w = Rect {
        x: 100,
        y: 100,
        width: 0,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
    let w = Rect {
        x: 100,
        y: 100,
        width: 300,
        height: 0,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn ne_deborde_pas_sur_des_coordonnees_extremes() {
    // Exact reproduction of the panic reported in review: the addition
    // `window.x + window.width` in i32 overflowed for x close to
    // i32::MAX. Must now be rejected cleanly, not panic.
    let w = Rect {
        x: i32::MAX - 10,
        y: 0,
        width: 1000,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);

    // Symmetric on the bottom/right side for y.
    let w = Rect {
        x: 0,
        y: i32::MAX - 10,
        width: 300,
        height: 1000,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);

    // Minimal coordinates: very negative x, must not overflow
    // either. The window stays entirely off screen (its right edge,
    // x + width, is still very negative), hence rejected — but
    // cleanly, without panic.
    let w = Rect {
        x: i32::MIN + 10,
        y: i32::MIN + 10,
        width: 1000,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn gere_une_largeur_superieure_a_i32_max() {
    // Reported in review as already correct — checked explicitly rather
    // than assumed: a width beyond i32::MAX must neither panic,
    // nor end up truncated into a negative value by an `as i32`.
    let w = Rect {
        x: 0,
        y: 0,
        width: u32::MAX,
        height: 300,
    };
    let r = crop_region(w, 1920, 1080).unwrap();
    assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 1920, 300));
}

#[test]
fn rects_overlap_detecte_une_intersection() {
    let a = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let b = Rect {
        x: 50,
        y: 50,
        width: 100,
        height: 100,
    };
    assert!(rects_overlap(a, b));
}

#[test]
fn rects_overlap_rejette_des_rectangles_disjoints() {
    let a = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let b = Rect {
        x: 200,
        y: 200,
        width: 100,
        height: 100,
    };
    assert!(!rects_overlap(a, b));
}

#[test]
fn rects_overlap_rejette_des_rectangles_qui_se_touchent_sans_se_recouvrir() {
    let a = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let b = Rect {
        x: 100,
        y: 0,
        width: 100,
        height: 100,
    };
    assert!(!rects_overlap(a, b));
}
