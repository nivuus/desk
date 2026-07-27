//! Boucle WebRTC : ICE, DTLS, SRTP et SCTP via str0m.
//!
//! str0m est une bibliothèque sans entrées-sorties : nous possédons le socket
//! UDP et la boucle d'événements. Règle impérative documentée par str0m : après
//! chaque mutation, drainer `poll_output` jusqu'à `Output::Timeout` avant la
//! mutation suivante.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{MediaTime, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

use crate::source::VideoSource;

/// Résultat d'un tour de boucle, pour piloter l'appelant.
pub enum Tick {
    Continue,
    Disconnected,
}

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    started: Instant,
}

impl Session {
    /// Prépare une session en attente d'offre.
    ///
    /// `local_ip` est l'adresse par laquelle le navigateur joindra l'agent.
    pub fn new(source: Box<dyn VideoSource + Send>, local_ip: IpAddr) -> Result<Self> {
        let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0))
            .context("ouverture du socket UDP")?;
        let addr = socket.local_addr()?;
        tracing::info!(%addr, "socket UDP de l'agent");

        // str0m 0.21 exige un fournisseur cryptographique installé pour le
        // processus (vérifié dans les sources de la crate : la feature Cargo
        // par défaut `aws-lc-rs` fournit `from_feature_flags()`, et
        // `install_process_default(self)` est une méthode consommante sur
        // `CryptoProvider`). Idempotent : `OnceLock::set` ignore silencieusement
        // un second appel, donc appeler `Session::new` plusieurs fois par
        // processus ne panique pas.
        str0m::crypto::from_feature_flags().install_process_default();

        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .set_stats_interval(Some(std::time::Duration::from_secs(1)))
            .build(Instant::now());

        // `add_local_candidate` ne renvoie pas de `Result` : elle retourne
        // `Option<&Candidate>` (le candidat précédent s'il était déjà connu).
        // Seule la construction du `Candidate` lui-même peut échouer.
        rtc.add_local_candidate(
            Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
        );

        Ok(Self {
            rtc,
            socket,
            source,
            video_mid: None,
            control_channel: None,
            started: Instant::now(),
        })
    }

    /// Accepte l'offre du navigateur et produit la réponse SDP.
    pub fn accept_offer(&mut self, offer_sdp: &str) -> Result<String> {
        let offer = str0m::change::SdpOffer::from_sdp_string(offer_sdp)
            .map_err(|e| anyhow!("offre SDP illisible : {e}"))?;
        let answer = self
            .rtc
            .sdp_api()
            .accept_offer(offer)
            .map_err(|e| anyhow!("offre refusée : {e}"))?;
        Ok(answer.to_sdp_string())
    }

    /// Envoie l'unité d'accès suivante si la connexion est prête.
    ///
    /// `Ok(false)` signifie « rien envoyé, la piste n'est pas encore négociée ».
    pub fn send_next_frame(&mut self) -> Result<bool> {
        let Some(mid) = self.video_mid else {
            return Ok(false);
        };
        let Some(unit) = self.source.next_frame() else {
            bail!("source vidéo épuisée");
        };

        let Some(writer) = self.rtc.writer(mid) else {
            return Ok(false);
        };
        let Some(pt) = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::H264)
            .map(|p| p.pt())
        else {
            // La négociation n'a pas encore abouti sur un profil H.264 commun.
            return Ok(false);
        };

        writer
            .write(pt, Instant::now(), MediaTime::from_90khz(unit.pts_90k), unit.data)
            .map_err(|e| anyhow!("écriture de l'image : {e}"))?;
        Ok(true)
    }

    /// Envoie un message de contrôle au navigateur.
    pub fn send_control(&mut self, message: &AgentControl) -> Result<()> {
        let Some(id) = self.control_channel else {
            return Ok(());
        };
        let json = serde_json::to_string(message)?;
        if let Some(mut channel) = self.rtc.channel(id) {
            channel
                .write(false, json.as_bytes())
                .map_err(|e| anyhow!("écriture sur le canal de contrôle : {e}"))?;
        }
        Ok(())
    }

    /// Un tour de boucle : draine les sorties, puis attend un paquet ou l'échéance.
    pub fn tick(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<Tick> {
        // 1. Drainer toutes les sorties jusqu'à obtenir une échéance.
        let timeout = loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(deadline) => break deadline,
                Output::Transmit(transmit) => {
                    self.socket.send_to(&transmit.contents, transmit.destination)?;
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(Tick::Disconnected);
                    }
                }
            }
        };

        // 2. Attendre un paquet entrant jusqu'à l'échéance.
        let now = Instant::now();
        let wait = timeout.saturating_duration_since(now);
        if wait.is_zero() {
            self.rtc
                .handle_input(Input::Timeout(now))
                .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            return Ok(Tick::Continue);
        }

        self.socket.set_read_timeout(Some(wait))?;
        let mut buffer = vec![0u8; 2000];
        match self.socket.recv_from(&mut buffer) {
            Ok((n, source_addr)) => {
                buffer.truncate(n);
                let receive = Receive {
                    proto: Protocol::Udp,
                    source: source_addr,
                    destination: self.socket.local_addr()?,
                    contents: buffer
                        .as_slice()
                        .try_into()
                        .map_err(|e| anyhow!("paquet illisible : {e}"))?,
                };
                self.rtc
                    .handle_input(Input::Receive(Instant::now(), receive))
                    .map_err(|e| anyhow!("handle_input receive : {e}"))?;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                self.rtc
                    .handle_input(Input::Timeout(Instant::now()))
                    .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            }
            Err(e) => return Err(e.into()),
        }
        Ok(Tick::Continue)
    }

    fn handle_event(
        &mut self,
        event: Event,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Tick {
        match event {
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                tracing::warn!("ICE déconnecté");
                return Tick::Disconnected;
            }
            Event::IceConnectionStateChange(state) => {
                tracing::info!(?state, écoulé = ?self.started.elapsed(), "état ICE");
            }
            Event::MediaAdded(media) => {
                tracing::info!(mid = ?media.mid, kind = ?media.kind, "piste négociée");
                if media.kind == str0m::media::MediaKind::Video {
                    self.video_mid = Some(media.mid);
                }
            }
            Event::ChannelOpen(id, label) => {
                tracing::info!(%label, "canal de données ouvert");
                if label == "control" {
                    self.control_channel = Some(id);
                }
            }
            Event::ChannelData(data) => {
                self.dispatch_channel_data(&data, on_input, on_control);
            }
            Event::KeyframeRequest(request) => {
                tracing::debug!(mid = ?request.mid, "image clé demandée");
            }
            _ => {}
        }
        Tick::Continue
    }

    fn dispatch_channel_data(
        &self,
        data: &str0m::channel::ChannelData,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) {
        if data.binary {
            match InputMessage::decode(&data.data) {
                Ok(message) => on_input(message),
                Err(e) => tracing::warn!(erreur = %e, "message d'entrée invalide"),
            }
        } else {
            match std::str::from_utf8(&data.data).map(serde_json::from_str::<ClientControl>) {
                Ok(Ok(message)) => on_control(message),
                Ok(Err(e)) => tracing::warn!(erreur = %e, "message de contrôle invalide"),
                Err(e) => tracing::warn!(erreur = %e, "contrôle non UTF-8"),
            }
        }
    }
}
