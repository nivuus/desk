//! The PURE reconnection decision of the supervisor's **control session**:
//! when to retry, and when the exponential fallback is allowed to
//! start again from the floor.
//!
//! 🔴 **THE DEFECT IT FIXES, MEASURED IN PRODUCTION.** Before this batch,
//! `superviseur/signalisation.rs` opened its socket ONCE and NEVER
//! reopened it. After a restart of the `desk-plateforme` service, the
//! supervisor logged "send to the shell failed
//! error=Trying to work with closed connection", then "control connection
//! to signaling lost", and **stayed that way until its own death**: no
//! window could be announced nor RE-ANNOUNCED any more, which made
//! batch 17's fix (`pair-present`) inoperative, since it needs this
//! socket to be delivered. The only known remedy was to restart
//! the agent — a gesture that, since batch 32I's ownership rule,
//! **orphans all of the owner's windows**. Legacy no. 1 of batch 17,
//! closed here.
//!
//! 🔴 **IT IS NOT THE BRIDGE'S CASE, AND CONFUSING THEM WOULD LEAD TO COPYING THE
//! WRONG REMEDY.** `boucle/surveillance_pont.rs` restarts a **PROCESS**
//! that a third party (the supervisor loop) OBSERVES from outside: the
//! process dies, `LanceurDeProcessus::etat_du_pont` observes it at the next
//! turn, and the exit outcome (`IssueDeSortie`) says whether the failure is
//! resolved. Here it is a **SOCKET in a live process**: no one
//! observes it from outside, there is **no exit code to read**, and
//! the resumption loop must therefore live **in the task owning the
//! socket**. That is why this module is NOT [`crate::relance_pont::
//! EtatRelance`] and does not derive from it:
//!
//! ① `EtatRelance::reset_the_backoff` takes a [`crate::relance_pont::
//!    IssueDeSortie`] — a process exit code, which does not exist here;
//! ② `EtatRelance::doit_relancer(ecoule_ms)` assumes a caller that POLLS at
//!    each clock turn; this loop **sleeps** the wanted delay, it
//!    polls nothing;
//! ③ `EtatRelance::stable` is guarded by `cycle_signale`, a TRACE
//!    boolean this module does not need — it logs **each**
//!    attempt, see below.
//!
//! **What is REUSED, and not copied**, is the primitive both
//! already share with the `/agent` channel: [`crate::plateforme::repli::
//! delai_de_repli`] and its ceiling `REPLI_MAX_MS`. The closest precedent
//! for the whole mechanism is actually not the bridge but
//! **`plateforme.rs::ouvrir`**: this same agent ALREADY knows how to reopen a lost
//! socket — the `/agent` channel — with this same fallback. The control session
//! was the only one of its three sockets not knowing how.
//!
//! 🔴 **MODULE CONVENTION (`CLAUDE.md`), APPLIED RATHER THAN GUESSED.**
//! This file is an ORDINARY child of `superviseur.rs`, declared by a
//! `pub mod reprise_controle;` without `#[path]`, and it does not have to ask the
//! prefix question: that question ONLY arises for a module
//! extracted from a `#[cfg(windows)]` parent that must therefore become a
//! top-level sibling. `superviseur.rs`, for its part, is NOT gated (only
//! some of its children are), so that an ordinary child already compiles
//! and is tested on the Linux host — exactly like its neighbours
//! `table.rs`, `fenetres.rs` and `reprise.rs`.
//!
//! ⚠️ **`reprise_controle` AND NOT `reprise`**: `superviseur::reprise` already
//! exists (batch 32E) and designates SOMETHING ELSE — the number of chances given to a
//! virtual output to attach. Two `reprise`s in the same module
//! graph would only wait for a hurried reader to be confused, exactly
//! the argument that got `surveillance_pont` and `relance_pont` named.

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// Lifetime beyond which a connection is deemed to have
/// **SERVED**, and where the exponential fallback therefore starts again from the floor.
///
/// 🔴 **STRICTLY GREATER THAN `REPLI_MAX_MS`, AND IT IS WHAT MAKES
/// RE-ARMING UNREACHABLE BY A REFUSAL.** A handshake refusal from the
/// platform (`signaling/relais.ts`: token absent, expired, role already
/// taken, volume budget exhausted) arrives in **milliseconds** — the relay
/// sends `{"type":"error",…}` then `close(1008)` in the same gesture. A
/// refused connection therefore structurally cannot reach this threshold, and
/// the fallback keeps growing up to its ceiling instead of hammering a
/// service that has just said no. It is the lesson `relance_pont.rs`
/// paid for in five fix rounds: a SHORT threshold (500 ms) made
/// re-arming systematic and the fallback **structurally unable to
/// grow**.
///
/// ⚠️ **NOT CALIBRATED** — same caveat as `REPLI_MIN_MS` and `REPLI_MAX_MS`:
/// no measurement of this repository says how long a real outage lasts.
/// The 5 s margin above the ceiling covers the time a refusal needs
/// to ARRIVE (WebSocket handshake, round trip, reading the
/// message), not measured, chosen wide rather than tight. The value follows that
/// of `relance_pont::SEUIL_STABILITE_MS` by the same arithmetic — without
/// importing it: that threshold speaks of a PROCESS sleeping before dying,
/// this one of a refused SOCKET, and welding them would make a recalibration of
/// one move the other without any measurement asking for it.
pub const SEUIL_CONNEXION_UTILE_MS: u64 = REPLI_MAX_MS + 5_000;

