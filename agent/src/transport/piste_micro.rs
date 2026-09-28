//! The UPSTREAM track: the browser's microphone to the agent (workstream E).
//!
//! **This module DEPOSITS, and nothing else.** It is workstream A's invariant taken
//! as a mirror (spec §7): the transport loop deposits, a dedicated thread works.
//! `handle_event` runs DURING the drain of `poll_output` and must
//! mutate no `Rtc` — a deposit into a sink mutates none, and
//! `TamponGigue::deposer` returns nothing, so it structurally cannot make
//! the loop wait.
//!
//! **No mutex here, and it is deliberate.** Exclusivity — "there is only one
//! cable" (spec §9) — lives in the SINK, in block E2, as a machine-wide
//! named mutex: since D1, N windows are N PROCESSES, and
//! an atomic flag guards nothing between them. This module only knows the
//! seam of that exclusivity: `PuitsMicro::deposer` returns `false` when the
//! sink refuses, we log it ONCE, and we do not insist.
//!
//! ⚠️ **"We do not insist" holds for the LOG, no longer for the browser.** Since
//! block E3, each CHANGE of the sink's verdict queues an
//! `AgentControl::MicState`: the log stays single, the announcement to the client
//! follows the transitions. See `deposer_trame_micro`.

use str0m::media::MediaData;

use crate::micro::{PuitsMicro, TrameMicro};
use crate::opus::{echantillons_de, SAMPLE_RATE_HZ};

use super::Session;

// The FIVE fields this module adds to `Session` (`transport.rs`) only
// carry one doc line each there; their reasoning is here.
//
// ❌ **TWO counts in this same comment were wrong, and block E3's cross-cutting
// review fixes them together** — they lived in two places, here and at the
// head of the field block of `transport.rs`:
//
//  - "the FOUR fields": made wrong by block E3 itself, which adds
//    `exclusivite_annoncee`. It is the usual pattern — the task that writes the
//    count and the one that invalidates it never reread each other;
//  - "this file is THREE lines from its ceiling": **never found true**.
//    At the commit that wrote it (`784f1fc`, E1), `transport.rs` had **491**
//    lines, that is a margin of **9** — the figure that the E1 section of
//    `CLAUDE.md` also carries. ⚠️ *Nothing establishes that it was wrong at the
//    INSTANT of writing; it was at the commit, the only verifiable state.*
//
// ⚠️ **And the number is NOT replaced by another number.** A line
// count copied into a comment ages at the first insertion — this
// repository has paid for it nine times. The rule that holds is `CLAUDE.md`'s:
// rerun the command, never copy a table.
//
// - `puits_micro` — absent as long as no sink has been installed: a session
//   without a microphone stays a perfectly normal video session, and that is what
//   lets workstream E cost nothing to sessions that ignore it.
// - `warned_micro_negotiation` — a track negotiated without a sink, or an RTP
//   clock that is not 48 kHz, are PERMANENT conditions: they are told
//   once, not at each packet. Modelled on `warned_audio_negotiation`.
// - `refus_micro_signale` — same reason, for the exclusivity refusal.
//   ❌ **This comment said "another window holds the cable FOR THE LIFE
//   OF ITS PROCESS", and that has been WRONG since Decision 2 of block E2**:
//   the acquisition attempt became NON-STICKY there, redone at each
//   deposit, so that a released cable is taken back. Only the LOG is single,
//   and that is all this flag guards. The false statement survived E2 —
//   `git log` returns a single commit on this file, `784f1fc` (E1), and
//   E2's cross-cutting review does not list it. Fixed by E3.
// - `exclusivite_annoncee` — the last verdict TOLD to the browser. Distinct from
//   `refus_micro_signale`, and both are needed: one bounds the log to one
//   line, the other follows transitions in BOTH directions.
// - `journaux_micro` — the count of lines ACTUALLY emitted, incremented at the
//   emission point and never at the call. It is what makes "the warning
//   only goes out once" assertable, hence able to fail: a counter
//   of calls would have made the test vacuous.

impl Session {
    /// Installs the sink that will receive upstream frames.
    ///
    /// To be set BEFORE `run()`, which takes the session by value.
    pub fn set_puits_micro(&mut self, puits: Box<dyn PuitsMicro + Send>) {
        self.puits_micro = Some(puits);
    }

    /// True when a microphone track has been negotiated AND a sink is there to
    /// receive it. Both conditions are necessary: a track without a sink
    /// leads nowhere, a sink without a track will never receive anything.
    pub fn micro_disponible(&self) -> bool {
        self.mic_mid.is_some() && self.puits_micro.is_some()
    }

