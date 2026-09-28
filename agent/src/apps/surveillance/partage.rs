//! What the watch thread shares with the discovery loop: **two
//! monotonic counters and a stop flag**, and nothing else.
//!
//! 🔴 THIS MODULE HAS NO `cfg`, AND THAT IS THE POINT — it is the exact shape
//! `apps/installation/partage.rs` set down, and its header gives the
//! reason: *"the installation thread is Windows; what it shares with
//! discovery is not […]. It is also what makes these two mechanisms
//! observable on the host."* Likewise here: the thread is
//! `ReadDirectoryChangesW`, what it publishes is two integers.
//!
//! ⚠️ **TWO DISTINCT SHARED STATES, AND IT IS INTENDED.** `installation::partage`
//! carries the installation request; this one carries the file notification.
//! The two have neither the same cause, nor the same semantics, nor the same
//! writing consumer — one is written by a `tokio` task at the exit
//! of an installer, the other by a Windows thread at every I/O completion. Merging
//! them would make an object that lies about both.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// The watch counters, shared by cloning an `Arc`.
#[derive(Clone, Default)]
pub struct Veille {
    /// 🔴 **MONOTONIC, NEVER RESET.** The loop compares with the value
    /// it kept at the previous round; an increment occurring **during**
    /// a reconciliation is therefore seen at the next poll.
    ///
    /// **A swapped boolean would lose it**, and with it exactly the
    /// notification that matters: the one arriving while we are already
    /// reading the disk, that is during an installation.
    notifications: Arc<AtomicU64>,
    /// Monotonic too. It is what makes criterion ② readable **on the
    /// `catalogue reconcilie` line**, in addition to the per-occurrence `warn!`: a cumulative
    /// count carried by a periodic line is read after the fact, where an isolated
    /// `warn!` has to be searched for.
    debordements: Arc<AtomicU64>,
    /// Set once and for all at shutdown. The thread re-reads it at every wait
    /// round, which bounds the stop delay by the duration of the `Wait`.
    arret: Arc<AtomicBool>,
}

impl Veille {
    /// The cumulative number of notifications received since the thread started.
    pub fn notifications(&self) -> u64 {
        self.notifications.load(Ordering::Relaxed)
    }

    /// The cumulative number of buffer overflows since the thread started.
    pub fn debordements(&self) -> u64 {
        self.debordements.load(Ordering::Relaxed)
    }

    /// Something moved under one of the roots.
    pub fn signaler(&self) {
        self.notifications.fetch_add(1, Ordering::Relaxed);
    }

    /// The buffer overflowed: the content is lost, but **the event is
    /// not**.
    ///
    /// 🔴 BOTH COUNTERS GO UP, AND IT IS DELIBERATE. An overflow **IS**
    /// an event: the buffer overflows, `lpBytesReturned` is 0, and **the
    /// completion happens anyway**. Counting it as a notification is
    /// what triggers the reconciliation that repairs — and a reconciliation
    /// re-reads the WHOLE disk, so it catches up on everything the buffer threw away.
    ///
    /// 🔴 INCREMENTING ONLY `debordements` WOULD **OPEN** THE LOSS PATH
    /// THIS DESIGN EXISTS TO CLOSE: the only signal saying
    /// "something changed" would be consumed by a counter nobody
    /// polls to decide, and the repair would wait for the period.
    pub fn signaler_debordement(&self) {
        self.debordements.fetch_add(1, Ordering::Relaxed);
        self.notifications.fetch_add(1, Ordering::Relaxed);
    }

    /// Requests the thread to stop.
    ///
    /// 🔴 **THIS METHOD HAS NO PRODUCTION CALLER, AND IT IS DECLARED
    /// RATHER THAN HIDDEN** — it is the only `dead_code` warning
    /// G4 adds to the repository's twenty-two, and removing it for convenience
    /// would mask a fact instead of settling it.
    ///
    /// **The mechanism itself is ALIVE**: `fil::boucler` re-reads `arretee()` at
    /// every wait round, hence at most one second after it would be
    /// set. What is missing is its TRIGGER, and it is missing for a reason
    /// beyond this sub-block: **the agent has no clean shutdown
    /// path** — the discovery and installation threads are not stopped
    /// either, and `CLAUDE.md` has written since sub-block D1 that this path
    /// "has still never been exercised".
    ///
    /// ⚠️ Building it anyway is a choice, and the alternative was to
    /// leave `fil::boucler` without an exit condition. A thread that CANNOT
    /// stop is a thread nobody will know how to stop the day the path
    /// exists; this one waits for its caller, and `Drop for Racine` already closes
    /// the handles whatever the exit path.
    #[cfg(test)]
    pub fn arreter(&self) {
        self.arret.store(true, Ordering::Relaxed);
    }

    /// Has a stop been requested?
    pub fn arretee(&self) -> bool {
        self.arret.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_veille_neuve_est_a_zero_et_n_est_pas_arretee() {
        let v = Veille::default();
        assert_eq!(v.notifications(), 0);
        assert_eq!(v.debordements(), 0);
        assert!(!v.arretee());
    }

    /// 🔴 MONOTONIC: the loop compares with the value it KEPT, never with
    /// zero. A counter reset on read would lose any
    /// notification arriving during a reconciliation — that is, during
    /// an installation, the only time they arrive in bursts.
    #[test]
    fn les_notifications_sont_monotones_et_la_lecture_ne_consomme_rien() {
        let v = Veille::default();
        v.signaler();
        v.signaler();
        assert_eq!(v.notifications(), 2);
        assert_eq!(v.notifications(), 2, "reading must consume NOTHING");
        v.signaler();
        assert_eq!(v.notifications(), 3);
    }

    /// 🔴 AN OVERFLOW IS AN EVENT. If only `debordements` went up, nothing
    /// would trigger the reconciliation that repairs, and the discarded buffer would be
    /// a loss instead of a delay.
    #[test]
    fn un_debordement_monte_les_deux_compteurs() {
        let v = Veille::default();
        v.signaler_debordement();
        assert_eq!(v.debordements(), 1);
        assert_eq!(
            v.notifications(),
            1,
            "an overflow MUST fire too: that is what closes the loss path"
        );
        v.signaler();
        assert_eq!(v.notifications(), 2);
        assert_eq!(
            v.debordements(),
            1,
            "a simple notification is not an overflow"
        );
    }

    #[test]
    fn the_stop_is_seen_from_all_clones() {
        let v = Veille::default();
        let jumelle = v.clone();
        assert!(!jumelle.arretee());
        v.arreter();
        assert!(jumelle.arretee(), "the clones share the Arc, not a copy");
        // And the counters too: that is what lets the thread write and the
        // loop read without any channel linking them.
        jumelle.signaler();
        assert_eq!(v.notifications(), 1);
    }
}
