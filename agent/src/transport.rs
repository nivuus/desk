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
//! dédié — `tokio::task::spawn_blocking` côté `demarrage.rs` — jamais depuis un
//! ouvrier async de tokio.
//!
//! Ce fichier ne porte plus que l'état de la session et la boucle qui
//! l'anime. Le reste est réparti par thème dans les sous-modules, tous
//! écrits en `impl Session` : `tick` (la liste de priorités d'un tour, dont
//! `act_on_timeout`), `controle` (canal de contrôle et fin de session),
//! `adaptation` (asservissement au réseau), `redimensionnement` (la fenêtre
//! que l'utilisateur retaille), `evenements` (ce que str0m remonte),
//! `piste_video` et `piste_audio` (les deux pistes média), `socket` (attente
//! et réception UDP), `fixtures` (les échafaudages de test partagés).

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use proto::control::{AgentControl, ClientControl};
use proto::input::InputMessage;
use str0m::bwe::Bitrate;
use str0m::channel::ChannelId;
use str0m::media::Mid;
use str0m::{Candidate, Output, Rtc};

use crate::audio::AudioSource;
use crate::congestion;
use crate::source::VideoSource;

#[cfg(test)]
mod fixtures;
mod adaptation;
mod controle;
mod evenements;
mod piste_audio;
mod piste_video;
mod redimensionnement;
mod socket;
mod tick;

use adaptation::ESTIMATION_INITIALE_BPS;
use piste_video::FRAME_INTERVAL;
use socket::TimerResolutionGuard;
use tick::Tick;

