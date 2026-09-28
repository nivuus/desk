//! TURN relay: allocation before the SDP answer, and routing packets between
//! the direct socket and the relay server.
//!
//! `impl Session` block in a sibling module — same split as
//! `redimensionnement` or `adaptation`, and for the same reason: `transport.rs`
//! only carries the state and the loop, and the 500-lines-per-file limit
//! forbids adding it there.
//!
//! Everything this module carries is inert when `Session::turn` is `None` —
//! that is, when no relay is configured. An allocation failure
//! degrades the session (host candidates only), it never kills it.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Input};

use super::tick::Tick;
use super::Session;

impl Session {
    /// Single emission point. Routes to the direct socket or to the TURN
    /// relay depending on what str0m indicates as source.
    ///
    /// A transient send error is logged and ignored, never
    /// propagated: the peer closing its port must not end the session
    /// (I2 of the milestone 1 review).
    pub(super) fn envoyer(&mut self, transmit: &str0m::net::Transmit) {
        let (donnees, destination) = match self.route_relayee(transmit) {
            Some(trame) => (trame, self.turn.as_ref().expect("relais présent").serveur()),
            None => (transmit.contents.to_vec(), transmit.destination),
        };
        if let Err(e) = self.socket.send_to(&donnees, destination) {
            tracing::warn!(erreur = %e, "échec d'envoi UDP, ignoré");
        }
    }

    /// ChannelData frame to send to the TURN server, or `None` if this packet
    /// goes direct.
    fn route_relayee(&mut self, transmit: &str0m::net::Transmit) -> Option<Vec<u8>> {
        let turn = self.turn.as_mut()?;
        let allocation = turn.allocation()?;
        // str0m names as source the address of the local candidate used, hence
        // the relayed address for a packet that must go through the relay.
        //
        // OBSERVED, not assumed (task 6, step 1): on a real session of
        // 07/30/2026, the `Transmit`s carried two distinct sources —
        // `192.168.3.2:60303` (the VM's local socket) for the direct path,
        // and `192.168.3.1:49183` for the relayed path, an address in coturn's
        // relay range (49160-49200), hence the relayed address itself.
        if transmit.source != allocation.relayee {
            return None;
        }
        // Common case, on the media path: the channel is already bound, a single
        // encapsulation happens. The plan encapsulated a first time to
        // TEST then a second to produce — that is, a complete frame
        // built then thrown away at every packet, 60 times per second.
        if let Some(trame) = turn.encapsuler(transmit.destination, &transmit.contents) {
            return Some(trame);
        }
        // Peer still without a channel: bind it on the fly. The very first packet
        // to a new peer then goes direct and will probably be lost —
        // ICE re-sends its connectivity checks, so it is not a gap,
        // only one round trip of delay.
        turn.lier_canal(transmit.destination);
        turn.encapsuler(transmit.destination, &transmit.contents)
    }

    /// Handles a datagram coming from the TURN server: either a service message
    /// (allocation response, 401, 438), or relayed data to
    /// decapsulate before presenting it to str0m as coming from the peer.
    ///
    /// Extracted from the receive loop of `socket.rs`, which only calls this
    /// when the datagram's source IS the relay server.
    pub(super) fn traiter_paquet_turn(&mut self, recu: &[u8]) -> Result<()> {
        if !crate::turn::est_channel_data(recu) {
            if let Some(turn) = self.turn.as_mut() {
                if let Err(e) = turn.handle_packet(recu) {
                    tracing::warn!(erreur = %e, "message TURN illisible, ignoré");
                }
            }
            return Ok(());
        }

        let turn = self
            .turn
            .as_ref()
            .expect("présent, testé par l'appelant avant de router ici");
        let Some((pair, charge)) = turn.desencapsuler(recu) else {
            tracing::debug!("trame ChannelData illisible, ignorée");
            return Ok(());
        };
        // The payload is copied: `charge` borrows `self.turn`, and
        // `handle_input` needs `&mut self.rtc`.
        let charge = charge.to_vec();
        let allocation_relayee = turn.allocation().map(|a| a.relayee);

        match DatagramRecv::try_from(&charge[..]) {
            Ok(contents) => {
                let receive = Receive {
                    proto: Protocol::Udp,
                    source: pair,
                    // The destination is the RELAYED address, not the
                    // local socket's: it is the candidate the peer wrote to,
                    // and str0m pairs its candidate pairs on that.
                    destination: match allocation_relayee {
                        Some(relayee) => relayee,
                        None => self.socket.local_addr()?,
                    },
                    contents,
                };
                self.rtc
                    .handle_input(Input::Receive(Instant::now(), receive))
                    .map_err(|e| anyhow!("handle_input relayé : {e}"))?;
            }
            Err(e) => {
                tracing::debug!(erreur = %e, "charge relayée non reconnue");
            }
        }
        Ok(())
    }

