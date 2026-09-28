//! The payload types of the INSTALLATION of an uploaded piece of software.
//!
//! They live in a sibling module of `apps`, for the same reason and by the
//! same mechanism: the 500-line rule of `CLAUDE.md`, and not the
//! "Child module convention", which targets modules extracted from a
//! `#[cfg(windows)]` parent.

use serde::{Deserialize, Serialize};

/// Where an installation stands.
///
/// ⚠️ **THE `empreinte` PHASE IS NOT HERE, AND IT IS NOT AN OVERSIGHT.** It
/// takes place in the BROWSER, before the platform has the slightest row to
/// write: it never crosses this channel, which goes from the agent to the
/// platform. The hub knows it because it lives it.
///
/// 🔴 **`Execution` CARRIES NO PERCENTAGE**, and it is a decision, not a
/// gap: a Windows installer does not publish one. Inventing one would be
/// lying to the user about a progress nobody measures. It
/// carries the ELAPSED TIME, and the interface shows an indeterminate state.
///
/// ⚠️ **NO VARIANT OF THIS ENUM HAS TWO WORDS, SO ITS `rename_all` IS
/// UNOBSERVABLE**, and it must be said rather than letting one believe the
/// opposite. Sub-block G1 MEASURED the gap: switching `kebab-case` to
/// `snake_case` on `IssueLancement` — four single-word variants — leaves
/// `cargo test -p proto` entirely green. **The G3 plan prescribed running
/// this red ON `Phase`, citing `sans-effet`: it is a contradiction of
/// its own text** — `sans-effet` belongs to [`Issue`], and `Phase` has
/// no two-word variant. The red is therefore run on [`Issue`], which has
/// two, and **we did NOT invent a fourth phase to make a mutation
/// observable**: that would add to the product a state it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// The bytes are going down from the platform to the VM.
    Transfert,
    /// The installer is running. No percentage — see above.
    Execution,
    /// The installer has exited; we are waiting for the reconciliation that closes the
    /// counting window.
    Reconciliation,
}

/// What an installation produced.
///
/// 🔴 **THE EXIT CODE DOES NOT ENTER THIS DECISION**, and it is the rule
/// that governs the whole sub-block: `msiexec` returns **3010** for a success that
/// requests a reboot, and many installers return **0** after a
/// cancellation. A product that judged by the code would be wrong in both
/// directions. The code is REPORTED, next to the outcome; it does not decide it.
///
/// ⚠️ **`Refusee` IS AN ADDITION TO THE SPECIFICATION**, which names only
/// three. The four cases it covers — wrong fingerprint, elevation required,
/// refused extension, process assigned to a job object — are neither a success,
/// nor a "no effect", nor an ignorance: **they are refusals, and they carry
/// their reason**. Melting them into `IssueInconnue` would read "we do not know"
/// where we know very well.
///
/// ✅ **TWO OF ITS FOUR VARIANTS HAVE TWO WORDS**, and it is deliberate: it is
/// what makes the `rename_all` of this enum OBSERVABLE, hence what closes the
/// gap that G1 measured without being able to make it red (its legacy no. 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Issue {
    /// The counting window saw **at least one** application appear.
    Reussie,
    /// The window closed at **zero**. It is the case of a cancelled installer.
    SansEffet,
    /// The exit code could not be collected: agent dead during
    /// execution, or timeout. **We do not know**, and we say so.
    IssueInconnue,
    /// The installation never started, and the `motif` field says why.
    Refusee,
}
