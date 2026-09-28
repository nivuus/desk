//! Watching the four shortcut roots: an ACCELERATOR, and nothing
//! else.
//!
//! 🔴 THIS MODULE ADDS NO FEATURE. The periodic reconciliation of
//! `apps::boucle` has existed since sub-block G1, it works, and **it
//! is never disarmed** in the shipped configuration. All the watch
//! sets up is a **trigger**: it brings the next reconciliation forward, it
//! does not replace it. The property that matters is therefore not "the watch
//! works" — it will work — but **"the watch can lose NOTHING"**,
//! and it is the only thing this sub-block seeks to make falsifiable.
//!
//! ⚠️ THIS FILE IS DECLARED WITHOUT `cfg` in `apps.rs`, AND IT IS ITS WINDOWS
//! CHILDREN THAT CARRY THEIRS. It is the shape of `apps::icone` and
//! `apps::installation`, one level down, and it is what makes
//! `apps::surveillance::mode`, `::rebond`, `::faute` and `::partage` exist on the Linux
//! host, where their tests run. The "Child module convention" of
//! `CLAUDE.md` is **not** invoked: no module crosses a
//! `#[cfg(windows)]` boundary here.
//!
//! ⚠️ **The tree of specification §6 puts `rebond.rs` NEXT TO
//! `surveillance.rs`, as a sibling.** This sub-block puts it BELOW, and the reason
//! is written in the header of `apps/icone.rs`: *a GRANDchild cannot be declared
//! from the grandparent without `#[path]`*. Following the spec to
//! the letter would force the `#[path]` that ④ has avoided since G2.

/// Watch fault injection. **PURE** + a global budget.
pub mod faute;
/// The four states of `APPS_SURVEILLANCE`. **PURE.**
pub mod mode;
/// The two monotonic counters and the stop flag. **NO `cfg`.**
pub mod partage;
/// The debounce, clock as a parameter. **PURE.**
pub mod rebond;

// ---------------------------------------------------------------------------
// What follows is `#[cfg(windows)]`: the handles, the wait and the
// completions. Nothing there DECIDES — the decisions live in the four modules
// above, and that is what makes them testable on the host.
// ---------------------------------------------------------------------------

/// The single thread: the wait, the N roots, the stop.
#[cfg(windows)]
mod fil;
/// A watched root: open, arm, complete, reopen.
#[cfg(windows)]
mod racine;

/// The buffer the kernel fills, **per root**, in bytes.
///
/// ⚠️ **NOT CALIBRATED, AND CHOSEN ON ITS MERIT.** 64 KiB is the cap the
/// documentation imposes for a network path and the one it recommends not
/// exceeding, the buffer being **locked in non-paged memory** — four
/// roots therefore make 256 KiB of pool.
///
/// 🔴 IT IS NOT CHOSEN TO MAKE A CRITERION MEASURABLE, AND IT MUST
/// NEVER BE. Shrinking it would make the buffer overflow more easily, hence
/// "pass" the criterion that asks to observe an overflow — that is,
/// tuning the product to its test.
///
/// 🔴 **AND THE VERDICT CAME IN: NOT MEASURABLE, AND IT IS WRITTEN AS SUCH.**
/// Gate S1 goes up to **60,000 files at 2,850/s** without a single
/// overflow (seven runs), and the burst replayed on the product yields
/// **96,742 then 96,328 real notifications for `debordements=0`** (two
/// runs). **The buffer was NOT shrunk**, and it must not be.
///
/// 🔵 **THE REASON IS ARITHMETIC, AND IT WILL OUTLIVE THIS MACHINE**: an
/// overflow requires more than ~1,260 events **between two re-arms**,
/// that is in the microseconds separating them. At 2,850 files per
/// second they arrive every ~350 µs. **The measured ceiling is that of the
/// FILE SYSTEM, not that of the buffer** — it is almost constant from 1,000 to
/// 60,000 files.
pub const TAMPON_NOTIFICATIONS: usize = 65_536;

/// Starts the watch thread, or returns an inert `Veille`.
///
/// ⚠️ THE HANDLE IS RETURNED TO BE **KEPT WITHOUT BEING AWAITED**: dropping
/// it would not end the thread — a dropped `JoinHandle` detaches —, but
/// `apps::Poignees` keeps it for symmetry with the two others, and because
/// it is what will one day make shutdown observable. **A clean stop goes
/// through `Veille::arreter`, never through the handle.**
///
/// ⚠️ `Mode::Desarmee` LOGS NOTHING HERE: the mode's `info!` is emitted by
/// the caller, **unconditionally**, which is better than a trace emitted
/// only by the branch that disarms.
#[cfg(windows)]
pub fn demarrer(mode: mode::Mode) -> (partage::Veille, Option<std::thread::JoinHandle<()>>) {
    let veille = partage::Veille::default();
    if !mode.surveille() {
        return (veille, None);
    }
    let pour_le_fil = veille.clone();
    let poignee = std::thread::Builder::new()
        .name("surveillance-apps".into())
        .spawn(move || fil::tourner(pour_le_fil))
        .map_err(|erreur| {
            // The thread does not start: discovery remains ENTIRELY
            // functional, at its period. That is exactly what "G4
            // adds no feature" means, and the trace says so
            // rather than letting it read as a discovery failure.
            tracing::error!(
                %erreur,
                "fil de surveillance non démarré : la réconciliation périodique reste la source de vérité"
            );
        })
        .ok();
    (veille, poignee)
}

/// The non-Windows variant: an inert `Veille`, **and nothing to log**.
///
/// Same reason as `apps::demarrer`: a Linux agent has no business complaining about
/// not watching Windows shortcuts, and confusing it with an explicit
/// disarm — which, for its part, SAYS it is disarmed — would blur two
/// distinct states.
#[cfg(not(windows))]
pub fn demarrer(_mode: mode::Mode) -> (partage::Veille, Option<std::thread::JoinHandle<()>>) {
    (partage::Veille::default(), None)
}
