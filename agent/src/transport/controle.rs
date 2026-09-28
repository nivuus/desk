//! The control channel: queueing outgoing messages, draining
//! those produced outside the loop, and session end.

use anyhow::Result;
use proto::control::AgentControl;

use super::tick::Tick;
use super::Session;

impl Session {
    /// Queues a control message to send as soon as the channel is
    /// available. Never mutates `Rtc`: the actual sending happens in
    /// `run()`, the only place that mutates the session once the loop has started.
    pub(super) fn queue_control(&mut self, message: AgentControl) {
        self.pending_control.push_back(message);
    }

    /// Wires an external source of control messages. Same pattern as
    /// `set_audio_source`: the session pulls, it is never pushed.
    pub fn set_control_source(&mut self, rx: std::sync::mpsc::Receiver<AgentControl>) {
        self.outbound_control = Some(rx);
    }

    /// Branch `a0bis` of the priority list (see `tick`): a control
    /// message produced outside the loop is waiting.
    ///
    /// `try_recv` never blocks. We give control back immediately after
    /// queueing it, like branches a1 and a2: `queue_control` does not
    /// mutate `Rtc`, but keeping a single action per round is what makes
    /// the priority list readable.
    ///
    /// The queue is bounded: if the control channel is not open yet,
    /// messages would pile up in it without limit. Beyond the ceiling we
    /// stop draining — the producers (cursor, rumble) emit
    /// STATES, of which only the last counts, and the mpsc channel will buffer
    /// meanwhile.
    ///
    /// Returns `None` when nothing was waiting: no mutation, neither of `Rtc` nor
    /// of the queue, then took place.
    pub(super) fn drainer_controle_externe(&mut self) -> Option<Tick> {
        const PLAFOND_CONTROLE_EN_FILE: usize = 32;
        if self.pending_control.len() >= PLAFOND_CONTROLE_EN_FILE {
            return None;
        }
        let message = self.outbound_control.as_ref()?.try_recv().ok()?;
        self.queue_control(message);
        Some(Tick::Continue)
    }

