//! Probe: does the virtual desktop extend to a virtual output, and does the
//! pointer get there?
//!
//! `input.rs` already sets `MOUSEEVENTF_VIRTUALDESK` and computes its coordinates
//! on `SM_*VIRTUALSCREEN`: nothing is to be written on the product side. What is
//! established by no reading of code is that Windows counts a virtual
//! output in these metrics. This probe settles it by a measurement.

use anyhow::{Context, Result};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

use crate::capture::enumerer_sorties;
use crate::moniteurs_virtuels::pilote::ouvrir_pilote;
use crate::moniteurs_virtuels::Sorties;

/// Rectangle of the virtual desktop, as Windows declares it.
fn bureau_virtuel() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

pub(super) fn sonder() -> Result<()> {
    let before = bureau_virtuel();
    tracing::info!(
        x = before.0,
        y = before.1,
        largeur = before.2,
        hauteur = before.3,
        "virtual desktop BEFORE creating the output"
    );

    let pilote = ouvrir_pilote()?;
    let mut sorties = Sorties::nouvelles(&pilote);
    let id = sorties
        .create(1280, 720, 60)
        .context("creating the virtual output")?;

    // The driver creates the output asynchronously from the point of view of
    // the desktop space: Windows still has to attach it. We give
    // the topology time to settle, then read again.
    std::thread::sleep(std::time::Duration::from_secs(3));
    pilote.pinguer()?;

    let apres = bureau_virtuel();
    tracing::info!(
        id,
        x = apres.0,
        y = apres.1,
        largeur = apres.2,
        hauteur = apres.3,
        elargi = (apres != before),
        "virtual desktop AFTER creating the output"
    );

    // Find the virtual output among the DXGI outputs: it is its
    // rectangle that gives the target to aim at. `GetDesc`/`DesktopCoordinates`
    // is the source of truth — WMI lies (field seen 68 s stale).
    let all = enumerer_sorties()?;
    for s in &all {
        tracing::info!(
            adaptateur = %s.adaptateur, nom = %s.nom_sortie,
            attachee = s.attachee_au_bureau,
            x = s.rect.x, y = s.rect.y, l = s.rect.width, h = s.rect.height,
            "DXGI output enumerated"
        );
    }
    let cible = all
        .iter()
        .filter(|s| s.attachee_au_bureau && s.rect.width == 1280 && s.rect.height == 720)
        .max_by_key(|s| s.rect.x)
        .context(
            "no attached 1280x720 output: the virtual output did not \
             enter the desktop topology",
        )?;

    // Centre of the output, in virtual desktop coordinates, converted into
    // the normalised 0..65535 space that `MOUSEEVENTF_ABSOLUTE` expects — the
    // exact conversion of `input.rs`.
    let vise_x = cible.rect.x + (cible.rect.width / 2) as i32;
    let vise_y = cible.rect.y + (cible.rect.height / 2) as i32;
    let normalise = |v: i32, origine: i32, etendue: i32| -> i32 {
        ((v - origine) as i64 * 65535 / etendue.max(1) as i64) as i32
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: normalise(vise_x, apres.0, apres.2),
                dy: normalise(vise_y, apres.1, apres.3),
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let envoyes = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    let mut ou = POINT::default();
    unsafe { GetCursorPos(&mut ou) }.context("GetCursorPos")?;

    let ecart = ((ou.x - vise_x).abs(), (ou.y - vise_y).abs());
    tracing::info!(
        envoyes,
        vise_x,
        vise_y,
        obtenu_x = ou.x,
        obtenu_y = ou.y,
        ecart_x = ecart.0,
        ecart_y = ecart.1,
        // Two pixels of tolerance: the normalised conversion is not
        // exactly reversible, and that is not what we measure here.
        verdict = if ecart.0 <= 2 && ecart.1 <= 2 {
            "ATTEINTE"
        } else {
            "NON ATTEINTE"
        },
        "absolute injection towards the centre of the virtual output"
    );

    Ok(())
}
