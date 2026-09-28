//! Hysteresis: how long a constraint must last before we
//! go down a rung, and how long before we go back up. Asymmetric on
//! purpose — see `DELAI_REMONTEE`.

use std::time::{Duration, Instant};

/// Duration for which the condition must hold before GOING DOWN.
const DELAI_DESCENTE: Duration = Duration::from_secs(2);
/// Duration for which the condition must hold before GOING BACK UP.
///
/// Ten times longer than the descent, and it is deliberate: an estimate
/// oscillating around a threshold would otherwise make the encoder flap, and each
/// flap costs a rebuild of the output type and a key frame.
/// We degrade fast to stay fluid, we restore slowly to stay
/// stable.
///
/// **Adjusted from 10 s to 20 s in task 12** (acceptance run), after measurement under the
/// `adsl` profile (8 Mb/s, 30 ms ±5 ms, no loss): 4 rung changes
/// observed in 49 s, whereas the expected property is "at most two in
/// 60 s". Evidence traced in the agent's log: a rise back to the
/// full rung (764×242 → 764×484, `size d'encodage changée` at
/// 16:24:09.545) is followed, one second later, by a ×10 collapse of
/// the BWE estimate in a single observation
/// (`estimation=Some(6639480)` at 16:24:10 then `estimation=Some(619982)` at
/// 16:24:11) — consistent with the key frame the resolution change
/// itself triggers, interpreted by the estimator as an overload. The
/// next rise then falls back immediately (5.0 s later, exactly the
/// `SEJOUR_MINIMAL` floor). Doubling `DELAI_REMONTEE` requires twice as much
/// confidence time before taking back full resolution, which
/// gives the network (and the effect of the controller's own key frame) the
/// time to stabilise before the next attempt. Re-measured after this
/// change (see the results document, §5): no regression
/// observed any more on this point, but the sample remains short (a single
/// window) — see the reservations of the results document.
const DELAI_REMONTEE: Duration = Duration::from_secs(20);
/// Minimum duration between two rung changes, whatever the
/// condition. Safety net against a quick back-and-forth around a threshold.
const SEJOUR_MINIMAL: Duration = Duration::from_secs(5);

/// Duration after the FIRST estimate during which the BWE ramp must
/// not be taken for a degradation.
///
/// The estimation subsystem deliberately starts low and probes upwards
/// (see `ESTIMATION_INITIALE_BPS` on the transport side): during this rise, the
/// available bitrate is low without the link being so. Without this window, any
/// session on a 1080p source would announce "Image réduite par le réseau" on
/// a perfect link, as a persistent banner — measured: the ramp reaches 8.7 to
/// 17.7 Mb/s in 1 to 3 s on gigabit.
///
/// `pub(super)`: read by `controleur::observer`, which is a sibling module —
/// see the head doc of `congestion.rs`.
pub(super) const DELAI_AMORCAGE: Duration = Duration::from_secs(5);

/// Asymmetric time filter on a rung index.
///
/// Returns `Some(nouvel_indice)` at the precise instant a change is kept,
/// and `None` otherwise. The caller has nothing to memorise.
///
/// **Not to be confused with `cursor::Hysteresis`**, which counts
/// consecutive boolean observations: here the filter is temporal,
/// asymmetric, and works on an ordered ladder.
pub struct Hysteresis {
    current: usize,
    /// Rung aimed at continuously since `vise_depuis`, if it differs from the
    /// current one.
    vise: Option<(usize, Instant)>,
    /// Instant of the last kept change.
    last_change: Instant,
}

impl Hysteresis {
    pub fn new(barreau_initial: usize, now: Instant) -> Self {
        Self {
            current: barreau_initial,
            vise: None,
            // Placed so that the dwell time has already elapsed at
            // start-up: the very first adaptation must not wait
            // 5 s more than its own condition.
            last_change: now - SEJOUR_MINIMAL,
        }
    }

    pub fn observer(&mut self, vise: usize, now: Instant) -> Option<usize> {
        if vise == self.current {
            // Back to the current rung: any change intent in progress
            // is cancelled.
            self.vise = None;
            return None;
        }

        // A targeted rung DIFFERENT from the one already under observation
        // restarts the countdown: the condition has not "held", it has
        // changed target.
        let depuis = match self.vise {
            Some((precedent, depuis)) if precedent == vise => depuis,
            _ => {
                self.vise = Some((vise, now));
                now
            }
        };

        // Increasing indices = decreasing resolutions: aiming higher
        // than the current one means going down.
        let delai = if vise > self.current {
            DELAI_DESCENTE
        } else {
            DELAI_REMONTEE
        };
        if now.duration_since(depuis) < delai {
            return None;
        }
        if now.duration_since(self.last_change) < SEJOUR_MINIMAL {
            return None;
        }

        self.current = vise;
        self.vise = None;
        self.last_change = now;
        Some(vise)
    }
}