    /// Branch `a` of the priority list (see `tick`): emits the first
    /// queued control message.
    ///
    /// Returns `Some(Tick::Continue)` after writing a message — writing
    /// on the channel is a mutation of `Rtc`, which therefore concludes the round.
    /// Returns `Some(Tick::Disconnected)` when the session closes without being able
    /// to inform the browser. Returns `None` — without having mutated `Rtc` — when
    /// the queue is empty, or the channel is not open yet while the
    /// session is not closing: the message then stays queued, we
    /// retry at the next round, and the priority list can move on to the
    /// next branch without breaking the drain invariant.
    pub(super) fn brancher_controle_en_file(&mut self) -> Result<Option<Tick>> {
        // The emptiness test is HERE and not at the caller: it is what
        // guarantees the `expect` of the `pop_front` below. A precondition
        // left in `tick` would be held by convention, whereas it is held
        // by construction as long as it stays in the method it protects.
        //
        // It precedes the channel test: on an EMPTY queue, there is nothing to
        // announce to the browser, hence nothing to regret not being able to
        // send it — the warning of the `else` below would have no reason
        // to be. Closing a session with nothing queued is handled by
        // the `if self.ending` of `tick`, right after this branch.
        if self.pending_control.is_empty() {
            return Ok(None);
        }

        let Some(id) = self.control_channel else {
            if self.ending {
                // Session end requested but control channel
                // unavailable (never opened, or closed): impossible to
                // inform the browser, but we do not wait indefinitely
                // for a channel that will not open.
                tracing::warn!(
                    "fin de session sans canal de contrôle disponible pour en informer le navigateur"
                );
                return Ok(Some(Tick::Disconnected));
            }
            // Channel not open yet, session not closing: the
            // message stays queued, we retry at the next round.
            return Ok(None);
        };

        let message = self.pending_control.pop_front().expect("non vide");
        // Variant name for logging purposes only: workstream B's
        // acceptance run (measurements 3 and 5) had to work around this
        // path's observability with temporary client instrumentation, for lack of a line
        // here — this `tracing::debug!` exists so that the next diagnosis
        // no longer needs that workaround.
        let type_message = match &message {
            AgentControl::Ready { .. } => "ready",
            AgentControl::SessionEnd { .. } => "session-end",
            AgentControl::Pointer { .. } => "pointer",
            AgentControl::Asleep { .. } => "asleep",
            AgentControl::Rumble { .. } => "rumble",
            AgentControl::Capabilities { .. } => "capabilities",
            AgentControl::Link { .. } => "link",
            AgentControl::Fullscreen { .. } => "fullscreen",
            AgentControl::Clipboard { .. } => "clipboard",
            AgentControl::Accent { .. } => "accent",
            AgentControl::MicState { .. } => "mic-state",
        };
        let json = serde_json::to_string(&message)?;
        if let Some(mut channel) = self.rtc.channel(id) {
            match channel.write(false, json.as_bytes()) {
                Ok(_) => {
                    tracing::debug!(type_message, "message de contrôle écrit");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "échec d'écriture sur le canal de contrôle");
                }
            }
        }
        Ok(Some(Tick::Continue))
    }

    /// Starts a clean session end: queues an
    /// `AgentControl::session_end` and stops sending new images.
    /// Idempotent.
    pub(super) fn begin_ending(&mut self, reason: &str) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.pending_control
            .push_back(AgentControl::session_end(reason));
        tracing::info!(reason, "clôture de session amorcée");
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use str0m::{Event, Input, Output};

    use super::*;
    use crate::transport::fixtures;

    #[test]
    fn relaie_au_pair_un_controle_pousse_depuis_l_exterieur_de_la_boucle() {
        use proto::control::CursorShape;
        use std::sync::mpsc;
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, MediaKind};

        let local_ip = fixtures::local_ip();
        let source = Box::new(fixtures::video_test_source());
        let mut session =
            Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");
        let answer_sdp = session
            .accept_offer(&offer.to_sdp_string())
            .expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée");

        // That is the point of the test: the message is produced NEITHER by the loop,
        // NOR by a str0m event — it comes from a third party, as the
        // cursor polling thread will.
        let (tx, rx) = mpsc::channel();
        session.set_control_source(rx);
        tx.send(AgentControl::pointer(false, CursorShape::Default))
            .expect("envoi dans le canal");

        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        // The peer's loop: drives its `Rtc` and watches for the expected message on
        // the control channel. Hard bound so as not to hang if nothing arrives.
        peer_socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("délai de lecture");
        let mut buf = vec![0u8; 4096];
        let debut = Instant::now();
        let mut recu = false;
        while !recu && debut.elapsed() < Duration::from_secs(15) {
            if let Ok((n, from)) = peer_socket.recv_from(&mut buf) {
                let contents: str0m::net::DatagramRecv = buf[..n].try_into().unwrap();
                let _ = peer_rtc.handle_input(Input::Receive(
                    Instant::now(),
                    str0m::net::Receive {
                        proto: str0m::net::Protocol::Udp,
                        source: from,
                        destination: peer_addr,
                        contents,
                    },
                ));
            }
            while let Ok(output) = peer_rtc.poll_output() {
                match output {
                    Output::Timeout(_) => break,
                    Output::Transmit(t) => {
                        let _ = peer_socket.send_to(&t.contents, t.destination);
                    }
                    Output::Event(Event::ChannelData(data)) => {
                        let texte = String::from_utf8_lossy(&data.data);
                        if texte.contains("\"type\":\"pointer\"") {
                            assert!(texte.contains("\"visible\":false"), "charge : {texte}");
                            recu = true;
                        }
                    }
                    Output::Event(_) => {}
                }
            }
        }

        assert!(recu, "le message de pointeur n'est jamais parvenu au pair");
    }
}
