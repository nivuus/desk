//! Boucle WebRTC : ICE, DTLS, SRTP et SCTP via str0m.
//!
//! str0m est une bibliothèque sans entrées-sorties : nous possédons le socket
//! UDP et la boucle d'événements. Règle impérative documentée par str0m :
//! après chaque mutation, drainer `poll_output` jusqu'à `Output::Timeout`
//! avant la mutation suivante — mais str0m précise aussi qu'une mutation émise
//! **depuis l'intérieur** de la boucle de drainage (avant qu'elle ait rendu la
//! main) est correcte. C'est le choix structurel de ce fichier : `Session::run`
//! est une unique boucle continue autour de `Rtc::poll_output`, et chaque
//! mutation (écriture d'image, de message de contrôle, `handle_input`) a lieu
//! à l'intérieur de cette boucle, immédiatement suivie d'un retour à
//! `poll_output`. Rien en dehors de `run()` ne mute jamais `Rtc` pendant que
//! la boucle tourne : il n'y a tout simplement aucun autre endroit qui le
//! pourrait, ce qui rend l'invariant structurel plutôt que dépendant de la
//! discipline de l'appelant.
//!
//! `run()` bloque volontairement (socket UDP non bloquant, sondé par petites
//! tranches de sommeil plutôt que par un `recv_from` bloquant à échéance —
//! voir `RECV_POLL_INTERVAL`) et doit donc être appelée depuis un thread
//! dédié — `tokio::task::spawn_blocking` côté `main.rs` — jamais depuis un
//! ouvrier async de tokio.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{MediaTime, Mid, Pt};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Cadence d'envoi des images : une toutes les 16,67 ms (~60 Hz).
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// Tranche maximale d'une attente sans donnée sur le socket, dans la boucle
/// de sondage non bloquant d'`act_on_timeout` (branche c).
///
/// Remplace `UdpSocket::set_read_timeout`, dont le délai déborde massivement
/// sous Windows (mesure indépendante : dépassement moyen +12,7 ms, jusqu'à
/// +37 ms ; un délai demandé de 617 µs a été honoré après 31 758 µs — cinq
/// fois le budget d'une image entière à 60 Hz). `recv_from` consommait ainsi
/// ~90 % du temps de boucle pendant qu'une capture/encodage capable de
/// 47-51 im/s en produisait à peine 23-25.
///
/// Un socket non bloquant sondé en boucle sans jamais dormir consommerait un
/// cœur de processeur entier pour rien — inacceptable pour un agent censé
/// tourner en arrière-plan. À l'inverse, un unique `sleep` couvrant toute
/// l'attente reproduirait l'imprécision mesurée (le défaut n'est pas propre à
/// `recv_from` : c'est la granularité du minuteur Windows sous-jacent). Le
/// compromis retenu revérifie le socket à intervalles courts et fixes : le
/// sur-sommeil d'un réveil donné, s'il survient, reste borné à cet intervalle
/// plutôt qu'à la durée totale de l'attente. 1 ms est nettement plus fin que
/// l'intervalle d'image (16,67 ms) tout en laissant le fil dormir l'essentiel
/// du temps.
const RECV_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Résultat du traitement d'un événement ou d'un tour de boucle interne.
enum Tick {
    Continue,
    Disconnected,
}

/// Vue minimale d'un profil de charge utile négocié, indépendante de str0m
/// pour rester testable sans session RTC réelle : les champs de
/// `str0m::format::PayloadParams` (dont `pt`) sont `pub(crate)` côté str0m,
/// donc impossibles à construire depuis ce crate pour un test.
#[derive(Debug, Clone, Copy)]
struct CandidatePt {
    codec: Codec,
    packetization_mode: Option<u8>,
    pt: Pt,
}

/// Sélectionne le type de charge utile à utiliser pour envoyer du H.264.
///
/// `enable_h264(true)` négocie sept profils (modes de paquetisation 0 et 1,
/// quatre profils de compatibilité) : prendre le premier de la liste, comme
/// le faisait la version initiale, ne garantit rien sur ce que produira
/// l'encodeur. On filtre explicitement sur le mode de paquetisation 1
/// (non-interleaved, RFC 6184 §6.2) — le seul que les tâches suivantes
/// produiront. Le profil exact (constrained-baseline, etc.) n'est pas
/// discriminé plus finement ici : ce n'est vérifiable qu'avec un navigateur
/// réel et un encodeur réel, pas avant les tâches 8/11.
fn select_h264_pt(candidates: impl Iterator<Item = CandidatePt>) -> Option<Pt> {
    candidates
        .filter(|p| p.codec == Codec::H264 && p.packetization_mode == Some(1))
        .map(|p| p.pt)
        .next()
}

/// Échéance de la prochaine image, calculée à partir de l'échéance
/// *précédente* plutôt que de l'instant courant, pour ne pas accumuler de
/// dérive : un léger retard sur une image ne retarde pas systématiquement
/// toutes les suivantes. Borné à un intervalle de rattrapage : au-delà, on
/// abandonne le calcul fondé sur `previous` (qui produirait une rafale
/// d'images pour rattraper tout le retard d'un coup) et on repart d'un
/// intervalle après `now`.
fn next_frame_deadline(previous: Instant, now: Instant, interval: Duration) -> Instant {
    let candidate = previous + interval;
    if now.saturating_duration_since(candidate) > interval {
        now + interval
    } else {
        candidate
    }
}

