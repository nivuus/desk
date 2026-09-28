//! Capture counters, **per session**.
//!
//! **Why this file exists**: `windows_source.rs` carried three process
//! statics (`TICKS`, `CAPTURED`, `PRODUCED`). Since sub-block D4, the
//! capture is shared in a single process holding N windows: the
//! three counters therefore mixed N sessions. Worse, their only reader
//! (`demarrage.rs`) lives in the CHILD, which writes nothing — `SOURCE_TRACE=1`
//! only displayed zeros, and the sensor's counters were read by
//! nobody. Recorded item no. 1 of sub-block D6, due since August 3rd, 2026.
//!
//! **Pure, no `cfg`**: that is what makes it testable on the host.

use std::sync::atomic::{AtomicU64, Ordering};

/// Capture counters of ONE window.
#[derive(Debug, Default)]
pub struct Telemetrie {
    ticks: AtomicU64,
    captured: AtomicU64,
    produced: AtomicU64,
}

impl Telemetrie {
    /// A capture loop round.
    pub fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
    }

    /// A frame acquired from the duplication.
    pub fn capturee(&self) {
        self.captured.fetch_add(1, Ordering::Relaxed);
    }

    /// A frame delivered downstream.
    pub fn produite(&self) {
        self.produced.fetch_add(1, Ordering::Relaxed);
    }

    /// `(ticks, captured, produced)`.
    pub fn lire(&self) -> (u64, u64, u64) {
        (
            self.ticks.load(Ordering::Relaxed),
            self.captured.load(Ordering::Relaxed),
            self.produced.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_telemetries_ne_se_melangent_pas() {
        // It is THE WHOLE point of the legacy: three process statics mixed
        // the sensor's N windows since D4, and `SOURCE_TRACE=1` only displayed
        // zeros in the child, which writes nothing.
        let a = Telemetrie::default();
        let b = Telemetrie::default();
        a.tick();
        a.tick();
        a.capturee();
        b.produite();
        assert_eq!(a.lire(), (2, 1, 0));
        assert_eq!(b.lire(), (0, 0, 1));
    }

    /// A reset telemetry starts again from zero — and this test PROVES it by
    /// first having made it count on its THREE counters.
    ///
    /// ⚠️ **It replaces `une_telemetrie_neuve_est_a_zero` (D9's legacy no. 11),
    /// which only exercised `#[derive(Default)]` and could not return
    /// the other value**: it passed whatever the body of
    /// `tick`/`capturee`/`produite`. The weakness was PROVEN, not
    /// asserted — with `tick()` made a no-op, the old test stays GREEN while
    /// this one turns red on `left: (0, 1, 1)`.
    ///
    /// The precondition compares the EXACT triple and not "different from
    /// zero": sabotaging a single one of the three counters would leave an
    /// `assert_ne!(…, (0,0,0))` green, the other two being enough to
    /// satisfy it. That was this test's first draft, and it was
    /// measured green under the sabotage it was meant to expose.
    #[test]
    fn une_telemetrie_remise_a_neuf_repart_de_zero() {
        let mut t = Telemetrie::default();
        t.tick();
        t.capturee();
        t.produite();
        assert_eq!(
            t.lire(),
            (1, 1, 1),
            "précondition : les TROIS compteurs ont compté"
        );
        t = Telemetrie::default();
        assert_eq!(t.lire(), (0, 0, 0));
    }
}
