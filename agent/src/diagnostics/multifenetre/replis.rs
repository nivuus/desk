//! Voie 4 : les replis par fenêtre, sondés en dernier parce qu'ils sont les
//! moins prometteurs.
//!
//! `PrintWindow(PW_RENDERFULLCONTENT)` passe par GDI et rend typiquement du
//! noir sur une fenêtre D3D — c'est précisément ce que cette sonde vérifie,
//! sur une mire peinte en D3D11 et non en GDI. Il rapatrie de surcroît les
//! pixels en mémoire centrale : même correct, il tomberait sur la porte
//! « chemin GPU » de la spec §5. On le sonde pour le CONSIGNER, pas dans
//! l'espoir de le retenir.
//!
//! `DwmGetDxSharedSurface` n'est pas documentée : elle est résolue
//! dynamiquement dans user32.dll, et son absence est un résultat, pas une
//! erreur.

use anyhow::{Context, Result};
use windows::core::s;
// Écart avec le plan : `PrintWindow` et `PRINT_WINDOW_FLAGS` ne sont PAS dans
// `Win32::UI::WindowsAndMessaging` sous `windows` 0.62 (comme l'énonçait le
// plan), mais dans `Win32::Storage::Xps` — la fonction y est déclarée sous
// `#[cfg(feature = "Win32_Graphics_Gdi")]`, mais le module qui la porte est
// lui-même gated par la feature `Win32_Storage_Xps`, ajoutée à `Cargo.toml`
// pour cette tâche. Suivi en source : `windows-0.62.2/src/Windows/Win32/
// Storage/Xps/mod.rs`.
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
    ReleaseDC, SelectObject,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;

/// Non exposée par `windows` 0.62 : `PW_RENDERFULLCONTENT` vaut 2 (WinUser.h).
/// Seule `PW_CLIENTONLY` (1) est exportée par ce crate à cette version.
const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

pub(super) fn eprouver() -> Result<()> {
    // Panne du banc, pas verdict sur les replis : si le bureau ne peut pas
    // être découpé en deux places ou si les fenêtres de mire ne s'ouvrent
    // pas, aucune mesure n'est possible. Comme dans `wgc.rs`, ces échecs
    // restent donc propagés par `?`.
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places =
        disposition::tuiles(Rect { x: 0, y: 0, width: largeur, height: hauteur }, 2)
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

/// À partir d'ici, tout échec de mesure devient un verdict ÉLIMINÉE
/// journalisé plutôt qu'une erreur propagée — le patron établi à la tâche 6
/// (`wgc.rs`) : un lecteur du journal doit toujours trouver l'un des
/// messages « verdict … » ci-dessous, jamais une erreur qui remonte
/// silencieusement jusqu'à `main()`. Seule la préparation du banc lui-même
/// (déjà passée dans `eprouver`) reste propagée par `?`.
fn eprouver_printwindow(mires: &Mires) -> Result<()> {
    let hwnd = mires.hwnd(0)?;
    let place = mires.place(0)?;
    let ecran = unsafe { GetDC(None) };
    let memoire = unsafe { CreateCompatibleDC(Some(ecran)) };
    let bitmap = unsafe { CreateCompatibleBitmap(ecran, place.width as i32, place.height as i32) };
    let ancien = unsafe { SelectObject(memoire, bitmap.into()) };

    let rendu = unsafe { PrintWindow(hwnd, memoire, PW_RENDERFULLCONTENT) }.as_bool();
    // `GetPixel` rend un COLORREF 0x00BBGGRR.
    let couleur = unsafe { GetPixel(memoire, place.width as i32 / 2, place.height as i32 / 2) };
    let brut = couleur.0;
    let pixel = ((brut & 0xFF) as u8, ((brut >> 8) & 0xFF) as u8, ((brut >> 16) & 0xFF) as u8);

    unsafe {
        SelectObject(memoire, ancien);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memoire);
        ReleaseDC(None, ecran);
    }

    let verdict = mire::verdict(0, pixel);
    tracing::info!(rendu, ?pixel, ?verdict, "PrintWindow(PW_RENDERFULLCONTENT) sur une mire D3D");
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
    // Résolution dynamique : la fonction n'est pas documentée et peut être
    // absente. Son absence est un résultat.
    // `LoadLibraryA` n'est enveloppée par aucun `.context(...)` ici : son
    // erreur brute (`windows::core::Error`) porte déjà le HRESULT dans son
    // `Display` (`{message} ({code})`), contrairement aux erreurs de
    // `wgc::preparer_session` qui traversaient plusieurs `.context(...)`
    // anyhow avant d'être journalisées — c'est CETTE traversée qui perdait
    // le HRESULT, pas l'absence de `causes()` en soi. Un `%erreur` direct
    // suffit donc à faire figurer le code natif au journal.
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
