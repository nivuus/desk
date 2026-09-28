//! Reading the icon of a window, by `hwnd` — **the Windows half of A1**.
//!
//! 🔴 **NO DECISION LIVES HERE.** This module returns RGBA bytes and a
//! geometry; the thresholds, the filters and the output format belong to
//! `accent.rs`, which is **pure** and tested on the host. It is the same separation
//! as `presse_papier/win32.rs` (the two Win32 calls, no decision).
//!
//! 🔴 **NO HOST TEST COVERS THIS FILE**, and it is declared rather than
//! kept quiet: it is `#[cfg(windows)]`, and `cargo check --target
//! x86_64-pc-windows-gnu` checks **types, borrows, visibility and
//! lifetimes — never behaviour**. Its only proof of working is the
//! acceptance run. It is the pattern of `apps/icone/extraction.rs`, which declares it
//! itself.
//!
//! **No new Cargo dependency or feature**: `Win32_UI_WindowsAndMessaging`
//! (`agent/Cargo.toml:157`) carries `SendMessageTimeoutW`, `WM_GETICON`,
//! `GetClassLongPtrW` and `GetIconInfo`; `Win32_Graphics_Gdi` (`:60`) carries
//! `GetDIBits`, `GetObjectW` and `DeleteObject`. **Re-read, not assumed.**

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassLongPtrW, GetIconInfo, SendMessageTimeoutW, GCLP_HICON, GCLP_HICONSM, HICON, ICONINFO,
    ICON_BIG, ICON_SMALL, ICON_SMALL2, SMTO_ABORTIFHUNG,
};

/// The timeout given to `WM_GETICON`, in milliseconds.
///
/// 🔴 **`WM_GETICON` is a SYNCHRONOUS `SendMessage` TO ANOTHER
/// APPLICATION** (R4 of the spec, and it is not theoretical): a frozen application
/// does not answer, and the call **would block indefinitely the thread that
/// sent it**. Hence `SMTO_ABORTIFHUNG` **and** this timeout.
///
/// ⚠️ **The severity is lower than the spec feared, and it does not
/// go away.** Under D-A1-1 the read lives on the **window thread**, not
/// on the wheel tick: a block freezes **only this window**. But this thread
/// is the one that produces its frames — the session would freeze, exactly as
/// the ProjFS bridge froze for nine minutes in F1.
///
/// ⚠️ **NOT CALIBRATED**, and **NEVER EXERCISED**: `SMTO_ABORTIFHUNG` is **set**,
/// no frozen application is provoked by the A1 acceptance run.
const DELAI_MS: u32 = 200;

/// Returns the icon of `hwnd` in **RGBA**, with its width and height.
///
/// The cascade, in decreasing order of size (D-A1-8):
/// 1. `WM_GETICON` with `ICON_BIG`, then `ICON_SMALL2`, then `ICON_SMALL`, all
///    three through `SendMessageTimeoutW` + `SMTO_ABORTIFHUNG`;
/// 2. **non-blocking** fallback `GetClassLongPtrW(GCLP_HICON)` then `GCLP_HICONSM`.
///
/// 🔴 **An exceeded timeout, a null `HICON`, or a failure of `GetIconInfo` /
/// `GetDIBits` are treated as "NO ICON": `None`. The tick produces
/// no announcement, and it produces no error either.** An icon that cannot
/// be read is not a failure of the product.
///
/// ⚠️ **The `GetClassLongPtrW` fallback only runs if `WM_GETICON` fails, which
/// no acceptance protocol provokes: code shipped, path probably
/// never taken** — as `borner_a_la_taille_max` was for a whole
/// sub-block.
pub fn lire_icone(hwnd: HWND) -> Option<(Vec<u8>, u32, u32)> {
    let icone = trouver_icone(hwnd)?;
    pixels_de(icone)
}

