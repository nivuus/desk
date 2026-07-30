//! Relais TURN : allocation avant la réponse SDP, et routage des paquets entre
//! le socket direct et le serveur de relais.
//!
//! Bloc `impl Session` dans un module frère — même découpage que
//! `redimensionnement` ou `adaptation`, et pour la même raison : `transport.rs`
//! ne porte que l'état et la boucle, et la limite de 500 lignes par fichier
//! interdit de l'y ajouter.
//!
//! Tout ce que porte ce module est inerte quand `Session::turn` vaut `None` —
//! c'est-à-dire quand aucun relais n'est configuré. Une panne d'allocation
//! dégrade la session (candidats hôtes seuls), elle ne la tue jamais.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Input};

use super::tick::Tick;
use super::Session;

impl Session {
    /// Point d'émission unique. Route vers le socket direct ou vers le relais
    /// TURN selon ce que str0m indique comme source.
    ///
    /// Une erreur d'envoi transitoire est journalisée et ignorée, jamais
    /// remontée : le pair qui ferme son port ne doit pas terminer la session
    /// (I2 de la revue du jalon 1).
    pub(super) fn envoyer(&mut self, transmit: &str0m::net::Transmit) {
        let (donnees, destination) = match self.route_relayee(transmit) {
            Some(trame) => (
                trame,
                self.turn.as_ref().expect("relais présent").serveur(),
            ),
            None => (transmit.contents.to_vec(), transmit.destination),
        };
        if let Err(e) = self.socket.send_to(&donnees, destination) {
            tracing::warn!(erreur = %e, "échec d'envoi UDP, ignoré");
        }
    }

    /// Trame ChannelData à envoyer au serveur TURN, ou `None` si ce paquet
    /// part en direct.
    fn route_relayee(&mut self, transmit: &str0m::net::Transmit) -> Option<Vec<u8>> {
        let turn = self.turn.as_mut()?;
        let allocation = turn.allocation()?;
        // str0m nomme comme source l'adresse du candidat local employé, donc
        // l'adresse relayée pour un paquet qui doit passer par le relais.
        // Constaté sur session réelle (tâche 6, étape 1), pas supposé.
        if transmit.source != allocation.relayee {
            return None;
        }
        // Le pair peut n'avoir pas encore de canal : le lier à la volée. Le
        // tout premier paquet vers un pair neuf part alors en direct et sera
        // probablement perdu — ICE réémet ses contrôles de connectivité, donc
        // ce n'est pas un trou, seulement un aller-retour de retard.
        if turn
            .encapsuler(transmit.destination, &transmit.contents)
            .is_none()
        {
            turn.lier_canal(transmit.destination);
        }
        turn.encapsuler(transmit.destination, &transmit.contents)
    }

    /// Traite un datagramme venu du serveur TURN : soit un message de service
    /// (réponse d'allocation, 401, 438), soit des données relayées à
    /// désencapsuler avant de les présenter à str0m comme venant du pair.
    ///
    /// Extrait de la boucle de réception de `socket.rs`, qui n'appelle ceci
    /// que lorsque la source du datagramme EST le serveur de relais.
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
        // La charge utile est recopiée : `charge` emprunte `self.turn`, et
        // `handle_input` a besoin de `&mut self.rtc`.
        let charge = charge.to_vec();
        let allocation_relayee = turn.allocation().map(|a| a.relayee);

        match DatagramRecv::try_from(&charge[..]) {
            Ok(contents) => {
                let receive = Receive {
                    proto: Protocol::Udp,
                    source: pair,
                    // La destination est l'adresse RELAYÉE, pas celle du
                    // socket local : c'est le candidat auquel le pair a écrit,
                    // et str0m apparie ses paires là-dessus.
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

    /// Émet la prochaine requête TURN en attente, s'il y en a une.
    ///
    /// Ne mute jamais `Rtc` : c'est un échange avec le serveur de relais,
    /// invisible de str0m. Rend `Some(Tick::Continue)` quand un paquet est
    /// parti, pour que le tour de boucle s'arrête là.
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
    /// Alloue un relais TURN et ajoute le candidat correspondant, en bloquant
    /// jusqu'au succès ou jusqu'au délai.
    ///
    /// Bloquer est ici volontaire et borné : sans trickle ICE, le candidat
    /// doit exister avant la réponse SDP (voir l'appelant). Un échec n'est
    /// pas fatal — la session continue avec les candidats hôtes.
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
                // Un datagramme venu d'ailleurs pendant l'allocation est du
                // bruit : le socket n'est pas encore connu du pair.
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
            // La même réponse Allocate porte l'adresse réflexive : un
            // candidat de plus, sans échange supplémentaire.
            match Candidate::server_reflexive(reflexive, local, "udp") {
                Ok(c) => {
                    self.rtc.add_local_candidate(c);
                }
                Err(e) => tracing::warn!(erreur = %e, "candidat réflexif invalide, ignoré"),
            }
        }
        self.turn = Some(turn);
        // `add_local_candidate` mute `Rtc` : drainer avant de rendre la main,
        // comme le fait déjà `Session::new`.
        self.drain_quietly()?;
        Ok(())
    }
}
