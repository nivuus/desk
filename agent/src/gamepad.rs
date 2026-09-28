//! Virtual gamepad: three files of different natures.
//!
//! - `gamepad.rs` (here) carries the pure logic ordering the received
//!   states (`plus_recent`) and limiting the rate of vibrations
//!   (`LimiteurVibration`). Nothing in it is Windows-specific: they live
//!   outside any `#[cfg(windows)]` and are tested on this Linux machine.
//! - `gamepad/win.rs` carries the final ViGEmBus module (`win`, under
//!   `#[cfg(windows)]`, task 10): `VirtualPad` plugs in a virtual Xbox
//!   360 gamepad and applies the received states to it, `spawn_rumble` relays
//!   its vibration notifications to the client through the control
//!   channel.
//! - `gamepad/probe.rs` carries the work stream B probe (`probe`): is ViGEmBus
//!   usable, and does its vibration callback restore the
//!   magnitudes? **Kept on purpose** despite the arrival of the final
//!   module: it is still called by `agent/src/diagnostics.rs`
//!   (`VIGEM_PROBE`), and task 16 (acceptance) explicitly plans to
//!   reuse it to read the gamepad's state back through `XInputGetState`.

use std::time::{Duration, Instant};

/// Maximum rate of vibration messages to the client. The control
/// channel is RELIABLE: flooding it would make it accumulate delay exactly
/// when the game produces the most vibrations.
pub const PERIODE_MIN: Duration = Duration::from_millis(20);

/// True if `new` follows `current` in the sequence space.
///
/// Subtracting in `u16` then rereading as `i16` handles wraparound
/// without a special case: 0 does follow 65535.
pub fn plus_recent(new: u16, current: u16) -> bool {
    (new.wrapping_sub(current)) as i16 > 0
}

/// Limits the rate of vibrations without ever losing the current state.
///
/// `observer` returns the state to emit immediately, or `None` if it is
/// memorised. `echu`, called periodically, returns the memorised state once the
/// delay has elapsed. A memorised state overwrites the previous one: only the last
/// describes what the game asks for.
pub struct LimiteurVibration {
    last_emitted: Option<(u8, u8)>,
    last_send: Option<Instant>,
    en_attente: Option<(u8, u8)>,
}

impl Default for LimiteurVibration {
    fn default() -> Self {
        Self::new()
    }
}

impl LimiteurVibration {
    pub fn new() -> Self {
        Self {
            last_emitted: None,
            last_send: None,
            en_attente: None,
        }
    }

    pub fn observer(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        if self.last_emitted == Some(etat) && self.en_attente.is_none() {
            return None;
        }
        let assez_tot = self
            .last_send
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if assez_tot {
            self.emettre(maintenant, etat)
        } else {
            self.en_attente = Some(etat);
            None
        }
    }

    pub fn echu(&mut self, maintenant: Instant) -> Option<(u8, u8)> {
        let etat = self.en_attente?;
        let assez_tot = self
            .last_send
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if !assez_tot {
            return None;
        }
        self.en_attente = None;
        if self.last_emitted == Some(etat) {
            return None;
        }
        self.emettre(maintenant, etat)
    }

    fn emettre(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        self.last_emitted = Some(etat);
        self.last_send = Some(maintenant);
        self.en_attente = None;
        Some(etat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_state_is_accepted() {
        assert!(plus_recent(11, 10));
        assert!(plus_recent(1000, 1));
    }

    #[test]
    fn an_older_state_is_rejected() {
        assert!(!plus_recent(9, 10));
        assert!(!plus_recent(1, 1000));
    }

    #[test]
    fn an_identical_state_is_rejected() {
        assert!(!plus_recent(10, 10));
    }

    #[test]
    fn the_sequence_wrap_is_crossed_correctly() {
        // The point of the `seq` field: at 250 Hz, the u16 wraps every
        // 4 minutes. A naive comparison `new > current` would then
        // reject all states for half a loop — that is two minutes
        // of frozen gamepad.
        assert!(plus_recent(0, 65535));
        assert!(plus_recent(3, 65533));
        assert!(!plus_recent(65535, 0));
    }

    #[test]
    fn the_first_rumble_goes_through_immediately() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        assert_eq!(limiteur.observer(t0, (200, 100)), Some((200, 100)));
    }

    #[test]
    fn an_identical_state_is_not_re_emitted() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (200, 100));
        assert_eq!(limiteur.observer(t0 + PERIODE_MIN * 2, (200, 100)), None);
    }

    #[test]
    fn a_too_close_change_is_deferred_then_emitted() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        assert_eq!(
            limiteur.observer(t0 + Duration::from_millis(5), (20, 0)),
            None
        );
        assert_eq!(limiteur.echu(t0 + Duration::from_millis(10)), None);
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((20, 0)));
    }

    #[test]
    fn only_the_last_deferred_state_is_emitted() {
        // A burst of vibrations during the limiting window must
        // not produce a queue of stale states: it is the LAST one that describes
        // what the game asks for now.
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        limiteur.observer(t0 + Duration::from_millis(2), (20, 0));
        limiteur.observer(t0 + Duration::from_millis(4), (30, 0));
        limiteur.observer(t0 + Duration::from_millis(6), (40, 0));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((40, 0)));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN * 3), None);
    }
}

#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use probe::probe;
#[cfg(windows)]
pub use win::{spawn_connect, spawn_rumble, VirtualPad};