/// The PURE state of the control session's resumption.
///
/// Knows neither socket, nor wall clock, nor tokio: it receives ELAPSED
/// milliseconds and returns milliseconds TO WAIT. It is what
/// makes it exercisable on the Linux host, whereas `signalisation.rs` lives behind
/// a `#![cfg(windows)]` that `cargo test --workspace` never compiles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Reprise {
    /// CONSECUTIVE reconnection attempts since the last re-arming.
    tentative: u32,
}

impl Reprise {
    pub fn neuve() -> Self {
        Self { tentative: 0 }
    }

    /// The number of consecutive attempts — to attach it to the
    /// caller's traces, never to decide anything here.
    pub fn tentative(&self) -> u32 {
        self.tentative
    }

    /// How many milliseconds to sleep BEFORE the next attempt — a mere
    /// rewrapping of `delai_de_repli(self.tentative)`.
    pub fn delai_ms(&self) -> u64 {
        delai_de_repli(self.tentative)
    }

    /// Records an attempt REALLY launched (a WebSocket connection
    /// actually attempted, whether it succeeds or not).
    ///
    /// ⚠️ Counting here and not on failure: a handshake ACCEPTED then
    /// refused three milliseconds later by the guard is an `Ok` on the
    /// TCP side, and a counter incremented only on `Err` would stay stuck at
    /// zero — hence at a 500 ms spacing — in EXACTLY the case that
    /// hammers. It is the defect `surveillance_pont.rs` paid for on its
    /// successful `lancer_pont()`.
    pub fn tentative_lancee(&mut self) {
        self.tentative = self.tentative.saturating_add(1);
    }

