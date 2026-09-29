//! The PURE restart/stability decision of a supervised but **non-fatal**
//! process — extracted from `superviseur::boucle::surveillance_pont` (fix
//! round 2, August 25th, 2026) to compile and be tested **on the Linux
//! host**: that latter file lives behind `boucle.rs::#![cfg(windows)]`,
//! and `EtatPont` read `Instant::now()` hard-coded there — none of the three tests
//! of fix round 1 could therefore cover the WIRING, only the
//! function `honour_suggested_retry` taken in isolation. The review measured it:
//! reverting `surveillance_pont.rs` to the earlier bug (fixed constant) or removing
//! both calls to `honour_suggested_retry` left BOTH `cargo test
//! --workspace` and `cargo check --target x86_64-pc-windows-gnu` intact.
//!
//! 🔴 **THIS ZERO IS NOT AN INEVITABILITY OF `#[cfg(windows)]` — IT IS A
//! TRADE-OFF, AND THE REVIEW OF FIX ROUND 3 POINTED IT OUT.** The
//! paragraph above read as if no other split were
//! possible; there is one: `surveiller` (`surveillance_pont.rs`) is
//! a THREE-branch decision — alive, dead-with-an-outcome, absent — that
//! could be written as a PURE function returning a VERDICT, leaving
//! in the gated file only some fifteen lines of I/O (read the
//! process state, read the clock, apply the verdict, trace). **Not done, neither
//! in round 3 nor in round 4** — the prescription each time bears on the
//! MEANING of the decision, not on the extraction boundary. **WHAT THIS CHOICE
//! LEAVES UNGUARDED, AND ROUND 4 ADDS A PIECE TO IT**: the WIRING
//! itself (which method is called, in which branch, with which
//! argument) stays `#[cfg(windows)]`, hence not exercised by
//! `cargo test --workspace` — and it now carries a property on which ALL of
//! this module depends: **[`EtatObserve::Mort`] is returned ONLY ONCE per
//! death**. It holds because `LanceurDeProcessus::etat_du_pont` sets
//! `*pont = None` in the `Ok(Some(code))` branch of `try_wait`, so that
//! a second call returns `Absent` and not a second `Mort` — returning `Mort`
//! at each turn would re-arm the fallback in a loop on an OLD clean
//! exit. Only the DECISION, once the observation is known, is exercised
//! here.
//!
//! 🔴 **NAMING CONVENTION (`docs/claude/module-conventions.md`, "Child module convention"),
//! APPLIED HERE, NOT GUESSED.** This module carries the prefix of NO existing
//! top-level module: `relance` is declared nowhere in
//! `main.rs` (`grep -n '^mod \|^pub mod ' agent/src/main.rs` returns no
//! `mod relance;`), so `relance_pont` does not satisfy the `<parent>_
//! <child>` form for ANY top-level `<parent>` — including `pont`
//! itself, although it is a TEXTUAL prefix of the name: the rule requires
//! the prefix to be `<parent>_`, that is, the name must START with
//! `pont_`, which `relance_pont` does not. It therefore lives at the BARE ROOT,
//! an ordinary `mod relance_pont;` in `main.rs`, exactly like
//! `survie_verdict` (extracted four levels down, without a short and
//! unique parent to prefix) and for the SAME reason `surveillance_pont` carries
//! its own name rather than `pont`: both avoid the confusion that
//! the header of `surveillance_pont.rs` names explicitly — "two
//! `pont`s in the same module graph would only wait for a hurried reader
//! to be confused". A `pont_relance`, for its part, would have satisfied the
//! form and would have gone under `pont/relance.rs` — deliberately rejected: this
//! module describes nothing of the BRIDGE itself, it describes a generic
//! SUPERVISION policy (spaced restart, with a stability threshold), as
//! indifferent to the bridge as to any other non-fatal process
//! a supervisor might one day want to follow the same way.
//!
//! 🔴 **WHAT THIS MODULE FIXES, AND WHAT JUSTIFIED THE EXTRACTION**: before
//! round 2, the STABILITY threshold ("has the bridge lived long enough for a
//! future death to be new information?") was THE SAME as the FLOOR
//! spacing between two attempts — 500 ms — a shortcut that held as long
//! as a refused bridge died within a few milliseconds. The fix for
//! critical ② of fix round 1 (`agent::signaling::
//! honour_suggested_retry`) changed that premise: a refused bridge now stays
//! **alive** (the process runs) while it honours the delay
//! suggested by the relay, up to `plateforme::repli::REPLI_MAX_MS` (30 s).
//! **A bridge alive for 500 ms can therefore be DYING
//! SLOWLY, not stable** — and confusing them reopens exactly the
//! trace loop that commit `7a00fcb` of this branch paid for a
//! first time on a neighbouring mechanism ("my own remedy turned the
//! trace into a loop"): a "bridge stable again" line followed by a "bridge
//! restarted" line, in a loop, at each refusal cycle.
//!
//! `SEUIL_STABILITE_MS`, used by [`EtatRelance::stable`], is therefore
//! STRICTLY GREATER than `REPLI_MAX_MS` — with a margin for the time
//! the bridge needs to REACH that sleep (WS handshake, refusal,
//! reading the message) — so that a bridge still dying during its
//! waiting sleep is NEVER declared stable in the meantime.
//!
//! 🔴 **FIX ROUND 4: RE-ARMING THE FALLBACK IS NO LONGER JUDGED ON
//! A DURATION, BUT ON THE EXIT OUTCOME — AND THAT IS WHAT CLOSES BOTH
//! DEFECTS AT ONCE.** Round 3 re-armed `tentative` as soon as a life
//! exceeded `ESPACEMENT_PLANCHER_MS` (500 ms), which **contradicted the
//! paragraph above in the same file**: a 500 ms life proves
//! nothing, since it is precisely the shape of a sleeping refusal. `tentative`
//! fell back to zero ~500 ms after EACH launch, and the fallback could
//! **structurally no longer grow** for any failure mode where the bridge lives
//! between ~0.5 s and ~35 s — that is, exactly the regime round 1's remedy
//! created. **Measured on the bench** (`/signal` connections per minute against
//! the SHARED budget `REQUETES_MAX_ADRESSE`, 120 per 60 s):
//!
//! | bridge life | round 3 | round 4, death in ERROR | round 4, CLEAN exit |
//! | --- | --- | --- | --- |
//! | 600 ms | **100** | 6 | **100** |
//! | 1 s | **60** | 6 | **60** |
//! | 2 s | **30** | 6 | **30** |
//! | 5 s | **12** | 6 | **12** |
//! | 10 s | 6 | 6 | 6 |
//!
//! ⚠️ **The 600 ms line was reached WITHOUT ANY `retryApresS`**: any
//! repeated death after half a second of life (ProjFS failure, crash) was enough —
//! 100 connections/minute, 83% of the shared budget consumed by the bridge alone.
//!
//! 🔴 **THE THIRD COLUMN IS AN OPEN CAVEAT, AND IT LIVES HERE BECAUSE
//! A PROOF THAT ONLY LIVES IN A GITIGNORED REPORT IS A LOST
//! PROOF** (`CLAUDE.md` forbids it by name, after losing six of them).
//! **A CLEAN exit re-arms the fallback without any cadence bound**: a
//! bridge that ended cleanly every 600 ms would still return
//! 100 connections/minute, and the 500 ms floor of
//! [`EtatRelance::doit_relancer`] would then be the ONLY bound. This regime is
//! out of reach of the supervisor loop alone — `pont::executer` only
//! returns `Ok(())` after its transport thread dies, which requires a
//! peer emitting offers, and that peer itself consumes the `/signal`
//! budget. ⚠️ **But the argument is weaker than it looks**: this
//! `Ok(())` ALSO runs on an ICE failure occurring AFTER the SDP answer
//! was sent, not only after a really established session. **Not measured,
//! not bounded, and written here rather than passed over in silence.**
//!
//! **It was NOT a threshold tuning, and one must not go back to it**:
//! a HEALTHY session that ends occupies the SAME interval (1 to 30 s)
//! as a refused bridge that slept. LONG threshold ⇒ the defect round 3
//! fixed (a healthy bridge with short sessions never re-arms, up to 29 s
//! of unavailability for an unrelated future failure); SHORT threshold ⇒ the
//! defect above.
//!
//! ⚠️ **A *SHORT* DURATION DOES NOT DISCRIMINATE — AND THE FIRST DRAFT OF
//! THIS PARAGRAPH WROTE "duration is not the discriminant", WHICH IS
//! TOO STRONG** (pointed out by the review of fix round 5). A duration
//! **above `REPLI_MAX_MS`** discriminates perfectly, and this module
//! has one: `SEUIL_STABILITE_MS` (35 s) is **structurally
//! unreachable by a sleeping refused bridge**, since that sleep is bounded
//! at 30 s — it is the invariant that
//! `the_stability_threshold_stays_strictly_above_the_backoff_ceiling`
//! and `stable_during_a_refusal_sleep_never_declares_stable` already
//! hold. Round 4 had removed duration **wholesale**, taking with it the only case
//! it handled right; round 5 gives it back, and **`stable()` resets
//! `tentative` to zero** (see its doc).
//!
//! 🔵 **THE DISCRIMINANT WAS ALREADY READ, THEN THROWN AWAY.** `pont::executer` returns
//! `Ok(())` on a normal end and `bail!` on refusal — right after honouring
//! `retryApresS` —, and `main() -> Result<()>` translates one into exit code
//! **0** and the other into a **non-zero** code; the former `pont_vivant()` received that
//! code in `Ok(Some(code))` **to log it and drop it**.
//! It now crosses the boundary in the form of an [`IssueDeSortie`],
//! and **it is that, never a duration, that re-arms the fallback**: a healthy
//! session that ends re-arms, a refused bridge dying in error does not re-arm,
//! whatever its lifetime was.
//!
//! 🔵 **WHAT ROUND 4 GIVES BACK TO `cycle_signale` AND TO `SEUIL_STABILITE_MS`.**
//! Round 3 had made `cycle_signale` a guard of
//! [`EtatRelance::reset_the_backoff`], hence a CADENCE governor —
//! a THIRD role its doc did not name, and the exact shape of the defect
//! that round fixed. The guard disappeared with duration: it
//! was no longer merely unnamed, it had become **wrong**, a bridge
//! declared stable then dying cleanly having `cycle_signale == false` and thus
//! re-arming nothing. `cycle_signale` once again governs ONLY the trace.
//!
//! 🔴 **ON THE OTHER HAND, `SEUIL_STABILITE_MS` GOVERNS A CADENCE AGAIN
//! SINCE ROUND 5, AND IT IS WRITTEN HERE RATHER THAN DISCOVERED** — round 4
//! claimed at this spot that it governed "only the trace, without
//! reservation", and that sentence would have silently become wrong. The
//! difference from round 3, which had paid for exactly this mechanism: the
//! threshold governing this cadence is **above `REPLI_MAX_MS`**,
//! hence out of reach of a sleeping refusal, whereas round 3's was the
//! 500 ms floor. See the constant's doc.

