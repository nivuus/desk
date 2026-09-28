//! Audio REBUILD fault injection — bench variable
//! `AUDIO_FAUTE_RECONSTRUCTION`, never a shipped configuration.
//!
//! It makes reachable the fallback of promoting a neighbour, which
//! sub-block D10 had NO means to exercise: its only trigger — killing
//! the target process tree — kills the window before the audio, the
//! *process loopback* target being tied to the HWND's owning PID. It is legacy 5
//! of D10.

use crate::audio::AudioSource;

/// The process-global budget of injected rebuild faults.
///
/// ⚠️ **BENCH variable, never a shipped configuration** — same status as
/// `PART_SONDAGE` (`transport/part.rs`). Absent: disarmed, and the binary
/// behaves exactly like sub-block D10's.
///
/// **Process-global, never per call**, and that is the lesson D10
/// paid for on `AUDIO_FAUTE_LECTURE` in its second acceptance pass: a
/// budget reread at each attempt replenishes indefinitely — each
/// rebuilt capture would receive a new budget and die again —, and the
/// judging figure it serves becomes structurally unable to leave zero.
/// A `static` decremented by `fetch_update` runs out ONCE for the whole
/// process.
///
/// `pub(super)` and not private: the tests of the full chain
/// "injection → refusal → exhaustion → `AudioMort`" live in
/// `transport/tick/tests/audio.rs`, and **seed the atomic directly**
/// rather than the environment — an already initialised `OnceLock` would
/// not reread `std::env` anyway.
pub(in crate::transport) fn budget_faute_reconstruction() -> &'static std::sync::atomic::AtomicU32 {
    static BUDGET: std::sync::OnceLock<std::sync::atomic::AtomicU32> = std::sync::OnceLock::new();
    BUDGET.get_or_init(|| {
        let n: u32 = std::env::var("AUDIO_FAUTE_RECONSTRUCTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            tracing::warn!(
                fautes_a_injecter = n,
                "injection de fautes de RECONSTRUCTION audio ARMEE : banc, jamais une configuration livrée"
            );
        }
        std::sync::atomic::AtomicU32::new(n)
    })
}

/// Interposes the injection in front of the real rebuilder.
///
/// `fetch_update` with `checked_sub(1)` atomically decrements IF the global
/// budget is not already at zero, and returns `Err` without touching it otherwise: an
/// exhausted budget — the nominal case, the variable being absent — therefore lets
/// the real call through from the first round, with no measurable overhead.
pub(super) fn intercepter<F>(reconstruire: F) -> anyhow::Result<Box<dyn AudioSource + Send>>
where
    F: FnOnce() -> anyhow::Result<Box<dyn AudioSource + Send>>,
{
    if budget_faute_reconstruction()
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_sub(1),
        )
        .is_ok()
    {
        Err(anyhow::anyhow!(
            "faute injectée (AUDIO_FAUTE_RECONSTRUCTION)"
        ))
    } else {
        reconstruire()
    }
}
