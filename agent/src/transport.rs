//! Boucle WebRTC : ICE, DTLS, SRTP et SCTP via str0m.
//!
//! str0m est une bibliothèque sans entrées-sorties : nous possédons le socket
//! UDP et la boucle d'événements. Règle impérative documentée par str0m : après
//! chaque mutation, drainer `poll_output` jusqu'à `Output::Timeout` avant la
//! mutation suivante.
//!
//! Cette règle est structurelle ici, pas laissée à la discipline de
//! l'appelant : `Session` est le seul type qui détient `Rtc`, ses méthodes
//! publiques (`new`, `accept_offer`, `tick`) ne mutent jamais `Rtc` sans
//! drainer juste avant, et `tick()` — la seule à être appelée en boucle —
//! n'effectue jamais plus d'une mutation par appel avant de rendre la main.
//! Rien en dehors de ce module ne peut donc enchaîner deux mutations sans
//! drainage intercalé, quel que soit l'ordre dans lequel `main.rs` appelle
//! ces méthodes.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{MediaTime, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

use crate::source::VideoSource;

/// Cadence d'envoi des images : une toutes les 16,67 ms (~60 Hz).
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// Résultat d'un tour de boucle, pour piloter l'appelant.
pub enum Tick {
    Continue,
    Disconnected,
}