use crate::plateforme::repli::{delai_de_repli, REPLI_MAX_MS};

/// FLOOR spacing between two attempts, AND value of the first term of
/// `delai_de_repli` (`delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, exercised
/// below). Taken over from the former `PERIODE_RELANCE_PONT_MIN`.
///
/// 🔴 **IT HAS TWO ROLES, AND HERE ARE BOTH — THE SECOND WAS NOT
/// NAMED BEFORE FIX ROUND 4** (the review pointed it out: the doc
/// listed two and the code served three, one of which — the guard of
/// re-arming the fallback — disappeared with round 4, see the header doc).
///
/// 1. **The FLOOR cadence of attempts**: `EtatRelance::doit_relancer`
///    only returns true beyond `delai_de_repli(tentative)`, of which it is the
///    first term.
/// 2. **The PULLBACK of `derniere_tentative` in `EtatPont::start`**
///    (`surveillance_pont.rs`): the clock there is pulled back by exactly this
///    value so that the VERY FIRST attempt happens now and not
///    in 500 ms. That role is not a cadence, it is a primer — and
///    it ties the two files: raising this constant would delay the
///    bridge's startup by as much, which cannot be read here.
///
/// ⚠️ **IT IS WELDED TO `plateforme::repli::REPLI_MIN_MS` BY A TEST**
/// (`the_first_spacing_equals_the_floor`, which requires
/// `delai_de_repli(0) == ESPACEMENT_PLANCHER_MS`, that is,
/// `REPLI_MIN_MS == ESPACEMENT_PLANCHER_MS`): **it therefore CANNOT be
/// raised alone**. Raising it requires raising `REPLI_MIN_MS` — which also governs
/// the `/agent` channel's resumption — or breaking this weld
/// deliberately. Neither is calibrated.
pub const ESPACEMENT_PLANCHER_MS: u64 = 500;