    /// A connection has just ended after living `vecu_ms`.
    ///
    /// Re-arms the fallback — resets the counter — **if and only if**
    /// this connection lived at least [`SEUIL_CONNEXION_UTILE_MS`], that
    /// is, if it really served. Returns `true` in that case, so that
    /// the caller can say so in its trace.
    pub fn connexion_terminee(&mut self, vecu_ms: u64) -> bool {
        if vecu_ms < SEUIL_CONNEXION_UTILE_MS {
            return false;
        }
        self.tentative = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plateforme::repli::REPLI_MIN_MS;

    #[test]
    fn the_first_reconnection_waits_for_the_floor() {
        // A one-second outage must not cost thirty seconds of
        // mute desktop: the very first resumption starts from the floor shared
        // with the `/agent` channel.
        assert_eq!(Reprise::neuve().delai_ms(), REPLI_MIN_MS);
        assert_eq!(Reprise::neuve().tentative(), 0);
    }

    #[test]
    fn the_delay_grows_then_caps() {
        let mut reprise = Reprise::neuve();
        let mut precedent = reprise.delai_ms();
        reprise.tentative_lancee();
        assert!(
            reprise.delai_ms() > precedent,
            "the delay must grow: {} is not > {precedent}",
            reprise.delai_ms()
        );
        for _ in 0..40 {
            precedent = reprise.delai_ms();
            reprise.tentative_lancee();
            assert!(
                reprise.delai_ms() >= precedent,
                "the delay must never go back without re-arming"
            );
            assert!(
                reprise.delai_ms() <= REPLI_MAX_MS,
                "the delay {} exceeds the ceiling {REPLI_MAX_MS}",
                reprise.delai_ms()
            );
        }
        assert_eq!(
            reprise.delai_ms(),
            REPLI_MAX_MS,
            "the ceiling must be REACHED, not merely respected"
        );
    }

    /// 🔴 **THE TEST THAT MAKES THE PREVIOUS ONE STRUCTURAL.** Without this inequality,
    /// a refusal could reach the re-arming threshold and the fallback
    /// would become unable to grow — the exact defect
    /// `relance_pont.rs` paid for in its fix round 3.
    #[test]
    fn the_useful_connection_threshold_stays_strictly_above_the_backoff_ceiling() {
        const {
            assert!(
                SEUIL_CONNEXION_UTILE_MS > REPLI_MAX_MS,
                "SEUIL_CONNEXION_UTILE_MS must be > REPLI_MAX_MS"
            )
        };
    }

    /// The looping refusal scenario: the platform accepts TCP then
    /// closes at once (expired token, role already taken, budget exhausted). The
    /// fallback must grow up to its ceiling, never start again from the floor.
    #[test]
    fn a_refused_connection_never_re_arms_the_backoff() {
        // 🔴 **THE DURATION EXERCISED IS `REPLI_MAX_MS`, NOT THE 3 ms OF A REAL
        // REFUSAL, AND IT IS A FIX OF THIS VERY TEST.** Written first
        // with `3` — the measured order of magnitude of a `{"type":"error"}` followed
        // by a `close(1008)` —, it stayed **GREEN** under the mutation that makes
        // `SEUIL_CONNEXION_UTILE_MS` fall back to 500 ms, that is, under the
        // exact defect `relance_pont.rs` paid for in its round 3: the only
        // red then came from the neighbouring invariant test. A red that stayed
        // green gets DIAGNOSED, it does not get filed away.
        //
        // The retained duration comes from the PRODUCT — `REPLI_MAX_MS`, the
        // fallback's ceiling — and never from a calculation on what is judged: it is the worst case
        // a refusal episode can occupy, since nothing in this
        // loop waits longer than that ceiling before retrying.
        // No life of this length or less must re-arm.
        let mut reprise = Reprise::neuve();
        for tour in 0..20 {
            reprise.tentative_lancee();
            assert!(
                !reprise.connexion_terminee(REPLI_MAX_MS),
                "a refusal at round {tour}, even REPLI_MAX_MS long, must NEVER re-arm the backoff"
            );
        }
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
        assert_eq!(reprise.tentative(), 20);
        // …and the really measured case stays covered too.
        assert!(!reprise.connexion_terminee(3));
    }

    /// The symmetric one: a connection that really served (the nominal case —
    /// a control session lives for hours) gives its floor back to the fallback,
    /// so that the NEXT outage does not resume at the ceiling of an
    /// already resolved failure.
    #[test]
    fn a_connection_that_served_re_arms_the_backoff() {
        let mut reprise = Reprise::neuve();
        for _ in 0..10 {
            reprise.tentative_lancee();
        }
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
        assert!(reprise.connexion_terminee(SEUIL_CONNEXION_UTILE_MS));
        assert_eq!(reprise.tentative(), 0);
        assert_eq!(reprise.delai_ms(), REPLI_MIN_MS);
    }

    /// The boundary, on both sides: one millisecond less does not re-arm,
    /// the exact threshold re-arms. Without this test, a `>` comparison instead
    /// of `>=` would go unnoticed.
    #[test]
    fn the_threshold_boundary_is_tested_on_both_sides() {
        let mut juste_en_dessous = Reprise::neuve();
        juste_en_dessous.tentative_lancee();
        assert!(!juste_en_dessous.connexion_terminee(SEUIL_CONNEXION_UTILE_MS - 1));
        assert_eq!(juste_en_dessous.tentative(), 1);

        let mut au_seuil = Reprise::neuve();
        au_seuil.tentative_lancee();
        assert!(au_seuil.connexion_terminee(SEUIL_CONNEXION_UTILE_MS));
        assert_eq!(au_seuil.tentative(), 0);
    }

    /// An agent living for weeks behind a dead platform must neither
    /// panic on an overflow, nor see its delay fall back through
    /// wraparound. `saturating_add` and `delai_de_repli` cover both;
    /// this test freezes it.
    #[test]
    fn the_counter_never_overflows() {
        let mut reprise = Reprise {
            tentative: u32::MAX - 1,
        };
        reprise.tentative_lancee();
        reprise.tentative_lancee();
        assert_eq!(reprise.tentative(), u32::MAX);
        assert_eq!(reprise.delai_ms(), REPLI_MAX_MS);
    }
}
