//! **PROBATION**: a window must SURVIVE before deserving a tab.
//!
//! 🔴 **MEASURED ON THE PRODUCT ON AUGUST 31ST, 2026, AFTER THE OWNER
//! REPORTED "lots of tabs opened on me".** A single launch
//! of Steam served **23 windows in three minutes**, and the log gives
//! their lifetimes:
//!
//! ```text
//! w-4  21:18:00.668 -> 21:18:00.776   0,11 s
//! w-12 21:18:08.101 -> 21:18:08.235   0,13 s
//! w-19 21:20:37.437 -> 21:20:37.589   0,15 s
//! w-20 21:20:37.587 -> 21:20:37.694   0,11 s
//! w-26 21:20:39.596 -> 21:20:39.703   0,11 s
//! w-27 21:20:40.480 -> 21:20:40.589   0,11 s
//! ```
//!
//! These are the transient windows of Steam's startup. Each one opened
//! a pop-up in the browser, and the pop-up outlives the Windows window.
//!
//! 🔴 **IT IS NOT A DEFECT OF THE STATIC CRITERION, AND MEASUREMENT REFUTED IT
//! BEFORE IT WAS WRONGLY FIXED.** An inventory of Steam's windows in
//! session 1 (`GetWindowTextW`/`GetClassNameW`/styles, once Steam had
//! settled) returns **26 top-level windows of which ONLY ONE** satisfies
//! `fenetres::merite_une_fenetre` — the 25 others are already set aside, by
//! owner, by `WS_EX_TOOLWINDOW`, by empty title or by invisibility.
//! **Hardening this criterion would therefore have set aside legitimate windows without touching
//! the cause.** What distinguishes the false ones from the real ones is not what they
//! ARE at the instant they appear, it is that they **do not last**.
//!
//! Hence the rule: a window is no longer judged on its birth snapshot
//! alone, it is given probation and **asked again** afterwards. Repository
//! precedent: the debounce of `APPS_SURVEILLANCE` (G4).
//!
//! ⚠️ **`DUREE_SURSIS` IS NOT CALIBRATED.** It is a prudence safeguard,
//! chosen between the longest transient measured (0.15 s) and the delay a
//! human would notice when a window opens. **No judgement in use
//! has judged it**, and it joins `CLAUDE.md`'s list of uncalibrated
//! constants.

use super::table::IdFenetre;
use std::time::{Duration, Instant};

/// The time a window must survive before a tab is opened for it.
///
/// ⚠️ **Not calibrated** — see the header. The cost of choosing it too LARGE is
/// a perceived latency at opening; too SMALL, ghost tabs.
pub const DUREE_SURSIS: Duration = Duration::from_millis(500);

/// The windows waiting to prove that they last.
#[derive(Debug, Default)]
pub struct Sursis {
    attentes: Vec<(IdFenetre, String, Instant)>,
}

impl Sursis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts a window on probation.
    ///
    /// **Idempotent by `IdFenetre`**, like `Table::fenetre_apparue` which it
    /// precedes: the hook can re-emit `Apparue` for the same `HWND`, and a
    /// second deposit must neither duplicate the tab nor **push back the deadline**
    /// of the first (which would leave a chatty window on probation
    /// forever).
    pub fn deposer(&mut self, fenetre: IdFenetre, titre: String, maintenant: Instant) {
        if self.attentes.iter().any(|(f, _, _)| *f == fenetre) {
            return;
        }
        self.attentes
            .push((fenetre, titre, maintenant + DUREE_SURSIS));
    }

    /// Removes a window that disappeared before its deadline.
    ///
    /// Returns `true` if it was indeed on probation — that is, **if a tab
    /// was just avoided**, and that is what the caller logs.
    pub fn retirer(&mut self, fenetre: IdFenetre) -> bool {
        let before = self.attentes.len();
        self.attentes.retain(|(f, _, _)| *f != fenetre);
        self.attentes.len() != before
    }

    /// The windows whose probation has elapsed, removed from the waiting list.
    ///
    /// ⚠️ **Returning them is NOT enough to announce them**: the caller must still
    /// check that they STILL deserve a window (`hook::merite_encore`).
    /// A window can survive probation and meanwhile have lost its
    /// title, been cloaked by DWM, or received an owner.
    pub fn murs(&mut self, maintenant: Instant) -> Vec<(IdFenetre, String)> {
        let (murs, encore): (Vec<_>, Vec<_>) = self
            .attentes
            .drain(..)
            .partition(|(_, _, echeance)| *echeance <= maintenant);
        self.attentes = encore;
        murs.into_iter().map(|(f, t, _)| (f, t)).collect()
    }

    /// How many windows are waiting — for the trace, never to decide.
    #[cfg(test)]
    pub fn en_attente(&self) -> usize {
        self.attentes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: u64) -> IdFenetre {
        IdFenetre(n)
    }

    /// The production case: a window dead within 110 ms never reaches
    /// `murs`, hence opens no tab.
    #[test]
    fn a_window_dead_before_the_deadline_opens_no_tab() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "splash".into(), t0);
        assert!(s.retirer(f(1)), "it was indeed in its grace period");
        assert!(s.murs(t0 + DUREE_SURSIS).is_empty());
    }

    /// 🔴 **THE WITNESS THAT MAKES THE PREVIOUS TEST DISCRIMINATING**: without it,
    /// a structurally empty `murs` would pass for a working
    /// debounce.
    #[test]
    fn a_surviving_window_is_indeed_returned() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        let murs = s.murs(t0 + DUREE_SURSIS);
        assert_eq!(murs, vec![(f(1), "Steam".to_string())]);
    }

    /// One second too early, nothing comes out — and the window stays waiting,
    /// it is not lost.
    #[test]
    fn before_the_deadline_nothing_leaves_and_nothing_is_lost() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        assert!(s
            .murs(t0 + DUREE_SURSIS - Duration::from_millis(1))
            .is_empty());
        assert_eq!(s.en_attente(), 1);
        assert_eq!(s.murs(t0 + DUREE_SURSIS).len(), 1);
    }

    /// Idempotence, and above all: **the second deposit does not push back
    /// the deadline**. Otherwise, a window whose hook re-emits `Apparue`
    /// regularly would stay on probation indefinitely and would never
    /// appear.
    #[test]
    fn a_second_deposit_does_not_push_back_the_deadline() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "Steam".into(), t0);
        s.deposer(f(1), "Steam".into(), t0 + Duration::from_millis(400));
        assert_eq!(s.en_attente(), 1, "no duplicate");
        assert_eq!(
            s.murs(t0 + DUREE_SURSIS).len(),
            1,
            "the deadline is that of the FIRST deposit"
        );
    }

    /// Removing a window that was not expected does not lie: it is the case
    /// of an already announced window, whose disappearance is the `Table`'s business.
    #[test]
    fn removing_an_unknown_one_returns_false() {
        let mut s = Sursis::new();
        assert!(!s.retirer(f(42)));
    }

    /// Several windows, distinct deadlines: only the ripe ones
    /// come out, in the order they were deposited.
    #[test]
    fn only_the_ripe_ones_come_out() {
        let t0 = Instant::now();
        let mut s = Sursis::new();
        s.deposer(f(1), "tot".into(), t0);
        s.deposer(f(2), "tard".into(), t0 + Duration::from_millis(300));
        let murs = s.murs(t0 + DUREE_SURSIS);
        assert_eq!(murs, vec![(f(1), "tot".to_string())]);
        assert_eq!(s.en_attente(), 1);
    }
}
