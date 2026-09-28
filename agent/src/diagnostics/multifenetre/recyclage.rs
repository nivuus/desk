//! The pivot measurement of sub-block D5: **does destroying an encoder free the
//! slot?**
//!
//! The question has been open since 31 July 2026 and conditions the whole
//! sub-block — the remedy for D4's defect as well as sleeping itself.
//! The sequence "create 8 → destroy 1 → attempt a 9th" had never been
//! played.
//!
//! **The set-up reproduces the PRODUCTION arrangement**: one process, one
//! D3D11 device per encoder (each `DesktopCapture` creates its own, see
//! `capture/ouverture.rs`). It is neither the `partage` mode nor the `separe` mode
//! of `nvenc.rs`, and it is the only set-up whose answer commits the product.
//!
//! **The repeated cycle is the heart of the measurement, not an extra.** A single
//! recycling does not distinguish a CONCURRENCY ceiling (8 alive at once)
//! from a CUMULATIVE CREATIONS ceiling with some slack: the first cycle would pass
//! in both cases. D5's pool recycles encoders by construction —
//! it is precisely what would trigger the second failure.

#![cfg(windows)]

use anyhow::Result;

use super::nvenc::peripherique_autonome;
use crate::encode::H264Encoder;

/// The exact parameters of D4's second acceptance run, so that the figure is
/// comparable to its own.
const LARGEUR: u32 = 1280;
const HAUTEUR: u32 = 720;
const FPS: u32 = 60;
const DEBIT: u32 = 8_000_000;

/// We stop searching beyond: the expected ceiling is 8.
const SEARCH_CEILING: usize = 16;

/// An encoder and the device that carries it. The device MUST live
/// as long as the encoder; releasing them separately would measure
/// something other than what we think.
///
/// The three fields are only lifetime carriers: none is
/// ever read, they exist only so that their objects stay alive
/// until the `drop` of the `Instance`, hence the `_` prefix on all three — a
/// `_x` field stays owned and is destroyed normally, only the read is
/// silenced.
struct Instance {
    _peripherique: windows::Win32::Graphics::Direct3D11::ID3D11Device,
    _contexte: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    _encodeur: H264Encoder,
}

/// Builds a complete instance, or returns the refusal's error.
fn construire() -> Result<Instance> {
    let (peripherique, contexte) = peripherique_autonome()?;
    let encodeur = H264Encoder::new(
        &peripherique,
        (LARGEUR, HAUTEUR),
        (LARGEUR, HAUTEUR),
        FPS,
        DEBIT,
    )?;
    Ok(Instance {
        _peripherique: peripherique,
        _contexte: contexte,
        _encodeur: encodeur,
    })
}

pub(super) fn mesurer(cycles: usize) -> Result<()> {
    tracing::info!(
        cycles,
        largeur = LARGEUR,
        hauteur = HAUTEUR,
        fps = FPS,
        debit = DEBIT,
        "pivot measurement D5: encoder recycling, one D3D11 device per encoder"
    );

    // ── Phase 1 : monter jusqu'au refus, et le NOMMER.
    //
    // Without this control, nothing that follows proves anything: a 9th
    // that succeeds after a destruction says nothing if the 9th already succeeded
    // before.
    let mut vivants: Vec<Instance> = Vec::new();
    let mut plafond = 0usize;
    for rang in 1..=SEARCH_CEILING {
        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                tracing::info!(rang, vivants = vivants.len(), "phase 1: encoder created");
            }
            Err(error) => {
                plafond = rang - 1;
                tracing::info!(
                    phase = 1,
                    plafond,
                    rang_refuse = rang,
                    causes = %super::causes(error),
                    "phase 1: ceiling reached — creation refused"
                );
                break;
            }
        }
    }
    if plafond == 0 {
        tracing::warn!(
            search_ceiling = SEARCH_CEILING,
            "phase 1: no refusal below the search ceiling — the pivot measurement is MOOT"
        );
        drop(vivants);
        return Ok(());
    }

    // ── Phase 2: the cycle. Destroy one, build one, k times.
    //
    // `vivants` contains exactly `plafond` instances. At each round we
    // remove one (real destruction: explicit `drop`, traced on either
    // side so that a freeze of `Drop for H264Encoder` reads as such) then
    // we attempt to build a new one.
    let mut reussis = 0usize;
    let mut premier_echec: Option<usize> = None;
    for cycle in 1..=cycles {
        let retiree = vivants.pop().expect("the pool cannot be empty here");
        tracing::info!(
            cycle,
            restants = vivants.len(),
            "cycle: releasing one encoder"
        );
        drop(retiree);
        tracing::info!(cycle, restants = vivants.len(), "cycle: release finished");

        match construire() {
            Ok(instance) => {
                vivants.push(instance);
                reussis += 1;
                tracing::info!(cycle, vivants = vivants.len(), "cycle: rebuild SUCCEEDED");
            }
            Err(error) => {
                premier_echec = Some(cycle);
                tracing::info!(
                    cycle,
                    vivants = vivants.len(),
                    causes = %super::causes(error),
                    "cycle: rebuild REFUSED"
                );
                break;
            }
        }
    }

    // ── Verdict, in one line readable without the rest of the log.
    match premier_echec {
        None => tracing::info!(
            plafond,
            cycles_demandes = cycles,
            cycles_reussis = reussis,
            verdict = "concurrence",
            "VERDICT: destroying an encoder frees the place, and the cycles hold"
        ),
        Some(0) | Some(1) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            verdict = "no-release",
            "VERDICT: destroying an encoder does NOT free the place"
        ),
        Some(k) => tracing::info!(
            plafond,
            cycles_reussis = reussis,
            premier_echec = k,
            verdict = "cumulative",
            "VERDICT: ceiling of CUMULATIVE creations — the recycling gives way at cycle {k}"
        ),
    }

    tracing::info!(vivants = vivants.len(), "final release");
    drop(vivants);
    tracing::info!("final release finished — the process survived");
    Ok(())
}