/// STABILITY threshold — see the module header comment. **Distinct from
/// `ESPACEMENT_PLANCHER_MS` since round 2**, and that is the whole fix:
/// confusing the two with `REPLI_MAX_MS` in play reopens the trace loop.
///
/// `REPLI_MAX_MS` covers the SLEEP `honour_suggested_retry` imposes on itself;
/// the margin covers the time it takes to REACH it (WS connection,
/// refusal, reading the error message) — not measured, chosen wide rather than
/// tight.
///
/// 🔴 **THIS THRESHOLD GOVERNS THREE THINGS, AND THE THIRD IS A CADENCE.**
/// Rounds 3 and 4 wrote in turn that it governed "only the
/// trace" — both times it was wrong, and not in the same way. Here are the
/// three, listed rather than discovered:
///
/// 1. **TRACE** — the moment the "bridge stable again" line can
///    come out;
/// 2. **TRACE** — the moment `cycle_signale` falls back, hence when a hammering
///    episode becomes noisy again on its NEXT launch;
/// 3. 🔴 **CADENCE, since fix round 5 — AND ITS ORIGINAL
///    JUSTIFICATION, REFUTED BY MEASUREMENT, IS CORRECTED HERE BY THE FINAL
///    REVIEW.** [`EtatRelance::stable`] resets `tentative` to zero in its
///    true branch. **It is NOT because, without this line, resuming
///    after an outage would wait 30 s**: `doit_relancer` compares
///    the time elapsed since the LAUNCH, so three days of life exceed any
///    fallback and the FIRST resumption is immediate in both cases.
///    **Measured at the loop level** (6 refusals → three days of life → outage
///    in error, restarts AFTER the outage): without the line, 0 / 30,000 /
///    60,000 / 90,000 ms; with it, 0 / 1,000 / 3,000 / 7,000 ms. **What
///    the line really changes: the RAMP of the NEXT refusal
///    episode starts again from the floor instead of resuming at the ceiling of an
///    already resolved failure** — useful, but "in half a second and not in
///    thirty" is **never** observed: it is the second attempt, not the
///    first, that this line shortens.
///
/// ⚠️ **WHY THIS IS NOT ROUND 3'S DEFECT, WHICH WAS EXACTLY THIS
/// MECHANISM.** There, the cadence was governed by a 500 ms threshold,
/// which a refused bridge ALWAYS reaches before dying: re-arming was
/// therefore systematic and the fallback could no longer grow. Here, the threshold is
/// `REPLI_MAX_MS + 5 s` = 35 s, and `honour_suggested_retry`'s sleep is
/// bounded at 30 s: **a refused bridge STRUCTURALLY cannot reach it**
/// — it is the invariant that
/// `the_stability_threshold_stays_strictly_above_the_backoff_ceiling`
/// sets, and that `stable_during_a_refusal_sleep_never_declares_stable`
/// exercises tick by tick. **Cost measured on the bench: ZERO** — the hammering
/// table of the header doc does not move by a single unit, for any lifetime
/// less than or equal to `REPLI_MAX_MS`.
///
/// ⚠️ **CONSEQUENCE: "a too-long trace threshold only costs a late `info!`"
/// BECOMES WRONG AGAIN.** Raising it beyond the real lifetime
/// of healthy sessions would make role ③ unreachable, and resuming after
/// a network failure would go back to 30 s.
pub const SEUIL_STABILITE_MS: u64 = REPLI_MAX_MS + 5_000;