    /// Emits the next pending TURN request, if there is one.
    ///
    /// Never mutates `Rtc`: it is an exchange with the relay server,
    /// invisible to str0m. Returns `Some(Tick::Continue)` when a packet
    /// went out, so that the loop round stops there.
    pub(super) fn emettre_requete_turn(&mut self) -> Option<Tick> {
        let turn = self.turn.as_mut()?;
        turn.avancer(Instant::now());
        let paquet = turn.poll_transmit()?;
        let serveur = turn.serveur();
        if let Err(e) = self.socket.send_to(&paquet, serveur) {
            tracing::warn!(erreur = %e, "échec d'envoi vers le serveur TURN, ignoré");
        }
        Some(Tick::Continue)
    }
    /// Allocates a TURN relay and adds the matching candidate, blocking
    /// until success or until the timeout.
    ///
    /// Blocking here is deliberate and bounded: without trickle ICE, the candidate
    /// must exist before the SDP answer (see the caller). A failure is
    /// not fatal — the session continues with host candidates.
    pub fn allouer_relais(
        &mut self,
        config: crate::signaling::ConfigIce,
        delai: Duration,
    ) -> Result<()> {
        let local = self.socket.local_addr()?;
        let mut turn = crate::turn::TurnClient::new(
            config.serveur,
            config.username,
            config.credential,
            Instant::now(),
        );

        let echeance = Instant::now() + delai;
        let mut buffer = vec![0u8; 2000];
        let allocation = loop {
            while let Some(paquet) = turn.poll_transmit() {
                self.socket.send_to(&paquet, config.serveur)?;
            }
            if let Some(a) = turn.allocation() {
                break a;
            }
            if Instant::now() >= echeance {
                anyhow::bail!("aucune allocation TURN obtenue en {:?}", delai);
            }
            match self.socket.recv_from(&mut buffer) {
                Ok((n, source)) if source == config.serveur => {
                    if let Err(e) = turn.handle_packet(&buffer[..n]) {
                        tracing::debug!(erreur = %e, "paquet TURN ignoré pendant l'allocation");
                    }
                }
                // A datagram coming from elsewhere during allocation is
                // noise: the socket is not yet known to the peer.
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(e).context("réception pendant l'allocation TURN"),
            }
        };

        self.rtc.add_local_candidate(
            Candidate::relayed(allocation.relayee, local, "udp")
                .map_err(|e| anyhow!("candidat relayé invalide : {e}"))?,
        );
        if let Some(reflexive) = allocation.reflexive {
            // The same Allocate response carries the reflexive address: one more
            // candidate, without an extra exchange.
            match Candidate::server_reflexive(reflexive, local, "udp") {
                Ok(c) => {
                    self.rtc.add_local_candidate(c);
                }
                Err(e) => tracing::warn!(erreur = %e, "candidat réflexif invalide, ignoré"),
            }
        }
        self.turn = Some(turn);
        // `add_local_candidate` mutates `Rtc`: drain before giving control back,
        // as `Session::new` already does.
        self.drain_quietly()?;
        Ok(())
    }
}
