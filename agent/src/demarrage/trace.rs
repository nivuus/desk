//! The trace thread of the child's real path (`SOURCE_TRACE=1`), extracted from
//! `demarrage.rs` by sub-block P2 of the clipboard work stream: that file
//! was at 491 lines for a cap of 500, and P2 adds reading
//! `PRESSE_PAPIER` and a field to the three `capabilities`. The extraction precedes
//! the addition, as the repository's rule requires.
//!
//! **The SENSOR twin of this file already exists**: `capteur/fenetre/trace.rs`,
//! born of sub-block D9, reads `Telemetrie` where the source actually lives. Both
//! sides of `SOURCE_TRACE` now each have their own module.
//!
//! ⚠️ This module is an ordinary `#[cfg(windows)]` child, declared by a plain `mod`
//! **inside** its parent: the "Child module convention"
//! of `CLAUDE.md` does not concern it — it never needs to leave its parent's
//! tree to compile on the host.

use std::time::Duration;

use crate::{capture, encode, windows_source};

// Watching the real path (`SOURCE_TRACE=1`): what remains here after
// task 11 of D9 is the ENCODER part (capture attempts/accumulation,
// inputs/outputs of the converter and the encoder). The cadence
// of `next_frame`, the new captures and the units produced — the three
// counters that lived in `windows_source::TICKS`/`CAPTURED`/
// `PRODUCED` — left with them: this thread is in the CHILD, which has
// had no `WindowsSource` since D4, hence nothing left to read from it. They are
// now a PER-SESSION field on `WindowsSource` itself
// (`telemetrie`, see `windows_source/telemetrie.rs`), traced on the
// SENSOR side by `capteur/fenetre.rs`, where the source actually lives.
//
// ⚠️ The FIFTEEN counters this thread still reads (`capture::ATTEMPTS`/
// `HITS`/`ACCUMULATED`, `encode::NEED_INPUT_EVENTS`/`ENCODER_INPUTS`/
// `DROPPED_STALE`/`CONVERTER_*`/`*_NS`, `windows_source::CAPTURE_NS`/
// `SUBMIT_NS`/`DRAIN_NS`) are dead for the SAME reason as the three
// above: they are crate statics, written only by
// `WindowsSource` (`capture.rs:311,328`, `encode.rs:522,850,868`), and
// `demarrage/source.rs` says in so many words that "the child no longer touches
// either DXGI or Media Foundation" in multi-window mode. This thread
// therefore cannot read anything there either, in multi-window mode.
//
// Second, reciprocal effect: in SINGLE-window mode, this thread does touch
// DXGI/MF in process (via `WindowsSource`) and these fifteen counters are
// alive there — but `Telemetrie` (the per-session replacement of `TICKS`/
// `CAPTURED`/`PRODUCED`) has only one reader, `capteur/fenetre/
// trace.rs`, which only runs in the SENSOR. The single-window path has
// therefore lost those three counters, without anything replacing them there.
pub(super) fn brancher() -> Option<std::thread::JoinHandle<()>> {
    std::env::var("SOURCE_TRACE").is_ok().then(|| {
        std::thread::spawn(|| {
            use std::sync::atomic::Ordering::Relaxed;
            let (mut a0, mut h0, mut ac0) = (0u64, 0u64, 0u64);
            let (mut ni0, mut ei0, mut ds0) = (0u64, 0u64, 0u64);
            let (mut cn0, mut sn0, mut dn0) = (0u64, 0u64, 0u64);
            let (mut cv0, mut in0, mut out0) = (0u64, 0u64, 0u64);
            let (mut ci0, mut co0, mut cs0) = (0u64, 0u64, 0u64);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let (a, h, ac) = (
                    capture::ATTEMPTS.load(Relaxed),
                    capture::HITS.load(Relaxed),
                    capture::ACCUMULATED.load(Relaxed),
                );
                let (ni, ei, ds) = (
                    encode::NEED_INPUT_EVENTS.load(Relaxed),
                    encode::ENCODER_INPUTS.load(Relaxed),
                    encode::DROPPED_STALE.load(Relaxed),
                );
                let (cap_ns, sub_ns, dr_ns) = (
                    windows_source::CAPTURE_NS.load(Relaxed),
                    windows_source::SUBMIT_NS.load(Relaxed),
                    windows_source::DRAIN_NS.load(Relaxed),
                );
                let (ci, co, cs) = (
                    encode::CONVERTER_INPUTS.load(Relaxed),
                    encode::CONVERTER_OUTPUTS.load(Relaxed),
                    encode::CONVERTER_SKIPPED.load(Relaxed),
                );
                let (cv_ns, in_ns, out_ns) = (
                    encode::CONVERT_NS.load(Relaxed),
                    encode::ENC_IN_NS.load(Relaxed),
                    encode::ENC_OUT_NS.load(Relaxed),
                );
                // Share of the observation window (2 s = 2e9 ns) actually
                // spent in each call: that is what distinguishes a stage
                // that saturates from a stage that waits.
                let pct = |now: u64, prev: u64| (now - prev) as f64 / 2e9 * 100.0;
                tracing::info!(
                    acquire_hz = (a - a0) as f64 / 2.0,
                    hits_hz = (h - h0) as f64 / 2.0,
                    // Desktop updates that actually happened, including
                    // those DXGI merged: it is this figure that says
                    // whether the window produces more than we retrieve from it.
                    desktop_updates_hz = (ac - ac0) as f64 / 2.0,
                    need_input_hz = (ni - ni0) as f64 / 2.0,
                    encoder_inputs_hz = (ei - ei0) as f64 / 2.0,
                    dropped_stale_hz = (ds - ds0) as f64 / 2.0,
                    conv_in_hz = (ci - ci0) as f64 / 2.0,
                    conv_out_hz = (co - co0) as f64 / 2.0,
                    conv_skipped_hz = (cs - cs0) as f64 / 2.0,
                    capture_pct = pct(cap_ns, cn0),
                    submit_pct = pct(sub_ns, sn0),
                    drain_pct = pct(dr_ns, dn0),
                    convert_pct = pct(cv_ns, cv0),
                    enc_in_pct = pct(in_ns, in0),
                    enc_out_pct = pct(out_ns, out0),
                    "source cadence (real path)"
                );
                (a0, h0, ac0) = (a, h, ac);
                (ni0, ei0, ds0) = (ni, ei, ds);
                (cn0, sn0, dn0) = (cap_ns, sub_ns, dr_ns);
                (cv0, in0, out0) = (cv_ns, in_ns, out_ns);
                (ci0, co0, cs0) = (ci, co, cs);
            }
        })
    })
}