/// Résultat interne du drainage : soit une échéance à attendre, soit la
/// détection d'une déconnexion pendant le traitement d'un événement.
enum DrainOutcome {
    Timeout(Instant),
    Disconnected,
}

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    started: Instant,
    /// Messages de contrôle en attente d'émission. `tick()` en envoie un au
    /// plus par tour, dès que le canal est ouvert.
    pending_control: VecDeque<AgentControl>,
    /// Vrai dès qu'un `AgentControl::session_end` a été mis en file : plus
    /// aucune image n'est envoyée, la session se termine dès que la file de
    /// contrôle est vidée (ou constatée impossible à vider).
    ending: bool,
    next_frame_at: Instant,
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
            .set_stats_interval(Some(Duration::from_secs(1)))
            .build(Instant::now());

        // `add_local_candidate` ne renvoie pas de `Result` : elle retourne
        // `Option<&Candidate>` (le candidat précédent s'il était déjà connu).
        // Seule la construction du `Candidate` lui-même peut échouer.
        rtc.add_local_candidate(
            Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
        );

        let mut session = Self {
            rtc,
            socket,
            source,
            video_mid: None,
            control_channel: None,
            started: Instant::now(),
            pending_control: VecDeque::new(),
            ending: false,
            next_frame_at: Instant::now() + FRAME_INTERVAL,
        };

        // `add_local_candidate` est une mutation : on draine avant de rendre
        // la main, pour ne jamais dépendre de ce que l'appelant fera après
        // `new()`. Aucune piste ni canal n'existe encore à ce stade, donc les
        // callbacks n'ont rien de significatif à traiter.
        session.drain(&mut |_| {}, &mut |_| {})?;

        Ok(session)
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

        // Même raisonnement que dans `new()` : `accept_offer` mute `Rtc`, on
        // draine avant de rendre la main. L'appelant enverra ensuite la
        // réponse SDP sur le signaling et pourra mettre en file un message de
        // contrôle via `queue_control` — aucune des deux opérations ne mute
        // `Rtc`, mais ce drainage ne doit pas reposer sur cette observation
        // pour rester correct.
        self.drain(&mut |_| {}, &mut |_| {})?;

        Ok(answer.to_sdp_string())
    }

    /// Met en file un message de contrôle à envoyer dès que le canal est
    /// disponible. Ne mute jamais `Rtc` : l'envoi effectif a lieu dans
    /// `tick()`, seul point de mutation de la session une fois la boucle
    /// démarrée.
    pub fn queue_control(&mut self, message: AgentControl) {
        self.pending_control.push_back(message);
    }

    /// Un tour de boucle : draine intégralement les sorties, puis effectue
    /// AU PLUS UNE mutation de `Rtc` avant de rendre la main — un message de
    /// contrôle en attente, une image vidéo si son échéance est atteinte, ou
    /// à défaut le traitement d'un paquet entrant (ou de l'échéance ICE).
    /// C'est cette structure qui garantit l'invariant de drainage : il n'y a
    /// tout simplement aucun autre endroit où muter `Rtc`.
    pub fn tick(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<Tick> {
        // 1. Drainer toutes les sorties jusqu'à obtenir une échéance : c'est
        //    la seule façon d'observer l'état courant de `Rtc` (pistes et
        //    canaux négociés) avant de décider la mutation de ce tour.
        let timeout = match self.drain(on_input, on_control)? {
            DrainOutcome::Timeout(deadline) => deadline,
            DrainOutcome::Disconnected => return Ok(Tick::Disconnected),
        };

        // 2a. Un message de contrôle est en attente : c'est la mutation de ce
        //     tour, si le canal est ouvert.
        if !self.pending_control.is_empty() {
            if let Some(id) = self.control_channel {
                let message = self.pending_control.pop_front().expect("non vide");
                let json = serde_json::to_string(&message)?;
                if let Some(mut channel) = self.rtc.channel(id) {
                    if let Err(e) = channel.write(false, json.as_bytes()) {
                        tracing::warn!(erreur = %e, "échec d'écriture sur le canal de contrôle");
                    }
                }
                return Ok(Tick::Continue);
            } else if self.ending {
                // Fin de session demandée mais aucun canal de contrôle
                // disponible pour prévenir le navigateur (jamais ouvert, ou
                // fermé) : impossible d'envoyer le message, mais on ne va
                // pas attendre indéfiniment un canal qui ne s'ouvrira pas.
                tracing::warn!(
                    "fin de session sans canal de contrôle disponible pour en informer le navigateur"
                );
                return Ok(Tick::Disconnected);
            }
            // Sinon : canal pas encore ouvert, session pas en cours de
            // clôture — le message reste en file, on retente au tour
            // suivant sans bloquer ce tour-ci.
        }

        if self.ending {
            // Le message de fin de session a bien été envoyé (file vidée
            // ci-dessus) : plus rien à faire.
            return Ok(Tick::Disconnected);
        }

        // 2b. Pas de contrôle en attente : une image vidéo si son échéance
        //     est atteinte et la piste négociée.
        if let Some(mid) = self.video_mid {
            let now = Instant::now();
            if now >= self.next_frame_at {
                self.next_frame_at = now + FRAME_INTERVAL;
                match self.source.next_frame() {
                    Some(unit) => self.write_frame(mid, unit),
                    None => {
                        // Source épuisée : clore la session proprement (chemin
                        // `AgentControl::session_end`), pas tuer le processus.
                        // Inatteignable avec `FileSource` (boucle à l'infini),
                        // mais deviendra réel avec la capture Windows (tâche 9).
                        self.begin_ending("source vidéo épuisée");
                    }
                }
                return Ok(Tick::Continue);
            }
        }

        // 2c. Rien à émettre ce tour-ci : attendre un paquet entrant, borné à
        //     la fois par l'échéance de `Rtc` et par la prochaine échéance
        //     d'image pour ne jamais la manquer.
        let now = Instant::now();
        let mut wait = timeout.saturating_duration_since(now);
        if self.video_mid.is_some() {
            wait = wait.min(self.next_frame_at.saturating_duration_since(now));
        }

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

    /// Écrit une unité d'accès sur la piste vidéo. Consomme `unit` (déjà
    /// tirée de la source par l'appelant — voir la limitation connue dans le
    /// rapport de tâche : une image tirée juste avant un échec de `writer`
    /// est perdue, non renvoyée).
    fn write_frame(&mut self, mid: Mid, unit: crate::h264::AccessUnit) {
        let Some(writer) = self.rtc.writer(mid) else {
            return;
        };
        let Some(pt) = writer
            .payload_params()
            .find(|p| p.spec().codec == Codec::H264)
            .map(|p| p.pt())
        else {
            // La négociation n'a pas encore abouti sur un profil H.264 commun.
            return;
        };

        if let Err(e) =
            writer.write(pt, Instant::now(), MediaTime::from_90khz(unit.pts_90k), unit.data)
        {
            // Échec d'écriture applicatif (ex. RID inconnu, PT retiré en
            // cours de route) : on clôt la session plutôt que de faire
            // remonter l'erreur jusqu'au processus. Seules `Session::new` et
            // `accept_offer` — avant qu'une session n'existe vraiment —
            // justifient de tuer le processus entier.
            tracing::warn!(erreur = %e, "échec d'écriture de l'image, fin de session");
            self.begin_ending("échec d'écriture vidéo");
        }
    }

    /// Amorce une fin de session propre : met en file un
    /// `AgentControl::session_end` et arrête l'envoi de nouvelles images.
    /// Idempotent.
    fn begin_ending(&mut self, reason: &str) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.pending_control.push_back(AgentControl::session_end(reason));
        tracing::info!(reason, "clôture de session amorcée");
    }

    /// Draine `poll_output` jusqu'à `Output::Timeout`, en envoyant les
    /// paquets sortants et en dispatchant les événements. C'est le seul
    /// endroit qui appelle `poll_output` — chaque site de mutation de `Rtc`
    /// (dans `new`, `accept_offer`, ou `tick`) l'appelle juste après avoir
    /// muté, avant de rendre la main ou de muter à nouveau.
    fn drain(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<DrainOutcome> {
        loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(deadline) => return Ok(DrainOutcome::Timeout(deadline)),
                Output::Transmit(transmit) => {
                    self.socket.send_to(&transmit.contents, transmit.destination)?;
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(DrainOutcome::Disconnected);
                    }
                }
            }
        }
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
