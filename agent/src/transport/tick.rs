//! The priority list of a loop round.
//!
//! `act_on_timeout` decides the SINGLE action taken per round. The order
//! is not arbitrary:
//!
//! - the due drain comes with absolute priority: it is the only way to
//!   guarantee that no mutation chains on without a full pass through
//!   `poll_output()` in between, whatever the state of the other queues;
//! - control and adaptation come before media: reconfiguring
//!   the encoder with a frame in flight would cost that frame;
//! - audio comes before video: a sound dropout is heard, a frame
//!   10 ms late is not seen;
//! - waiting on the socket only comes last, when there is nothing to
//!   emit.
//!
//! The body of each branch lives in its thematic module; this file only
//! carries the order.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use proto::control::AgentControl;
use str0m::Input;

use super::Session;

/// Result of handling an event or an internal loop round.
pub(super) enum Tick {
    Continue,
    Disconnected,
}

/// Minimal interval between two checks of `source.is_alive()` in
/// `act_on_timeout`. This call costs a system call at each round on the
/// Windows side (window lookup); a closed window stays closed, no need
/// to recheck it at 60 Hz.
const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl Session {
    /// Reacts to `Output::Timeout`: decides and performs AT MOST ONE mutation
    /// of `Rtc` (deferred drain of an already written frame, pending control
    /// message, due video frame, or handling of an incoming packet
    /// / str0m deadline), then gives control back to `run()`, which immediately
    /// calls `poll_output` again — it is this structure that guarantees
    /// draining before any following mutation (C2 of the review): there
    /// is no code path that mutates `Rtc` without `run()`
    /// calling `poll_output` right after. The priority given to the deferred
    /// drain (see `video_write_pending_drain`) is what makes this
    /// guarantee true even right after writing a frame: without it,
    /// `write_frame` (one mutation) followed directly by `handle_input`
    /// (a second) would break the same rule.
    ///
    /// Thirteen additional branches (a0bis: draining a control
    /// message produced outside the loop into `pending_control`; a0ter:
    /// pending adaptation decision; a1: pending resize;
    /// a1bis: pending visibility; a1ter: announcing a sleep
    /// change; a1ter-bis: announcing a fullscreen change
    /// (sub-block D8); a1quater: budget share granted by the sensor
    /// (sub-block D6); a1quinquies: audio order decided by the sensor
    /// (sub-block D7); a1sexies: rebuilding a dead audio capture
    /// detected locally, `AudioMort` as a fallback if the attempt budget
    /// is exhausted, and announcing a recovery PROVEN by a real packet
    /// (sub-block D9, full remedy brought by D10); a1septies: announcing
    /// a change of the VM's clipboard (sub-block P1); a1octies:
    /// writing a paste from the browser into the VM's clipboard,
    /// then arming the `Ctrl+V` injection (sub-block P2); a1nonies:
    /// announcing a change of the window's accent colour — the dominant
    /// hue of its icon (sub-block A1); a2:
    /// window check) NEVER queue, before giving control
    /// back, a write that would remain to be drained — that is the invariant
    /// this enumeration exists to audit. **Twelve of them (all but
    /// a1quater) do not even touch `self.rtc`**: only `self.source`,
    /// `self.audio_source`, the audio rebuild budget (a1sexies
    /// only) and/or `self.pending_control`, at most by queueing a
    /// control message there (`queue_control`, which only pushes onto a `VecDeque`,
    /// with no effect on `Rtc` before the next round).
    ///
    /// **a1quater is an exception, and it must be stated precisely**:
    /// `rtc.bwe().set_desired_bitrate` (body in `part`) DOES mutate an internal
    /// field of `Rtc` — and reconfigures str0m's pacer
    /// (`configure_pacer`) if a bandwidth estimate already exists.
    /// But this call queues NO packet: the effect it schedules
    /// on the probing side (str0m's `ProbeControl`, which can bring forward the deadline of
    /// the next probe and cause padding to be emitted) is only evaluated at the
    /// NEXT handling of `Input::Timeout`, never during this call.
    /// It is this absence of queueing — not the absence of mutation of
    /// `Rtc` — that preserves the drain invariant for this branch.
    ///
    /// Each still gives control back immediately after its action rather
    /// than chaining on to the next branch in the same call: the
    /// resize rebuilds a whole encoding chain
    /// (potentially long, see `WindowsSource::resize`), and treating it
    /// as a step in its own right — just like the branches that
    /// really write or read packets on `Rtc` (a0, a3, b,
    /// c…) — keeps this function readable as a single priority list
    /// rather than mixing two different styles.
    ///
    /// **To whoever reads this after one more branch**: this count and this
    /// enumeration are the audit point of the invariant "none of these
    /// branches queues, before giving control back, a write that
    /// would remain to be drained" — **NOT** "none of these branches mutates
    /// `Rtc`": a1quater does mutate a field of it (see above, and do not
    /// let this wording be copied into a future addition
    /// without rechecking this distinction). An addition that forgets to confront
    /// this invariant is checked against an incomplete list. Update them
    /// in the same gesture as the branch.
    ///
    /// Does not take `on_input`/`on_control`: `handle_input` never produces
    /// an application event directly (the events that
    /// result only come out through a future `poll_output`, hence through
    /// `run()`, which dispatches them itself).
    pub(super) fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drain due after the last written video frame or audio
        // packet. Checked with absolute priority, before everything else:
        // it is the only way to guarantee that no mutation ever chains on
        // without a full pass through `poll_output()` in between,
        // whatever the state of the other queues (see the comment of the
        // field and fix round 1 of task 11).
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            // A single `handle_input(Timeout)` pops `to_payload` for ALL
            // tracks: the two flags therefore fall back together. Keeping
            // them separate stays necessary upstream — it is what lets
            // `write_audio` and `write_frame` report independently
            // that a write did happen.
            self.video_write_pending_drain = false;
            self.audio_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (media drain): {e}"))?;
            return Ok(Tick::Continue);
        }

        // a0bis) A control message produced outside the loop is waiting.
        //        Body in `controle`.
        if let Some(tick) = self.drainer_controle_externe() {
            return Ok(tick);
        }

        // a) A control message is pending. Body in `controle`,
        //    emptiness test included: the branch only lets through (`None`)
        //    without having mutated `Rtc` — empty queue, or channel not yet
        //    open while the session is not closing, in which case the
        //    message stays queued.
        if let Some(tick) = self.brancher_controle_en_file()? {
            return Ok(tick);
        }

        if self.ending {
            // End message sent (queue emptied above): done.
            return Ok(Tick::Disconnected);
        }

        // a0ter) Pending adaptation decision. Handled before the video
        //        branch and before the resize: reconfiguring
        //        the encoder with a frame in flight would cost that frame.
        //        Never mutates `Rtc`. Body in `adaptation`.
        if let Some(decision) = self.pending_decision.take() {
            self.appliquer_decision(decision);
            return Ok(Tick::Continue);
        }

        // a1) Pending resize, to handle before the video
        //     branch. Never mutates `Rtc` either, but stays a potentially
        //     long operation — new window AND D3D11 device,
        //     see `WindowsSource::resize` — handled here as a step in
        //     its own right rather than mixed with others in the same call, like
        //     the other branches. Body in `redimensionnement`.
        if let Some((width, height)) = self.pending_resize.take() {
            self.appliquer_redimensionnement(width, height);
            return Ok(Tick::Continue);
        }

        // a1bis) Pending visibility. After the resize and before
        //        video, for the same reason as it: the decision may
        //        release an encoder on the sensor side, which is long, and
        //        never mutates `Rtc`.
        if let Some((visible, focalisee)) = self.pending_visibility.take() {
            if let Err(error) = self.source.set_awake(visible, focalisee) {
                // Not fatal: losing the arbitration is not losing the session.
                // `cause::chain` and not `%error`: `set_awake` goes through
                // `commander_simple`, which stacks a context — `anyhow`'s plain
                // `Display` would only render that one. See `crate::cause`.
                tracing::warn!(
                    error = %crate::cause::chain(&error),
                    visible,
                    focalisee,
                    "visibility refused by the sensor"
                );
            }
            return Ok(Tick::Continue);
        }

        // a1ter) A sleep change to announce to the browser. Queried
        //        at each round where a1bis did not fire (otherwise that one
        //        has already exited through an early return); but
        //        `sommeil_a_annoncer` consumes: no message is ever
        //        re-emitted, so this branch cannot flood the control
        //        channel even at ~100 Hz.
        if let Some((endormie, raison)) = self.source.sommeil_a_annoncer() {
            self.queue_control(AgentControl::asleep(endormie, &raison));
            return Ok(Tick::Continue);
        }

        // a1ter-bis) A fullscreen change to announce to the browser.
        //            Same regime as a1ter just above:
        //            `plein_ecran_a_annoncer` CONSUMES, so no message
        //            is ever re-emitted and this branch cannot flood
        //            the control channel even at ~100 Hz.
        if let Some(actif) = self.source.plein_ecran_a_annoncer() {
            self.queue_control(AgentControl::fullscreen(actif));
            return Ok(Tick::Continue);
        }

        // a1quater) A budget share granted by the sensor. After a1ter
        //           (which puts nothing to sleep: it ANNOUNCES to the browser a
        //           sleep already decided on the sensor side — the actual
        //           falling asleep happens on the sensor side, not here). Consistency
        //           between a sleep and the share that follows from it is NOT played
        //           in this local order: it is played UPSTREAM, on the
        //           sensor side, where `distribuer` (the sleep orders) precedes
        //           `distribuer_les_parts` on all entry paths of the
        //           registry (`inscrire`, `retirer`, `signaler`,
        //           `echec_de_reveil`, wheel round — see
        //           `capteur/sommeil.rs`).
        //
        //           ⚠️ **All BUT ONE, and it must not be kept quiet** (I2, final
        //           branch review). The `rompus` path of
        //           `distribuer_les_parts` sends the shares FIRST, then
        //           detects broken channels, removes their sessions from the
        //           pool, and only then relays the orders that this
        //           removal generates. A session WOKEN by the place
        //           a dead one frees therefore receives its `Reveiller` AFTER the
        //           sleeping share computed just before, and only gets its
        //           awake share at the next wheel round.
        //           **Bound: `PERIODE_REARBITRAGE`, that is 250 ms**, during
        //           which this window encodes at the
        //           `PART_DORMANTE_BPS` floor. The single channel guarantees the order of
        //           DELIVERY, never the order of COMPUTATION — it is this
        //           distinction the previous wording missed.
        //
        //           Handling a1quater right after a1ter stays the most
        //           readable choice: it respects the arrival order rather than
        //           inverting it for no reason.
        //           Queues no packet — see this function's header doc
        //           on what `set_desired_bitrate` really
        //           mutates — but sets a decision that branch
        //           a0ter will apply at the next round.
        //           `part_a_appliquer` CONSUMES: no re-emission, hence
        //           no reconfiguration looping at ~100 Hz.
        if let Some(bps) = self.source.part_a_appliquer() {
            self.appliquer_part(bps);
            return Ok(Tick::Continue);
        }

        // a1quinquies) An audio order decided by the sensor. After a1quater,
        //              for the same readability reason: we respect the arrival
        //              order rather than inverting it for no reason.
        //
        //              Queues NO packet: `appliquer_audio` (task
        //              7, see `piste_audio.rs`) only touches the audio
        //              source (`AudioSource::set_actif`, which writes a boolean
        //              read by the capture thread) and the controller's budget
        //              (`Controleur::changer_audio_bps`, which only writes a
        //              field of `Config`) — neither queues a
        //              packet. This function's drain invariant
        //              is therefore preserved.
        //
        //              `audio_a_appliquer` CONSUMES: a `Start()`/`Stop()` per
        //              round at ~100 Hz is exactly what this consumption
        //              prevents.
        if let Some(actif) = self.source.audio_a_appliquer() {
            self.appliquer_audio(actif);
            return Ok(Tick::Continue);
        }

        // a1sexies) Two transitions detected LOCALLY, never pushed by
        //           the sensor: this window's audio capture has just
        //           died for good (`crate::audio::LECTURES_ECHOUEES_MAX`
        //           consecutive WASAPI read errors, `windows_audio.rs`),
        //           or it has just brought PROOF that it restarted
        //           (a real packet, set by `brancher_audio` — see
        //           `piste_audio`).
        //
        //           **D10 inverts the order of the remedy** (D9 only knew how to
        //           report `AudioMort`, never rebuild).
        //           `reconstruire_ou_signaler` (body in `piste_audio`)
        //           FIRST TRIES to rebuild the source; `AudioMort` is no
        //           longer the first gesture but the FALLBACK — that of the case where the
        //           attempt budget is exhausted, or where no rebuilder
        //           exists — and where only the sensor's promotion of a neighbour
        //           can still give sound back to the group.
        //
        //           ❌ **"Single-window path" appeared in this
        //           parenthesis and it is WRONG** (cross-cutting end-of-branch
        //           review, second round): `demarrage/audio.rs::brancher`
        //           sets a rebuilder UNCONDITIONALLY in its
        //           `Ok` arm, `None` branch INCLUDED. The only cases really
        //           without a rebuilder are `AUDIO=0`, `TEST_FILE`, and an
        //           initial opening failure.
        //
        //           🔴 **And the error had consequences HERE more
        //           than elsewhere, because this file is the one that CALLS
        //           `reconstruire_ou_signaler`**: it gave whoever
        //           takes over legacy no. 1 the mental model exactly
        //           OPPOSITE to the real one. In single-window mode the source IS
        //           rebuilt — then the re-arming MADE IT SILENT,
        //           `audio_porteuse` then always being `false` for lack of a
        //           sensor to write it.
        //
        //           ✅ **FIXED IN SUB-BLOCK D11 (legacy 4), and measured**:
        //           `demarrage/audio.rs::brancher` sets
        //           `audio_porteuse = true` in its single-window
        //           branch only, and acceptance ① records 441 Hz received on
        //           green against the sentinel on red. See
        //           `Session::reconstruire_ou_signaler` (`piste_audio.rs`).
        //
        //           `appliquer_audio` (a1quinquies just above) only runs
        //           on the ARRIVAL of an order, never periodically: without this
        //           check at the tick, a capture that dies (or recovers)
        //           between two orders would never be reported. An atomic
        //           `load` or a rebuild attempt bounded by its
        //           own respite are both cheap per
        //           round.
        //
        //           The `audio_mort_signale` latch is what prevents
        //           flooding the sensor with `AudioMort`: once set, it only
        //           falls back on two transitions — a reattachment (see
        //           below), or a RE-ELECTION by the sensor
        //           (`appliquer_audio`, a1quinquies), which also replenishes
        //           the attempt budget. Without this second fall-back
        //           point, found in the review of task 12, the budget set once
        //           at the construction of the `Session` would only have allowed
        //           a single dead → rebuilt → proven cycle per session,
        //           never several CONSECUTIVE failures — the opposite of what
        //           `REARMEMENTS_MAX` (`capteur/sommeil.rs`) is meant to
        //           count. `audio_vivant_a_annoncer`, for its part, is CONSUMED on
        //           reading (same regime as `sommeil_a_annoncer` /
        //           `part_a_appliquer`), so it cannot re-emit
        //           `AudioVivant` in a loop either.
        //
        //           Resetting the latch (`rattachement_survenu`) IS
        //           NOT itself an action: it mutates neither `Rtc` nor the
        //           source, queues nothing, and therefore does not break
        //           the drain invariant even without `return` — same regime
        //           as `last_alive_check` in a2. A reattachment (sensor
        //           restarted) makes the sensor lose the memory of any
        //           `AudioMort` reported before the break: without this
        //           reset, this window would never inform it again.
        if self.source.rattachement_survenu() {
            self.audio_mort_signale = false;
        }
        // D10: we try to REBUILD first. `AudioMort` is no longer the
        // first gesture but the fallback — that of the case where the process tree has
        // disappeared, and where only promoting a neighbour can still give
        // sound back to the group.
        if !self.audio_mort_signale && self.reconstruire_ou_signaler(Instant::now()) {
            self.audio_mort_signale = true;
            self.source.signaler_audio_mort();
            return Ok(Tick::Continue);
        }
        if self.audio_vivant_a_annoncer {
            self.audio_vivant_a_annoncer = false;
            self.source.signaler_audio_vivant();
            return Ok(Tick::Continue);
        }

        // a1septies) The VM's clipboard changed (sub-block P1). Same
        //            regime as a1ter-bis: `presse_papier_a_annoncer` CONSUMES,
        //            so no message is ever re-emitted and this branch
        //            cannot flood the control channel even at ~100 Hz. This
        //            point matters more here than elsewhere: the text can
        //            weigh up to `presse_papier::PRESSE_PAPIER_MAX` (64 KiB),
        //            where an `Asleep` or a `Fullscreen` weighs a few bytes.
        //
        //            `texte` at `None` is NOT "nothing to announce": it is a
        //            size REFUSAL, which the browser must tell
        //            the user (D-P1-1). It is the OUTER `Option`, the one
        //            the method returns, that carries "nothing to announce".
        if let Some((texte, octets)) = self.source.presse_papier_a_annoncer() {
            self.queue_control(AgentControl::clipboard(texte, octets));
            return Ok(Tick::Continue);
        }

        // a1octies) The browser pasted (sub-block P2). 🔴 **THIS IS WHERE
        //           D6's ORDER IS PRODUCED** — write the VM's clipboard
        //           first, only arm the `Ctrl+V` injection afterwards, and
        //           only if the write SUCCEEDED. Body in `collage`, which
        //           also carries the second half of this order (the injection
        //           itself, drained by `boucle::run`): the two links
        //           read in the same place rather than two files apart.
        //
        //           **This branch does NOT mutate `self.rtc`** — like a1quater
        //           and a1quinquies. It touches `self.source` (through the sensor's
        //           pipe) and two fields of its own, and queues no
        //           packet: the drain invariant audited at the head of the function
        //           is preserved.
        if let Some(texte) = self.pending_clipboard.take() {
            self.traiter_le_collage(&texte);
            return Ok(Tick::Continue);
        }

        // a1nonies) The window's accent colour changed (sub-block A1).
        //           Same regime as a1ter-bis and a1septies:
        //           `accent_a_annoncer` CONSUMES, so no message is
        //           ever re-emitted and this branch cannot flood the control
        //           channel even at ~100 Hz.
        //
        //           ⚠️ **The SENSOR already announces on change only** — it is
        //           `accent::SuiviAccent`, on the window thread. The
        //           consumption here is therefore a SECOND guard, in another
        //           process, and it is not redundant: nothing in the child
        //           knows what the sensor has already emitted, and the
        //           resumption window of a broken media connection can make the
        //           same state arrive twice.
        //
        //           **This branch does NOT mutate `self.rtc`** — like a1quater,
        //           a1quinquies and a1octies. It reads `self.source` and queues at
        //           most one message in `self.pending_control`, with no
        //           effect on `Rtc` before the next round: the drain invariant
        //           audited at the head of the function is preserved.
        if let Some(couleur) = self.source.accent_a_annoncer() {
            self.queue_control(AgentControl::accent(couleur));
            return Ok(Tick::Continue);
        }

        // a2) Has the captured window disappeared? Costs a system call
        // on the Windows side (window lookup): spaced out by
        // `ALIVE_CHECK_INTERVAL` rather than checked at every loop
        // round — a closed window stays closed.
        let now = Instant::now();
        if now.saturating_duration_since(self.last_alive_check) >= ALIVE_CHECK_INTERVAL {
            self.last_alive_check = now;
            if !self.source.is_alive() {
                self.begin_ending("window closed");
                return Ok(Tick::Continue);
            }
        }

        // a3) An audio packet, if the track is negotiated and a packet
        //     is waiting. BEFORE video: a sound dropout is heard, a frame
        //     10 ms late is not seen. Audio moreover has a
        //     hard 10 ms cadence, whereas video is opportunistic by
        //     nature. Body in `piste_audio`.
        if let Some(tick) = self.brancher_audio() {
            return Ok(tick);
        }

        // b) A video frame, if its deadline is reached and the track
        //    negotiated. Body in `piste_video`.
        if let Some(tick) = self.brancher_video() {
            return Ok(tick);
        }

        // b0) TURN request waiting to be sent (allocation, lease
        //     refresh, permission, channel binding). Never mutates `Rtc`:
        //     it is an exchange with the relay server, invisible to str0m.
        //     Placed just before waiting so that the lease refresh
        //     does not depend on a packet arriving. Body in
        //     `relais`.
        if let Some(tick) = self.emettre_requete_turn() {
            return Ok(tick);
        }

        // c) Nothing to emit: wait for an incoming packet, bounded both
        //    by `Rtc`'s deadline and by the next media
        //    deadlines. Body in `socket`.
        self.brancher_attente(deadline)
    }
}

// The tests live in a neighbouring file: this file crossed 500
// lines by adding coverage of branches a1bis/a1ter (task 8,
// sub-block D5). See the header of `tick/tests.rs`.
#[cfg(test)]
mod tests;
