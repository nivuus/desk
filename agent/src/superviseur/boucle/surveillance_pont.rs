//! Launching and supervising the **files bridge**.
//!
//! Twin of `surveillance_capteur.rs`, of which it transposes the three
//! mechanisms — the spacing OF restarts, signalling once per
//! cycle, and the duration condition for re-arming. **Only one thing differs,
//! and it is this module's only decision: startup is NOT fatal.**
//!
//! **`surveillance_pont` and not `pont`**, for the exact reason that got
//! its twin named `surveillance_capteur` (I7 of the branch's final review
//! of sub-block D4): `crate::pont` already exists and designates SOMETHING ELSE — the
//! bridge itself, that is, the process holding the ProjFS virtualisation
//! root. This module is only its supervision, seen from the
//! supervisor; it virtualises nothing. This file does `use super::*`, and
//! two `pont`s in the same module graph would only wait for a hurried
//! reader to be confused.
//!
//! 🔴 **FIX FOR THE MISSING-BRAKES LEGACY (fix round 1,
//! August 25th, 2026), CRITICAL ③.** The restart spacing was a
//! FIXED constant (`PERIODE_RELANCE_PONT_MIN`, 500 ms): a FLOOR between
//! two attempts, never a CEILING. MEASURED chain: the bridge opens
//! `/signal` → the platform's "any request" budget
//! (`plateforme/src/securite/frein.ts::BUDGET_REQUETES`, 60/minute) is already
//! exhausted for this VM's address → the relay refuses and closes with `1008`
//! → `agent/src/signaling.rs` sees its `offers` channel close without an offer
//! → `pont::executer` returns an `Err` → the PROCESS dies → this module
//! restarts it 500 ms later → refused again. **120 restarts per minute
//! against a budget of 60, on a key SHARED with the supervisor's control
//! session AND each window child** (same source address):
//! no new window can attach any more as long as this bridge persists.
//!
//! **The remedy REUSES `plateforme::repli::delai_de_repli`** — the one that
//! already protects the `/agent` channel (`plateforme.rs`, resumption loop of
//! `une_session`) — rather than writing a second one. The PURE state that counts
//! attempts and decides stability now lives in
//! [`crate::relance_pont::EtatRelance`] (fix round 2, see its doc
//! for the reason of the extraction and the naming convention applied):
//! a bridge dying in a loop therefore ends up only coming back at
//! 2 attempts/minute — far under the shared budget — rather than 120.
//!
//! 🔴 **THE TWO HALVES OF THIS REMEDY DO NOT ADD UP — NOR DO THEY
//! SUBSTITUTE FOR EACH OTHER IN THE SENSE OF ONE REPLACING THE OTHER: THEY
//! COMBINE THROUGH A MAXIMUM**, and an earlier wording of this
//! paragraph (round 2) wrongly claimed that the exponential fallback here
//! remained "the ONLY thing" bounding the cadence — corrected in turn,
//! so as not to replace a too-optimistic wording with another.
//!
//! `agent::signaling::honour_suggested_retry` (round 1, critical ②) makes
//! the refused bridge sleep until the delay the relay suggested (bounded at
//! `REPLI_MAX_MS`, 30 s); the bridge only dies AFTER that sleep. The next
//! `tenter()` then compares the time REALLY elapsed since the last
//! launch — which includes that sleep — with `EtatRelance::espacement_ms()`
//! (`delai_de_repli(tentative)`), and only restarts IF the former exceeds the
//! latter: **the effective delay between two launches is therefore the LARGER
//! of the two**, never their sum.
//!
//! **Both cap at THE SAME VALUE** (`REPLI_MAX_MS`, 30 s) —
//! `delai_de_repli` by construction (`repli.rs`), the sleep of
//! `honour_suggested_retry` through the bound it imposes on itself on `retryApresS` —
//! so that neither can ever durably EXCEED the
//! other: `delai_de_repli(tentative)` reaches exactly 30 s at `tentative = 6`
//! (`500 × 2⁶ = 32,000`, CLIPPED to `30,000`, never 32,000) and stays there.
//! On the REALLY measured scenario — a saturated volume budget that
//! suggests a `retryApresS` close to 60 s, hence a sleep constantly
//! clipped to 30 s — the sleep DOMINATES (is the REALLY constraining value
//! of the `max`) for the first six consecutive attempts, where `delai_de_
//! repli` stays under 30 s: the fallback here then ONLY observes that
//! the spacing is already satisfied, without ever imposing it. From the
//! sixth on, both are 30 s and become indistinguishable from each
//! other — neither "dominates" the other, they coincide. **Fix
//! round 1 therefore documented a cadence ceiling correct in its
//! conclusion ("2 attempts/minute"), but for an INCOMPLETE reason**:
//! on this precise scenario, that ceiling comes from the sleep of `honorer_retry_
//! suggere` at least as much as from the exponential fallback, never from the latter
//! ALONE. A shorter `retryApresS` (less saturated window) would reverse
//! the balance in favour of the fallback — not measured either.

