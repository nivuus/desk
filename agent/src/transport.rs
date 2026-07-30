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
use str0m::media::Mid;
use str0m::bwe::Bitrate;
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};

use crate::audio::AudioSource;
use crate::congestion;
use crate::source::VideoSource;

#[cfg(test)]
mod fixtures;
mod piste_audio;
mod piste_video;
mod socket;

use piste_video::{next_frame_deadline, FRAME_INTERVAL};
use socket::{bounded_wait, classify_recv_error, recv_error_backoff, RecvErrorAction};
use socket::{TimerResolutionGuard, RECV_POLL_INTERVAL};

/// Intervalle minimal entre deux vérifications de `source.is_alive()` dans
/// `act_on_timeout`. Cet appel coûte un appel système à chaque tour côté
/// Windows (recherche de la fenêtre) ; une fenêtre fermée le reste, inutile
/// de le revérifier à 60 Hz.
const ALIVE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// Estimation de bande passante de départ, avant toute rétroaction du pair.
///
/// Compromis mesuré à la tâche 12 : trop bas, le démarrage sur LAN met du
/// temps à rejoindre le plafond et la recette perd des images par seconde ;
/// trop haut, le premier instant d'une session sur lien étroit sature avant
/// la première correction. 2,5 Mb/s est le point de départ, à confirmer.
const ESTIMATION_INITIALE_BPS: u32 = 2_500_000;

/// Durée au-delà de laquelle une estimation de bande passante non renouvelée
/// est traitée comme absente (I4, revue finale de branche).
///
/// `Event::EgressBitrateEstimate` et `Event::MediaEgressStats` n'arrivent pas
/// ensemble (voir le commentaire du champ `derniere_estimation_bps`) : sans
/// cette borne, une estimation reçue une seule fois puis plus jamais (TWCC qui
/// se tarit alors que la session survit) resterait utilisée indéfiniment par
/// le contrôleur — potentiellement la dernière valeur haute avant l'incident,
/// ce qui annoncerait « Bonne » sur un lien mort. `MediaEgressStats` arrive
/// environ une fois par seconde (`set_stats_interval`) : 5 s laisse plusieurs
/// occasions manquées avant de conclure à l'absence, sans laisser une
/// estimation figée vivre des dizaines de secondes.
const EXPIRATION_ESTIMATION: Duration = Duration::from_secs(5);

