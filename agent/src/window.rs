//! Locating and driving the window to capture.

#![cfg(windows)]

use anyhow::{anyhow, bail, Context, Result};
// windows-rs 0.62 API deviation: `BOOL` was moved into `windows::core`
// (it is no longer re-exported under `Win32::Foundation`), unlike `TRUE`
// which stays accessible there.
use windows::core::BOOL;
// windows-rs 0.62 API deviation: `ClientToScreen` lives in `Win32::Graphics::Gdi`
// (gdi32 module), not in `WindowsAndMessaging` (user32) where one would expect it
// by analogy with `GetClientRect`. It moreover returns a raw `BOOL`
// (historical gdi32 convention), not a `windows::core::Result<()>` like
// the success/failure-annotated user32 functions of the same file.
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, IsWindow, IsWindowVisible, SetWindowPos, SM_CXBORDER, SM_CYBORDER, SWP_NOMOVE,
    SWP_NOZORDER,
};

use crate::geometry::Rect;

struct SearchContext {
    fragment: String,
    found: Option<HWND>,
}

/// Finds the first visible window whose title contains `fragment`.
///
/// The comparison is case-insensitive: browser titles
/// change with the page displayed, we cannot require an exact title.
pub fn find_window_by_title(fragment: &str) -> Result<HWND> {
    let mut context = SearchContext {
        fragment: fragment.to_lowercase(),
        found: None,
    };

    unsafe {
        // EnumWindows returns an error if the callback interrupts the enumeration,
        // which is precisely what we do on success.
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut context as *mut SearchContext as isize),
        );
    }

    context
        .found
        .ok_or_else(|| anyhow!("aucune fenêtre visible dont le titre contient « {fragment} »"))
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SearchContext);

    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let length = GetWindowTextLengthW(hwnd);
    if length <= 0 {
        return TRUE;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let written = GetWindowTextW(hwnd, &mut buffer);
    if written <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buffer[..written as usize]).to_lowercase();

    if title.contains(&context.fragment) {
        context.found = Some(hwnd);
        return BOOL(0); // interrupts the enumeration
    }
    TRUE
}

/// Client area of the window, converted to screen coordinates.
///
/// Needed for cropping: Desktop Duplication returns an image of the whole
/// screen, expressed in screen coordinates, whereas `GetClientRect` returns
/// a rectangle in client coordinates (origin always at (0, 0)).
pub fn client_rect_on_screen(hwnd: HWND) -> Result<Rect> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect)? };
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width == 0 || height == 0 {
        bail!("la fenêtre a une zone client vide");
    }

    // The client area origin (0, 0) is enough: GetClientRect guarantees that
    // `left`/`top` are always 0, so this point represents the top-left
    // corner of the client area in its own frame of reference.
    let mut origin = POINT { x: 0, y: 0 };
    unsafe { ClientToScreen(hwnd, &mut origin) }
        .ok()
        .context("ClientToScreen")?;

    Ok(Rect {
        x: origin.x,
        y: origin.y,
        width,
        height,
    })
}

/// The window's rectangle as `GetWindowRect` returns it — **DWM's invisible
/// borders INCLUDED**.
///
/// ⚠️ **It is NOT the rectangle one sees**: see
/// `superviseur::placement::Lisere`. This function is the raw one; the visible
/// frame is [`cadre_visible`].
pub fn rectangle_brut(hwnd: HWND) -> Result<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) }.context("GetWindowRect")?;
    Ok(depuis_rect(r))
}

/// The window's **VISIBLE** frame, through `DWMWA_EXTENDED_FRAME_BOUNDS`.
///
/// 🔴 **IT IS THE ONLY RECTANGLE THAT MATCHES WHAT THE EYE SEES.** Since
/// Windows 10 the resize borders are transparent and
/// `GetWindowRect` includes them: measured in session 1 on August 31st, 2026, a
/// served window returned `1732x1032+1280+0` raw and `1718x1025+1287+0`
/// as visible frame — 7 px on the left, right and bottom, 0 at the top.
///
/// `Err` when DWM refuses (composition disabled, window destroyed):
/// the caller then falls back to the raw one, that is, to the behaviour
/// from before this fix.
pub fn cadre_visible(hwnd: HWND) -> Result<Rect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .context("DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)")?;
    Ok(depuis_rect(r))
}

/// The thickness of the border Windows **PAINTS** around a window, in
/// pixels, `(x, y)`.
///
/// 🔴 **MEASURED, NOT HARDCODED**: `SM_CXBORDER` / `SM_CYBORDER` are
/// documented metrics that **follow DPI**. Noted at `(1, 1)` for a
/// system DPI of `96` on this machine — and it is exactly the thickness of
/// the dark line read on the four edges of the crop (see
/// `superviseur::placement::enveloppe`, which carries the colours noted).
///
/// ⚠️ **They do not distinguish a window WITHOUT a painted border** (frameless
/// fullscreen): we would then remove 1 px of real content. Known cost,
/// named, and judged smaller than a permanent dark line on the four edges.
pub fn bordure_peinte() -> (i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_CXBORDER).max(0),
            GetSystemMetrics(SM_CYBORDER).max(0),
        )
    }
}

