//! Enumeration of DXGI outputs, independently of any open duplication.
//!
//! Extracted from `capture.rs` in task 2 of sub-block D2: adding
//! `CibleCapture` and `DesktopCapture::rouvrir` took the parent file to
//! 535 lines, above the cap of 500 (`CLAUDE.md`). This module does not
//! touch any private field of `DesktopCapture` — it therefore did not
//! need to be a child module for access, only to remain
//! `capture::enumerer_sorties` in the eyes of the existing callers, via the
//! re-export at the head of `capture.rs`.

use anyhow::{Context, Result};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

use crate::sortie_dxgi::SortieDxgi;

/// Enumerates all outputs of all adapters, **logging** the
/// adapters that have no output.
///
/// Serves the time-1 reading of the multi-window probe: two documents of the
/// repository contradict each other on which adapter actually drives the desktop
/// (`plans/fix-debit-socket-report.md:163` against commit `4493b24`), and
/// it is this reading that settles it.
///
/// **Reserved for one-off readings.** For a repeated call — the supervisor's
/// placement check runs at 1 Hz — see `enumerer_sorties_silencieux`.
pub fn enumerer_sorties() -> Result<Vec<SortieDxgi>> {
    enumerer(true)
}

/// The same enumeration, **without a single log line**.
///
/// **Fix I2 from the final branch review.** The periodic check of
/// `superviseur::boucle` called `enumerer_sorties`, whose `tracing::info!`
/// per adapter without an output is unconditional: two lines per second,
/// indefinitely, written to a CIFS share — 1220 lines found in an
/// acceptance log of this branch. The repository has already paid for this failure
/// mode ("the measurement destroyed what it measured", `CLAUDE.md`).
///
/// A variant rather than a downgrade to `debug!`: the trace has real
/// value for the probes, which read it once — it is
/// precisely the blind spot where a virtual display adapter that is
/// present but inactive would hide.
pub fn enumerer_sorties_silencieux() -> Result<Vec<SortieDxgi>> {
    enumerer(false)
}

fn enumerer(journaliser: bool) -> Result<Vec<SortieDxgi>> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;
    let mut sorties = Vec::new();
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let adaptateur = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
            Err(_) => "<inconnu>".to_string(),
        };
        let mut index_sortie = 0u32;
        let count_before = sorties.len();
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            if let Ok(desc) = unsafe { output.GetDesc() } {
                let r = desc.DesktopCoordinates;
                sorties.push(SortieDxgi {
                    index_adaptateur,
                    index_sortie,
                    adaptateur: adaptateur.clone(),
                    nom_sortie: String::from_utf16_lossy(&desc.DeviceName)
                        .trim_end_matches('\0')
                        .to_string(),
                    attachee_au_bureau: desc.AttachedToDesktop.as_bool(),
                    rect: crate::geometry::Rect {
                        x: r.left,
                        y: r.top,
                        width: (r.right - r.left).max(0) as u32,
                        height: (r.bottom - r.top).max(0) as u32,
                    },
                });
            }
            index_sortie += 1;
        }
        if journaliser && sorties.len() == count_before {
            // An adapter with no output never produces a
            // `SortieDxgi`: without this trace, it would stay invisible in the
            // reading, which today only logs per output. It is
            // precisely the blind spot where a virtual display adapter
            // that is present but inactive would hide.
            tracing::info!(
                adaptateur = %adaptateur,
                index_adaptateur,
                "adaptateur DXGI sans sortie"
            );
        }
        index_adaptateur += 1;
    }
    Ok(sorties)
}
