//! Handling of the events str0m reports: ICE state change,
//! channel opening, data received, outgoing bitrate estimate.
//!
//! These functions run DURING the drain of `poll_output`: none
//! may mutate `Rtc`. What requires a reconfiguration (resize,
//! adaptation decision) is only stored — `pending_resize`,
//! `pending_decision` — and `tick`'s priority list applies it at the next
//! round, as a step in its own right.

use std::time::Instant;

use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::{Event, IceConnectionState};

/// What to do with a frame received on a data channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Destination {
    Entree,
    Controle,
    /// The channel is neither `input` nor `control` — or that one is not open
    /// yet. The frame is **refused**, never interpreted at random.
    Ignoree,
}

/// Decides where a frame goes, **by its CHANNEL and not by its binary flag**.
///
/// 🔴 `binaire` IS NO LONGER A PARAMETER, AND THAT IS THE WHOLE FIX. The old
/// version routed on it alone: any binary frame, whatever its
/// channel, went into `InputMessage::decode`. The label was nevertheless
/// available in `Event::ChannelOpen(id, label)` and simply unused.
/// Now each channel carries the contract of its payload — `input` binary,
/// `control` JSON —, and a frame arriving through neither is
/// refused rather than guessed.
///
/// ⚠️ **No existing frame changes destination**: today
/// `control` writes as `false` (`transport/controle.rs`) and `input` is the only
/// binary channel of a child's `PeerConnection`. What changes is what
/// happens to a frame from a THIRD channel — yesterday a mouse input decoded at
/// random, today a named refusal.
///
/// 🔴 **GENERIC OVER THE IDENTIFIER, and that is what makes it testable.**
/// `str0m::channel::ChannelId` cannot be built outside str0m's
/// crate ("Deliberately not Deref or From to avoid this Id being created
/// outside of this module"): a signature requiring it would only let us
/// exercise the cases a real SDP negotiation can produce. Yet the
/// most important case — a frame received BEFORE its channel's `ChannelOpen`,
/// that is, the INITIAL state of every session — is not one of them.
/// ⚠️ **THE ORDER OF THE TWO COMPARISONS IS UNOBSERVABLE, and that is MEASURED, not
/// assumed**: the mutation that swaps them was played and SURVIVED the
/// nine tests. It is EQUIVALENT, and the proof fits in one sentence — an
/// `Event::ChannelOpen(id, label)` carries ONE label, str0m gives a distinct `ChannelId`
/// per SCTP stream, so `canal_entree` and `canal_controle` can
/// never carry the same identifier. Writing a test that froze the priority
/// would pin an unreachable behaviour; that is why there is none.
pub(super) fn destination<T: PartialEq>(
    recu: T,
    canal_entree: Option<T>,
    canal_controle: Option<T>,
) -> Destination {
    if canal_entree.is_some_and(|c| c == recu) {
        Destination::Entree
    } else if canal_controle.is_some_and(|c| c == recu) {
        Destination::Controle
    } else {
        Destination::Ignoree
    }
}

use super::tick::Tick;
use super::Session;
use crate::congestion;