/// The invisible edge of THIS window: `GetWindowRect` minus the visible
/// frame, side by side.
///
/// ⚠️ **To reread AFTER `ShowWindow`**: on a minimised window, DWM returns a
/// frame that means nothing (rectangle at `-32000`).
pub fn lisere_dwm(hwnd: HWND) -> Result<crate::superviseur::placement::Lisere> {
    let brut = rectangle_brut(hwnd)?;
    let vu = cadre_visible(hwnd)?;
    Ok(crate::superviseur::placement::Lisere {
        gauche: vu.x - brut.x,
        haut: vu.y - brut.y,
        droite: (brut.x + brut.width as i32) - (vu.x + vu.width as i32),
        bas: (brut.y + brut.height as i32) - (vu.y + vu.height as i32),
    })
}

/// The monitor's rectangle and its **work area**, for the monitor that
/// contains a point of the virtual desktop.
///
/// 🔴 **FIRST READER OF `rcWork` IN THE WHOLE REPOSITORY** (batch 33): before it,
/// a case-insensitive grep for `rcWork`, `SPI_GETWORKAREA` or a work-area name over `agent/`,
/// `client/`, `plateforme/` and `proto/` returned **zero**, and the
/// monitor / work area distinction therefore existed nowhere in this product.
/// It is what put the 48 rows of the secondary taskbar into
/// the crop of every served window — see
/// `windows_source_sortie::borne_de_la_sortie`, which carries the measurement.
///
/// **Deux points d'entrée, un seul corps, et c'est délibéré** : le SUPERVISEUR
/// interroge par l'ORIGINE de la sortie (il connaît le rectangle DXGI avant
/// même d'avoir posé la fenêtre), le CAPTEUR par SA FENÊTRE (il ne connaît que
/// le nom de la sortie, mais sa fenêtre est posée dessus). Les deux tombent
/// sur le même `HMONITOR`, donc sur la même réponse — c'est ce qui permet aux
/// deux processus de calculer la même borne sans échanger un message.
pub fn zones_du_moniteur_au_point(x: i32, y: i32) -> Result<(Rect, Rect)> {
    // `MONITOR_DEFAULTTONEAREST` : un point hors de tout moniteur — une sortie
    // que Windows vient de détacher — rendrait `NULL` avec
    // `MONITOR_DEFAULTTONULL`, et l'appelant retomberait sur le rectangle de
    // la sortie. Le plus proche est une réponse, pas une devinette : le point
    // vient de l'origine d'une sortie que DXGI énumère encore.
    let moniteur = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    zones_de_l_hmoniteur(moniteur)
}

/// Le rectangle du moniteur et sa zone de travail, pour le moniteur qui porte
/// une fenêtre. Voir [`zones_du_moniteur_au_point`] pour le pourquoi des deux
/// points d'entrée.
pub fn zones_du_moniteur_de(hwnd: HWND) -> Result<(Rect, Rect)> {
    let moniteur = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    zones_de_l_hmoniteur(moniteur)
}

fn zones_de_l_hmoniteur(moniteur: HMONITOR) -> Result<(Rect, Rect)> {
    if moniteur.is_invalid() {
        bail!("aucun moniteur pour ce repère");
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // `GetMonitorInfoW` rend un `BOOL` brut : un `false` n'est pas une
    // `windows::core::Error`, et un `?` ne l'attraperait pas.
    if !unsafe { GetMonitorInfoW(moniteur, &mut info) }.as_bool() {
        bail!("GetMonitorInfoW a refusé");
    }
    Ok((depuis_rect(info.rcMonitor), depuis_rect(info.rcWork)))
}

fn depuis_rect(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        width: (r.right - r.left).max(0) as u32,
        height: (r.bottom - r.top).max(0) as u32,
    }
}

/// Redimensionne la fenêtre sans la déplacer ni changer son ordre d'affichage.
pub fn resize_window(hwnd: HWND, width: u32, height: u32) -> Result<()> {
    // Les dimensions nulles font échouer la capture ; on impose un plancher.
    let width = width.max(160) as i32;
    let height = height.max(120) as i32;
    unsafe { SetWindowPos(hwnd, None, 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER)? };
    Ok(())
}

/// Vrai tant que la fenêtre existe.
pub fn is_window_alive(hwnd: HWND) -> bool {
    unsafe { IsWindow(Some(hwnd)).as_bool() }
}
