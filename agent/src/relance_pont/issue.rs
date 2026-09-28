//! The two OBSERVATION types the supervisor passes to
//! [`super::EtatRelance`]: the outcome of a dead process, and what a loop turn
//! sees of the process.
//!
//! **Extracted from `relance_pont.rs` on August 25th, 2026, IN A DEDICATED COMMIT AND
//! BEFORE the addition that made it necessary** — the 500-line rule of
//! `CLAUDE.md`, and its strong form: "the strong form is the extraction played
//! in a DEDICATED task, BEFORE the one that adds". The parent was at 399 and
//! round 5's addition would have taken it to 499, a margin of ONE line,
//! which this repository has seen lost again six times.
//!
//! ⚠️ **`CLAUDE.md`'s "child module convention" DOES NOT APPLY
//! HERE, and it is checked rather than assumed**: it only targets modules
//! extracted from a `#[cfg(windows)]` parent to make them compile on the
//! host. `relance_pont` is portable end to end, this file too;
//! it is an ordinary child, declared by a `mod issue;` inside
//! its parent, without `#[path]` and without a name prefix.

/// What a supervised process leaves behind when dying — **the
/// discriminant for re-arming the fallback since fix round 4**, in
/// place of a lifetime that did not distinguish a healthy session from a
/// sleeping refusal (see the header doc of [`super`], module `relance_pont`).
///
/// The type is PURE: it knows neither `std::process::ExitStatus`, nor Windows,
/// nor `tracing`. The conversion from the exit code is
/// [`IssueDeSortie::depuis_le_code`], and it is the only point of contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueDeSortie {
    /// Exit code **0**: the process finished its work and
    /// stopped normally. For the bridge, it is the `Ok(())` of
    /// `pont::executer` — the shell page closed its session.
    Propre,
    /// **Non-zero** exit code: `bail!`, panic, `exit(n)`. For the
    /// bridge, it is the relay's refusal, honoured then propagated.
    Erreur,
    /// **No code is available**: the process was killed by a signal
    /// (POSIX), or the state could not be read.
    Inconnue,
}

impl IssueDeSortie {
    /// From the exit code, as `std::process::ExitStatus::code()`
    /// returns it — `None` when there is none.
    ///
    /// 🔴 **`None` BECOMES `Inconnue`, AND `Inconnue` DOES NOT RE-ARM.** It is
    /// the SAFE direction, and here is why: the cost of the two errors is not
    /// symmetric. Re-arming wrongly reopens the defect this round closes — the
    /// hammering at 100 connections/minute against a shared budget of 120,
    /// that is, the **locking of the whole VM**, no new window
    /// being able to attach any more. NOT re-arming wrongly costs, at
    /// worst, a healthy reconnection delayed by `REPLI_MAX_MS` (30 s) once
    /// — a bounded discomfort, on a service framing §4 declares
    /// OPTIONAL and whose failure never touches the video stream.
    ///
    /// ⚠️ **On the real target, this case does not run**: Windows always
    /// returns an exit code, `ExitStatus::code()` being `Some(_)` there
    /// even for a `TerminateProcess`. `Inconnue` covers the POSIX host (where
    /// `relance_pont` compiles and is tested) and the future — it is shipped, exercised,
    /// and **never exercised in production**: it is said rather than assumed.
    pub fn depuis_le_code(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Propre,
            Some(_) => Self::Erreur,
            None => Self::Inconnue,
        }
    }

    /// Does this outcome prove that a past failure is RESOLVED, hence that the
    /// exponential fallback can start again from its floor?
    pub fn prouve_une_panne_resolue(self) -> bool {
        matches!(self, Self::Propre)
    }
}

/// What the supervisor OBSERVES of a process at a loop turn.
///
/// 🔴 **`Mort` IS RETURNED ONLY ONCE PER DEATH**, and all of `relance_pont`
/// depends on it — see the header doc of [`super`]: the property lives in the
/// `#[cfg(windows)]` wiring (`etat_du_pont` sets `*pont = None` when observing the
/// death), hence out of reach of `cargo test --workspace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatObserve {
    /// The process runs — or its state is unreadable and **held to be
    /// alive**, which is `etat_du_pont`'s decision (two bridges
    /// contending for the same ProjFS root cost more than a lost turn).
    Vivant,
    /// The process has just been seen dead, with this outcome.
    Mort(IssueDeSortie),
    /// No process: never launched (a failed `spawn`), or death already
    /// observed at a previous turn. **Re-arms nothing** — a `spawn` that
    /// fails in a loop must see its fallback grow, it is the very case
    /// round 1's critical ③ measured.
    Absent,
}
