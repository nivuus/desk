//! The delay before the next attempt to open the `/agent` channel.
//!
//! Pure, without clock and without socket: it is the only part of the channel client
//! that is testable on the Linux host, and that is why it lives in
//! its own file.

/// Wait before the FIRST reconnection. ⚠️ **NOT CALIBRATED**: no measurement of
/// this repository says how long an outage lasts in practice. Chosen
/// short enough that a one-second outage does not cost a minute of
/// stale `vu_a`, long enough that a relay that refuses is not
/// hammered.
pub const REPLI_MIN_MS: u64 = 500;

/// Ceiling of the wait. ⚠️ **NOT CALIBRATED** in the same way.
///
/// 🔴 **This ceiling is NOT a convenience, it is the safeguard of criterion ④.**
/// Without it, exponential backoff reaches an hour at the thirteenth attempt and
/// a day at the eighteenth: the VM would stay `injoignable` while the
/// network has come back, and **nothing would say so** — the agent would be alive,
/// waiting.
pub const REPLI_MAX_MS: u64 = 30_000;

/// Delay before attempt no. `tentative + 1`, attempt 0 being the
/// first reconnection after a fall.
pub fn delai_de_repli(tentative: u32) -> u64 {
    // `checked_shl` and not `1 << tentative`: the shift overflows at 64, and an
    // overflow in `debug` is a `panic`, hence the death of the reconnection thread —
    // the only thread that could still bring the VM back. `saturating_mul` covers
    // the same risk one step further.
    let facteur = 1u64.checked_shl(tentative).unwrap_or(u64::MAX);
    REPLI_MIN_MS.saturating_mul(facteur).min(REPLI_MAX_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_premiere_reprise_attend_le_minimum() {
        assert_eq!(delai_de_repli(0), REPLI_MIN_MS);
    }

    /// A constant delay is hammering: ten thousand attempts per
    /// hour against a relay that just refused.
    #[test]
    fn the_delay_grows_with_the_attempt() {
        assert!(
            delai_de_repli(1) > delai_de_repli(0),
            "delai_de_repli(1) = {} n'est pas > delai_de_repli(0) = {}",
            delai_de_repli(1),
            delai_de_repli(0)
        );
        assert!(
            delai_de_repli(2) > delai_de_repli(1),
            "delai_de_repli(2) = {} n'est pas > delai_de_repli(1) = {}",
            delai_de_repli(2),
            delai_de_repli(1)
        );
    }

    /// 🔴 The test that matters. Without the ceiling, the twentieth attempt waits
    /// **145 hours**: the VM is unreachable forever, and the agent that
    /// waits looks exactly like an agent about to reconnect.
    #[test]
    fn le_delai_est_borne_par_le_plafond() {
        for tentative in 0..200u32 {
            assert!(
                delai_de_repli(tentative) <= REPLI_MAX_MS,
                "delai_de_repli({tentative}) = {} dépasse REPLI_MAX_MS = {REPLI_MAX_MS}",
                delai_de_repli(tentative)
            );
        }
        assert_eq!(
            delai_de_repli(20),
            REPLI_MAX_MS,
            "le plafond doit être ATTEINT, pas seulement respecté"
        );
    }

    /// `1 << tentative` overflows at 64 — and a reconnection loop that has been
    /// running long enough gets there. An overflow in `debug` is a
    /// `panic`, hence the death of the reconnection thread: the VM never comes back.
    #[test]
    fn le_delai_ne_deborde_pas_sur_une_tentative_enorme() {
        assert_eq!(delai_de_repli(63), REPLI_MAX_MS);
        assert_eq!(delai_de_repli(64), REPLI_MAX_MS);
        assert_eq!(delai_de_repli(u32::MAX), REPLI_MAX_MS);
    }
}