impl Session {
    pub(super) fn handle_event(
        &mut self,
        event: Event,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Tick {
        match event {
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                tracing::warn!("ICE disconnected");
                return Tick::Disconnected;
            }
            Event::IceConnectionStateChange(state) => {
                tracing::info!(?state, elapsed = ?self.started.elapsed(), "ICE state");
            }
            Event::Closed => {
                // I1: emitted on receiving the DTLS close_notify — typically
                // when the user closes the tab. Without this arm, this
                // event fell into `_ => {}` and the agent kept
                // emitting until ICE expiry, long after the peer
                // had left.
                tracing::info!("connection closed by the peer (DTLS close_notify)");
                return Tick::Disconnected;
            }
            Event::MediaAdded(media) => {
                // ⚠️ The `direction` field is carried by the trace ON PURPOSE:
                // with TWO audio m-lines, an acceptance log does not let one
                // tell the two tracks apart without it. It is the lesson
                // "an unattributable trace costs a re-imputation" (D6).
                tracing::info!(
                    mid = ?media.mid,
                    kind = ?media.kind,
                    direction = ?media.direction,
                    "track negotiated"
                );
                // ⚠️ **The direction discriminates, and without it this `match` is a
                // SILENT defect.** It set `audio_mid` for any audio track:
                // as soon as a SECOND audio m-line exists — workstream
                // E's microphone, `recvonly` on our side —, it overwrote the first, and
                // workstream A's downstream sound went out on a track that
                // we cannot emit. No error, no `WARN`,
                // just silence. The semantic red that exhibits it is filed
                // in `journaux-micro/tache-7-rouge.txt`.
                //
                // `MediaAdded::direction` is the LOCAL direction: str0m
                // inverts the remote direction when accepting an offer
                // (`change/sdp.rs`, `let new_dir = m.direction().invert();`).
                // Checked by MEASUREMENT and not by reading alone — probe
                // 1 (`transport::sonde_montante`) sees the receiver
                // announce `RecvOnly` on a track offered as `SendOnly`.
                use str0m::media::{Direction, MediaKind};
                match (media.kind, media.direction) {
                    (MediaKind::Video, _) => self.video_mid = Some(media.mid),
                    // Workstream A's sound: the agent EMITS.
                    (MediaKind::Audio, Direction::SendOnly | Direction::SendRecv) => {
                        self.audio_mid = Some(media.mid)
                    }
                    // Workstream E's microphone: the agent RECEIVES.
                    (MediaKind::Audio, Direction::RecvOnly) => self.mic_mid = Some(media.mid),
                    // Switched off by the peer: neither one nor the other. Filing it
                    // somewhere would make it take the place of a live
                    // track.
                    (MediaKind::Audio, Direction::Inactive) => {}
                }
            }
            Event::MediaData(data) => {
                if Some(data.mid) == self.mic_mid {
                    self.deposer_micro(&data);
                }
                // Any other `MediaData` is ignored: the agent receives no
                // other media. This arm fell into the `_ => {}` catch-all; it
                // is NAMED here so that the next incoming media does not fall
                // silently.
                //
                // ⚠️ A catch-all arm has already cost FOUR times in this repository
                // (`capteur/pont_media.rs`, D5/D6/D7/D8). This one is benign —
                // it ignores, it kills nothing —, but naming it costs three
                // lines and avoids the fifth.
            }
            Event::ChannelOpen(id, label) => {
                tracing::info!(%label, "data channel open");
                // 🔴 THE LABEL IS RETAINED FOR BOTH CHANNELS, no longer only
                // for `control`. Without `input_channel`, `dispatch_channel_data`
                // only has the binary flag to decide, and takes any binary
                // frame for a mouse input.
                if label == "input" {
                    self.input_channel = Some(id);
                }
                if label == "control" {
                    self.control_channel = Some(id);
                    // I3: send `ready` here, not right after the SDP
                    // answer — at that moment SCTP is not open yet,
                    // `control_channel` was `None`, and the message was
                    // silently lost. Task 8's client waits for
                    // this message to clear its status banner.
                    let (width, height) = self.dimensions;
                    // The microphone (workstream E): available only if an
                    // upstream track was negotiated AND a sink is there.
                    let mic = self.micro_disponible();
                    self.queue_control(AgentControl::ready(width, height, mic));
                }
            }
            Event::ChannelData(data) => {
                self.dispatch_channel_data(&data, on_input, on_control);
            }
            Event::KeyframeRequest(request) => {
                // The browser requests a keyframe, typically after a
                // packet loss detected by the decoder. The hardware encoder's group
                // of pictures is open (see
                // `encode::configure_rate_control`): without this relay, no
                // keyframe is ever produced again after startup, and
                // the loss corrupts the video until reconnection. Does not mutate
                // `Rtc` — only the encoder (on the `VideoSource` side) is affected —
                // so this relay respects the drain invariant documented
                // at the head of the file even when called from `handle_event`.
                tracing::debug!(mid = ?request.mid, "key frame requested by the peer");
                if let Err(e) = self.source.request_keyframe() {
                    // `cause::chain`: same `commander_simple` chain as
                    // the wake-up. See `crate::cause`.
                    tracing::warn!(
                        error = %crate::cause::chain(&e),
                        mid = ?request.mid,
                        "key frame request failed"
                    );
                }
            }
            Event::EgressBitrateEstimate(kind) => {
                // Both variants carry an estimate; only REMB
                // also names the `mid` concerned, which we have no use for
                // with a single video track.
                let bps = match kind {
                    str0m::bwe::BweKind::Twcc(b) => b.as_u64(),
                    str0m::bwe::BweKind::Remb(_, b) => b.as_u64(),
                    // `BweKind` is `#[non_exhaustive]` on str0m's side: a
                    // future variant would fall here rather than prevent
                    // compilation. Nothing better to do than ignore an
                    // estimate we do not yet know how to interpret.
                    _ => return Tick::Continue,
                };
                self.derniere_estimation_bps = Some((bps as u32, Instant::now()));
            }
            Event::MediaEgressStats(stats) => {
                // Only the video track feeds the decision: audio has a
                // fixed bitrate and its budget is already subtracted by the controller.
                if Some(stats.mid) != self.video_mid {
                    return Tick::Continue;
                }
                // I4 (final branch review): an estimate received
                // once and then never again (TWCC drying up while the
                // session survives) is treated as absent beyond
                // `EXPIRATION_ESTIMATION`, rather than being used
                // indefinitely — potentially the last high value
                // before the incident, which would announce "Good" on a dead
                // link.
                let now = Instant::now();
                let estimate_bps = self.estimation_fraiche(now);
                let observation = congestion::Observation {
                    estimate_bps,
                    rtt: stats.rtt,
                    loss: stats.loss,
                    at: now,
                };
                let absence = observation.estimate_bps.is_none();
                if absence && !self.absence_bwe_signalee {
                    self.absence_bwe_signalee = true;
                    tracing::warn!(
                        "no bandwidth estimate received: the adaptation stays \
                         unavailable and the bitrate remains at the configured ceiling"
                    );
                }
                tracing::debug!(
                    estimation = ?observation.estimate_bps,
                    rtt = ?observation.rtt,
                    perte = ?observation.loss,
                    "network observation"
                );
                // `observer` MUST be called before reading `current()`
                // below: it is what, in its branch without an
                // estimate, updates `current.adaptation` to
                // `Indisponible` (see its comment). Reading `current()`
                // before this call would return a stale snapshot (still
                // `Active`) on the transition we care about most.
                if let Some(decision) = self.congestion.observer(observation) {
                    // Stored, not applied: see the field's comment.
                    self.pending_decision = Some(decision);
                }
                if absence {
                    // I2 (final branch review): `Controleur::observer`
                    // NEVER produces a decision when the estimate
                    // is missing (see its comment, early return) — without
                    // this explicit relay, `Adaptation::Indisponible`
                    // therefore never reaches the browser, whereas the spec
                    // demands it by name. We set `self.congestion.current()`,
                    // read AFTER the call above: its `adaptation` field is
                    // now up to date, and the rest (bitrate, size) reflects
                    // the last real decision — the only sensible thing to
                    // announce as long as no new data arrives.
                    if !self.indisponibilite_annoncee {
                        self.indisponibilite_annoncee = true;
                        self.pending_decision = Some(self.congestion.current());
                    }
                } else {
                    // A fresh estimate comes back: a later unavailability
                    // will become new information again.
                    self.indisponibilite_annoncee = false;
                }
            }
            _ => {}
        }
        Tick::Continue
    }