/// Issue de la classification d'une erreur de réception UDP (voir
/// `classify_recv_error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecvErrorAction {
    /// Erreur transitoire connue, qui n'indique aucune corruption durable du
    /// socket : on continue de recevoir, après une temporisation (voir
    /// `recv_error_backoff`) pour ne pas transformer une rafale de telles
    /// erreurs en boucle serrée.
    RetryWithBackoff,
    /// Erreur qui n'a aucune raison de se résorber d'elle-même (permissions,
    /// socket dans un état invalide, interface réseau disparue...) :
    /// continuer à boucler dessus ne ferait que masquer un problème réel
    /// sans jamais le résoudre. On clôt la session proprement plutôt que de
    /// journaliser indéfiniment.
    Fatal,
}

/// Classe une erreur de `UdpSocket::recv_from` (hors `WouldBlock`/`TimedOut`,
/// déjà traités séparément comme des échéances normales) selon qu'elle
/// justifie une nouvelle tentative ou la fin de la session.
///
/// Le cas motivant : sous Windows, la plateforme cible, un socket UDP non
/// connecté reçoit `WSAECONNRESET` quand un message ICMP « port injoignable »
/// revient — typiquement après la fermeture brutale de l'onglet du
/// navigateur, avant qu'ICE n'ait eu le temps de détecter la déconnexion.
/// `std::io::ErrorKind::ConnectionReset` est la variante portable vers
/// laquelle Rust normalise `WSAECONNRESET` (voir `std::io::Error::kind`) :
/// on teste ce nom cross-plateforme, jamais une valeur d'erreur spécifique à
/// Windows, pour que ce fichier reste indépendant de la plateforme de
/// compilation. Sur Linux, avec un socket non connecté comme celui-ci, cette
/// variante n'est en pratique jamais produite pour ce scénario — le test
/// couvre donc la classification elle-même, pas un comportement observable
/// uniquement sous Windows. `Interrupted` (signal reçu pendant l'appel
/// bloquant) suit la même logique : retenter est le comportement standard
/// documenté par `std::io::Error`.
///
/// Toute autre erreur (permissions, socket fermé, argument invalide...) est
/// classée fatale : rien n'indique qu'elle se résorbera d'elle-même, et
/// boucler dessus sans fin masquerait un problème réel plutôt que de le
/// signaler.
fn classify_recv_error(kind: std::io::ErrorKind) -> RecvErrorAction {
    use std::io::ErrorKind::{ConnectionReset, Interrupted};
    match kind {
        ConnectionReset | Interrupted => RecvErrorAction::RetryWithBackoff,
        _ => RecvErrorAction::Fatal,
    }
}

/// Temporisation appliquée après `consecutive_errors` erreurs de réception
/// UDP transitoires d'affilée : backoff exponentiel borné (1 ms, 2 ms, 4
/// ms, ... jusqu'à `RECV_ERROR_BACKOFF_MAX`).
///
/// Sans cette borne, une rafale de `WSAECONNRESET` (un ICMP « port
/// injoignable » par paquet renvoyé pendant qu'ICE n'a pas encore détecté la
/// déconnexion, ce qui prend plusieurs secondes) tournerait en boucle serrée
/// — `recv_from` renvoyant l'erreur immédiatement, sans jamais attendre le
/// délai de lecture demandé — journalisant à chaque tour et consommant un
/// cœur de processeur jusqu'à la détection ICE. Le plafond est choisi assez
/// bas pour ne pas retarder sensiblement la réception d'un paquet légitime
/// qui arriverait entre-temps (ni la détection ICE elle-même, qui ne dépend
/// pas de cette boucle mais des échéances de `Rtc`).
const RECV_ERROR_BACKOFF_BASE: Duration = Duration::from_millis(1);
const RECV_ERROR_BACKOFF_MAX: Duration = Duration::from_millis(200);

fn recv_error_backoff(consecutive_errors: u32) -> Duration {
    // `1u32 << exponent` déborderait au-delà de 31 : borner l'exposant avant
    // le décalage, plutôt que de compter sur `saturating_mul` seul, qui
    // opère sur des `Duration` (pas d'overflow arithmétique là), mais dont
    // l'opérande `2^exponent` aurait déjà débordé silencieusement en `u32`
    // avant de lui être passé.
    let exponent = consecutive_errors.min(31);
    RECV_ERROR_BACKOFF_BASE
        .saturating_mul(1u32 << exponent)
        .min(RECV_ERROR_BACKOFF_MAX)
}

// `timeBeginPeriod`/`timeEndPeriod` (winmm.dll) sont déclarées à la main :
// la crate `windows` 0.62 (même avec la fonctionnalité
// `Win32_Media_Multimedia` activée) ne les génère pas — vérifié par
// recherche exhaustive dans les sources vendues de la crate, aucune
// occurrence de `timeBeginPeriod`/`BeginPeriod`. L'API est stable et
// documentée par Microsoft depuis Windows XP ; la déclarer directement évite
// de dépendre d'une fonctionnalité absente. (`//`, pas `///` : rustdoc ne
// documente pas les blocs `extern`, et un tel commentaire s'attacherait de
// toute façon à l'élément suivant plutôt qu'à celui-ci.)
#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(uperiod: u32) -> u32;
    fn timeEndPeriod(uperiod: u32) -> u32;
}