/// The two observation types — [`IssueDeSortie`] and [`EtatObserve`] —
/// have lived in their own file since August 25th, 2026: see its header
/// for the reason (500-line rule, extraction played BEFORE the addition
/// that made it necessary). **Re-exported here**, so that no
/// caller changes path: `crate::relance_pont::IssueDeSortie` stays
/// valid.
mod issue;
pub use issue::{EtatObserve, IssueDeSortie};

/// The PURE state of a supervised process restarted with exponential fallback.
///
/// Knows neither PID, nor wall clock, nor `LanceurDeProcessus`: those
/// timestamps and that I/O stay in `surveillance_pont.rs`, which receives
/// its decisions from here as ELAPSED milliseconds and outcomes —
/// it is what makes this structure exercisable on the Linux host
/// (`#[cfg(windows)]` gates nothing here).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EtatRelance {
    /// CONSECUTIVE attempts without the fallback re-armed. See `tentative()`.
    tentative: u32,
    /// True as soon as a restart cycle in progress has been signalled — see the doc
    /// of `cycle_signale` in `surveillance_pont.rs`, which stays the only one
    /// responsible for the trace itself (this module logs nothing).
    ///
    /// ⚠️ **IT AGAIN GUARDS A RESET OF `tentative` SINCE
    /// ROUND 5, AND IT IS NAMED HERE RATHER THAN DISCOVERED** — it is the
    /// form defect the round 4 review had pointed out on this field, and it will not
    /// be paid twice. [`EtatRelance::stable`] requires `cycle_signale`
    /// BEFORE resetting `tentative` to zero, so this boolean does
    /// indeed condition a cadence. **It can however never BLOCK it**, by
    /// an invariant that reads on the field's only three writes:
    /// `tentative` grows ONLY in `tentative_lancee`, which sets
    /// `cycle_signale = true` in the same gesture; and both resets
    /// leave `tentative` at zero. So **`tentative > 0` implies
    /// `cycle_signale == true`**, and the guard can only refuse a
    /// re-arming that would have nothing to re-arm. Exercised by
    /// `a_really_stable_process_ends_up_declared_stable_once`.
    cycle_signale: bool,
}