/// Reference instant of the tests. Placed far in the past so that any
/// duration subtraction stays valid.
///
/// `pub(super)`: the tests of `controleur` and `reconfiguration`, sibling
/// modules, need it too — see the head doc of `congestion.rs`.
#[cfg(test)]
pub(super) fn t0() -> Instant {
    Instant::now() - Duration::from_secs(3600)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_down_requires_two_seconds_below_the_rung() {
        // Base bound ONLY ONCE: `t0()` returns a new instant at each
        // call, and assertions set on exact bounds (2.000 s)
        // would become unstable to within a few microseconds.
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // First observation of rung 1: the countdown STARTS here, nothing
        // has elapsed yet.
        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // 1.999 s after the start of the countdown: not yet.
        assert_eq!(h.observer(1, base + Duration::from_millis(3899)), None);
        // 2,000 s pile : on descend.
        assert_eq!(h.observer(1, base + Duration::from_millis(3900)), Some(1));
    }

    #[test]
    fn un_repit_remet_le_compteur_de_descente_a_zero() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        assert_eq!(h.observer(1, base + Duration::from_millis(1900)), None);
        // A single observation back at the current rung cancels the countdown.
        assert_eq!(h.observer(0, base + Duration::from_millis(1950)), None);
        // The countdown restarts from zero at 3000 ms.
        assert_eq!(h.observer(1, base + Duration::from_millis(3000)), None);
        // 1.9 s after this new start: still not.
        assert_eq!(h.observer(1, base + Duration::from_millis(4900)), None);
        // 2.0 s after: this time yes.
        assert_eq!(h.observer(1, base + Duration::from_millis(5000)), Some(1));
    }

    #[test]
    fn remonter_exige_vingt_secondes_et_non_deux() {
        let base = t0();
        // Start at rung 1: `new` places the last change in the
        // past, so the dwell time does not hinder this test.
        let mut h = Hysteresis::new(1, base);

        assert_eq!(h.observer(0, base + Duration::from_millis(2000)), None);
        // Exactly 2.0 s after the start of the countdown: a DESCENT would have switched
        // here, the threshold being reached. A rise, no — that is the whole point
        // of this test.
        assert_eq!(h.observer(0, base + Duration::from_millis(4000)), None);
        // 19.999 s: still not (DELAI_REMONTEE = 20 s since task 12).
        assert_eq!(h.observer(0, base + Duration::from_millis(21_999)), None);
        // 20,000 s pile : on remonte.
        assert_eq!(h.observer(0, base + Duration::from_millis(22_000)), Some(0));
    }

    #[test]
    fn le_temps_de_sejour_bloque_un_second_changement_trop_proche() {
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // First descent: countdown started at 0, kept at 2.0 s.
        assert_eq!(h.observer(1, base), None);
        assert_eq!(h.observer(1, base + Duration::from_millis(2000)), Some(1));

        // The descent condition towards 2 is met 2 s later, but the
        // 5 s dwell time since the last change forbids it.
        assert_eq!(h.observer(2, base + Duration::from_millis(2001)), None);
        assert_eq!(h.observer(2, base + Duration::from_millis(4001)), None);
        // At 7.000 s: 5.0 s of dwell elapsed AND the condition has held for
        // 4.999 s. Both locks are lifted.
        assert_eq!(h.observer(2, base + Duration::from_millis(7000)), Some(2));
    }

    #[test]
    fn targeting_a_second_rung_without_going_back_through_the_current_restarts_the_countdown() {
        // Trivial finding of the final review: this path (branch
        // `_ => { self.vise = Some((vise, now)); now }` of `observer`) was
        // exercised by no test. We first aim at 1, then change target
        // to 2 WITHOUT ever going back through the current rung (0) in
        // between — the countdown must restart from zero for the new target, not
        // continue from the first one.
        let base = t0();
        let mut h = Hysteresis::new(0, base);

        // Aims at 1: countdown started at 0 ms.
        assert_eq!(h.observer(1, base + Duration::from_millis(500)), None);
        // Changes target to 2 at 1000 ms, without going back through 0: the
        // countdown for 2 must restart from 1000 ms, not from 0 ms.
        assert_eq!(h.observer(2, base + Duration::from_millis(1000)), None);
        // 1.999 s after this restart (2999 ms): if the countdown had
        // continued from the very first `observer` (0 ms), it would already
        // be at 2.999 s and would have switched — the proof that it is not the case.
        assert_eq!(h.observer(2, base + Duration::from_millis(2999)), None);
        // Exactly 2.000 s after the restart at 1000 ms: switches to 2, the
        // most recent target — never to 1.
        assert_eq!(h.observer(2, base + Duration::from_millis(3000)), Some(2));
    }
}
