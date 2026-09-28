use super::*;

#[test]
fn fully_visible_window() {
    let w = Rect {
        x: 100,
        y: 50,
        width: 800,
        height: 600,
    };
    assert_eq!(crop_region(w, 1920, 1080), Some(w));
}

#[test]
fn aligns_odd_dimensions() {
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
fn bounds_a_window_overflowing_on_the_right() {
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
fn bounds_a_window_with_negative_coordinates() {
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
fn rejects_an_off_screen_window() {
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
fn rejects_a_too_small_intersection() {
    let w = Rect {
        x: 1919,
        y: 0,
        width: 400,
        height: 300,
    };
    assert_eq!(crop_region(w, 1920, 1080), None);
}

#[test]
fn window_larger_than_the_screen_in_both_dimensions() {
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
fn window_exactly_at_the_screen_edge() {
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
fn rejects_a_zero_width_or_height() {
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
fn does_not_overflow_on_extreme_coordinates() {
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
fn handles_a_width_above_i32_max() {
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
fn rects_overlap_detects_an_intersection() {
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
fn rects_overlap_rejects_disjoint_rectangles() {
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
fn rects_overlap_rejects_rectangles_that_touch_without_overlapping() {
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
