//! Boucle WebRTC : ICE, DTLS, SRTP et SCTP via str0m.
//!
//! str0m is a library without I/O: we own the UDP
//! socket and the event loop. Imperative rule documented by str0m:
//! after each mutation, drain `poll_output` until `Output::Timeout`
//! before the next mutation — but str0m also states that a mutation issued
//! **from inside** the drain loop (before it has given control
//! back) is correct. That is this file's structural choice: `Session::run`
//! is a single continuous loop around `Rtc::poll_output`, and each
//! mutation (image write, control message write, `handle_input`) happens
//! inside that loop, immediately followed by a return to
//! `poll_output`. Nothing outside `run()` ever mutates `Rtc` while
//! the loop runs: there is simply no other place that
//! could, which makes the invariant structural rather than dependent on the
//! caller's discipline.
//!
//! `run()` blocks on purpose (non-blocking UDP socket, polled in small
//! sleep slices rather than by a blocking `recv_from` with a deadline —
//! see `RECV_POLL_INTERVAL`) and must therefore be called from a
//! dedicated thread — `tokio::task::spawn_blocking` on the `demarrage.rs` side — never from a
//! tokio async worker.
//!
//! This file now only carries the session state. ❌ *It said "and the
//! loop driving it": wrong since sub-block F1, which extracted
//! `Session::run` — with `accept_offer` and `drain_quietly` — to
//! [`boucle`], the file having crossed 500 lines (495 → 501 → 440). The
//! two paragraphs above, which describe `run()` in the present tense, are in the
//! same case: they describe a function that now lives in `boucle`.*
//! The rest is split by theme across the submodules, almost
//! all written as `impl Session`: `tick` (a round's priority list,
//! including `act_on_timeout`), `controle` (control channel and session end),
//! `adaptation` (network feedback control), `redimensionnement` (the window
//! the user resizes), `evenements` (what str0m reports),
//! `piste_video`, `piste_audio` and `piste_micro` (the three media tracks, the
//! last being the only UPSTREAM one), `socket` (UDP waiting
//! and receiving), `boucle` (`run` and draining), `fixtures` (the
//! shared test scaffolding). ⚠️ *This list is not exhaustive and never
//! has been — `cadence_video`, `part` and `relais` were missing from it before
//! F1; only the closing clause below carries a claim.*
//! **Only `initialisation` is an exception**: a FREE function
//! (`construire_rtc`), not a method of `Session` — it builds the
//! UDP socket and the str0m `Rtc` before `Session` itself exists, hence
//! before there is a `self` to attach it to.

use std::collections::VecDeque;
use std::net::{IpAddr, UdpSocket};
use std::time::Instant;

use anyhow::Result;
use proto::control::AgentControl;
use str0m::channel::ChannelId;
use str0m::media::Mid;
use str0m::Rtc;

use crate::audio::{AudioSource, Reconstructeur};
use crate::congestion;
use crate::source::VideoSource;

mod adaptation;
/// The transport loop and the two drain points prior to `run`.
///
/// 🔴 EXTRACTED BECAUSE THIS FILE CROSSED 500 LINES — for the SECOND time,
/// and on the same battlefield: sub-block D10 had already taken it to 501
/// and moved `initialisation.rs` out of it (the socket and
/// `Rtc` constructor); sub-block F1 brings it back there by adding `input_channel`, and moves
/// the loop out. This repository has written since D6 that "the margin regained by an
/// extraction is lost again in the next round if treated as acquired":
/// it is the fifth time it pays for it, and the second on this very file.
/// EXTRACTED, never compressed — `CLAUDE.md`'s doctrine forbids by name
/// shortening a comment to get back under the line.
mod boucle;
mod cadence_video;
/// The paste coming from the browser: BOTH halves of D6's order, written
/// in the same place. See its header comment.
mod collage;
mod controle;
mod evenements;
#[cfg(test)]
mod fixtures;
mod initialisation;
mod part;
mod piste_audio;
mod piste_micro;
mod piste_video;
mod redimensionnement;
mod relais;
mod socket;
#[cfg(test)]
mod sonde_montante;
mod tick;

