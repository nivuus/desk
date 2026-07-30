//! Le canal de contrôle : mise en file des messages sortants, drainage de
//! ceux produits hors de la boucle, et fin de session.

use anyhow::Result;
use proto::control::AgentControl;

use super::tick::Tick;
use super::Session;

impl Session {
    /// Met en file un message de contrôle à envoyer dès que le canal est
    /// disponible. Ne mute jamais `Rtc` : l'envoi effectif a lieu dans
    /// `run()`, seul endroit qui mute la session une fois la boucle démarrée.
    pub(super) fn queue_control(&mut self, message: AgentControl) {
        self.pending_control.push_back(message);
    }

    /// Branche une source externe de messages de contrôle. Même patron que
    /// `set_audio_source` : la session tire, elle n'est jamais poussée.
    pub fn set_control_source(&mut self, rx: std::sync::mpsc::Receiver<AgentControl>) {
        self.outbound_control = Some(rx);
    }

    /// Branche `a0bis` de la liste de priorités (voir `tick`) : un message de
    /// contrôle produit hors de la boucle attend.
    ///
    /// `try_recv` ne bloque jamais. On rend la main immédiatement après
    /// l'avoir mis en file, comme les branches a1 et a2 : `queue_control` ne
    /// mute pas `Rtc`, mais garder une seule action par tour est ce qui rend
    /// la liste de priorités lisible.
    ///
    /// La file est bornée : si le canal de contrôle n'est pas encore ouvert,
    /// les messages s'y accumuleraient sans limite. Au-delà du plafond on
    /// cesse de drainer — les producteurs (curseur, vibration) émettent des
    /// ÉTATS, dont seul le dernier compte, et le canal mpsc fera tampon en
    /// attendant.
    ///
    /// Rend `None` quand rien n'attendait : aucune mutation, ni de `Rtc` ni
    /// de la file, n'a alors eu lieu.
    pub(super) fn drainer_controle_externe(&mut self) -> Option<Tick> {
        const PLAFOND_CONTROLE_EN_FILE: usize = 32;
        if self.pending_control.len() >= PLAFOND_CONTROLE_EN_FILE {
            return None;
        }
        let message = self.outbound_control.as_ref()?.try_recv().ok()?;
        self.queue_control(message);
        Some(Tick::Continue)
    }

    /// Branche `a` de la liste de priorités (voir `tick`) : émet le premier
    /// message de contrôle en file. Appelée uniquement quand la file n'est
    /// pas vide.
    ///
    /// Rend `Some(Tick::Continue)` après avoir écrit un message — l'écriture
    /// sur le canal est une mutation de `Rtc`, qui conclut donc le tour.
    /// Rend `Some(Tick::Disconnected)` quand la session se clôt sans pouvoir
    /// en informer le navigateur. Rend `None` — sans avoir muté `Rtc` — quand
    /// le canal n'est pas encore ouvert et que la session n'est pas en
    /// clôture : le message reste en file, on retente au tour suivant, et la
    /// liste de priorités peut passer à la branche suivante sans rompre
    /// l'invariant de drainage.
    pub(super) fn brancher_controle_en_file(&mut self) -> Result<Option<Tick>> {
        let Some(id) = self.control_channel else {
            if self.ending {
                // Fin de session demandée mais canal de contrôle
                // indisponible (jamais ouvert, ou fermé) : impossible d'en
                // informer le navigateur, mais on n'attend pas indéfiniment
                // un canal qui ne s'ouvrira pas.
                tracing::warn!(
                    "fin de session sans canal de contrôle disponible pour en informer le navigateur"
                );
                return Ok(Some(Tick::Disconnected));
            }
            // Canal pas encore ouvert, session pas en cours de clôture : le
            // message reste en file, on retente au tour suivant.
            return Ok(None);
        };

        let message = self.pending_control.pop_front().expect("non vide");
        // Nom du variant à des fins de journal uniquement : la recette du
        // chantier B (mesures 3 et 5) a dû contourner l'observabilité de ce
        // chemin par une instrumentation client temporaire, faute d'une ligne
        // ici — ce `tracing::debug!` existe pour que le prochain diagnostic
        // n'ait plus besoin de ce contournement.
        let type_message = match &message {
            AgentControl::Ready { .. } => "ready",
            AgentControl::SessionEnd { .. } => "session-end",
            AgentControl::Pointer { .. } => "pointer",
            AgentControl::Rumble { .. } => "rumble",
            AgentControl::Capabilities { .. } => "capabilities",
            AgentControl::Link { .. } => "link",
        };
        let json = serde_json::to_string(&message)?;
        if let Some(mut channel) = self.rtc.channel(id) {
            match channel.write(false, json.as_bytes()) {
                Ok(_) => {
                    tracing::debug!(type_message, "message de contrôle écrit");
                }
                Err(e) => {
                    tracing::warn!(erreur = %e, "échec d'écriture sur le canal de contrôle");
                }
            }
        }
        Ok(Some(Tick::Continue))
    }

    /// Amorce une fin de session propre : met en file un
    /// `AgentControl::session_end` et arrête l'envoi de nouvelles images.
    /// Idempotent.
    pub(super) fn begin_ending(&mut self, reason: &str) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.pending_control.push_back(AgentControl::session_end(reason));
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
        use std::sync::mpsc;
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, MediaKind};
        use proto::control::CursorShape;

        let local_ip = fixtures::local_ip();
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");
        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc.sdp_api().accept_answer(pending, answer).expect("réponse acceptée");

        // C'est le point du test : le message n'est produit NI par la boucle,
        // NI par un événement str0m — il vient d'un tiers, comme le fera le
        // fil de sondage du curseur.
        let (tx, rx) = mpsc::channel();
        session.set_control_source(rx);
        tx.send(AgentControl::pointer(false, CursorShape::Default))
            .expect("envoi dans le canal");

        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        // Boucle du pair : pilote son `Rtc` et guette le message attendu sur
        // le canal de contrôle. Borne dure pour ne pas pendre si rien n'arrive.
        peer_socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("délai de lecture");
        let mut buf = vec![0u8; 4096];
        let debut = Instant::now();
        let mut recu = false;
        while !recu && debut.elapsed() < Duration::from_secs(15) {
            match peer_socket.recv_from(&mut buf) {
                Ok((n, from)) => {
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
                Err(_) => {}
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