use super::*;
use crate::relance_pont::{EtatObserve, EtatRelance};

/// What the loop keeps of the bridge from one turn to the next.
///
/// `pid` is `None` as long as no launch has succeeded — a state that does not exist
/// for the capturer, whose startup is fatal and which therefore always has a
/// PID from its construction.
pub(super) struct EtatPont {
    pid: Option<u32>,
    derniere_tentative: std::time::Instant,
    /// The PURE decision — attempts, spacing, stability — lives in
    /// [`EtatRelance`] since fix round 2: this field no longer carries
    /// a counter or a boolean itself, only the wall clock
    /// (`derniere_tentative`, above) and the process identity (`pid`),
    /// which stay specific to THIS module because they touch
    /// `std::time::Instant` and `LanceurDeProcessus` — not portable, hence
    /// not extracted.
    relance: EtatRelance,
}

impl EtatPont {
    /// Launches the files bridge.
    ///
    /// 🔴 **Unlike `EtatCapteur::start`, a failure here is NOT
    /// fatal, and it is this module's only decision.**
    ///
    /// The capturer serves the media to any child that attaches: without it,
    /// each child would capture into the void, and the supervisor would have nothing
    /// left to supervise — hence its fatal startup, just like a
    /// driver or a hook that does not open. The bridge, for its part, only serves the
    /// file reader. **Framing §4, principle 4, requires that a failure on
    /// that side NEVER touches the video stream**; making its startup fatal
    /// would do exactly the opposite — a VM without ProjFS, or a bridge refusing
    /// to start for any other reason, would lose the whole
    /// capture.
    ///
    /// It therefore returns no `Result`: there is no error to propagate. A
    /// failure is logged and **retried indefinitely** by `surveiller`,
    /// exactly like a restart — the bridge can therefore appear during a
    /// session, without restarting the supervisor.
    pub(super) fn start(lanceur: &LanceurDeProcessus) -> Self {
        let mut etat = Self {
            pid: None,
            // Pulled back by one floor spacing: the very first attempt
            // must happen NOW, not in 500 ms. Without this pullback,
            // `tenter` would return control without doing anything and the bridge would only
            // start at the loop turn following the spacing — which
            // would go unnoticed, the bridge not being on the critical path.
            //
            // 🔴 **IT IS THE SECOND ROLE OF `ESPACEMENT_PLANCHER_MS`, AND IT
            // WAS NAMED NOWHERE BEFORE FIX ROUND 4** —
            // pointed out by the review: the constant's doc listed two
            // when the code served three. It is not a cadence, it is
            // a PRIMER, and it ties the two files: raising the
            // constant would delay the bridge's startup by as much, which cannot
            // be read from `relance_pont.rs`. The constant now says so
            // on its side, and ⚠️ it is **welded** to
            // `plateforme::repli::REPLI_MIN_MS` by the test
            // `the_first_spacing_equals_the_floor`: it cannot
            // be raised alone.
            derniere_tentative: std::time::Instant::now()
                - std::time::Duration::from_millis(crate::relance_pont::ESPACEMENT_PLANCHER_MS),
            relance: EtatRelance::neuve(),
        };
        etat.tenter(lanceur, "initial file bridge launch failed");
        etat
    }