/// The cascade of the doc of [`lire_icone`], and nothing else.
fn trouver_icone(hwnd: HWND) -> Option<HICON> {
    for taille in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
        let mut resultat: usize = 0;
        // `SendMessageTimeoutW` returns 0 on timeout as on failure: both
        // are treated the same — "no icon by this route".
        let rendu = unsafe {
            SendMessageTimeoutW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::WM_GETICON,
                WPARAM(taille as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                DELAI_MS,
                Some(&mut resultat as *mut usize),
            )
        };
        if rendu.0 != 0 && resultat != 0 {
            return Some(HICON(resultat as *mut _));
        }
    }
    // NON-BLOCKING fallback: the window class, which is data local to the
    // calling process and asks nothing of the target application.
    for index in [GCLP_HICON, GCLP_HICONSM] {
        let brut = unsafe { GetClassLongPtrW(hwnd, index) };
        if brut != 0 {
            return Some(HICON(brut as *mut _));
        }
    }
    None
}

/// Decodes an `HICON` into RGBA.
///
/// 🔴 **`GetDIBits` RETURNS BGRA. THE CONVERSION HAPPENS HERE, AND NOWHERE
/// ELSE.** Getting the direction wrong would swap red and blue — a
/// **plausible and silent** defect, which **no host test would see** since it would
/// live behind this `#[cfg(windows)]` (RA1-6). The pure module receives RGBA,
/// and its test says so; **the only real check is criterion ① of the
/// acceptance run**, and it would only see it if the two chosen applications have
/// icons of opposite hues.
fn pixels_de(icone: HICON) -> Option<(Vec<u8>, u32, u32)> {
    let mut info = ICONINFO::default();
    if unsafe { GetIconInfo(icone, &mut info) }.is_err() {
        return None;
    }
    // 🔴 `DeleteObject` ON EVERY EXIT PATH, ERROR PATHS INCLUDED:
    // `GetIconInfo` creates TWO bitmaps the caller becomes the owner of, and
    // forgetting them is one leak per read tick — i.e. one leak every
    // `PERIODE_ACCENT`, for the whole life of the session. The pattern is
    // `apps/icone/extraction.rs`.
    let resultat = decoder(info.hbmColor);
    unsafe {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(info.hbmColor.into());
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(info.hbmMask.into());
        }
    }
    resultat
}

/// La lecture des octets d'un `HBITMAP` 32 bits, top-down.
///
/// **Split out so that the caller's `DeleteObject` runs on ALL
/// paths**, including those that return `None` — that is the reason for the
/// same split in `apps/icone/extraction.rs`.
fn decoder(bitmap: HBITMAP) -> Option<(Vec<u8>, u32, u32)> {
    if bitmap.is_invalid() {
        // A monochrome icon has no colour plane: it has no
        // hue to give, and that is not an error.
        return None;
    }
    let mut brut = BITMAP::default();
    let lu = unsafe {
        GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut brut as *mut BITMAP as *mut _),
        )
    };
    if lu == 0 || brut.bmWidth <= 0 || brut.bmHeight <= 0 {
        return None;
    }
    let (largeur, hauteur) = (brut.bmWidth as u32, brut.bmHeight as u32);
    let octets = (largeur as usize)
        .checked_mul(hauteur as usize)?
        .checked_mul(4)?;
    let mut tampon = vec![0u8; octets];

    let mut entete = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: largeur as i32,
            // Negative: TOP-DOWN bitmap. Without it the rows would come back
            // in reverse order — harmless for a dominant
            // colour, which does not depend on the order, but saying so keeps a
            // successor who reused this module to render an IMAGE
            // from inheriting an upside-down image. The model is
            // `diagnostics/multifenetre/voies.rs`.
            biHeight: -(hauteur as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let ecran = unsafe { GetDC(None) };
    let lignes = unsafe {
        GetDIBits(
            ecran,
            bitmap,
            0,
            hauteur,
            Some(tampon.as_mut_ptr() as *mut _),
            &mut entete,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, ecran) };
    if lignes == 0 {
        return None;
    }

    // 🔴 BGRA -> RGBA, BY THE PURE RULE AND NOT BY A COPY. This loop
    // lived here, behind the `#[cfg(windows)]`, and was covered by nothing
    // (legacy RA1-6). Sub-block G5 needed it a second time, for the
    // icon of an APPLICATION: it moved down into `accent.rs`, where it
    // is tested, rather than being copied — two copies of a rule that
    // nobody checks would diverge without anything saying so.
    crate::accent::bgra_en_rgba(&mut tampon);
    Some((tampon, largeur, hauteur))
}
