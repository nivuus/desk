//! Path 4: the per-window fallbacks, probed last because they are the
//! least promising.
//!
//! `PrintWindow(PW_RENDERFULLCONTENT)` goes through GDI, which makes it
//! generally fail (black image) on the D3D content of windows —
//! that is what this probe checks, on a test pattern painted in D3D11 and not in
//! GDI. **Measured on this VM: that is NOT the case** — the covered test pattern
//! is rendered correctly (`verdict=Juste`, exact pixel), probably
//! thanks to the flip presentation model of the test pattern's swapchain. It
//! nevertheless brings the pixels back to main memory: even the correct
//! image falls on the "GPU path" gate of spec §5. We probe it
//! to RECORD it, not in the hope of keeping it.
//!
//! `DwmGetDxSharedSurface` is not documented: it is resolved
//! dynamically in user32.dll, and its absence is a result, not an
//! error.

use anyhow::{Context, Result};
use windows::core::s;
// Gap from the plan: `PrintWindow` and `PRINT_WINDOW_FLAGS` are NOT in
// `Win32::UI::WindowsAndMessaging` under `windows` 0.62 (as the
// plan stated), but in `Win32::Storage::Xps` — the function is declared there under
// `#[cfg(feature = "Win32_Graphics_Gdi")]`, but the module that carries it is
// itself gated by the `Win32_Storage_Xps` feature, added to `Cargo.toml`
// for this task. Followed in source: `windows-0.62.2/src/Windows/Win32/
// Storage/Xps/mod.rs`.
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel, ReleaseDC,
    SelectObject,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;

/// Not exposed by `windows` 0.62: `PW_RENDERFULLCONTENT` is 2 (WinUser.h).
/// Only `PW_CLIENTONLY` (1) is exported by this crate at this version.
///
/// `pub(super)`: the bench's `printwindow` path (`voies.rs`) reuses this
/// same constant rather than redefining it a second time.
pub(super) const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

pub(super) fn eprouver() -> Result<()> {
    // A bench failure, not a verdict on the fallbacks: if the desktop cannot
    // be split into two slots or if the test pattern windows do not open,
    // no measurement is possible. As in `wgc.rs`, these failures
    // are therefore still propagated by `?`.
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect {
            x: 0,
            y: 0,
            width: largeur,
            height: hauteur,
        },
        2,
    )
    .context("deux places sur ce bureau")?;
    let mut mires = Mires::ouvrir(capture.device(), &places)?;
    mires.peindre()?;
    mires.pomper();
    mires.recouvrir(1, 0)?;
    mires.peindre()?;
    mires.pomper();

    eprouver_printwindow(&mires)?;
    eprouver_surface_dwm();
    Ok(())
}

/// From here on, any measurement failure becomes an ÉLIMINÉE verdict
/// logged rather than a propagated error — the pattern established at task 6
/// (`wgc.rs`): a reader of the log must always find one of the
/// "verdict …" messages below, never an error that silently goes up
/// to `main()`. Only the preparation of the bench itself
/// (already done in `eprouver`) is still propagated by `?`.
fn eprouver_printwindow(mires: &Mires) -> Result<()> {
    let hwnd = mires.hwnd(0)?;
    let place = mires.place(0)?;
    let ecran = unsafe { GetDC(None) };
    let memoire = unsafe { CreateCompatibleDC(Some(ecran)) };
    let bitmap = unsafe { CreateCompatibleBitmap(ecran, place.width as i32, place.height as i32) };
    let ancien = unsafe { SelectObject(memoire, bitmap.into()) };

    let rendu = unsafe { PrintWindow(hwnd, memoire, PW_RENDERFULLCONTENT) }.as_bool();
    // `GetPixel` returns a 0x00BBGGRR COLORREF.
    let couleur = unsafe { GetPixel(memoire, place.width as i32 / 2, place.height as i32 / 2) };
    let brut = couleur.0;
    let pixel = (
        (brut & 0xFF) as u8,
        ((brut >> 8) & 0xFF) as u8,
        ((brut >> 16) & 0xFF) as u8,
    );

    unsafe {
        SelectObject(memoire, ancien);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memoire);
        ReleaseDC(None, ecran);
    }

    let verdict = mire::verdict(0, pixel);
    tracing::info!(
        rendu,
        ?pixel,
        ?verdict,
        "PrintWindow(PW_RENDERFULLCONTENT) sur une mire D3D"
    );
    match verdict {
        mire::Verdict::Juste => tracing::info!(
            "verdict PrintWindow : CONDITIONNELLE — image correcte, mais chemin CPU \
             (porte « chemin GPU » de la spec §5)"
        ),
        autre => tracing::error!(?autre, "verdict PrintWindow : ÉLIMINÉE"),
    }
    Ok(())
}

fn eprouver_surface_dwm() {
    // Dynamic resolution: the function is not documented and may be
    // absent. Its absence is a result.
    // `LoadLibraryA` is wrapped by no `.context(...)` here: its
    // raw error (`windows::core::Error`) already carries the HRESULT in its
    // `Display` (`{message} ({code})`), unlike the errors of
    // `wgc::preparer_session` which went through several anyhow `.context(...)`
    // before being logged — it is THAT traversal that lost
    // the HRESULT, not the absence of `causes()` in itself. A direct `%erreur`
    // is therefore enough to get the native code into the log.
    let module = match unsafe { LoadLibraryA(s!("user32.dll")) } {
        Ok(module) => module,
        Err(erreur) => {
            tracing::error!(
                %erreur,
                "verdict DwmGetDxSharedSurface : ÉLIMINÉE — user32 introuvable"
            );
            return;
        }
    };
    let adresse = unsafe { GetProcAddress(module, s!("DwmGetDxSharedSurface")) };
    match adresse {
        Some(_) => tracing::info!(
            "DwmGetDxSharedSurface est exportée par user32 — voie CONDITIONNELLE, \
             à instrumenter seulement si les voies 1 à 3 tombent toutes"
        ),
        None => tracing::error!(
            "verdict DwmGetDxSharedSurface : ÉLIMINÉE — absente de user32 sur ce build"
        ),
    }
}