/// Résultat du traitement d'un événement ou d'un tour de boucle interne.
enum Tick {
    Continue,
    Disconnected,
}

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
                    // de branche). `ENCODER_FPS` (défaut 90, voir `main.rs`)
                    // est la cadence de SOLLICITATION de l'encodeur, pas la
                    // cadence DÉLIVRÉE — la recette mesure 55 à 63 im/s
                    // réellement décodées, bien plus proche de 60 que de 90.
                    // Et surtout : `BPP_MIN` (voir `congestion.rs`) a été
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

    /// Met en file un message de contrôle à envoyer dès que le canal est
    /// disponible. Ne mute jamais `Rtc` : l'envoi effectif a lieu dans
    /// `run()`, seul endroit qui mute la session une fois la boucle démarrée.
    fn queue_control(&mut self, message: AgentControl) {
        self.pending_control.push_back(message);
    }

    /// Décision d'adaptation actuellement retenue. Alimente le message d'état
    /// du lien envoyé au navigateur (tâche 10).
    ///
    /// `encode_size` et `video_bitrate_bps` viennent de ce que le transport a
    /// RÉELLEMENT réussi à appliquer (`encode_size_appliquee`,
    /// `bitrate_applique`), pas de ce que le contrôleur a décidé : celui-ci
    /// reste optimiste par construction (voir `congestion::Controleur`), et
    /// seul le transport sait si l'encodeur a accepté le dernier réglage.
    /// Annoncer au navigateur une taille ou un débit que la piste n'émet pas
    /// serait un mensonge de la même famille que celui déjà corrigé sur
    /// `qualite` à la tâche 5. Les autres champs (`qualite`, `adaptation`,
    /// `opus_loss_perc`) restent ceux du contrôleur : aucun mécanisme de
    /// refus équivalent n'existe pour eux ici.
    pub fn decision_courante(&self) -> congestion::Decision {
        congestion::Decision {
            encode_size: self.encode_size_appliquee,
            video_bitrate_bps: self.bitrate_applique,
            ..self.congestion.courant()
        }
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
    /// de `Rtc` (drainage différé d'une image déjà écrite, message de
    /// contrôle en attente, image vidéo due, ou traitement d'un paquet
    /// entrant / échéance str0m), puis rend la main à `run()`, qui rappelle
    /// immédiatement `poll_output` — c'est cette structure qui garantit le
    /// drainage avant toute mutation suivante (C2 de la revue) : il
    /// n'existe aucun chemin de code qui mute `Rtc` sans que `run()` ne
    /// rappelle `poll_output` juste après. La priorité donnée au drainage
    /// différé (voir `video_write_pending_drain`) est ce qui rend cette
    /// garantie vraie même juste après l'écriture d'une image : sans elle,
    /// `write_frame` (une mutation) suivi directement de `handle_input`
    /// (une seconde) violerait la même règle.
    ///
    /// Quatre branches supplémentaires (a0bis : drainage d'un message de
    /// contrôle produit hors boucle vers `pending_control` ; a0ter :
    /// décision d'adaptation en attente ; a1, a2 : redimensionnement en
    /// attente et vérification de la fenêtre) ne mutent JAMAIS `Rtc` — elles
    /// ne touchent que `self.source`, `self.audio_source` et/ou
    /// `self.pending_control`, au plus en y mettant en file un message de
    /// contrôle (`queue_control`, qui n'empile qu'un `VecDeque`, sans effet
    /// sur `Rtc` avant le tour suivant). Chacune rend quand même la main
    /// immédiatement après son action plutôt que d'enchaîner sur la branche
    /// suivante dans le même appel : le redimensionnement reconstruit une
    /// chaîne d'encodage entière (potentiellement long, voir
    /// `WindowsSource::resize`), et le traiter comme une étape à part
    /// entière — au même titre que les branches qui, elles, mutent
    /// réellement `Rtc` — garde cette fonction lisible comme une seule
    /// liste de priorités plutôt que de mêler deux styles différents.
    ///
    /// Ne prend pas `on_input`/`on_control` : `handle_input` ne produit
    /// jamais d'événement applicatif directement (les événements qui en
    /// résultent ne sortent que via un futur `poll_output`, donc via
    /// `run()`, qui les dispatche lui-même).
    fn act_on_timeout(&mut self, deadline: Instant) -> Result<Tick> {
        // a0) Drainage dû après la dernière image vidéo ou le dernier paquet
        // audio écrit. Vérifié en priorité absolue, avant tout le reste :
        // c'est la seule façon de garantir qu'aucune mutation ne s'enchaîne
        // jamais sans un passage complet par `poll_output()` entre les deux,
        // quel que soit l'état des autres files (voir le commentaire du
        // champ et la ronde de correction 1 de la tâche 11).
        if self.video_write_pending_drain || self.audio_write_pending_drain {
            // Un seul `handle_input(Timeout)` dépile `to_payload` pour TOUTES
            // les pistes : les deux drapeaux retombent donc ensemble. Les
            // garder séparés reste nécessaire en amont — c'est ce qui permet
            // à `write_audio` et `write_frame` de signaler indépendamment
            // qu'une écriture a bien eu lieu.
            self.video_write_pending_drain = false;
            self.audio_write_pending_drain = false;
            self.rtc
                .handle_input(Input::Timeout(Instant::now()))
                .map_err(|e| anyhow!("handle_input timeout (drainage média) : {e}"))?;
            return Ok(Tick::Continue);
        }

        // a0bis) Un message de contrôle produit hors de la boucle attend.
        // `try_recv` ne bloque jamais. On rend la main immédiatement après
        // l'avoir mis en file, comme les branches a1 et a2 : `queue_control`
        // ne mute pas `Rtc`, mais garder une seule action par tour est ce qui
        // rend cette fonction lisible comme une liste de priorités.
        //
        // La file est bornée : si le canal de contrôle n'est pas encore
        // ouvert, les messages s'y accumuleraient sans limite. Au-delà du
        // plafond on cesse de drainer — les producteurs (curseur, vibration)
        // émettent des ÉTATS, dont seul le dernier compte, et le canal mpsc
        // fera tampon en attendant.
        const PLAFOND_CONTROLE_EN_FILE: usize = 32;
        if self.pending_control.len() < PLAFOND_CONTROLE_EN_FILE {
            if let Some(rx) = &self.outbound_control {
                if let Ok(message) = rx.try_recv() {
                    self.queue_control(message);
                    return Ok(Tick::Continue);
                }
            }
        }

        // a) Un message de contrôle est en attente.
        if !self.pending_control.is_empty() {
            if let Some(id) = self.control_channel {
                let message = self.pending_control.pop_front().expect("non vide");
                // Nom du variant à des fins de journal uniquement : la
                // recette du chantier B (mesures 3 et 5) a dû contourner
                // l'observabilité de ce chemin par une instrumentation
                // client temporaire, faute d'une ligne ici — ce
                // `tracing::debug!` existe pour que le prochain diagnostic
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

        // a0ter) Décision d'adaptation en attente. Traitée avant la branche
        // vidéo et avant le redimensionnement : reconfigurer l'encodeur avec
        // une image en vol coûterait cette image.
        //
        // Ne mute jamais `Rtc` — seuls la source vidéo et l'encodeur audio
        // sont touchés — donc cette branche respecte l'invariant de drainage.
        if let Some(decision) = self.pending_decision.take() {
            match self.source.set_bitrate(decision.video_bitrate_bps) {
                Ok(()) => self.bitrate_applique = decision.video_bitrate_bps,
                Err(e) => {
                    // L'encodeur refuse le débit à chaud : on garde le débit
                    // courant et on continue d'adapter par la résolution. Une
                    // seule ligne, pas une par seconde.
                    if !self.refus_debit_signale {
                        self.refus_debit_signale = true;
                        tracing::warn!(erreur = %e, "l'encodeur refuse le réglage du débit à chaud");
                    }
                }
            }
            if decision.encode_size != self.encode_size_appliquee {
                match self.source.set_encode_size(decision.encode_size.0, decision.encode_size.1) {
                    Ok(()) => {
                        tracing::info!(
                            largeur = decision.encode_size.0,
                            hauteur = decision.encode_size.1,
                            "taille d'encodage changée"
                        );
                        self.encode_size_appliquee = decision.encode_size;
                        // Un refus ultérieur de cette même taille (ou d'une
                        // autre) redeviendra une information neuve.
                        self.taille_refus_signalee = None;
                    }
                    Err(e) => {
                        // On reste au barreau courant. La session vit. Une
                        // cible DIFFÉRENTE refusée est une information
                        // neuve ; la même cible répétée à chaque décision
                        // (une par seconde, potentiellement des heures sous
                        // congestion soutenue) ne l'est pas.
                        if self.taille_refus_signalee != Some(decision.encode_size) {
                            self.taille_refus_signalee = Some(decision.encode_size);
                            tracing::warn!(
                                erreur = %e,
                                largeur = decision.encode_size.0,
                                hauteur = decision.encode_size.1,
                                "changement de taille d'encodage refusé, barreau conservé"
                            );
                        }
                    }
                }
            }
            if let Some(audio) = self.audio_source.as_mut() {
                if let Err(e) = audio.set_packet_loss_perc(decision.opus_loss_perc) {
                    tracing::warn!(erreur = %e, "réglage du taux de perte Opus refusé");
                }
            }
            // On annonce `decision_courante()`, pas `decision` : `bitrate` et
            // `encode_size` doivent refléter ce que l'encodeur a RÉELLEMENT
            // accepté ci-dessus (`self.bitrate_applique`,
            // `self.encode_size_appliquee`), pas la cible visée par le
            // contrôleur — un refus d'encodeur laisserait sinon passer au
            // navigateur exactement le mensonge que `decision_courante()`
            // existe pour éviter (voir sa documentation et le test
            // `un_refus_repete_de_set_encode_size_ne_remonte_pas_dans_decision_courante`).
            let etat_lien = self.decision_courante();
            self.queue_control(AgentControl::link(
                etat_lien.video_bitrate_bps,
                etat_lien.encode_size,
                match etat_lien.qualite {
                    congestion::Qualite::Bonne => proto::control::LinkQuality::Bonne,
                    congestion::Qualite::Degradee => proto::control::LinkQuality::Degradee,
                    congestion::Qualite::Insuffisante => proto::control::LinkQuality::Insuffisante,
                },
                match etat_lien.adaptation {
                    congestion::Adaptation::Active => proto::control::LinkAdaptation::Active,
                    congestion::Adaptation::Indisponible => {
                        proto::control::LinkAdaptation::Indisponible
                    }
                },
            ));
            return Ok(Tick::Continue);
        }

        // a1) Redimensionnement en attente, à traiter avant la branche
        // vidéo. `self.source.resize()` ne mute jamais `Rtc` (elle
        // reconstruit uniquement la source vidéo, pas la session WebRTC),
        // mais reste une opération potentiellement longue — fenêtre ET
        // périphérique D3D11 neufs, voir `WindowsSource::resize` — traitée
        // ici comme une étape à part entière plutôt que mêlée à d'autres
        // dans le même appel, à l'image des autres branches de cette
        // fonction.
        if let Some((width, height)) = self.pending_resize.take() {
            match self.source.resize(width, height) {
                Ok(()) => {
                    // La fenêtre peut refuser la taille demandée (bornes
                    // minimales, alignement pair...) : le navigateur doit
                    // connaître les dimensions RÉELLEMENT obtenues, pas
                    // celles demandées.
                    let (actual_width, actual_height) = self.source.dimensions();
                    self.dimensions = (actual_width, actual_height);

                    // C1 (revue finale de branche). `WindowsSource::resize`
                    // reconstruit désormais TOUJOURS l'encodeur à la taille
                    // pleine de la nouvelle capture (voir son commentaire) :
                    // la taille réellement appliquée vient donc de changer
                    // par ce seul fait, sans être jamais passée par
                    // `set_encode_size`. On l'enregistre directement — il n'y
                    // a rien à « appliquer » ici, c'est déjà fait — plutôt
                    // que de la laisser transiter par `pending_decision`
                    // comme le ferait une décision normale du contrôleur.
                    self.encode_size_appliquee = (actual_width, actual_height);
                    // Une cible refusée avant ce redimensionnement n'a plus
                    // cours : la taille encodée vient de changer sous elle.
                    self.taille_refus_signalee = None;

                    // Le contrôleur doit être reconstruit pour la nouvelle
                    // taille de source : ses seuils (`min_bps` par barreau)
                    // sont dérivés de la taille de capture, qui vient de
                    // changer. Sans cela, l'échelle resterait calibrée pour
                    // une source qui n'existe plus — et pourrait viser une
                    // taille d'encodage supérieure à la nouvelle capture.
                    // `changer_source` conserve le barreau (le NIVEAU de
                    // réduction), pas la taille absolue ; la décision qui en
                    // résulte est mémorisée pour que la branche a0ter,
                    // au tour SUIVANT, la compare à `encode_size_appliquee`
                    // (celle ci-dessus, la taille pleine) et rappelle
                    // `set_encode_size` si le barreau conservé exige encore
                    // une réduction.
                    let decision = self
                        .congestion
                        .changer_source((actual_width, actual_height), Instant::now());
                    self.pending_decision = Some(decision);

                    self.queue_control(AgentControl::ready(actual_width, actual_height));
                }
                Err(e) => {
                    // Un échec de redimensionnement ne doit pas terminer la
                    // session : on journalise et la session continue avec
                    // les dimensions précédentes.
                    tracing::warn!(
                        erreur = %e,
                        width,
                        height,
                        "échec du redimensionnement, ignoré"
                    );
                }
            }
            return Ok(Tick::Continue);
        }

        // a2) La fenêtre capturée a-t-elle disparu ? Coûte un appel système
        // côté Windows (recherche de la fenêtre) : espacé par
        // `ALIVE_CHECK_INTERVAL` plutôt que vérifié à chaque tour de
        // boucle — une fenêtre fermée le reste.
        let now = Instant::now();
        if now.saturating_duration_since(self.last_alive_check) >= ALIVE_CHECK_INTERVAL {
            self.last_alive_check = now;
            if !self.source.is_alive() {
                self.begin_ending("fenêtre fermée");
                return Ok(Tick::Continue);
            }
        }

        // a3) Un paquet audio, si la piste est négociée et qu'un paquet
        //     attend. AVANT la vidéo : une coupure sonore s'entend, une image
        //     en retard de 10 ms ne se voit pas. L'audio a de plus une
        //     cadence dure de 10 ms, quand la vidéo est opportuniste par
        //     nature.
        //
        //     Pas d'échéance à surveiller ici : le fil de capture dépose dans
        //     un tampon, il suffit de regarder s'il y a quelque chose. Le
        //     réveil régulier vient d'`AUDIO_POLL_INTERVAL`, appliqué en
        //     branche `c`.
        if let (Some(mid), false) = (self.audio_mid, self.ending) {
            let paquet = self
                .audio_source
                .as_mut()
                .and_then(|source| source.next_packet());
            if let Some(paquet) = paquet {
                if self.write_audio(mid, paquet) {
                    self.audio_write_pending_drain = true;
                }
                return Ok(Tick::Continue);
            }
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
        let wait = bounded_wait(now, deadline, next_frame_at, self.audio_wait_cap());

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

    /// Branche une source externe de messages de contrôle. Même patron que
    /// `set_audio_source` : la session tire, elle n'est jamais poussée.
    pub fn set_control_source(&mut self, rx: std::sync::mpsc::Receiver<AgentControl>) {
        self.outbound_control = Some(rx);
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
                match media.kind {
                    str0m::media::MediaKind::Video => self.video_mid = Some(media.mid),
                    str0m::media::MediaKind::Audio => self.audio_mid = Some(media.mid),
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
                // Le navigateur demande une image clé, typiquement après une
                // perte de paquet détectée par le décodeur. Le groupe
                // d'images de l'encodeur matériel est ouvert (voir
                // `encode::configure_rate_control`) : sans ce relais, aucune
                // image clé n'est plus jamais produite après le démarrage, et
                // la perte corrompt la vidéo jusqu'à reconnexion. Ne mute pas
                // `Rtc` — seul l'encodeur (côté `VideoSource`) est affecté —
                // donc ce relais respecte l'invariant de drainage documenté
                // en tête de fichier même appelé depuis `handle_event`.
                tracing::debug!(mid = ?request.mid, "image clé demandée par le pair");
                if let Err(e) = self.source.request_keyframe() {
                    tracing::warn!(erreur = %e, mid = ?request.mid, "échec de la demande d'image clé");
                }
            }
            Event::EgressBitrateEstimate(kind) => {
                // Les deux variantes portent une estimation ; seule REMB
                // nomme en plus le `mid` concerné, dont on n'a pas l'usage
                // avec une piste vidéo unique.
                let bps = match kind {
                    str0m::bwe::BweKind::Twcc(b) => b.as_u64(),
                    str0m::bwe::BweKind::Remb(_, b) => b.as_u64(),
                    // `BweKind` est `#[non_exhaustive]` côté str0m : une
                    // variante future retomberait ici plutôt que d'empêcher
                    // la compilation. Rien à faire de mieux qu'ignorer une
                    // estimation qu'on ne sait pas encore interpréter.
                    _ => return Tick::Continue,
                };
                self.derniere_estimation_bps = Some((bps as u32, Instant::now()));
            }
            Event::MediaEgressStats(stats) => {
                // Seule la piste vidéo alimente la décision : l'audio a un
                // débit fixe et son budget est déjà retiré par le contrôleur.
                if Some(stats.mid) != self.video_mid {
                    return Tick::Continue;
                }
                // I4 (revue finale de branche) : une estimation reçue une
                // seule fois puis plus jamais (TWCC qui se tarit alors que la
                // session survit) est traitée comme absente au-delà
                // d'`EXPIRATION_ESTIMATION`, plutôt que d'être utilisée
                // indéfiniment — potentiellement la dernière valeur haute
                // avant l'incident, ce qui annoncerait « Bonne » sur un lien
                // mort.
                let now = Instant::now();
                let estimate_bps = self.derniere_estimation_bps.and_then(|(bps, at)| {
                    (now.saturating_duration_since(at) <= EXPIRATION_ESTIMATION).then_some(bps)
                });
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
                        "aucune estimation de bande passante reçue : l'adaptation reste \
                         indisponible et le débit demeure au plafond configuré"
                    );
                }
                tracing::debug!(
                    estimation = ?observation.estimate_bps,
                    rtt = ?observation.rtt,
                    perte = ?observation.loss,
                    "observation réseau"
                );
                // `observer` DOIT être appelé avant de lire `courant()`
                // ci-dessous : c'est lui qui, dans sa branche sans
                // estimation, met `courant.adaptation` à jour vers
                // `Indisponible` (voir son commentaire). Lire `courant()`
                // avant cet appel rendrait un instantané périmé (encore
                // `Active`) sur la transition qui nous intéresse le plus.
                if let Some(decision) = self.congestion.observer(observation) {
                    // Mémorisée, pas appliquée : voir le commentaire du champ.
                    self.pending_decision = Some(decision);
                }
                if absence {
                    // I2 (revue finale de branche) : `Controleur::observer`
                    // ne produit JAMAIS de décision quand l'estimation
                    // manque (voir son commentaire, retour anticipé) — sans
                    // ce relais explicite, `Adaptation::Indisponible`
                    // n'atteint donc jamais le navigateur, alors que la spec
                    // l'exige nommément. On pose `self.congestion.courant()`,
                    // lu APRÈS l'appel ci-dessus : son champ `adaptation` est
                    // désormais à jour, et le reste (débit, taille) reflète
                    // la dernière décision réelle — la seule chose de sensé à
                    // annoncer tant qu'aucune nouvelle donnée n'arrive.
                    if !self.indisponibilite_annoncee {
                        self.indisponibilite_annoncee = true;
                        self.pending_decision = Some(self.congestion.courant());
                    }
                } else {
                    // Une estimation fraîche revient : une indisponibilité
                    // ultérieure redeviendra une information neuve.
                    self.indisponibilite_annoncee = false;
                }
            }
            _ => {}
        }
        Tick::Continue
    }

    fn dispatch_channel_data(
        &mut self,
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
                Ok(Ok(message)) => {
                    // Le redimensionnement ne s'applique pas ici : ce code
                    // s'exécute pendant le drainage de `poll_output`, et
                    // reconstruire la chaîne d'encodage y serait long et
                    // romprait l'invariant de drainage de str0m (une seule
                    // mutation de `Rtc` par appel). On mémorise seulement la
                    // demande la plus récente ; `act_on_timeout` l'applique à
                    // son tour, comme une étape à part entière.
                    //
                    // `ClientControl` n'a qu'une seule variante aujourd'hui :
                    // une déstructuration directe (pas `if let`) évite
                    // l'avertissement « pattern irréfutable ». Par référence,
                    // pour laisser `message` intact et le transmettre
                    // ensuite, inchangé, à `on_control`.
                    let ClientControl::Resize { width, height, .. } = &message;
                    self.pending_resize = Some((*width, *height));
                    on_control(message);
                }
                Ok(Err(e)) => tracing::warn!(erreur = %e, "message de contrôle invalide"),
                Err(e) => tracing::warn!(erreur = %e, "contrôle non UTF-8"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use str0m::format::Codec;

    use crate::audio::AudioPacket;
    use crate::h264::AccessUnit;

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

        /// Source audio de test : rend un paquet toutes les 10 ms au plus
        /// tôt, avec un `pts_48k` qui avance de 480 (une trame de 10 ms) à
        /// chaque paquet rendu — comme le ferait `WindowsAudioSource`
        /// (`PacketRing` alimenté par un fil de capture paçé, jamais
        /// disponible en continu). La charge utile n'a pas besoin d'être un
        /// Opus valide : ce test vérifie que la `Session` ACHEMINE les
        /// paquets jusqu'au pair, pas ce qu'un décodeur en ferait.
        ///
        /// **Constaté pendant l'écriture de ce test (ronde de correction
        /// 1)** : une première version rendait un paquet à CHAQUE appel, sans
        /// pacage. La branche `a3` passant avant la branche `b` (par
        /// construction, voir plus haut), un flux audio en continu
        /// affamait totalement la vidéo — `video_count` retombait à 0 sur
        /// toute la fenêtre de mesure. Ce n'est pas un défaut de la source
        /// réelle (`PacketRing`, bornée à 10 paquets et alimentée par un fil
        /// séparé au rythme de la capture, ne peut pas rendre en continu),
        /// mais un artefact d'une source de test irréaliste. Le pacage à
        /// 10 ms ci-dessous restaure un comportement fidèle à
        /// `WindowsAudioSource` : la plupart des appels à `next_packet`
        /// rendent `None`, exactement comme en production.
        struct DummyAudioSource {
            next_pts_48k: u64,
            next_due: Instant,
        }

        impl AudioSource for DummyAudioSource {
            fn next_packet(&mut self) -> Option<AudioPacket> {
                let now = Instant::now();
                if now < self.next_due {
                    return None;
                }
                self.next_due += Duration::from_millis(10);
                let pts_48k = self.next_pts_48k;
                self.next_pts_48k += 480;
                Some(AudioPacket {
                    data: vec![0xF8, 0xFF, 0xFE],
                    pts_48k,
                    captured_at: now,
                })
            }
        }

        let local_ip = fixtures::local_ip();
        let source = Box::new(fixtures::video_test_source());

        let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        // Pair « navigateur » minimal : un second `Rtc`, offrant, avec une
        // piste vidéo recvonly et les deux canaux de données.
        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, true);

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        // Non lié à une variable lue plus loin : le décompte plus bas
        // distingue audio et vidéo par codec, pas par `mid` (voir plus bas).
        api.add_media(MediaKind::Audio, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        // Ronde de correction 1 (revue) : la première version de ce test
        // faisait écrire le PAIR lui-même sur `audio_mid`, ce qui ne passait
        // jamais par `Session::write_audio` ni par la branche `a3` — la
        // suppression pure et simple de cette branche aurait laissé ce test
        // vert (constaté, voir le rapport de tâche). Ce qui doit réellement
        // être prouvé : une `Session` munie d'une source audio
        // (`set_audio_source`) ÉMET des paquets Opus que le pair reçoit. Le
        // décompte, plus bas, distingue les paquets audio des paquets vidéo
        // par leur codec (`Codec::Opus` vs `Codec::H264`), pas par leur
        // `mid` : `audio_mid` n'a donc plus besoin d'être lu après la
        // négociation SDP.
        session.set_audio_source(Box::new(DummyAudioSource {
            next_pts_48k: 0,
            next_due: Instant::now(),
        }));

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
        // compte les images vidéo ET les paquets audio reçus pendant une
        // fenêtre fixe démarrée à la connexion (pas avant : le temps de
        // poignée de main ICE/DTLS ne doit pas être compté contre la cadence
        // mesurée).
        //
        // Ronde de correction 1 (revue) : `media_count` comptait auparavant
        // tout `Event::MediaData` sous le nom d'« images vidéo ». Une fois la
        // source audio de test posée sur `Session` (ci-dessus), l'assertion
        // de cadence vidéo aurait aussi compté des paquets audio et serait
        // devenue fausse (silencieusement, sans jamais échouer pour la
        // mauvaise raison qu'un décompte trop haut). Les deux compteurs sont
        // désormais séparés par codec (`data.params.spec().codec`), pas par
        // `mid` — un paquet Opus reste un paquet Opus quel que soit le `mid`
        // qui le porte.
        let hard_deadline = Instant::now() + Duration::from_secs(10);
        let measure_window = Duration::from_secs(2);
        let mut connected_at: Option<Instant> = None;
        let mut video_count = 0usize;
        let mut audio_count = 0usize;

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
                    if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, now, wait) {
                        continue;
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    connected_at = Some(Instant::now());
                }
                Output::Event(Event::MediaData(data)) => {
                    if connected_at.is_some() {
                        match data.params.spec().codec {
                            Codec::Opus => audio_count += 1,
                            Codec::H264 => video_count += 1,
                            _ => {}
                        }
                    }
                }
                Output::Event(_) => {}
            }
        }

        let per_second = video_count as f64 / measure_window.as_secs_f64();
        eprintln!(
            "cadence mesurée : {video_count} images vidéo et {audio_count} paquets audio reçus en {measure_window:?} ({per_second:.1} images/s)"
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

        // Preuve de la tâche 8 (ronde de correction 1) : une `Session` munie
        // d'une source audio (`set_audio_source`, plus haut) doit
        // effectivement émettre des paquets Opus que le pair reçoit — pas
        // seulement négocier la piste. Sans la branche `a3` d'`act_on_timeout`
        // (celle qui appelle `write_audio`), ce compteur resterait à zéro :
        // constaté en la retirant temporairement (voir le rapport de tâche).
        assert!(
            audio_count > 0,
            "aucun paquet audio reçu par le pair : la Session, munie d'une source audio, \
             n'a émis aucun paquet Opus (la branche a3 d'act_on_timeout est-elle bien avant b, \
             ou write_audio échoue-t-il silencieusement ?)"
        );
    }

    /// Preuve d'intégration que `Event::KeyframeRequest` (émis par str0m
    /// quand le pair envoie un PLI/FIR RTCP — ce que fait un navigateur après
    /// une perte de paquet détectée par son décodeur) est bien relayé jusqu'à
    /// `VideoSource::request_keyframe`, sans passer par un mock du trait
    /// `Event` : le pair local ici est un vrai second `Rtc` str0m, comme dans
    /// `atteint_la_cadence_video_visee_avec_un_pair_local`.
    ///
    /// N'exerce PAS le chemin `WindowsSource`/`H264Encoder::request_keyframe`
    /// réel (`#![cfg(windows)]`, indisponible sur la machine de compilation
    /// Linux) : seul le relais `handle_event` → `Session::source` est prouvé
    /// ici. Le câblage `WindowsSource::request_keyframe` →
    /// `H264Encoder::request_keyframe` (`SetValue` sur
    /// `CODECAPI_AVEncVideoForceKeyFrame`) reste vérifié par lecture et par
    /// la compilation croisée Windows, pas par un test automatisé.
    #[test]
    fn relaie_une_demande_d_image_cle_du_pair_vers_la_source() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        use std::sync::Arc;
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, KeyframeRequestKind, MediaKind};

        /// Enveloppe `FileSource` en comptant les appels à
        /// `request_keyframe`, seule façon d'observer depuis ce test que le
        /// relais a bien eu lieu (le compteur est partagé via `Arc` avant que
        /// la source ne soit déplacée dans `Session`, qui la possède ensuite
        /// depuis le thread dédié de `Session::run`).
        struct CountingSource {
            inner: crate::source::FileSource,
            keyframe_requests: Arc<AtomicUsize>,
        }

        impl VideoSource for CountingSource {
            fn next_frame(&mut self) -> Option<AccessUnit> {
                self.inner.next_frame()
            }
            fn dimensions(&self) -> (u32, u32) {
                self.inner.dimensions()
            }
            fn request_keyframe(&mut self) -> Result<()> {
                self.keyframe_requests.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(())
            }
        }

        let local_ip = fixtures::local_ip();
        let keyframe_requests = Arc::new(AtomicUsize::new(0));
        let source = Box::new(CountingSource {
            inner: fixtures::video_test_source(),
            keyframe_requests: keyframe_requests.clone(),
        });

        let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

        let mut api = peer_rtc.sdp_api();
        // Recvonly côté pair == la piste vidéo que le navigateur reçoit
        // réellement de l'agent ; c'est sur ce `mid` que `writer(...)` émettra
        // le PLI plus bas (str0m nomme cet accès « writer » indépendamment du
        // sens du média — c'est l'API par laquelle la rétroaction RTCP sort).
        let video_mid = api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        let hard_deadline = Instant::now() + Duration::from_secs(10);
        let mut keyframe_requested_at_peer = false;

        loop {
            let now = Instant::now();
            if keyframe_requests.load(AtomicOrdering::SeqCst) > 0 {
                break; // Preuve faite : le relais a atteint la source.
            }
            if now >= hard_deadline {
                panic!(
                    "délai dépassé : le pair local ne s'est jamais connecté, ou \
                     Event::KeyframeRequest n'a jamais atteint VideoSource::request_keyframe \
                     (compteur toujours à 0)"
                );
            }

            match peer_rtc.poll_output().expect("poll_output du pair") {
                Output::Timeout(t) => {
                    let wait = t
                        .saturating_duration_since(now)
                        .min(hard_deadline.saturating_duration_since(now));
                    if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, now, wait) {
                        continue;
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    if !keyframe_requested_at_peer {
                        keyframe_requested_at_peer = true;
                        // Exactement ce que fait un navigateur après une
                        // perte de paquet détectée par son décodeur : demander
                        // une image clé via un PLI RTCP. `fb_pli` est vrai par
                        // défaut pour un codec vidéo dans str0m (voir
                        // `format::payload_params::PayloadParams::new`), donc
                        // cette négociation n'a rien de spécial à activer côté
                        // offre/réponse SDP.
                        let mut writer = peer_rtc.writer(video_mid).expect("writer vidéo");
                        writer
                            .request_keyframe(None, KeyframeRequestKind::Pli)
                            .expect("PLI négocié par défaut sur un codec vidéo (fb_pli)");
                    }
                }
                Output::Event(_) => {}
            }
        }

        assert!(
            keyframe_requests.load(AtomicOrdering::SeqCst) > 0,
            "Event::KeyframeRequest du pair n'a jamais atteint VideoSource::request_keyframe"
        );
    }

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

    #[test]
    fn un_refus_repete_de_set_encode_size_ne_remonte_pas_dans_decision_courante() {
        // Ronde de correction (revue post-tâche 9) : `Controleur::observer`
        // reste optimiste par construction — il met à jour `courant.encode_size`
        // que l'encodeur accepte ou non le changement. Sans la distinction
        // que ce test vérifie, `decision_courante()` annoncerait au
        // navigateur (message d'état du lien, tâche 10) une taille que la
        // piste vidéo n'émet jamais.
        //
        // Ce test couvre aussi la déduplication du journal côté refus
        // (`taille_refus_signalee`) : trois décisions identiques de suite,
        // comme le ferait le contrôleur une fois par seconde sous
        // congestion soutenue, ne doivent faire grandir ni changer cette
        // mémoire au-delà de sa première écriture — compter les lignes de
        // journal elles-mêmes n'est pas praticable dans ce harnais (aucune
        // capture de `tracing` n'existe dans ce module).
        struct SourceRefusant {
            inner: crate::source::FileSource,
        }

        impl VideoSource for SourceRefusant {
            fn next_frame(&mut self) -> Option<AccessUnit> {
                self.inner.next_frame()
            }
            fn dimensions(&self) -> (u32, u32) {
                self.inner.dimensions()
            }
            fn set_encode_size(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
                Err(anyhow!("pilote imaginaire : refuse toujours"))
            }
        }

        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let source_path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let source = Box::new(SourceRefusant {
            inner: crate::source::FileSource::from_path(source_path, 1280, 720, 60)
                .expect("chargement du flux de test"),
        });

        let mut session =
            Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");
        let taille_originale = session.encode_size_appliquee;
        let taille_visee = (640, 360);
        assert_ne!(taille_visee, taille_originale, "précondition du test");

        let decision = congestion::Decision {
            video_bitrate_bps: 5_000_000,
            encode_size: taille_visee,
            opus_loss_perc: 0,
            qualite: congestion::Qualite::Degradee,
            adaptation: congestion::Adaptation::Active,
        };

        // Trois décisions successives, comme le ferait le contrôleur une
        // fois par seconde sous congestion soutenue : la même taille
        // refusée à chaque tour.
        for _ in 0..3 {
            session.pending_decision = Some(decision);
            session
                .act_on_timeout(Instant::now())
                .expect("un refus de l'encodeur ne doit jamais faire échouer la session");
        }

        // Trouvaille 2 : `decision_courante()` doit continuer à rapporter
        // l'ANCIENNE taille, celle réellement émise — pas celle refusée.
        assert_eq!(
            session.decision_courante().encode_size,
            taille_originale,
            "un refus de l'encodeur ne doit jamais se refléter dans la décision annoncée"
        );

        // Trouvaille 1 : la mémoire de dédoublonnage retient la cible
        // refusée, stable sur les trois tours identiques — c'est elle qui
        // empêche la répétition du journal à chaque décision.
        assert_eq!(
            session.taille_refus_signalee,
            Some(taille_visee),
            "la cible refusée doit être mémorisée pour éviter de rejournaliser à chaque tour"
        );
    }
}