use piste_video::FRAME_INTERVAL;
use socket::TimerResolutionGuard;

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    dimensions: (u32, u32),
    /// Clock origin of the session, shared with the sources. Used to
    /// rebuild an image's capture instant from its
    /// timestamp (see `write_frame`).
    clock_origin: Instant,
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    /// The `input` channel, retained as `control_channel` is — it is what
    /// lets `evenements::destination` route by CHANNEL rather than by
    /// the binary flag alone. `None` as long as no `ChannelOpen` has
    /// named it: a frame received before then is refused, not guessed.
    input_channel: Option<ChannelId>,
    started: Instant,
    /// Control messages waiting to be sent. `run()` sends at most one
    /// per mutation, as soon as the channel is open.
    pending_control: VecDeque<AgentControl>,
    /// Control messages produced OUTSIDE the loop: cursor polling thread,
    /// ViGEmBus driver rumble callback. Neither
    /// can touch the `Session`, which is owned only by `run()`.
    outbound_control: Option<std::sync::mpsc::Receiver<AgentControl>>,
    /// True as soon as an `AgentControl::session_end` has been queued: no
    /// more images are sent, the session ends as soon as the control
    /// queue is emptied (or found impossible to empty).
    ending: bool,
    next_frame_at: Instant,
    /// Prevents flooding the logs: the incomplete negotiation (I4) is
    /// reported once only, not at each dropped image.
    warned_negotiation: bool,
    /// Number of consecutive transient UDP receive errors (see
    /// `classify_recv_error`/`recv_error_backoff`): reset to zero as soon as a
    /// loop round runs without such an error (packet received, or
    /// mere deadline without data). Used to grow the backoff
    /// applied between two attempts during a burst.
    consecutive_recv_errors: u32,
    /// True right after a video image has been written (`writer.write()`),
    /// as long as the str0m drain that actually sends it
    /// (`Rtc::handle_input(Input::Timeout(..))`) has not happened yet.
    ///
    /// `writer.write()` pushes the image onto str0m's internal `to_payload` queue;
    /// only `handle_input(Input::Timeout(..))` pops it
    /// (`do_payload`), never `poll_output()` alone (see `act_on_timeout`,
    /// fix round 1). This flag defers that drain to the next
    /// round rather than chaining it in the same call: `write_frame`
    /// (one mutation) and `handle_input` (a second mutation) thus stay
    /// each separated by a full pass through `poll_output()`, as
    /// str0m requires — chaining them directly, as the
    /// first version of this fix did, reproduced exactly the
    /// violation it claimed to resolve.
    video_write_pending_drain: bool,
    /// Audio source, absent as long as none has been provided (video test
    /// source, platform without audio, or loopback opening failure — in
    /// all cases the video session continues).
    audio_source: Option<Box<dyn AudioSource + Send>>,
    /// `mid` of the DOWNSTREAM audio track (workstream A, agent → browser),
    /// filled at negotiation. ⚠️ TWO audio m-lines since workstream E:
    /// the upstream one has its own, `mic_mid`, and it is DIRECTION that separates them
    /// (`evenements.rs`, which carries the silent defect this field long had).
    audio_mid: Option<Mid>,
    /// `mid` of the MICROPHONE track (workstream E), filled at negotiation.
    mic_mid: Option<Mid>,
    // The FIVE MICROPHONE fields (workstream E). Their reasoning lives in full
    // in `transport/piste_micro.rs`, next to the code that uses them — and
    // that is also where the refutation of the two counts this comment
    // carried until block E3 ("the FOUR fields", "three lines from its
    // ceiling") lives, by the same PLACEMENT and not by compression.
    /// Sink of the upstream flow, absent as long as none has been installed.
    puits_micro: Option<Box<dyn crate::micro::PuitsMicro + Send>>,
    /// Unexpected negotiation or clock: reported once only.
    warned_micro_negotiation: bool,
    /// Sink refusal (exclusivity not acquired): reported once only.
    refus_micro_signale: bool,
    /// Log lines actually EMITTED about the microphone.
    journaux_micro: u64,
    /// Last exclusivity verdict ANNOUNCED to the browser (block E3).
    /// `None` as long as no upstream packet has been deposited: that is what makes
    /// the sink's FIRST answer a transition, hence announced.
    exclusivite_annoncee: Option<bool>,
    /// Audio counterpart of `video_write_pending_drain`. Distinct from it: without
    /// its own flag, an audio write followed by a video write in the next
    /// round would lose a drain.
    audio_write_pending_drain: bool,
    /// Reports once only that no Opus payload type was
    /// negotiated, rather than at each dropped packet.
    warned_audio_negotiation: bool,
    /// Last requested resize, not yet applied. Only the most
    /// recent is kept: while a user drags an edge, the
    /// intermediate requests are of no interest. Applied in
    /// `act_on_timeout`, never from `dispatch_channel_data` — see the
    /// comment of this field at its point of consumption.
    pending_resize: Option<(u32, u32)>,
    /// Last visibility announced by the browser, waiting to be
    /// applied. Same reason for deferring as `pending_resize`.
    pending_visibility: Option<(bool, bool)>,
    /// Last paste announced by the browser, waiting to be applied
    /// (sub-block P2 of the clipboard workstream). Same reason for deferring as
    /// `pending_resize`: `memoriser_controle` runs during the drain of
    /// `poll_output`, and str0m imposes a single mutation of `Rtc` per call.
    ///
    /// ⚠️ **A paste is an EVENT, and yet it is stored as a
    /// STATE — overwriting the last one. The cost is real, and it is written down:** two
    /// pastes arriving between two loop rounds reduce to the second, the
    /// first being **lost without a trace**. It is acceptable because a loop
    /// round is bounded by the video cadence and a human does not produce
    /// two `Ctrl+V` in that interval — **but a misbehaving client
    /// could**. The remedy would be a bounded queue; it is not
    /// delivered, and it is a legacy of P2.
    pending_clipboard: Option<String>,
    /// The paste was written into the VM's clipboard: `Ctrl+V` remains to
    /// be injected.
    ///
    /// 🔴 **This flag carries D6's ORDER on its own**, and that is why it
    /// exists rather than a direct call. It is only set when the write
    /// **succeeded** (`act_on_timeout`, branch `a1octies`), and it is consumed
    /// by `run` (`transport/boucle.rs`) right after. The write being
    /// synchronous and preceding the setting, the order "the Windows clipboard
    /// first, the key afterwards" is guaranteed **by construction** — no
    /// channel ordering comes into it.
    ///
    /// On write failure, it is **not** set: the `V` key is LOST,
    /// not deferred (D6). A `Ctrl+V` on an unchanged clipboard would paste
    /// the PREVIOUS content, which D6 exists entirely to avoid.
    collage_a_injecter: bool,
    /// Congestion controller. Fed by `Event::EgressBitrateEstimate`
    /// and `Event::MediaEgressStats`, both already emitted by str0m — the
    /// second was even already emitted before this workstream, and fell into the `_ =>
    /// {}` of `handle_event`.
    congestion: congestion::Controleur,
    /// Last estimate received, with the instant it was received, waiting
    /// to be matched against the statistics. The two events do not arrive
    /// together.
    ///
    /// **Timestamped since I4 (final branch review).** Without the instant, an
    /// estimate received once and then never again (TWCC drying up)
    /// would stay in use indefinitely — see `EXPIRATION_ESTIMATION`, which
    /// treats it as absent beyond its delay.
    derniere_estimation_bps: Option<(u32, Instant)>,
    /// Decision decided but not yet applied. Applied in
    /// `act_on_timeout`, never from `handle_event` — rebuilding
    /// the encoder during the drain of `poll_output` would break str0m's
    /// invariant (a single mutation per call), exactly as for
    /// `pending_resize`.
    pending_decision: Option<congestion::Decision>,
    /// True once the unavailability of adaptation has been logged.
    /// A permanent condition is not logged every second.
    absence_bwe_signalee: bool,
    /// True once the unavailability of adaptation has been announced TO THE
    /// BROWSER (`Link` message). A flag distinct from `absence_bwe_signalee`,
    /// which only covers the log.
    ///
    /// **Added for I2 (final branch review).** Before this fix,
    /// `Controleur::observer` returned `None` straight away when no estimate
    /// was available, so no `pending_decision` was ever
    /// produced for that case — `Adaptation::Indisponible` never reached
    /// the browser, whereas the spec demands it by name ("above all not a
    /// silence that looks like everything is fine").
    ///
    /// Reset to `false` as soon as a fresh estimate comes back: a later
    /// unavailability (new TWCC outage, see I4) is new
    /// information, to be announced again — as `taille_refus_signalee`
    /// resets to `None` as soon as a size change succeeds.
    indisponibilite_annoncee: bool,
    /// Encoding size actually applied. Distinct from the decided one:
    /// an encoder refusal leaves the decision unapplied, and it must not
    /// be retried at every round.
    encode_size_appliquee: (u32, u32),
    /// Last encoding size whose refusal was logged. A
    /// permanent condition is not logged every second; on the
    /// other hand, a NEW refused target is new information.
    /// Reset to `None` as soon as a size change succeeds, so that a
    /// later refusal of the same size is told again.
    taille_refus_signalee: Option<(u32, u32)>,
    /// True once the refusal of the hot bitrate change has been logged.
    refus_debit_signale: bool,
    /// Bitrate actually applied by the encoder. Distinct from the decided one:
    /// a driver refusal leaves the encoder at the previous bitrate, and announcing
    /// to the browser a bitrate it does not emit would be a lie of the same
    /// family as the one already fixed on quality (task 5).
    bitrate_applique: u32,
    /// Last instant `source.is_alive()` was queried. This call
    /// costs a system call on the Windows side (window lookup): it is
    /// spaced out rather than redone at every loop round — a
    /// closed window stays closed (see `ALIVE_CHECK_INTERVAL`).
    last_alive_check: Instant,
    /// TURN client, absent as long as no relay is configured or allocated.
    /// Its absence makes the whole relayed path inert.
    pub(super) turn: Option<crate::turn::TurnClient>,
    /// Windows timer resolution lowered to 1 ms for the lifetime of
    /// the session (see `TimerResolutionGuard`). Field never read: its only
    /// reason for being is to live as long as `Session` and to
    /// restore the original resolution on destruction.
    _timer_resolution: TimerResolutionGuard,
    /// Session identifier, set by `set_session_id` (see
    /// `cadence_video.rs`) — empty as long as it has not been (test
    /// paths). Only used to pair the video track's cadence line with
    /// the sensor's (`capteur/fenetre.rs`) in an `agent.log` that
    /// several windows share.
    session_id: String,
    /// Video access units actually written on the track since the
    /// last cadence reading (see `cadence_video::PERIODE_COMPTEURS`).
    /// Incremented by `write_frame` (`piste_video.rs`), never by a loop
    /// round that produces nothing.
    unites_video_ecrites: u64,
    /// Instant of the last cadence reading of the video track.
    dernier_compte_video: Instant,
    /// True once `VideoSource::signaler_audio_mort` has been called for this
    /// capture — the latch that prevents flooding the sensor: `capture_morte`
    /// (`crate::audio::AudioSource`) stays true forever once set, whereas
    /// this field, for its part, falls back to `false` at each reattachment of the channel
    /// to the sensor (`VideoSource::rattachement_survenu`, sub-block D9) —
    /// a restarted sensor has lost the memory of any earlier report.
    audio_mort_signale: bool,
    /// What is needed to rebuild the audio source after its capture died
    /// (sub-block D10). Absent when `AUDIO=0`, under `TEST_FILE`, and when
    /// the initial audio opening failed: the behaviour from before D10 —
    /// report immediately — stays exactly preserved in those cases.
    ///
    /// ❌ **"Absent on the single-window path" appeared here and it is
    /// WRONG**: `demarrage/audio.rs::brancher` sets this field
    /// UNCONDITIONALLY in its `Ok` arm, `None` branch included. The
    /// single-window path therefore does rebuild — its own defect, the rebuilt
    /// source being re-armed there at `false`, ✅ **is legacy no. 4 of D10,
    /// FIXED in D11** (see `reconstruire_ou_signaler`).
    /// ⚠️ **Third occurrence
    /// of this same sentence, and this one was found neither by the cross-cutting
    /// review nor by the final branch review**: both fixed
    /// the twins in `tick.rs` and `tick/tests/audio.rs` without sweeping
    /// this far. A recursive grep of `agent/src` for the phrase listed it nonetheless.
    audio_reconstructeur: Option<Reconstructeur>,
    /// Remaining budget of rebuild attempts, initialised to
    /// `crate::audio::RECONSTRUCTIONS_MAX`. Once exhausted, `reconstruire_ou_signaler`
    /// falls back to reporting — that is where the sensor's promotion of a neighbour
    /// takes its role back.
    reconstructions_restantes: u32,
    /// Instant from which a new rebuild attempt is
    /// allowed. `None`: no attempt has happened yet, or no respite
    /// is in progress.
    ///
    /// **Without this respite**, `reconstruire_ou_signaler` runs on the
    /// `Session::run` thread and opening a WASAPI source there is a blocking call of
    /// unbounded duration: without respite, the tick loop would attempt an
    /// opening at every round.
    prochaine_reconstruction: Option<Instant>,
    /// True as soon as a rebuild has succeeded, as long as no packet has
    /// yet come to confirm it. Distinguishes a DECISION (the rebuild
    /// returned `Ok`) from a PROOF (a packet was actually produced) — that is
    /// the whole difference `SourceVivante::sans_paquet` exists to
    /// exercise (legacy 6).
    audio_reconstruit_sans_preuve: bool,
    /// True as soon as a REAL packet has confirmed — the PROOF, not the decision —
    /// that the rebuilt audio capture produces sound again: it remains to
    /// announce `VersCapteur::AudioVivant` to the sensor.
    ///
    /// Set by `brancher_audio`, right after `audio_reconstruit_sans_preuve`
    /// falls back (see that field); consumed — reset to `false` — by the
    /// a1sexies branch of `act_on_timeout`, which then calls
    /// `VideoSource::signaler_audio_vivant`. It is what closes legacy 6 of
    /// D9: `REARMEMENTS_MAX` (`capteur/sommeil.rs`) restarts from zero on THIS
    /// signal, never on the re-election decision alone.
    audio_vivant_a_annoncer: bool,
    /// LOCAL mirror of the last audio order received — or, in single-window mode, of the mode
    /// itself. **TWO writers**: `appliquer_audio` on the sensor's order, and
    /// `set_audio_porteuse` at single-window wiring (`demarrage/audio.rs`,
    /// legacy 4 of D10), where no sensor will ever arbitrate this session.
    ///
    /// ❌ **"Used ONLY to detect the TRANSITION to `actif = true`" —
    /// written here, and WRONG since D10 itself.** The field has **two** readers:
    /// `appliquer_audio`'s transition, which replenishes
    /// `reconstructions_restantes` and lifts `audio_mort_signale`; and the
    /// `set_actif(self.audio_porteuse)` re-arming of
    /// `reconstruire_ou_signaler`, which is not one. The doc predates this
    /// second reader and was not reread when it arrived.
    ///
    /// ⚠️ **Without this field, the dead → rebuilt → proven cycle would only turn
    /// ONCE** (review of task 12, D10): `reconstructions_restantes`
    /// was never reloaded, and `audio_mort_signale`, never lifted, closed
    /// the door of `reconstruire_ou_signaler` for good.
    audio_porteuse: bool,
}