/// Garde RAII appariant `timeBeginPeriod`/`timeEndPeriod` (winmm) pour la
/// durée de vie d'une `Session`.
///
/// Sans cet appel, `std::thread::sleep` sous Windows hérite de la résolution
/// par défaut du minuteur système — typiquement 15,6 ms tant qu'aucun
/// processus n'a demandé mieux. Mesuré expérimentalement sur cet agent :
/// `RECV_POLL_INTERVAL` (1 ms) sans cette garde ne réduisait quasiment pas le
/// débit (~24 im/s, contre ~23 im/s avant tout correctif) — la boucle de
/// sondage héritait du même défaut de granularité que celui mesuré sur
/// `recv_from`, juste déplacé vers `sleep`. Avec la résolution ramenée à
/// 1 ms, `sleep` honore effectivement des attentes de l'ordre de la
/// milliseconde. `timeBeginPeriod`/`timeEndPeriod` doivent être appariés
/// (documentation Microsoft) : cette garde le fait même en cas de retour
/// anticipé (`?`) ou de panique, jamais par un chemin de code qui pourrait
/// être sauté.
///
/// N'existe que sous Windows : sous Linux (utilisé par les tests), le SDK
/// n'expose pas `timeBeginPeriod` et le défaut mesuré n'a pas cours.
#[cfg(windows)]
struct TimerResolutionGuard;

#[cfg(windows)]
impl TimerResolutionGuard {
    fn new() -> Self {
        // Retour ignoré : `TIMERR_NOERROR` (succès) ou `TIMERR_NOCANDO` (déjà
        // à la résolution maximale, ou hors bornes) — dans les deux cas, rien
        // d'exploitable à faire ici ; un échec silencieux dégraderait au pire
        // vers le comportement précédent (résolution par défaut), jamais vers
        // une erreur fonctionnelle.
        unsafe {
            timeBeginPeriod(1);
        }
        Self
    }
}

#[cfg(windows)]
impl Drop for TimerResolutionGuard {
    fn drop(&mut self) {
        unsafe {
            timeEndPeriod(1);
        }
    }
}

/// Sous Linux (tests), aucun équivalent à appeler : la granularité mesurée
/// est un défaut propre au minuteur Windows.
#[cfg(not(windows))]
struct TimerResolutionGuard;

#[cfg(not(windows))]
impl TimerResolutionGuard {
    fn new() -> Self {
        Self
    }
}

/// Durée à attendre avant le prochain réveil, bornée par la plus proche de
/// deux échéances : celle que réclame `Rtc` (`rtc_deadline`) et, si une piste
/// vidéo est négociée et la session n'est pas en cours de clôture,
/// `next_frame_at`.
///
/// C'est ici que se logeait la panne de cadence (C1 de la revue) : sans
/// borne sur l'échéance d'image, l'attente valait jusqu'à l'échéance que
/// réclame `Rtc` — jusqu'à la seconde entière, imposée par l'intervalle de
/// rapport RTCP ou de statistiques, dès que rien d'autre n'était dû. Le
/// rythme d'envoi était alors dicté par les réveils de str0m, pas par
/// `FRAME_INTERVAL`.
fn bounded_wait(now: Instant, rtc_deadline: Instant, next_frame_at: Option<Instant>) -> Duration {
    let mut wait = rtc_deadline.saturating_duration_since(now);
    if let Some(next_frame_at) = next_frame_at {
        wait = wait.min(next_frame_at.saturating_duration_since(now));
    }
    wait
}

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    dimensions: (u32, u32),
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    started: Instant,
    /// Messages de contrôle en attente d'émission. `run()` en envoie un au
    /// plus par mutation, dès que le canal est ouvert.
    pending_control: VecDeque<AgentControl>,
    /// Vrai dès qu'un `AgentControl::session_end` a été mis en file : plus
    /// aucune image n'est envoyée, la session se termine dès que la file de
    /// contrôle est vidée (ou constatée impossible à vider).
    ending: bool,
    next_frame_at: Instant,
    /// Empêche de noyer les journaux : la négociation incomplète (I4) est
    /// signalée une seule fois, pas à chaque image jetée.
    warned_negotiation: bool,
    /// Nombre d'erreurs de réception UDP transitoires consécutives (voir
    /// `classify_recv_error`/`recv_error_backoff`) : remis à zéro dès qu'un
    /// tour de boucle se déroule sans une telle erreur (paquet reçu, ou
    /// simple échéance sans donnée). Sert à faire croître la temporisation
    /// appliquée entre deux tentatives pendant une rafale.
    consecutive_recv_errors: u32,
    /// Vrai juste après qu'une image vidéo a été écrite (`writer.write()`),
    /// tant que le drainage str0m qui la fait réellement partir
    /// (`Rtc::handle_input(Input::Timeout(..))`) n'a pas encore eu lieu.
    ///
    /// `writer.write()` empile l'image dans la file interne `to_payload` de
    /// str0m ; seul `handle_input(Input::Timeout(..))` la dépile
    /// (`do_payload`), jamais `poll_output()` seul (voir `act_on_timeout`,
    /// ronde de correction 1). Ce drapeau reporte ce drainage au tour
    /// suivant plutôt que de l'enchaîner dans le même appel : `write_frame`
    /// (une mutation) et `handle_input` (une seconde mutation) restent ainsi
    /// chacun séparés par un passage complet dans `poll_output()`, comme
    /// l'exige str0m — les enchaîner directement, comme le faisait la
    /// première version de ce correctif, reproduisait exactement la
    /// violation qu'il prétendait résoudre.
    video_write_pending_drain: bool,
    /// Résolution du minuteur Windows abaissée à 1 ms pour la durée de vie de
    /// la session (voir `TimerResolutionGuard`). Champ jamais lu : sa seule
    /// raison d'être est de vivre aussi longtemps que `Session` et de
    /// restaurer la résolution d'origine à la destruction.
    _timer_resolution: TimerResolutionGuard,
}