    /// Deposits an upstream Opus packet. **Returns nothing, never fails, and never
    /// kills the session**: a microphone defect must not compromise
    /// a working video session (spec §10) — same rule as
    /// `write_audio`, for the same reason.
    pub(super) fn deposer_micro(&mut self, data: &MediaData) {
        if self.puits_micro.is_none() {
            // Spec §10: "upstream track not negotiated → no packet expected,
            // SINGLE warning". Modelled on `warn_audio_negotiation_once`.
            self.avertir_micro_une_fois(
                "mic track negotiated but no sink installed, packet dropped",
            );
            return;
        }

        // ⚠️ The RTP clock is CHECKED, not assumed. All of `micro.rs` reasons
        // in samples at 48 kHz: a peer that negotiated another clock
        // would make the timeline drift without any error saying so.
        if data.time.denom() != SAMPLE_RATE_HZ {
            self.avertir_micro_une_fois(
                "mic track RTP clock different from 48 kHz, packets dropped",
            );
            return;
        }

        // The duration is READ from the packet, never assumed (spec §7).
        let echantillons = match echantillons_de(&data.data) {
            Ok(n) if n > 0 => n,
            _ => {
                self.avertir_micro_une_fois(
                    "mic packet whose Opus duration is unreadable, packets dropped",
                );
                return;
            }
        };

        self.deposer_trame_micro(TrameMicro {
            opus: data.data.to_vec(),
            rtp_48k: data.time.numer(),
            echantillons,
        });
    }

    /// The deposit itself, separated from the extraction from `MediaData`.
    ///
    /// ⚠️ **The separation is not cosmetic.** str0m deliberately forbids
    /// building a `MediaData` outside its crate: without this
    /// function, the unit tests had to go through a `#[cfg(test)]`
    /// entry point that COPIED the logic — and they then exercised a
    /// copy, not the production path. The mutation "the session ends
    /// when the microphone refuses" went green under that setup, because it
    /// hit code the tests did not take (task 8, step 3).
    pub(super) fn deposer_trame_micro(&mut self, trame: TrameMicro) {
        // The sink's borrow ends BEFORE any other read of `self`:
        // without this binding, the borrow checker would refuse the `self.refus_micro_signale`
        // that follows — same precaution as `select_negotiated_opus_pt`.
        let accepte = match self.puits_micro.as_mut() {
            Some(puits) => puits.deposer(trame),
            None => {
                self.avertir_micro_une_fois(
                    "mic track negotiated but no sink installed, packet dropped",
                );
                return;
            }
        };

        if !accepte && !self.refus_micro_signale {
            self.refus_micro_signale = true;
            self.journaux_micro += 1;
            // ONCE — but **not because the refusal would be permanent**.
            //
            // ❌ This sentence said "the refusal is a PERMANENT condition,
            // another window holds the cable for the life of its process
            // (spec §9)". **Wrong since Decision 2 of block E2**: the
            // acquisition attempt became NON-STICKY there, redone at
            // each deposit, so that a window that dies gives the cable back and
            // the next one acquires it. What stays true of spec §9 is
            // "logged ONCE"; "refused" as a FINAL STATE, no.
            //
            // What justifies the uniqueness is therefore narrower, and is enough: one
            // line per packet would make fifty lines per second, and the
            // resumption, for its part, has its own line on the sink side
            // (mic: cable acquired after a refusal).
            tracing::warn!(
                "the mic sink refuses the frames (exclusivity not acquired): \
                 another window already carries the mic"
            );
        }

        // ✅ **The refusal IS told to the client since block E3**, and this half
        // of the original comment became wrong in turn — both
        // are fixed, not just one of the two.
        //
        // 🔴 **ON TRANSITION, never at each deposit.** The microphone deposits a
        // frame every 20 ms; announcing at each deposit would put fifty
        // messages per second into a queue bounded at 32
        // (`PLAFOND_CONTROLE_EN_FILE`), which would overflow in less than a second
        // and **drown the cursor, rumble and clipboard**.
        //
        // ⚠️ **`None` counts as a transition, and it is intended**: the very
        // first deposit announces its verdict. Without that, a window that loses
        // the cable from its first packet would never learn anything —
        // `Ready.mic` has already been emitted, and it says `true`. It is the pattern
        // of `Accent`: "on change only, its FIRST reading
        // included".
        //
        // ⚠️ **The transition is derived from the BOOLEAN, not from an `Issue` of the sink.**
        // `PuitsCable::deposer` knows its four `Issue`s but ignores the control
        // channel; this module knows the channel and only sees a boolean.
        // Enriching the `PuitsMicro` trait to carry the `Issue` up to here
        // would have made a vocabulary cross the boundary that this module
        // has no use for: **a boolean's transition IS a transition**,
        // and E2's two transition `Issue`s (`AccepteApresRefus`,
        // `RefusePremierement`) are exactly the two changes of this
        // boolean. The trait does not move.
        if self.exclusivite_annoncee != Some(accepte) {
            self.exclusivite_annoncee = Some(accepte);
            self.queue_control(proto::control::AgentControl::mic_state(accepte));
        }
    }

    /// `#[cfg(test)]` entry point that short-circuits `MediaData` — str0m
    /// deliberately forbids building it outside the crate.
    ///
    /// **It DELEGATES, it does not copy**: that is what guarantees the unit
    /// tests exercise the production path and not a twin. Same approach
    /// as `dispatch_controle_de_test` for `ChannelData`.
    #[cfg(test)]
    pub(super) fn deposer_trame_micro_de_test(&mut self, trame: TrameMicro) {
        self.deposer_trame_micro(trame);
    }

    /// Logs once, not at each packet.
    fn avertir_micro_une_fois(&mut self, message: &'static str) {
        if self.warned_micro_negotiation {
            return;
        }
        self.warned_micro_negotiation = true;
        self.journaux_micro += 1;
        tracing::warn!("{message}");
    }
}

#[cfg(test)]
#[path = "piste_micro/tests.rs"]
mod tests;