    /// Restarts the bridge if it is dead, spaced according to
    /// `EtatRelance::espacement_ms` (`plateforme::repli::delai_de_repli`).
    ///
    /// **Never closes any window, and touches nothing else.** A
    /// bridge failure has no effect on video sessions: that is the whole
    /// point of having put it in its own process, and adding the
    /// slightest side effect on the table or on the children would cancel this
    /// property.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        // 🔴 THREE BRANCHES SINCE FIX ROUND 4, AND THE TWO
        // DECISIONS OF `EtatRelance` CAN NO LONGER CROSS: `stable`
        // (LONG threshold, a TRACE question) is only asked of a LIVE bridge;
        // `reset_the_backoff` (a CADENCE question) is only asked of
        // a DEATH, and of its OUTCOME — never again of a lifetime.
        //
        // The round 4 review measured it: re-arming on "alive for
        // 500 ms" made `tentative` fall back to zero after EACH
        // launch, and the fallback could no longer grow for any failure
        // mode where the bridge lives between ~0.5 s and ~35 s — up to 100
        // `/signal` connections per minute against a SHARED budget of 120,
        // without any `retryApresS` having to step in. A duration could
        // not decide: a HEALTHY session that ends occupies the
        // same interval as a REFUSED bridge that slept. The outcome, for its part,
        // decides — see the header doc of `crate::relance_pont`.
        match lanceur.etat_du_pont() {
            EtatObserve::Vivant => {
                let ecoule_ms = self.derniere_tentative.elapsed().as_millis() as u64;
                if self.relance.stable(ecoule_ms) {
                    tracing::info!(pid = self.pid, "file bridge stable again");
                }
                return;
            }
            // A CLEAN exit (`pont::executer` returns `Ok(())`) proves
            // that a past failure is resolved: the fallback starts again from the floor.
            // A `bail!` — including the relay's refusal, honoured then propagated —
            // proves nothing, and the fallback keeps growing.
            EtatObserve::Mort(issue) => self.relance.reset_the_backoff(issue),
            // ⚠️ NO RE-ARMING HERE, AND THAT IS THE POINT: `Absent` covers
            // a repeatedly failing `spawn`, exactly the case round 1's critical
            // ③ measured, whose fallback must grow.
            EtatObserve::Absent => {}
        }
        self.tenter(lanceur, "file bridge relaunch failed");
    }

    /// One launch attempt, spaced and logged once per cycle.
    ///
    /// Shared between `start` and `surveiller`: the two paths are the
    /// same gesture, and writing them twice would make them diverge — it is
    /// precisely what distinguishes this module from its twin, where `start`
    /// retries nothing because it is fatal.
    fn tenter(&mut self, lanceur: &LanceurDeProcessus, quoi: &str) {
        // 🔴 THE SPACING COMES FROM `EtatRelance::doit_relancer`, NOT FROM A
        // FIXED CONSTANT — that is the whole fix of D1's critical ③.
        // `espacement_ms()` is exactly `ESPACEMENT_PLANCHER_MS` (500 ms)
        // at a cycle's first attempt: a bridge that never dies
        // twice in a row therefore observes NO change of behaviour.
        // Only from the second consecutive attempt without
        // stability does the spacing lengthen.
        let ecoule_ms = self.derniere_tentative.elapsed().as_millis() as u64;
        if !self.relance.doit_relancer(ecoule_ms) {
            return;
        }
        let espacement_ms = self.relance.espacement_ms();
        self.derniere_tentative = std::time::Instant::now();
        // 🔴 RECORDED HERE, NOT ONLY IN THE `Err` BRANCH BELOW —
        // and it is the point that makes this fix correct on the
        // REALLY measured case: `lancer_pont()` SUCCEEDS (`Ok`), the process
        // launches, gets refused by `/signal`, and dies after having honoured
        // the suggested delay. That cycle NEVER goes through the `Err` branch
        // of `lancer_pont()` — it is a perfectly successful `Command::spawn` —,
        // and a counter incremented only on `Err` would stay
        // stuck at `tentative = 0`, hence at a 500 ms spacing, in
        // EXACTLY the case this batch must fix.
        let premier_du_cycle = self.relance.tentative_lancee();
        match lanceur.lancer_pont() {
            Ok(new) => {
                if premier_du_cycle {
                    tracing::warn!(
                        pid_mort = self.pid,
                        pid_neuf = new,
                        tentative = self.relance.tentative(),
                        espacement_ms,
                        "file bridge launched or relaunched (following attempts silent \
                         as long as the cycle repeats)"
                    );
                }
                self.pid = Some(new);
            }
            Err(error) => {
                // `self.pid` is NOT updated: it stays the last known
                // value, so that the next successful restart
                // logs an exact "dead" one — `lancer_pont` only returns `Ok`
                // atomically, a failure never made a process live.
                //
                // ⚠️ `warn!` and not `error!`: the bridge is an OPTIONAL
                // service of the product, and an `error!` would make a VM without
                // ProjFS — the one from before F0 — fill the log with errors
                // for a capture that, for its part, works perfectly. It is
                // framing principle 4 down to the trace level.
                if premier_du_cycle {
                    tracing::warn!(
                        %error,
                        tentative = self.relance.tentative(),
                        espacement_ms,
                        "{quoi} — the capture is NOT affected; retried indefinitely, at an \
                         INCREASING spacing (plateforme::repli::delai_de_repli), and \
                         silently as long as the failure repeats"
                    );
                }
            }
        }
    }
}