pub struct Session {
    rtc: Rtc,
    socket: UdpSocket,
    source: Box<dyn VideoSource + Send>,
    dimensions: (u32, u32),
    /// Origine d'horloge de la session, partagée avec les sources. Sert à
    /// reconstruire l'instant de capture d'une image à partir de son
    /// horodatage (voir `write_frame`).
    clock_origin: Instant,
    video_mid: Option<Mid>,
    control_channel: Option<ChannelId>,
    started: Instant,
    /// Messages de contrôle en attente d'émission. `run()` en envoie un au
    /// plus par mutation, dès que le canal est ouvert.
    pending_control: VecDeque<AgentControl>,
    /// Messages de contrôle produits HORS de la boucle : fil de sondage du
    /// curseur, rappel de vibration du pilote ViGEmBus. Ni l'un ni l'autre ne
    /// peut toucher la `Session`, qui n'est possédée que par `run()`.
    outbound_control: Option<std::sync::mpsc::Receiver<AgentControl>>,
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
    /// Source audio, absente tant qu'aucune n'a été fournie (source de test
    /// vidéo, plateforme sans audio, ou échec d'ouverture du loopback — dans
    /// tous les cas la session vidéo continue).
    audio_source: Option<Box<dyn AudioSource + Send>>,
    /// `mid` de la piste audio, renseigné à la négociation.
    audio_mid: Option<Mid>,
    /// Pendant audio de `video_write_pending_drain`. Distinct de lui : sans
    /// drapeau propre, une écriture audio suivie d'une écriture vidéo au tour
    /// suivant perdrait un drainage.
    audio_write_pending_drain: bool,
    /// Signale une seule fois qu'aucun type de charge utile Opus n'a été
    /// négocié, plutôt qu'à chaque paquet jeté.
    warned_audio_negotiation: bool,
    /// Dernier redimensionnement demandé, pas encore appliqué. On ne garde
    /// que le plus récent : pendant qu'un utilisateur tire un bord, les
    /// demandes intermédiaires n'ont aucun intérêt. Appliqué dans
    /// `act_on_timeout`, jamais depuis `dispatch_channel_data` — voir le
    /// commentaire de ce champ à son point de consommation.
    pending_resize: Option<(u32, u32)>,
    /// Contrôleur de congestion. Alimenté par `Event::EgressBitrateEstimate`
    /// et `Event::MediaEgressStats`, tous deux déjà émis par str0m — le
    /// second l'était même déjà avant ce chantier, et tombait dans le `_ =>
    /// {}` de `handle_event`.
    congestion: congestion::Controleur,
    /// Dernière estimation reçue, avec l'instant de sa réception, en attente
    /// d'être confrontée aux statistiques. Les deux événements n'arrivent pas
    /// ensemble.
    ///
    /// **Horodatée depuis I4 (revue finale de branche).** Sans l'instant, une
    /// estimation reçue une seule fois puis plus jamais (TWCC qui se tarit)
    /// resterait utilisée indéfiniment — voir `EXPIRATION_ESTIMATION`, qui la
    /// traite comme absente au-delà de son délai.
    derniere_estimation_bps: Option<(u32, Instant)>,
    /// Décision décidée mais pas encore appliquée. Appliquée dans
    /// `act_on_timeout`, jamais depuis `handle_event` — reconstruire
    /// l'encodeur pendant le drainage de `poll_output` romprait l'invariant
    /// de str0m (une seule mutation par appel), exactement comme pour
    /// `pending_resize`.
    pending_decision: Option<congestion::Decision>,
    /// Vrai une fois que l'indisponibilité de l'adaptation a été journalisée.
    /// Une condition permanente ne se journalise pas chaque seconde.
    absence_bwe_signalee: bool,
    /// Vrai une fois que l'indisponibilité de l'adaptation a été annoncée AU
    /// NAVIGATEUR (message `Link`). Drapeau distinct d'`absence_bwe_signalee`,
    /// qui ne couvre que le journal.
    ///
    /// **Ajouté pour I2 (revue finale de branche).** Avant ce correctif,
    /// `Controleur::observer` rendait `None` d'entrée quand aucune estimation
    /// n'était disponible, donc aucune `pending_decision` n'était jamais
    /// produite pour ce cas — `Adaptation::Indisponible` n'atteignait jamais
    /// le navigateur, alors que la spec l'exige nommément (« surtout pas un
    /// silence qui ressemble à tout va bien »).
    ///
    /// Remis à `false` dès qu'une estimation fraîche revient : une
    /// indisponibilité ultérieure (nouvelle coupure de TWCC, voir I4) est une
    /// information neuve, à annoncer de nouveau — comme `taille_refus_signalee`
    /// se remet à `None` dès qu'un changement de taille réussit.
    indisponibilite_annoncee: bool,
    /// Taille d'encodage réellement appliquée. Distincte de celle décidée :
    /// un refus de l'encodeur laisse la décision non appliquée, et il ne faut
    /// pas la retenter à chaque tour.
    encode_size_appliquee: (u32, u32),
    /// Dernière taille d'encodage dont le refus a été journalisé. Une
    /// condition permanente ne se journalise pas chaque seconde ; en
    /// revanche, une NOUVELLE cible refusée est une information neuve.
    /// Remis à `None` dès qu'un changement de taille réussit, pour qu'un
    /// refus ultérieur de la même taille soit à nouveau dit.
    taille_refus_signalee: Option<(u32, u32)>,
    /// Vrai une fois le refus du débit à chaud journalisé.
    refus_debit_signale: bool,
    /// Débit réellement appliqué par l'encodeur. Distinct de celui décidé :
    /// un refus du pilote laisse l'encodeur au débit précédent, et annoncer
    /// au navigateur un débit qu'il n'émet pas serait un mensonge de la même
    /// famille que celui déjà corrigé sur la qualité (tâche 5).
    bitrate_applique: u32,
    /// Dernier instant où `source.is_alive()` a été interrogée. Cet appel
    /// coûte un appel système côté Windows (recherche de fenêtre) : on
    /// l'espace plutôt que de le refaire à chaque tour de boucle — une
    /// fenêtre fermée le reste (voir `ALIVE_CHECK_INTERVAL`).
    last_alive_check: Instant,
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
    /// `clock_origin` est l'origine d'horloge de la session, partagée avec la
    /// source audio (voir `capture_instant`) : c'est elle qui rend les deux
    /// lignes de temps comparables et donc la synchro A/V exacte.
    pub fn new(
        source: Box<dyn VideoSource + Send>,
        local_ip: IpAddr,
        clock_origin: Instant,
        plafond_bps: u32,
    ) -> Result<Self> {
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

        // `enable_opus(true)` : sans cette ligne, aucun type de charge utile
        // Opus n'est jamais proposé dans la réponse SDP, quoi que le pair
        // négocie de son côté — `select_negotiated_opus_pt` ne trouverait
        // alors jamais rien, et l'audio resterait muet même avec une source
        // ouverte avec succès. Absente du brief original, ajoutée ici : sans
        // elle, la piste audio ne se négocie tout simplement jamais (voir le
        // rapport de tâche).
        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .enable_opus(true)
            // Sans cet appel, `Event::EgressBitrateEstimate` n'est JAMAIS
            // émis et tout l'asservissement reste muet. L'estimation
            // initiale est volontairement modeste : le sous-système sonde à
            // la hausse vers `set_desired_bitrate` (posé plus bas), et
            // partir trop haut ferait saturer le lien avant la première
            // correction.
            .enable_bwe(Some(Bitrate::bps(ESTIMATION_INITIALE_BPS as u64)))
            .set_stats_interval(Some(Duration::from_secs(1)))
            .build(Instant::now());

        // Cible que le sondage cherche à atteindre : le plafond configuré.
        rtc.bwe().set_desired_bitrate(Bitrate::bps(plafond_bps as u64));

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
            clock_origin,
            video_mid: None,
            control_channel: None,
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
            audio_write_pending_drain: false,
            warned_audio_negotiation: false,
            pending_resize: None,
            congestion: congestion::Controleur::new(
                congestion::Config {
                    plafond_bps,
                    // Référence `opus::BITRATE_BPS` plutôt qu'une constante
                    // dupliquée (I5, revue finale de branche) : une valeur en
                    // dur ici pouvait diverger silencieusement de ce que
                    // l'encodeur Opus utilise réellement.
                    audio_bps: crate::opus::BITRATE_BPS as u32,
                    source: dimensions,
                    // **Délibérément 60, PAS `ENCODER_FPS`** (I5, revue finale
                    // de branche). `ENCODER_FPS` (défaut 90, voir `demarrage.rs`)
                    // est la cadence de SOLLICITATION de l'encodeur, pas la
                    // cadence DÉLIVRÉE — la recette mesure 55 à 63 im/s
                    // réellement décodées, bien plus proche de 60 que de 90.
                    // Et surtout : `BPP_MIN` (voir `congestion/echelle.rs`) a été
                    // calibrée avec `fps = 60`. `fps` multiplie directement
                    // tous les `min_bps` de l'échelle — le faire suivre
                    // `ENCODER_FPS` multiplierait tous les seuils par 1,5 et
                    // invaliderait une calibration déjà fragile (reconduite
                    // sans preuve visuelle, voir le commentaire de
                    // `BPP_MIN`), sans mesure pour la refaire. `BPP_MIN` et ce
                    // `fps` sont COUPLÉS et doivent être recalibrés ENSEMBLE,
                    // jamais l'un sans l'autre.
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
            // Comme le contrôleur initialise le sien : avant toute décision
            // appliquée, le débit réel est celui de repli, le plafond.
            bitrate_applique: plafond_bps,
            last_alive_check: Instant::now(),
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
}