impl EtatRelance {
    pub fn neuve() -> Self {
        Self {
            tentative: 0,
            cycle_signale: false,
        }
    }

    /// The number of consecutive attempts — to attach it to the
    /// caller's traces (`tentative = self.relance.tentative()`), never to
    /// decide anything here.
    pub fn tentative(&self) -> u32 {
        self.tentative
    }

    /// The spacing expected before the NEXT attempt, in milliseconds —
    /// a mere rewrapping of `delai_de_repli(self.tentative)`.
    pub fn espacement_ms(&self) -> u64 {
        delai_de_repli(self.tentative)
    }

    /// Is it time to restart, knowing that `ecoule_ms` milliseconds have
    /// elapsed since the last attempt?
    pub fn doit_relancer(&self, ecoule_ms: u64) -> bool {
        ecoule_ms >= self.espacement_ms()
    }

    /// Records an attempt REALLY launched (a `Command::spawn`
    /// performed, whether it succeeds or not — see the doc of `tentative` in
    /// `surveillance_pont.rs` for the reason: it is the case where `spawn`
    /// succeeds and the process dies right away that must make this
    /// counter grow). Returns `true` if it is the FIRST launch of the current
    /// cycle — it is what must govern the emission of a trace line
    /// at the caller, never a second launch of the same cycle.
    pub fn tentative_lancee(&mut self) -> bool {
        self.tentative = self.tentative.saturating_add(1);
        let premier_du_cycle = !self.cycle_signale;
        self.cycle_signale = true;
        premier_du_cycle
    }