    /// The destination of a received frame, decided by the CHANNEL and not by the
    /// binary flag alone.
    ///
    /// 🔴 FREE and PURE function, and that is what makes it testable: it does not
    /// take a `ChannelId`, which str0m deliberately forbids building
    /// outside its crate ("Deliberately not Deref or From to avoid this Id
    /// being created outside of this module"). Without it, the case "a binary
    /// frame arrives BEFORE any `ChannelOpen`" — that is, the INITIAL state
    /// of every session — would be covered by nothing: no local-peer
    /// setup can produce it, str0m always emitting `ChannelOpen`
    /// first.
    fn dispatch_channel_data(
        &mut self,
        data: &str0m::channel::ChannelData,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) {
        match destination(data.id, self.input_channel, self.control_channel) {
            Destination::Entree => match InputMessage::decode(&data.data) {
                Ok(message) => on_input(message),
                Err(e) => tracing::warn!(error = %e, "invalid input message"),
            },
            Destination::Controle => {
                match std::str::from_utf8(&data.data).map(serde_json::from_str::<ClientControl>) {
                    Ok(Ok(message)) => {
                        self.memoriser_controle(&message);
                        on_control(message);
                    }
                    Ok(Err(e)) => tracing::warn!(error = %e, "invalid control message"),
                    Err(e) => tracing::warn!(error = %e, "non-UTF-8 control"),
                }
            }
            Destination::Ignoree => {
                // ⚠️ `WARN` and not `debug`: it is a channel nobody
                // negotiated for this transport, or a frame arrived before its
                // `ChannelOpen`. Both deserve to be seen.
                tracing::warn!(
                    binaire = data.binary,
                    octets = data.data.len(),
                    "frame received on a channel that is neither `input` nor `control`, ignored"
                );
            }
        }
    }

    /// Stores a received `ClientControl`, without ever applying it on the spot.
    ///
    /// This code runs during the drain of `poll_output`: rebuilding
    /// the encoding chain or releasing an encoder there would be long and would break
    /// str0m's drain invariant (a single mutation of `Rtc` per
    /// call). We only store the most recent request;
    /// `act_on_timeout` applies it in turn, as a step in its own
    /// right.
    fn memoriser_controle(&mut self, message: &ClientControl) {
        match message {
            ClientControl::Resize { width, height, .. } => {
                self.pending_resize = Some((*width, *height));
            }
            ClientControl::Visibility {
                visible, focused, ..
            } => {
                self.pending_visibility = Some((*visible, *focused));
            }
            // Overwriting the last one, like the two above — and for this
            // field the cost is NOT the same: an intermediate resize
            // is of no interest, a lost paste is. See
            // the field's doc, which carries the cost and the undelivered remedy.
            ClientControl::Clipboard { text, .. } => {
                self.pending_clipboard = Some(text.clone());
            }
        }
    }

    /// `#[cfg(test)]` entry point that exercises `memoriser_controle` without
    /// going through a real `str0m::channel::ChannelData` — str0m deliberately
    /// forbids building it outside its own crate (see
    /// `ChannelId`, "Deliberately not Deref or From to avoid this Id being
    /// created outside of this module"). The module's existing integration
    /// tests work around this by mounting a second str0m `Rtc` as a
    /// local peer; this path is lighter for a test that only checks
    /// the storing itself.
    #[cfg(test)]
    pub(super) fn dispatch_controle_de_test(&mut self, json: &str) {
        let message: ClientControl =
            serde_json::from_str(json).expect("valid test json in dispatch_controle_de_test");
        self.memoriser_controle(&message);
    }
}

#[cfg(test)]
#[path = "evenements/tests.rs"]
mod tests;
