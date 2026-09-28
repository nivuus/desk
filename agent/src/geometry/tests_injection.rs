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
fn the_image_middle_targets_the_middle_of_what_is_shown() {
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
        to_virtual_desktop_visible(32768, 32768, window, desktop).expect("the window is visible");

    // The middle of the shown image is screen pixel 540, that is 32768 once
    // normalised on the desktop. Mapping onto the complete client area
    // would give 589 px, that is 35742 — the gap the user saw.
    assert!((y - 32768).abs() <= 40, "y = {y}, expected ~32768");
}

#[test]
fn the_image_bottom_targets_the_bottom_of_what_is_shown() {
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
        to_virtual_desktop_visible(0, 65535, window, desktop).expect("the window is visible");

    assert_eq!(
        y, 65535,
        "the bottom of the image must target the bottom of the desktop"
    );
}

#[test]
fn the_horizontal_axis_stays_intact_when_only_the_bottom_overflows() {
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

    let (with, _) = to_virtual_desktop_visible(32768, 0, window, desktop).unwrap();
    let (sans, _) = to_virtual_desktop(32768, 0, window, desktop);
    assert_eq!(with, sans);
}

#[test]
fn a_fully_visible_window_is_mapped_identically() {
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
fn an_off_screen_window_produces_no_coordinate() {
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
fn bounds_a_height_that_would_exceed_the_desktop_bottom() {
    // The real case: 1187 requested from a viewport taller than the
    // VM's desktop.
    assert_eq!(
        borner_au_bureau(62, 0, 1550, 1187, 2400, 1080),
        (1550, 1080)
    );
}

#[test]
fn accounts_for_the_window_origin() {
    // Window already moved down by 100 px: only 980 remain for it.
    assert_eq!(borner_au_bureau(0, 100, 800, 1187, 2400, 1080), (800, 980));
}

#[test]
fn leaves_a_size_that_already_fits_untouched() {
    assert_eq!(borner_au_bureau(62, 0, 1550, 900, 2400, 1080), (1550, 900));
}

#[test]
fn also_bounds_the_width() {
    assert_eq!(borner_au_bureau(2000, 0, 800, 500, 2400, 1080), (400, 500));
}

#[test]
fn a_negative_origin_does_not_produce_an_absurd_size() {
    // Window whose top-left corner is off screen: the clamping must
    // neither overflow, nor return a zero size that would make the capture fail.
    let (w, h) = borner_au_bureau(-500, -300, 800, 600, 2400, 1080);
    assert!(w > 0 && h > 0, "size = {w}x{h}");
    assert!(w <= 2400 && h <= 1080, "size = {w}x{h}");
}

#[test]
fn top_left_corner_of_a_window_at_the_origin() {
    let window = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    assert_eq!(to_virtual_desktop(0, 0, window, DESKTOP), (0, 0));
}

#[test]
fn bottom_right_corner_of_a_fullscreen_window() {
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
fn centre_of_an_offset_window() {
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
fn origin_of_an_offset_window() {
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
fn bounds_overflows_on_a_multi_screen_desktop() {
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
fn never_divides_by_zero() {
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
fn neither_overflows_nor_panics_on_extreme_coordinates() {
    // Same spirit as `handles_a_width_above_i32_max` for
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