    /// Re-arms the exponential fallback — resets `tentative` to zero — **if and
    /// only if the dead process's outcome proves that a past failure is
    /// resolved**, that is, on a CLEAN exit. Takes NO duration,
    /// and that is the whole of fix round 4: see the header doc for the
    /// measurement that required it.
    ///
    /// 🔴 **RESTORES AN ARGUMENT FIX ROUND 2 DELETED WITHOUT
    /// RELOCATING IT** (pointed out by the review of fix round 3).
    /// Before this module was extracted, this text lived on the reset
    /// of `EtatPont::tentative`, in `surveillance_pont.rs`:
    ///
    /// > "RESET HERE, AND NOWHERE ELSE: it is what makes
    /// > a FUTURE failure start again from the minimal spacing rather than
    /// > stay stuck at the ceiling reached by a PAST, already
    /// > resolved failure […] an agent connected for three days that loses its
    /// > network for one second must resume in half a second, not in
    /// > thirty."
    ///
    /// 🔴 **THIS METHOD ALONE DOES NOT HOLD THIS QUOTE — AND ROUND
    /// 4 CLAIMED THE OPPOSITE RIGHT HERE** ("the property still holds,
    /// but its PROOF has changed"). **It did not hold**: "losing one's
    /// network" is a death IN ERROR, not a clean exit. Scenario
    /// simulated by the round 5 review: six refusals (spacing at the ceiling),
    /// then a seventh launch that SERVES FOR THREE DAYS, then a network
    /// outage — the fallback stayed at **30,000 ms**, exactly the defect
    /// round 3 existed to fix, reopened for the dead-in-error case.
    ///
    /// ⚠️ **THIS "30,000 ms" IS A NOMINAL `espacement_ms()`, NOT A
    /// WAIT THE SUPERVISOR IMPOSES** — see the loop-level measurement
    /// in the doc of [`SEUIL_STABILITE_MS`] (point 3): the FIRST resumption
    /// after an outage is immediate in both cases (`doit_relancer`
    /// compares the time elapsed since the LAUNCH), and it is the RAMP of the
    /// FOLLOWING resumptions that this reset corrects.
    ///
    /// **The quote is now held by TWO gates, and it takes
    /// two**: this one, `IssueDeSortie::Propre`, for a short session
    /// that ends well; and [`Self::stable`], which resets `tentative` to
    /// zero as soon as a life exceeds `SEUIL_STABILITE_MS` (35 s), for a
    /// long life that ends BADLY. **It is therefore no longer the only reset
    /// of the module**, and saying it wrongly cost a defect.
    ///
    /// 🔴 **NO GUARD ON `cycle_signale`, AND IT IS DELIBERATE.** Round 3
    /// set one; it would have become WRONG here: a bridge declared
    /// stable (hence `cycle_signale == false`) then ended cleanly would
    /// re-arm nothing, and the next failure would inherit a ceiling. It
    /// also made `cycle_signale` a CADENCE governor, which
    /// its doc denies. ⚠️ **`SEUIL_STABILITE_MS`, FOR ITS PART, DOES GOVERN ONE
    /// SINCE ROUND 5** — but through `stable`, on a threshold out of reach
    /// of a sleeping refusal, and its doc lists it rather than denying it.
    ///
    /// 🔴 **DOES NOT REOPEN THE TRACE LOOP**: this method never touches
    /// `cycle_signale`, so can never make `tentative_lancee` return `true`
    /// prematurely — it is `cycle_signale`, never
    /// `tentative`, that governs the silence of traces.
    pub fn reset_the_backoff(&mut self, issue: IssueDeSortie) {
        if issue.prouve_une_panne_resolue() {
            self.tentative = 0;
        }
    }