impl Session {
    /// Prépare une session en attente d'offre.
    ///
    /// `local_ip` est l'adresse par laquelle le navigateur joindra l'agent.
    pub fn new(source: Box<dyn VideoSource + Send>, local_ip: IpAddr) -> Result<Self> {
        let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0))
            .context("ouverture du socket UDP")?;
        // Non bloquant une fois pour toutes : `act_on_timeout` ne dépend plus
        // de `set_read_timeout`, dont le délai déborde massivement sous
        // Windows (mesuré : dépassement moyen +12,7 ms, jusqu'à +37 ms sur un
        // délai demandé de 617 µs — voir `poll_recv_or_timeout`). Le rythme
        // d'attente est désormais entièrement piloté par notre propre boucle
        // de sondage, indépendante de la précision du minuteur du socket.
        socket.set_nonblocking(true).context("passage du socket UDP en non bloquant")?;
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

        let dimensions = source.dimensions();
        let mut session = Self {
            rtc,
            socket,
            source,
            dimensions,
            video_mid: None,
            control_channel: None,
            started: Instant::now(),
            pending_control: VecDeque::new(),
            ending: false,
            next_frame_at: Instant::now() + FRAME_INTERVAL,
            warned_negotiation: false,
            consecutive_recv_errors: 0,
            video_write_pending_drain: false,
            _timer_resolution: TimerResolutionGuard::new(),
        };

        // `add_local_candidate` est une mutation : on draine avant de rendre
        // la main, pour ne jamais dépendre de ce que l'appelant fera après
        // `new()`.
        session.drain_quietly()?;

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
        // draine avant de rendre la main, sans dépendre de ce que fera
        // l'appelant ensuite.
        self.drain_quietly()?;

        Ok(answer.to_sdp_string())
    }

    /// Met en file un message de contrôle à envoyer dès que le canal est
    /// disponible. Ne mute jamais `Rtc` : l'envoi effectif a lieu dans
    /// `run()`, seul endroit qui mute la session une fois la boucle démarrée.
    fn queue_control(&mut self, message: AgentControl) {
        self.pending_control.push_back(message);
    }

    /// Boucle de transport : tourne jusqu'à déconnexion ou erreur fatale.
    ///
    /// Bloque volontairement (lecture UDP synchrone) — à appeler depuis un
    /// thread dédié (`tokio::task::spawn_blocking`), jamais depuis un
    /// ouvrier async de tokio (voir le commentaire de module, I6 de la revue).
    pub fn run(
        &mut self,
        on_input: &mut impl FnMut(InputMessage),
        on_control: &mut impl FnMut(ClientControl),
    ) -> Result<()> {
        loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(deadline) => {
                    if let Tick::Disconnected = self.act_on_timeout(deadline)? {
                        return Ok(());
                    }
                }
                Output::Transmit(transmit) => {
                    // I2 : une erreur d'envoi transitoire (ex. ENETUNREACH,
                    // le pair a fermé son port) ne doit pas terminer la
                    // session — seulement être journalisée.
                    if let Err(e) = self.socket.send_to(&transmit.contents, transmit.destination) {
                        tracing::warn!(erreur = %e, "échec d'envoi UDP, ignoré");
                    }
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, on_input, on_control) {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Réagit à `Output::Timeout` : décide et effectue AU PLUS UNE mutation
    /// (drainage différé d'une image déjà écrite, message de contrôle en
    /// attente, image vidéo due, ou traitement d'un paquet entrant /
    /// échéance str0m), puis rend la main à `run()`, qui rappelle
    /// immédiatement `poll_output` — c'est cette structure qui garantit le
    /// drainage avant toute mutation suivante (C2 de la revue) : il
    /// n'existe aucun chemin de code qui mute `Rtc` sans que `run()` ne
    /// rappelle `poll_output` juste après. La priorité donnée au drainage
    /// différé (voir `video_write_pending_drain`) est ce qui rend cette
    /// garantie vraie même juste après l'écriture d'une image : sans elle,
    /// `write_frame` (une mutation) suivi directement de `handle_input`
    /// (une seconde) violerait la même règle.
    ///
    /// Ne prend pas `on_input`/`on_control` : `handle_input` ne produit
    /// jamais d'événement applicatif directement (les événements qui en
    /// résultent ne sortent que via un futur `poll_output`, donc via
    /// `run()`, qui les dispatche lui-même).
    fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drainage dû après la dernière image vidéo écrite. Vérifié en
        // priorité absolue, avant tout le reste : c'est la seule façon de
        // garantir qu'aucune mutation ne s'enchaîne jamais sans un passage
        // complet par `poll_output()` entre les deux, quel que soit l'état
        // des autres files (voir le commentaire du champ et la ronde de
        // correction 1 de la tâche 11).
        if self.video_write_pending_drain {
            self.video_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (drainage vidéo) : {e}"))?;
            return Ok(Tick::Continue);
        }

        // a) Un message de contrôle est en attente.
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
                // Fin de session demandée mais canal de contrôle
                // indisponible (jamais ouvert, ou fermé) : impossible
                // d'en informer le navigateur, mais on n'attend pas
                // indéfiniment un canal qui ne s'ouvrira pas.
                tracing::warn!(
                    "fin de session sans canal de contrôle disponible pour en informer le navigateur"
                );
                return Ok(Tick::Disconnected);
            }
            // Canal pas encore ouvert, session pas en cours de clôture : le
            // message reste en file, on retente au tour suivant.
        }

        if self.ending {
            // Message de fin envoyé (file vidée ci-dessus) : terminé.
            return Ok(Tick::Disconnected);
        }

        // b) Une image vidéo, si son échéance est atteinte et la piste
        //    négociée.
        if let Some(mid) = self.video_mid {
            let now = Instant::now();
            if now >= self.next_frame_at {
                self.next_frame_at = next_frame_deadline(self.next_frame_at, now, FRAME_INTERVAL);
                match self.source.next_frame() {
                    Some(unit) => {
                        // `writer.write()` ne fait qu'empiler l'image dans la
                        // file interne `to_payload` de str0m — c'est
                        // `Rtc::handle_input(Input::Timeout(..))` qui la
                        // dépile réellement en paquets RTP (`do_payload`),
                        // jamais `poll_output()` seul (voir `session.rs` de
                        // str0m). L'appeler ICI serait une seconde mutation
                        // dans le même appel à `act_on_timeout`, sans
                        // `poll_output` entre les deux — exactement la
                        // violation que ce mécanisme doit éviter (ronde de
                        // correction 1). On pose donc un drapeau : la
                        // PROCHAINE invocation de `act_on_timeout` le traite
                        // en priorité absolue (voir a0 plus haut). La file de
                        // charge non vide fait renvoyer une échéance
                        // immédiate par `poll_output()`, donc `run()`
                        // rappelle aussitôt.
                        if self.write_frame(mid, unit) {
                            self.video_write_pending_drain = true;
                        }
                    }
                    None => {
                        // Ronde de correction 1 : l'absence de nouvelle image
                        // est le cas courant et normal d'une capture en
                        // direct (bureau immobile) — pas une fin de session.
                        // Seule une source réellement épuisée (fenêtre
                        // fermée, erreur non récupérable) le justifie, via
                        // `VideoSource::is_exhausted`. `FileSource` ne
                        // renvoie jamais `None` et n'atteint donc jamais ce
                        // chemin.
                        if self.source.is_exhausted() {
                            self.begin_ending("source vidéo épuisée");
                        }
                    }
                }
                return Ok(Tick::Continue);
            }
        }

        // c) Rien à émettre ce tour-ci : attendre un paquet entrant, borné à
        //    la fois par l'échéance de `Rtc` et par la prochaine échéance
        //    d'image (voir `bounded_wait` — c'est le correctif de C1).
        let now = Instant::now();
        let next_frame_at =
            (self.video_mid.is_some() && !self.ending).then_some(self.next_frame_at);
        let wait = bounded_wait(now, deadline, next_frame_at);

        if wait.is_zero() {
            self.rtc
                .handle_input(Input::Timeout(now))
                .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            return Ok(Tick::Continue);
        }

        // Sonde le socket (non bloquant depuis `Session::new`) par petites
        // tranches plutôt que de confier l'attente à `set_read_timeout` :
        // c'est le correctif du défaut mesuré (voir le commentaire de
        // `RECV_POLL_INTERVAL`). Aucune mutation de `Rtc` ne se produit tant
        // que cette boucle n'a pas soit reçu un datagramme, soit atteint
        // `poll_deadline` — une seule mutation en sort, comme l'exige le
        // docstring de la méthode.
        let poll_deadline = now + wait;
        let mut buffer = vec![0u8; 2000];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((n, source_addr)) => {
                    // Un tour de boucle sans erreur de réception met fin à
                    // une éventuelle rafale : la prochaine erreur, s'il y en
                    // a une, repart d'une temporisation minimale plutôt que
                    // de poursuivre la croissance entamée par une rafale
                    // passée.
                    self.consecutive_recv_errors = 0;
                    let destination = self.socket.local_addr()?;
                    // I2 : un datagramme qui n'est ni STUN, ni DTLS, ni
                    // RTP/RTCP (bruit réseau, sonde de port, paquet vide)
                    // fait échouer cette conversion. Il ne doit pas faire
                    // tomber l'agent — seulement être ignoré.
                    match DatagramRecv::try_from(&buffer[..n]) {
                        Ok(contents) => {
                            let receive = Receive {
                                proto: Protocol::Udp,
                                source: source_addr,
                                destination,
                                contents,
                            };
                            self.rtc
                                .handle_input(Input::Receive(Instant::now(), receive))
                                .map_err(|e| anyhow!("handle_input receive : {e}"))?;
                        }
                        Err(e) => {
                            tracing::debug!(erreur = %e, "paquet UDP ignoré (non reconnu)");
                        }
                    }
                    return Ok(Tick::Continue);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Pas de donnée disponible pour l'instant : le cas
                    // courant. N'affecte pas le compteur de rafale (ce n'est
                    // pas une erreur).
                    self.consecutive_recv_errors = 0;
                    let remaining = poll_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        // Échéance atteinte sans donnée : rendre la main à
                        // `Rtc` via un timeout, exactement comme le faisait
                        // l'ancien `recv_from` bloquant à l'expiration de
                        // `set_read_timeout`.
                        self.rtc
                            .handle_input(Input::Timeout(Instant::now()))
                            .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
                        return Ok(Tick::Continue);
                    }
                    // Ni spin serré (consommerait un cœur entier), ni sommeil
                    // unique sur toute la durée (reproduirait l'imprécision
                    // mesurée) : on dort par petites tranches bornées par
                    // `RECV_POLL_INTERVAL`, en revérifiant le socket à
                    // chaque réveil. Le sur-sommeil éventuel d'un seul appel
                    // à `sleep` (même défaut de granularité que celui mesuré
                    // sur `recv_from`) reste borné à un intervalle de
                    // sondage, jamais à la totalité de `wait`.
                    std::thread::sleep(remaining.min(RECV_POLL_INTERVAL));
                }
                Err(e) => match classify_recv_error(e.kind()) {
                    // I2 : erreur de réception transitoire — journalisée,
                    // pas fatale. Une rafale bouclerait à vide sans cette
                    // temporisation croissante (voir `recv_error_backoff`) —
                    // le socket lui-même n'est pas mis en cause, seul le
                    // rythme de nouvelles tentatives l'est.
                    RecvErrorAction::RetryWithBackoff => {
                        self.consecutive_recv_errors =
                            self.consecutive_recv_errors.saturating_add(1);
                        let backoff = recv_error_backoff(self.consecutive_recv_errors);
                        tracing::warn!(
                            erreur = %e,
                            consecutives = self.consecutive_recv_errors,
                            backoff_ms = backoff.as_millis(),
                            "échec de réception UDP transitoire, ignoré"
                        );
                        std::thread::sleep(backoff);
                        return Ok(Tick::Continue);
                    }
                    // Erreur qui n'a aucune raison de se résorber
                    // d'elle-même : clôture propre de la session (comme I5
                    // pour une source épuisée), pas boucle indéfinie ni
                    // panique du processus — seules `Session::new`/
                    // `accept_offer` justifient de tuer le processus entier
                    // (voir le commentaire de module).
                    RecvErrorAction::Fatal => {
                        tracing::warn!(
                            erreur = %e,
                            "échec de réception UDP non transitoire, fin de session"
                        );
                        self.begin_ending("échec de réception UDP non transitoire");
                        return Ok(Tick::Continue);
                    }
                },
            }
        }
    }

    /// Sélectionne le type de charge utile H.264 négocié pour `mid`, s'il y
    /// en a un. Appel séparé de `write_frame` pour que l'emprunt sur `self`
    /// via `Rtc::writer` se termine avant tout appel `&mut self` ultérieur
    /// (le journal d'avertissement, notamment).
    fn select_negotiated_h264_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        select_h264_pt(writer.payload_params().map(|p| CandidatePt {
            codec: p.spec().codec,
            packetization_mode: p.spec().format.packetization_mode,
            pt: p.pt(),
        }))
    }

    /// Écrit une unité d'accès sur la piste vidéo. Mutation émise depuis
    /// l'intérieur de la boucle de `run()` (voir `act_on_timeout`), donc
    /// suivie d'un retour immédiat à `poll_output` — conforme à la règle de
    /// drainage de str0m.
    ///
    /// Renvoie `true` si `writer.write()` a réellement été appelée et a
    /// réussi (donc qu'une entrée a bien été empilée dans `to_payload` et
    /// nécessite le drainage différé — voir `video_write_pending_drain`),
    /// `false` si l'écriture n'a pas eu lieu (négociation incomplète,
    /// piste indisponible) ou a échoué : dans ces deux cas, aucune entrée
    /// n'a été ajoutée à `to_payload`, poser le drapeau de drainage serait
    /// à tort et provoquerait un `handle_input(Timeout)` inutile.
    fn write_frame(&mut self, mid: Mid, unit: AccessUnit) -> bool {
        let Some(pt) = self.select_negotiated_h264_pt(mid) else {
            // I4 : négociation incomplète (aucun profil H.264 en mode de
            // paquetisation 1) — sans ce journal, l'image est jetée
            // silencieusement, produisant un écran noir muet indéfiniment
            // sans le moindre indice dans les journaux.
            self.warn_negotiation_once(
                "aucun type de charge utile H.264 négocié (mode de paquetisation 1) : images jetées",
            );
            return false;
        };
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_negotiation_once("piste vidéo plus accessible en écriture : images jetées");
            return false;
        };

        match writer.write(pt, Instant::now(), MediaTime::from_90khz(unit.pts_90k), unit.data) {
            Ok(()) => true,
            Err(e) => {
                // Échec d'écriture applicatif (ex. RID inconnu) : on clôt la
                // session plutôt que de faire remonter l'erreur jusqu'au
                // processus. Seules `Session::new` et `accept_offer` — avant
                // qu'une session n'existe vraiment — justifient de tuer le
                // processus entier.
                tracing::warn!(erreur = %e, "échec d'écriture de l'image, fin de session");
                self.begin_ending("échec d'écriture vidéo");
                false
            }
        }
    }

    fn warn_negotiation_once(&mut self, message: &str) {
        if !self.warned_negotiation {
            self.warned_negotiation = true;
            tracing::warn!(message, "négociation vidéo incomplète (avertissement unique)");
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

    /// Draine `poll_output` jusqu'à `Output::Timeout`, sans callbacks
    /// applicatifs. Utilisé uniquement aux points de mutation antérieurs à
    /// `run()` (`new`, `accept_offer`) : aucune piste ni canal ne peut
    /// encore produire de données applicatives à ce stade.
    fn drain_quietly(&mut self) -> Result<()> {
        loop {
            match self.rtc.poll_output().map_err(|e| anyhow!("poll_output : {e}"))? {
                Output::Timeout(_) => return Ok(()),
                Output::Transmit(transmit) => {
                    if let Err(e) = self.socket.send_to(&transmit.contents, transmit.destination) {
                        tracing::warn!(erreur = %e, "échec d'envoi UDP, ignoré");
                    }
                }
                Output::Event(event) => {
                    if let Tick::Disconnected = self.handle_event(event, &mut |_| {}, &mut |_| {}) {
                        return Ok(());
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
            Event::Closed => {
                // I1 : émis à la réception du close_notify DTLS — typiquement
                // quand l'utilisateur ferme l'onglet. Sans ce bras, cet
                // événement tombait dans `_ => {}` et l'agent continuait
                // d'émettre jusqu'à l'expiration ICE, bien après le départ
                // du pair.
                tracing::info!("connexion fermée par le pair (close_notify DTLS)");
                return Tick::Disconnected;
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
                    // I3 : envoyer `ready` ici, pas juste après la réponse
                    // SDP — à ce moment-là SCTP n'est pas encore ouvert,
                    // `control_channel` valait `None`, et le message était
                    // silencieusement perdu. Le client de la tâche 8 attend
                    // ce message pour effacer sa bannière de statut.
                    let (width, height) = self.dimensions;
                    self.queue_control(AgentControl::ready(width, height));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(v: u8) -> Pt {
        Pt::from(v)
    }

    // -- classify_recv_error / recv_error_backoff -------------------------
    //
    // Pas de socket réelle ici : provoquer un WSAECONNRESET déterministe
    // demanderait une vraie machine Windows et un pair qui ferme sa
    // connexion au bon moment, ce que la revue exclut explicitement comme
    // non testable de façon fiable. On teste donc la logique pure de
    // classification et de temporisation, indépendamment de toute E/S.

    #[test]
    fn connection_reset_est_transitoire() {
        // Le cas motivant (I2 étendu) : `ConnectionReset` est la variante
        // portable vers laquelle Rust normalise `WSAECONNRESET`, reçu sur
        // une socket UDP Windows après un ICMP « port injoignable ».
        assert_eq!(
            classify_recv_error(std::io::ErrorKind::ConnectionReset),
            RecvErrorAction::RetryWithBackoff
        );
    }

    #[test]
    fn interrupted_est_transitoire() {
        assert_eq!(
            classify_recv_error(std::io::ErrorKind::Interrupted),
            RecvErrorAction::RetryWithBackoff
        );
    }

    #[test]
    fn erreurs_non_reconnues_sont_fatales() {
        // Une sélection représentative d'erreurs qui n'ont aucune raison de
        // se résorber d'elles-mêmes : pas de liste exhaustive nécessaire,
        // seulement la preuve que le classement par défaut est bien fatal
        // (pas transitoire), pas l'inverse d'une liste d'exceptions
        // ouverte.
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::NotConnected,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::Unsupported,
            std::io::ErrorKind::Other,
        ] {
            assert_eq!(classify_recv_error(kind), RecvErrorAction::Fatal, "{kind:?}");
        }
    }

    #[test]
    fn backoff_croit_avec_le_nombre_d_erreurs_consecutives() {
        let un = recv_error_backoff(1);
        let deux = recv_error_backoff(2);
        let trois = recv_error_backoff(3);
        assert!(un < deux, "{un:?} devrait être < {deux:?}");
        assert!(deux < trois, "{deux:?} devrait être < {trois:?}");
    }

    #[test]
    fn backoff_reste_borne_meme_apres_une_tres_longue_rafale() {
        // Preuve directe du défaut visé : sans borne, une rafale
        // d'erreurs consécutives ferait croître le délai sans limite (ou
        // déborderait l'arithmétique). Ici, même après un nombre d'erreurs
        // qui ferait déborder `1u32 << n` en `u32` sans la borne sur
        // l'exposant, le résultat reste fini et plafonné.
        assert_eq!(recv_error_backoff(1_000_000), RECV_ERROR_BACKOFF_MAX);
        assert!(recv_error_backoff(50) <= RECV_ERROR_BACKOFF_MAX);
    }

    #[test]
    fn backoff_est_non_nul_des_la_premiere_erreur() {
        // Une seule erreur suffit déjà à introduire une temporisation : pas
        // de « premier coup gratuit » qui laisserait passer un tour de
        // boucle serrée avant que le mécanisme ne s'engage.
        assert!(recv_error_backoff(1) > Duration::ZERO);
    }

    #[test]
    fn selectionne_le_mode_de_paquetisation_1() {
        let candidates = vec![
            CandidatePt { codec: Codec::H264, packetization_mode: Some(0), pt: pt(96) },
            CandidatePt { codec: Codec::H264, packetization_mode: Some(1), pt: pt(98) },
            CandidatePt { codec: Codec::Opus, packetization_mode: None, pt: pt(111) },
        ];
        assert_eq!(select_h264_pt(candidates.into_iter()), Some(pt(98)));
    }

    #[test]
    fn ignore_les_profils_sans_mode_1() {
        let candidates = vec![
            CandidatePt { codec: Codec::H264, packetization_mode: Some(0), pt: pt(96) },
            CandidatePt { codec: Codec::H264, packetization_mode: None, pt: pt(97) },
            CandidatePt { codec: Codec::Opus, packetization_mode: None, pt: pt(111) },
        ];
        assert_eq!(select_h264_pt(candidates.into_iter()), None);
    }

    #[test]
    fn ignore_les_codecs_non_h264_meme_en_mode_1() {
        let candidates = vec![CandidatePt {
            codec: Codec::Vp8,
            packetization_mode: Some(1),
            pt: pt(100),
        }];
        assert_eq!(select_h264_pt(candidates.into_iter()), None);
    }

    #[test]
    fn cadence_normale_basee_sur_l_echeance_precedente_sans_derive() {
        let start = Instant::now();
        let interval = Duration::from_micros(16_667);
        let previous = start;
        let now = start + Duration::from_micros(100); // léger retard d'envoi
        let next = next_frame_deadline(previous, now, interval);
        // Basé sur `previous`, pas sur `now` : le retard ne s'accumule pas.
        assert_eq!(next, previous + interval);
    }

    #[test]
    fn rattrapage_borne_apres_un_long_blocage() {
        let start = Instant::now();
        let interval = Duration::from_micros(16_667);
        let previous = start;
        let now = start + Duration::from_millis(500); // bloqué bien plus d'un intervalle
        let next = next_frame_deadline(previous, now, interval);
        // Pas de rafale de rattrapage : on repart d'un intervalle après
        // maintenant plutôt que de tenter de renvoyer toutes les images
        // manquées d'un coup.
        assert_eq!(next, now + interval);
    }

    #[test]
    fn attente_bornee_par_l_echeance_d_image_la_plus_proche() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_secs(1);
        let next_frame_at = now + Duration::from_micros(5_000);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at));
        assert_eq!(wait, Duration::from_micros(5_000));
    }

    #[test]
    fn attente_bornee_par_l_echeance_rtc_si_plus_proche() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_micros(2_000);
        let next_frame_at = now + Duration::from_secs(1);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at));
        assert_eq!(wait, Duration::from_micros(2_000));
    }

    #[test]
    fn attente_dictee_par_rtc_seul_sans_piste_video() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_millis(10);
        assert_eq!(bounded_wait(now, rtc_deadline, None), Duration::from_millis(10));
    }

    /// Preuve d'intégration pour C1 (cadence) et C2 (drainage) : les tests
    /// ci-dessus valident les fonctions pures, mais la revue demandait une
    /// mesure réelle de cadence. Sans navigateur disponible, on simule le
    /// pair offrant avec un second `Rtc` str0m en loopback UDP — exactement
    /// la forme que le brief attribue au navigateur (piste vidéo recvonly et
    /// deux canaux de données). `Session::run` tourne sur un thread dédié,
    /// comme en production, pendant que ce test pilote le pair et compte les
    /// images vidéo reçues sur une fenêtre fixe après connexion.
    ///
    /// Avant le correctif de C1, l'attente entre deux images valait jusqu'à
    /// l'échéance que réclame `Rtc` (jusqu'à 1 s, imposée par les timers
    /// RTCP/statistiques) au lieu d'être bornée par `next_frame_at` : ce test
    /// aurait alors mesuré environ 1 image/s au lieu de ~60.
    #[test]
    fn atteint_la_cadence_video_visee_avec_un_pair_local() {
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, MediaKind};

        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let source_path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let source = Box::new(
            crate::source::FileSource::from_path(source_path, 1280, 720, 60)
                .expect("chargement du flux de test"),
        );

        let mut session = Session::new(source, local_ip).expect("session");

        // Pair « navigateur » minimal : un second `Rtc`, offrant, avec une
        // piste vidéo recvonly et les deux canaux de données.
        let peer_socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket du pair");
        let peer_addr = peer_socket.local_addr().unwrap();
        let mut peer_rtc = Rtc::builder().clear_codecs().enable_h264(true).build(Instant::now());
        peer_rtc.add_local_candidate(Candidate::host(peer_addr, "udp").unwrap());

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        // La session tourne sur un thread dédié, comme en production (voir
        // `main.rs` / `tokio::task::spawn_blocking`). Le thread n'est pas
        // rejoint : `Session::run` ne se termine qu'à la détection d'une
        // déconnexion ICE (délai de plusieurs secondes), ce qui ralentirait
        // ce test sans rien y ajouter. Le processus de test se termine de
        // toute façon en fin de suite ; le thread ne fuit pas au-delà.
        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        // Boucle du pair : pilote son propre `Rtc` (STUN, ACKs DTLS...) et
        // compte les images vidéo reçues pendant une fenêtre fixe démarrée
        // à la connexion (pas avant : le temps de poignée de main ICE/DTLS
        // ne doit pas être compté contre la cadence mesurée).
        let hard_deadline = Instant::now() + Duration::from_secs(10);
        let measure_window = Duration::from_secs(2);
        let mut connected_at: Option<Instant> = None;
        let mut media_count = 0usize;

        loop {
            let now = Instant::now();
            if now >= hard_deadline {
                panic!("le pair local ne s'est jamais connecté dans le délai imparti");
            }
            if let Some(connected_at) = connected_at {
                if now >= connected_at + measure_window {
                    break;
                }
            }

            match peer_rtc.poll_output().expect("poll_output du pair") {
                Output::Timeout(t) => {
                    let cap = match connected_at {
                        Some(c) => hard_deadline.min(c + measure_window),
                        None => hard_deadline,
                    };
                    let wait = t.saturating_duration_since(now).min(cap.saturating_duration_since(now));
                    if wait.is_zero() {
                        let _ = peer_rtc.handle_input(Input::Timeout(now));
                        continue;
                    }
                    peer_socket.set_read_timeout(Some(wait)).unwrap();
                    let mut buffer = vec![0u8; 2000];
                    match peer_socket.recv_from(&mut buffer) {
                        Ok((n, source_addr)) => {
                            if let Ok(contents) = DatagramRecv::try_from(&buffer[..n]) {
                                let receive = Receive {
                                    proto: Protocol::Udp,
                                    source: source_addr,
                                    destination: peer_addr,
                                    contents,
                                };
                                let _ =
                                    peer_rtc.handle_input(Input::Receive(Instant::now(), receive));
                            }
                        }
                        Err(_) => {
                            let _ = peer_rtc.handle_input(Input::Timeout(Instant::now()));
                        }
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    connected_at = Some(Instant::now());
                }
                Output::Event(Event::MediaData(_)) => {
                    if connected_at.is_some() {
                        media_count += 1;
                    }
                }
                Output::Event(_) => {}
            }
        }

        let per_second = media_count as f64 / measure_window.as_secs_f64();
        eprintln!(
            "cadence mesurée : {media_count} images vidéo reçues en {measure_window:?} ({per_second:.1}/s)"
        );

        // Preuve de C1 : au rythme voulu (~60 Hz), on attend nettement plus
        // de 10 images par seconde. Le bug de cadence corrigé n'en aurait
        // produit qu'environ une par seconde — le seuil ci-dessous exclut
        // sans ambiguïté ce rythme tout en restant robuste à une machine de
        // test lente ou une CI chargée.
        assert!(
            per_second > 10.0,
            "cadence trop basse : {per_second:.1} images/s (attendu très supérieur à 1/s, la marque du bug de cadence C1)"
        );
    }
}