impl Session {
    /// Prepares a session waiting for an offer.
    ///
    /// `local_ip` is the address through which the browser will reach the agent.
    /// `clock_origin` is the session's clock origin, shared with the
    /// audio source (see `capture_instant`): it is what makes both
    /// timelines comparable and hence A/V sync exact.
    pub fn new(
        source: Box<dyn VideoSource + Send>,
        local_ip: IpAddr,
        clock_origin: Instant,
        plafond_bps: u32,
    ) -> Result<Self> {
        // UDP socket and str0m `Rtc` in their initial state: self-contained
        // code, without access to `Session`'s fields, extracted to
        // `initialisation.rs` (review of task 12, sub-block D10).
        let (socket, rtc) = initialisation::construire_rtc(local_ip, plafond_bps)?;

        let dimensions = source.dimensions();
        let mut session = Self {
            rtc,
            socket,
            source,
            dimensions,
            clock_origin,
            video_mid: None,
            control_channel: None,
            input_channel: None,
            started: Instant::now(),
            pending_control: VecDeque::new(),
            outbound_control: None,
            ending: false,
            next_frame_at: Instant::now() + FRAME_INTERVAL,
            warned_negotiation: false,
            consecutive_recv_errors: 0,
            video_write_pending_drain: false,
            audio_source: None,
            audio_mid: None,
            mic_mid: None,
            puits_micro: None,
            warned_micro_negotiation: false,
            refus_micro_signale: false,
            journaux_micro: 0,
            exclusivite_annoncee: None,
            audio_write_pending_drain: false,
            warned_audio_negotiation: false,
            pending_resize: None,
            pending_visibility: None,
            pending_clipboard: None,
            collage_a_injecter: false,
            congestion: congestion::Controleur::new(
                congestion::Config {
                    plafond_bps,
                    // References `opus::BITRATE_BPS` rather than a duplicated
                    // constant (I5, final branch review): a value hardcoded
                    // here could silently diverge from what the
                    // Opus encoder really uses.
                    audio_bps: crate::opus::BITRATE_BPS as u32,
                    source: dimensions,
                    // **Deliberately 60, NOT `ENCODER_FPS`** (I5, final
                    // branch review). `ENCODER_FPS` (default 90, see `demarrage.rs`)
                    // is the encoder's SOLICITATION cadence, not the
                    // DELIVERED cadence — acceptance measures 55 to 63 fps
                    // actually decoded, much closer to 60 than to 90.
                    // And above all: `BPP_MIN` (see `congestion/echelle.rs`) was
                    // calibrated with `fps = 60`. `fps` directly multiplies
                    // all the ladder's `min_bps` — making it follow
                    // `ENCODER_FPS` would multiply all thresholds by 1.5 and
                    // invalidate an already fragile calibration (carried over
                    // without visual proof, see the comment of
                    // `BPP_MIN`), without a measurement to redo it. `BPP_MIN` and this
                    // `fps` are COUPLED and must be recalibrated TOGETHER,
                    // never one without the other.
                    fps: 60,
                },
                Instant::now(),
            ),
            derniere_estimation_bps: None,
            pending_decision: None,
            absence_bwe_signalee: false,
            indisponibilite_annoncee: false,
            encode_size_appliquee: dimensions,
            taille_refus_signalee: None,
            refus_debit_signale: false,
            // As the controller initialises its own: before any applied
            // decision, the real bitrate is the fallback one, the ceiling.
            bitrate_applique: plafond_bps,
            last_alive_check: Instant::now(),
            turn: None,
            _timer_resolution: TimerResolutionGuard::new(),
            session_id: String::new(),
            unites_video_ecrites: 0,
            dernier_compte_video: Instant::now(),
            audio_mort_signale: false,
            audio_reconstructeur: None,
            reconstructions_restantes: crate::audio::RECONSTRUCTIONS_MAX,
            prochaine_reconstruction: None,
            audio_reconstruit_sans_preuve: false,
            audio_vivant_a_annoncer: false,
            audio_porteuse: false,
        };

        // `add_local_candidate` is a mutation: we drain before giving
        // control back, so as never to depend on what the caller will do after
        // `new()`.
        session.drain_quietly()?;

        Ok(session)
    }
}
