//! Choosing and building a session's Windows video source.
//!
//! Extracted from `demarrage.rs` in sub-block D1: this path gained two
//! branches there (imposed window or found by title, whole DXGI output
//! or window crop) that took the parent file above the
//! project's 500-line cap. The addition therefore comes with its
//! extraction, as the rule requires.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::source::VideoSource;
use crate::Config;
use crate::{window, windows_source};

/// What `demarrage` needs to know about the built source.
///
/// `hwnd_addr` is a raw address (`isize`, which is `Send`) and not an
/// `HWND`: `HWND` wraps a non-`Send` `*mut c_void` in windows-rs 0.62,
/// and this value crosses the `move` closure of `spawn_blocking` on the
/// caller side. An `HWND` is only an opaque identifier, never dereferenced.
pub(super) struct SourceWindows {
    pub source: Box<dyn VideoSource + Send>,
    pub hwnd_addr: isize,
    pub bitrate: u32,
    /// 🔴 **THE RECTANGLE ON WHICH INPUTS ARE UNMAPPED, BUILT BY THE
    /// SAME `match` THAT CHOSE THE CAPTURE MODE.** There are therefore not two
    /// descriptions to keep in agreement: that is the lesson of batch 32M, and the
    /// frame size there is a CLONE of the source's cell, never
    /// a copy of its value (batch 32T). See `crate::entrees`.
    pub reference_entrees: crate::entrees::Reference,
}

pub(super) fn construire(
    config: &Config,
    clock_origin: std::time::Instant,
) -> Result<SourceWindows> {
    // Window imposed by the supervisor, or search by title for an agent
    // launched by hand.
    let hwnd = match config.fenetre_hwnd {
        Some(brut) => {
            let hwnd = HWND(brut as *mut core::ffi::c_void);
            anyhow::ensure!(
                window::is_window_alive(hwnd),
                "window {brut:#x} imposed by the supervisor no longer exists"
            );
            hwnd
        }
        None => {
            let title = std::env::var("WINDOW_TITLE").unwrap_or_else(|_| "firefox".into());
            window::find_window_by_title(&title)?
        }
    };

    let bitrate: u32 = std::env::var("BITRATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12_000_000);

    // 90 and not 60: this value is not a target frame rate, it is the
    // `MF_MT_FRAME_RATE` announced to the two MFTs — and the Video Processor uses it
    // as its OUTPUT cadence, which it holds by replaying the last converted
    // frame when nothing new has reached it. Announcing 60 therefore capped
    // the whole pipeline at 60 outputs/s for a desktop producing 68.5,
    // hence 47.5 encoded fps and ~44 fps at the browser.
    //
    // Measured (desktop at 68.5 Hz): 60 → 44.3 fps · 75 → 53.1 · 90 → 58.5 ·
    // 120 → 63.0. Beyond 90, the gain is replay: at 120, the NEW
    // converted frames fall back from 62 to 54/s because the converter,
    // busy holding its declared cadence, refuses more inputs.
    let fps: u32 = std::env::var("ENCODER_FPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    let (source, reference_entrees): (Box<dyn VideoSource + Send>, crate::entrees::Reference) =
        match &config.sortie_dxgi {
            Some(nom_sortie) => {
                // Multi-window mode: capture and encoding live in the
                // SENSOR, a single process for all windows. That is what
                // lifts the cap of four processes holding a DXGI duplication
                // (sub-block D3). The child no longer touches either DXGI or Media
                // Foundation.
                tracing::info!(
                    nom_sortie = %nom_sortie,
                    bitrate,
                    fps,
                    "remote source served by the sensor (multi-window mode)"
                );
                let distante = crate::capteur::tube::connecter(
                    &config.session_id,
                    hwnd.0 as u64,
                    nom_sortie,
                    fps,
                    bitrate,
                    // Set by the supervisor (`TAILLE_FENETRE`, read in policy: allow-fr (env var name)
                    // `Config`); absent — a case that should not happen
                    // in practice for this path, `sortie_dxgi` itself being
                    // set only by the supervisor —, `(u32::MAX, u32::MAX)`
                    // reproduces the behaviour from before this sub-block:
                    // `retained_size` brings it back to the output size
                    // (task 8).
                    config.window_size.unwrap_or((u32::MAX, u32::MAX)),
                    clock_origin,
                )?;
                // 🔴 **THE FRAME SIZE IS TAKEN HERE, AND IT IS AN `Arc`
                // CLONE.** It is the sensor that did the cropping
                // (`retained_size`, `capteur/fenetre/ouverture.rs`) and
                // announced it through `DepuisCapteur::Attachee`; `SourceDistante` is
                // its only storage. Recomputing it here — even with the same
                // pure function and the same inputs — would re-establish TWO
                // descriptions of the same rectangle, which is the mechanism of the
                // defect of batch 32M.
                let reference = crate::entrees::Reference::SortieCapturee {
                    nom: nom_sortie.clone(),
                    image: distante.shared_size(),
                };
                (Box::new(distante), reference)
            }
            None => {
                // Single-window mode, unchanged: agent launched by hand, no
                // sensor. This path must lose NOTHING in the process.
                tracing::info!(bitrate, fps, "capture of the Windows window (crop)");
                // The capture crops the window: the client area IS the frame, there
                // is no second size to carry.
                (
                    Box::new(windows_source::WindowsSource::new(
                        hwnd,
                        fps,
                        bitrate,
                        clock_origin,
                    )?),
                    crate::entrees::Reference::ZoneClientDeLaFenetre,
                )
            }
        };

    Ok(SourceWindows {
        source,
        hwnd_addr: hwnd.0 as isize,
        bitrate,
        reference_entrees,
    })
}
