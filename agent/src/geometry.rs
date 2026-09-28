//! Shared geometric computations, independent of any system API.

/// Rectangle in screen coordinates, unsigned dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Intersects a window's rectangle with the screen and aligns the dimensions
/// on even values.
///
/// Even alignment is not cosmetic: the H.264 encoder works in
/// macroblocks and refuses odd dimensions in 4:2:0. Returns `None` if the
/// window is entirely off screen or if the intersection is too small
/// to be encoded.
///
/// All the arithmetic is done in `i64`: `window.x` (`i32`) and
/// `window.width`/`window.height` (`u32`) both fit in an
/// `i64` without loss, and so does their sum (at worst `i32::MAX + u32::MAX`,
/// very far from `i64::MAX`). An equivalent computation in `i32` would overflow as soon
/// as `window.x` is close to `i32::MAX` — reachable in practice on the
/// mouse side (task 12), where the coordinates come from browser events and
/// are not guaranteed reasonable as those from
/// `client_rect_on_screen` are.
pub fn crop_region(window: Rect, desktop_width: u32, desktop_height: u32) -> Option<Rect> {
    let desktop_width = desktop_width as i64;
    let desktop_height = desktop_height as i64;

    let window_left = window.x as i64;
    let window_top = window.y as i64;
    let window_right = window_left + window.width as i64;
    let window_bottom = window_top + window.height as i64;

    let left = window_left.max(0);
    let top = window_top.max(0);
    let right = window_right.min(desktop_width);
    let bottom = window_bottom.min(desktop_height);

    if right <= left || bottom <= top {
        return None;
    }

    let width = ((right - left) as u32) & !1;
    let height = ((bottom - top) as u32) & !1;
    if width < 2 || height < 2 {
        return None;
    }

    // `left`/`top` are bounded by `desktop_width`/`desktop_height` (through the
    // `.min()` above): for any realistic screen resolution (far
    // below `i32::MAX`), they fit in `i32` without truncation.
    Some(Rect {
        x: left as i32,
        y: top as i32,
        width,
        height,
    })
}

/// Converts a coordinate normalised on the window into a coordinate normalised
/// on the virtual desktop, the only form accepted by `SendInput` in absolute mode
/// (task 12).
///
/// `x`/`y` are in `0..=65535` relative to the client area of `window`.
/// The result is in `0..=65535` relative to `desktop`. The computation
/// composes two steps: normalised coordinate → screen pixel (through
/// `window`), then screen pixel → coordinate normalised on the desktop (through
/// `desktop`).
///
/// Arithmetic in `f64` rather than integer, on purpose: `window.x`/
/// `desktop.x` (`i32`) and `window.width`/`desktop.width` (`u32`) all fit
/// exactly in the 52-bit mantissa of an `f64` (the largest, any
/// `u32`, fits in 32 bits), so no loss of precision — and
/// unlike a multiplication in `i32`/`i64`, an `f64` value never
/// panics on overflow: at worst it saturates towards infinity, and the
/// final `as i32` conversion of an out-of-range float saturates too
/// (behaviour guaranteed by Rust since 1.45) rather than producing an
/// undefined result. The `.clamp(0.0, 65535.0)` before conversion therefore covers
/// both representable overflows and cases already within bounds.
pub fn to_virtual_desktop(x: u16, y: u16, window: Rect, desktop: Rect) -> (i32, i32) {
    // Position in screen pixels, at the centre of the targeted pixel.
    let screen_x = window.x as f64 + (x as f64 / 65535.0) * window.width as f64;
    let screen_y = window.y as f64 + (y as f64 / 65535.0) * window.height as f64;

    // `.max(1.0)` avoids any division by zero for a degenerate desktop
    // (zero width or height) without having to handle that case separately.
    let width = (desktop.width as f64).max(1.0);
    let height = (desktop.height as f64).max(1.0);
    let normalized_x = ((screen_x - desktop.x as f64) / width * 65535.0).round();
    let normalized_y = ((screen_y - desktop.y as f64) / height * 65535.0).round();

    (
        clamp_normalized(normalized_x),
        clamp_normalized(normalized_y),
    )
}

/// Clamps a normalised coordinate into `0..=65535`, including for a
/// float already out of range of an `i32` (see the note of
/// `to_virtual_desktop` on the saturation of `as` conversions).
fn clamp_normalized(value: f64) -> i32 {
    value.clamp(0.0, 65535.0) as i32
}

/// Like [`to_virtual_desktop`], but mapping onto the region **actually
/// shown to the client** rather than onto the complete client area.
///
/// The distinction is not theoretical. The capture encodes
/// `crop_region(window, …)`, that is the intersection of the window with
/// the screen; the browser therefore normalises its coordinates on this
/// intersection. Mapping the injection onto the whole client area makes the
/// pointer drift by everything that sticks out: found on 29/07/2026 with a client
/// area of 1178 px for a desktop of 1080, that is 98 px of error at the bottom of
/// the image and zero at the top.
///
/// Both functions must therefore keep being called with the same region. Returns
/// `None` when the window is entirely off screen — there is then
/// no image, hence no coordinate to convert.
pub fn to_virtual_desktop_visible(
    x: u16,
    y: u16,
    window: Rect,
    desktop: Rect,
) -> Option<(i32, i32)> {
    let visible = crop_region(window, desktop.width, desktop.height)?;
    Some(to_virtual_desktop(x, y, visible, desktop))
}

/// Clamps a requested size so that the window, whose top-left corner does not
/// move (`SWP_NOMOVE`), fits entirely within the desktop.
///
/// Without this clamping, a client viewport taller than the VM's desktop
/// produces a window that sticks out: the capture trims it, the image takes an
/// aspect ratio the browser's container does not have — hence black
/// bars — and the lower part of the application becomes unreachable.
///
/// The 2 px floor is not cosmetic: `crop_region` refuses any
/// smaller region, and a zero size would make the capture fail.
pub fn borner_au_bureau(
    origin_x: i32,
    origin_y: i32,
    width: u32,
    height: u32,
    desktop_width: u32,
    desktop_height: u32,
) -> (u32, u32) {
    // A negative origin leaves on the contrary MORE room downwards and to the
    // right: `max(0)` avoids concluding a negative size from it, `saturating_sub`
    // avoids overflowing for an origin beyond the desktop.
    let disponible_x = (desktop_width as i64 - origin_x.max(0) as i64).max(2) as u32;
    let disponible_y = (desktop_height as i64 - origin_y.max(0) as i64).max(2) as u32;
    (width.min(disponible_x), height.min(disponible_y))
}

/// True if the two rectangles have a non-empty intersection (touching
/// boundaries excluded).
///
/// Serves the `CAPTURE_TEST` diagnostic mode to check that a control
/// region placed opposite on the desktop does not overlap, even
/// partially, the window actually captured: without this guarantee, a
/// proof by pixel comparison would be invalidated by construction (the
/// two areas could legitimately show the same thing).
pub fn rects_overlap(a: Rect, b: Rect) -> bool {
    let a_left = a.x as i64;
    let a_top = a.y as i64;
    let a_right = a_left + a.width as i64;
    let a_bottom = a_top + a.height as i64;

    let b_left = b.x as i64;
    let b_top = b.y as i64;
    let b_right = b_left + b.width as i64;
    let b_bottom = b_top + b.height as i64;

    a_left < b_right && b_left < a_right && a_top < b_bottom && b_top < a_bottom
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_injection;