    /// The process is seen ALIVE for `ecoule_ms` milliseconds elapsed
    /// since the last attempt. Returns `true` — and RE-ARMS `cycle_signale`
    /// for the next cycle, AND resets `tentative` to zero (see below) —
    /// IF AND ONLY IF a cycle was in progress AND `ecoule_ms` exceeds
    /// `SEUIL_STABILITE_MS`, **never** `ESPACEMENT_PLANCHER_MS` alone.
    ///
    /// 🔴 **IT IS THE LINE THAT FIXES FIX ROUND 2**: before it,
    /// the threshold here was `ESPACEMENT_PLANCHER_MS` (500 ms), so that a
    /// refused process asleep up to `REPLI_MAX_MS` (30 s) before
    /// dying was declared stable from 500 ms — see
    /// `stable_during_a_refusal_sleep_never_declares_stable`, the
    /// exact red of that defect, below.
    ///
    /// 🔴 **AND IT RESETS `tentative` TO ZERO, SINCE FIX ROUND
    /// 5 — THIS LINE CLOSES A DEFECT INSTEAD OF DOCUMENTING IT.** Round 3
    /// did this reset on a 500 ms threshold (hence systematic,
    /// hence a fallback that could no longer grow); round 4 removed it
    /// **wholesale**, taking with it the only case it handled right:
    /// **a bridge alive for THREE DAYS then cut by a network failure dies IN
    /// ERROR**, so `reset_the_backoff` refuses — rightly — to
    /// see proof in it, and the wait stayed at `REPLI_MAX_MS` (30 s) if a
    /// refusal episode had preceded it. See the quote of
    /// [`Self::reset_the_backoff`], which this round finally makes true.
    ///
    /// ⚠️ **WHAT MAKES THIS RESET SAFE, WHERE ROUND 3'S WAS
    /// NOT**: the threshold is `REPLI_MAX_MS + 5 s`, which a sleeping refused bridge
    /// **cannot reach** — its sleep is bounded at 30 s. The
    /// hammering table of the header doc therefore does not move by ANY unit,
    /// and it is measured, not deduced. ⚠️ This gives `SEUIL_STABILITE_MS` a
    /// CADENCE role again: its doc lists it, in third position.
    pub fn stable(&mut self, ecoule_ms: u64) -> bool {
        if self.cycle_signale && ecoule_ms >= SEUIL_STABILITE_MS {
            self.cycle_signale = false;
            // 🔴 ROUND 5'S LINE. See the doc above: a life that
            // exceeds `SEUIL_STABILITE_MS` is a proof of health no
            // sleeping refusal can fabricate, and it is the ONLY gate through
            // which a long life ended IN ERROR re-arms the fallback.
            self.tentative = 0;
            true
        } else {
            false
        }
    }
}

// Test module extracted into its own file — the 500-line rule
// (`CLAUDE.md`) requires it: this module's tests, with their comments
// (each documents a distinct property, notably the reds replayed
// in rounds 3 and 4), weigh on their own more than the whole file before
// the extraction. ⚠️ **NO COUNT IS WRITTEN HERE, AND IT IS INTENDED**: an
// earlier draft announced "this module's eleven tests" while there
// were fifteen — wrong to the byte at the moment the sentence was written. The count is
// read, it is not copied:
// `grep -c '^#\[test\]' agent/src/relance_pont/tests.rs`.
// Same mechanism as `superviseur/table.rs::#[path = "table/tests.rs"] mod
// tests;` — the clause of `CLAUDE.md` that exempts it from the child module naming
// convention says so explicitly: "the same Rust
// mechanism, used for a different reason (the 500-line rule)".
#[cfg(test)]
#[path = "relance_pont/tests.rs"]
mod tests;
