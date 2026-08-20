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
//! Ce fichier ne porte plus que l'état de la session. ❌ *Il portait « et la
//! boucle qui l'anime » : faux depuis le sous-bloc F1, qui a extrait
//! `Session::run` — avec `accept_offer` et `drain_quietly` — vers
//! [`boucle`], le fichier ayant franchi 500 lignes (495 → 501 → 440). Les
//! deux paragraphes ci-dessus, qui décrivent `run()` au présent, sont dans le
//! même cas : ils décrivent une fonction qui vit maintenant dans `boucle`.*
//! Le reste est réparti par thème dans les sous-modules, presque
//! tous écrits en `impl Session` : `tick` (la liste de priorités d'un tour,
//! dont `act_on_timeout`), `controle` (canal de contrôle et fin de session),
//! `adaptation` (asservissement au réseau), `redimensionnement` (la fenêtre
//! que l'utilisateur retaille), `evenements` (ce que str0m remonte),
//! `piste_video`, `piste_audio` et `piste_micro` (les trois pistes média, la
//! dernière étant la seule MONTANTE), `socket` (attente
//! et réception UDP), `boucle` (`run` et le drainage), `fixtures` (les
//! échafaudages de test partagés). ⚠️ *Cette liste n'est pas exhaustive et ne
//! l'a jamais été — `cadence_video`, `part` et `relais` y manquaient avant
//! F1 ; seule la clause de clôture ci-dessous porte une affirmation.*
//! **Seul `initialisation` fait exception** : une fonction LIBRE
//! (`construire_rtc`), pas une méthode de `Session` — elle construit le
//! socket UDP et le `Rtc` str0m avant que `Session` elle-même n'existe, donc
//! avant qu'il y ait un `self` à qui l'attacher.

use std::collections::VecDeque;
use std::net::{IpAddr, UdpSocket};
use std::time::Instant;

use anyhow::Result;
use proto::control::AgentControl;
use str0m::channel::ChannelId;
use str0m::media::Mid;
use str0m::Rtc;

use crate::audio::{AudioSource, Reconstructeur};
use crate::congestion;
use crate::source::VideoSource;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod sonde_montante;
mod adaptation;
/// La boucle de transport et les deux points de drainage antérieurs à `run`.
///
/// 🔴 EXTRAIT PARCE QUE CE FICHIER A FRANCHI 500 LIGNES — pour la SECONDE fois,
/// et sur le même champ de bataille : le sous-bloc D10 l'avait déjà porté à 501
/// et en avait sorti `initialisation.rs` (le constructeur du socket et du
/// `Rtc`) ; le sous-bloc F1 l'y ramène en ajoutant `input_channel`, et en sort
/// la boucle. Ce dépôt écrit depuis D6 que « la marge regagnée par une
/// extraction se reperd à la ronde suivante si on la traite comme acquise » :
/// c'est la cinquième fois qu'il le paie, et la deuxième sur ce fichier-ci.
/// EXTRAIT, jamais compressé — la doctrine de `CLAUDE.md` interdit nommément
/// de raccourcir un commentaire pour repasser sous la ligne.
mod boucle;
mod cadence_video;
/// Le collage venu du navigateur : les DEUX moitiés de l'ordre de D6, écrites
/// au même endroit. Voir son commentaire de tête.
mod collage;
mod controle;
mod evenements;
mod initialisation;
mod part;
mod piste_audio;
mod piste_micro;
mod piste_video;
mod redimensionnement;
mod relais;
mod socket;
mod tick;

use piste_video::FRAME_INTERVAL;
use socket::TimerResolutionGuard;

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
    /// Le canal `input`, retenu comme `control_channel` l'est — c'est ce qui
    /// permet à `evenements::destination` d'aiguiller par CANAL plutôt que par
    /// le seul drapeau binaire. `None` tant qu'aucun `ChannelOpen` ne l'a
    /// nommé : une trame reçue d'ici là est refusée, pas devinée.
    input_channel: Option<ChannelId>,
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
    /// `mid` de la piste audio DESCENDANTE (chantier A, agent → navigateur),
    /// renseigné à la négociation. ⚠️ DEUX m-lines audio depuis le chantier E :
    /// la montante a le sien, `mic_mid`, et c'est la DIRECTION qui les sépare
    /// (`evenements.rs`, qui porte le défaut muet que ce champ a longtemps eu).
    audio_mid: Option<Mid>,
    /// `mid` de la piste du MICRO (chantier E), renseigné à la négociation.
    mic_mid: Option<Mid>,
    // Les quatre champs du MICRO (chantier E). Leur raisonnement vit en
    // entier dans `transport/piste_micro.rs`, auprès du code qui les emploie —
    // c'est un PLACEMENT de la documentation neuve, pas une compression : ce
    // fichier est à trois lignes de son plafond.
    /// Puits du flux montant, absent tant qu'aucun n'a été installé.
    puits_micro: Option<Box<dyn crate::micro::PuitsMicro + Send>>,
    /// Négociation ou horloge inattendue : signalées une seule fois.
    warned_micro_negotiation: bool,
    /// Refus du puits (exclusivité non acquise) : signalé une seule fois.
    refus_micro_signale: bool,
    /// Lignes de journal réellement ÉMISES au sujet du micro.
    journaux_micro: u64,
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
    /// Dernière visibilité annoncée par le navigateur, en attente
    /// d'application. Même raison de différer que `pending_resize`.
    pending_visibility: Option<(bool, bool)>,
    /// Dernier collage annoncé par le navigateur, en attente d'application
    /// (sous-bloc P2 du chantier presse-papier). Même raison de différer que
    /// `pending_resize` : `memoriser_controle` court pendant le drainage de
    /// `poll_output`, et str0m impose une seule mutation de `Rtc` par appel.
    ///
    /// ⚠️ **Un collage est un ÉVÉNEMENT, et il est pourtant mémorisé comme un
    /// ÉTAT — écrasement du dernier. Le coût est réel, et il est écrit :** deux
    /// collages arrivés entre deux tours de boucle se réduisent au second, le
    /// premier étant **perdu sans trace**. C'est acceptable parce qu'un tour de
    /// boucle est borné par la cadence vidéo et qu'un humain ne produit pas
    /// deux `Ctrl+V` dans cet intervalle — **mais un client qui se conduirait
    /// mal, lui, le pourrait**. Le remède serait une file bornée ; il n'est pas
    /// livré, et c'est un legs de P2.
    pending_clipboard: Option<String>,
    /// Le collage a été écrit dans le presse-papier de la VM : il reste à
    /// injecter `Ctrl+V`.
    ///
    /// 🔴 **Ce drapeau porte L'ORDRE de D6 à lui seul**, et c'est pourquoi il
    /// existe plutôt qu'un appel direct. Il n'est posé que lorsque l'écriture
    /// a **réussi** (`act_on_timeout`, branche `a1octies`), et il est consommé
    /// par `run` (`transport/boucle.rs`) juste après. L'écriture étant
    /// synchrone et précédant la pose, l'ordre « le presse-papier Windows
    /// d'abord, la touche ensuite » est garanti **par construction** — aucun
    /// ordonnancement de canal n'y entre.
    ///
    /// Sur échec d'écriture, il n'est **pas** posé : la touche `V` est PERDUE,
    /// pas reportée (D6). Un `Ctrl+V` sur un presse-papier inchangé collerait
    /// le contenu PRÉCÉDENT, ce que D6 existe entièrement pour éviter.
    collage_a_injecter: bool,
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
    /// Client TURN, absent tant qu'aucun relais n'est configuré ou alloué.
    /// Son absence rend tout le chemin relayé inerte.
    pub(super) turn: Option<crate::turn::TurnClient>,
    /// Résolution du minuteur Windows abaissée à 1 ms pour la durée de vie de
    /// la session (voir `TimerResolutionGuard`). Champ jamais lu : sa seule
    /// raison d'être est de vivre aussi longtemps que `Session` et de
    /// restaurer la résolution d'origine à la destruction.
    _timer_resolution: TimerResolutionGuard,
    /// Identifiant de session, posé par `set_session_id` (voir
    /// `cadence_video.rs`) — vide tant qu'il ne l'a pas été (chemins de
    /// test). Ne sert qu'à apparier la ligne de cadence de la piste vidéo à
    /// celle du capteur (`capteur/fenetre.rs`) dans un `agent.log` que
    /// plusieurs fenêtres se partagent.
    session_id: String,
    /// Unités d'accès vidéo réellement écrites sur la piste depuis le
    /// dernier relevé de cadence (voir `cadence_video::PERIODE_COMPTEURS`).
    /// Incrémenté par `write_frame` (`piste_video.rs`), jamais par un tour de
    /// boucle qui ne produit rien.
    unites_video_ecrites: u64,
    /// Instant du dernier relevé de cadence de la piste vidéo.
    dernier_compte_video: Instant,
    /// Vrai une fois `VideoSource::signaler_audio_mort` appelée pour cette
    /// capture — le verrou qui empêche d'inonder le capteur : `capture_morte`
    /// (`crate::audio::AudioSource`) reste vrai à jamais une fois posé, alors
    /// que ce champ, lui, retombe à `false` à chaque rattachement du canal
    /// vers le capteur (`VideoSource::rattachement_survenu`, sous-bloc D9) —
    /// un capteur relancé a perdu la mémoire de tout signalement antérieur.
    audio_mort_signale: bool,
    /// De quoi refabriquer la source audio après la mort de sa capture
    /// (sous-bloc D10). Absent quand `AUDIO=0`, sous `TEST_FILE`, et quand
    /// l'ouverture audio initiale a échoué : le comportement d'avant D10 —
    /// signaler immédiatement — reste exactement conservé dans ces cas.
    ///
    /// ❌ **« Absent sur le chemin mono-fenêtre » figurait ici et c'est
    /// FAUX** : `demarrage/audio.rs::brancher` pose ce champ
    /// INCONDITIONNELLEMENT dans son bras `Ok`, branche `None` comprise. Le
    /// mono-fenêtre reconstruit donc bien — son défaut propre, la source
    /// reconstruite y étant réarmée à `false`, ✅ **est le leg n°4 de D10,
    /// CORRIGÉ en D11** (voir `reconstruire_ou_signaler`).
    /// ⚠️ **Troisième occurrence
    /// de cette même phrase, et celle-ci n'a été trouvée ni par la revue
    /// transverse ni par la revue finale de branche** : les deux ont corrigé
    /// les jumelles de `tick.rs` et de `tick/tests/audio.rs` sans balayer
    /// jusqu'ici. Le `grep -rn "mono-fenêtre" agent/src` la listait pourtant.
    audio_reconstructeur: Option<Reconstructeur>,
    /// Budget de tentatives de reconstruction restant, initialisé à
    /// `crate::audio::RECONSTRUCTIONS_MAX`. Épuisé, `reconstruire_ou_signaler`
    /// retombe sur le signalement — c'est là que la promotion d'une voisine
    /// par le capteur reprend son rôle.
    reconstructions_restantes: u32,
    /// Instant à partir duquel une nouvelle tentative de reconstruction est
    /// permise. `None` : aucune tentative n'a encore eu lieu, ou aucun répit
    /// n'est en cours.
    ///
    /// **Sans ce répit**, `reconstruire_ou_signaler` court sur le fil de
    /// `Session::run` et ouvrir une source WASAPI y est un appel bloquant de
    /// durée non bornée : sans répit, la boucle de tick tenterait une
    /// ouverture à chaque tour.
    prochaine_reconstruction: Option<Instant>,
    /// Vrai dès qu'une reconstruction a réussi, tant qu'aucun paquet n'est
    /// encore venu la confirmer. Distingue une DÉCISION (la reconstruction a
    /// rendu `Ok`) d'une PREUVE (un paquet a réellement été produit) — c'est
    /// toute la différence que `SourceVivante::sans_paquet` existe pour
    /// exercer (leg 6).
    audio_reconstruit_sans_preuve: bool,
    /// Vrai dès qu'un paquet RÉEL a confirmé — la PREUVE, pas la décision —
    /// que la capture audio reconstruite produit de nouveau du son : reste à
    /// annoncer `VersCapteur::AudioVivant` au capteur.
    ///
    /// Posé par `brancher_audio`, juste après qu'`audio_reconstruit_sans_preuve`
    /// retombe (voir ce champ) ; consommé — remis à `false` — par la branche
    /// a1sexies de `act_on_timeout`, qui appelle alors
    /// `VideoSource::signaler_audio_vivant`. C'est ce qui referme le leg 6 de
    /// D9 : `REARMEMENTS_MAX` (`capteur/sommeil.rs`) repart de zéro sur CE
    /// signal, jamais sur la seule décision de réélection.
    audio_vivant_a_annoncer: bool,
    /// Miroir LOCAL du dernier ordre audio reçu — ou, en mono-fenêtre, du mode
    /// lui-même. **DEUX écrivains** : `appliquer_audio` sur ordre du capteur, et
    /// `set_audio_porteuse` au branchement mono-fenêtre (`demarrage/audio.rs`,
    /// leg 4 de D10), où aucun capteur n'arbitrera jamais cette session.
    ///
    /// ❌ **« Sert UNIQUEMENT à détecter la TRANSITION vers `actif = true` » —
    /// écrit ici, et FAUX depuis D10 lui-même.** Le champ a **deux** lecteurs :
    /// la transition d'`appliquer_audio`, qui réapprovisionne
    /// `reconstructions_restantes` et lève `audio_mort_signale` ; et le
    /// réarmement `set_actif(self.audio_porteuse)` de
    /// `reconstruire_ou_signaler`, qui n'en est pas une. La doc précède ce
    /// second lecteur et n'a pas été relue quand il est arrivé.
    ///
    /// ⚠️ **Sans ce champ, le cycle mort → reconstruit → prouvé ne tournerait
    /// qu'UNE FOIS** (revue de la tâche 12, D10) : `reconstructions_restantes`
    /// n'était jamais rechargé, et `audio_mort_signale`, jamais levé, fermait
    /// définitivement la porte de `reconstruire_ou_signaler`.
    audio_porteuse: bool,
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
        // Socket UDP et `Rtc` str0m dans leur état initial : code
        // auto-contenu, sans accès aux champs de `Session`, extrait vers
        // `initialisation.rs` (revue de la tâche 12, sous-bloc D10).
        let (socket, rtc) = initialisation::construire_rtc(local_ip, plafond_bps)?;

        let dimensions = source.dimensions();
        let mut session = Self {
            rtc,
            socket,
            source,
            dimensions,
            clock_origin,
            video_mid: None,
            control_channel: None,
            input_channel: None,
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
            mic_mid: None,
            puits_micro: None,
            warned_micro_negotiation: false,
            refus_micro_signale: false,
            journaux_micro: 0,
            audio_write_pending_drain: false,
            warned_audio_negotiation: false,
            pending_resize: None,
            pending_visibility: None,
            pending_clipboard: None,
            collage_a_injecter: false,
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
            turn: None,
            _timer_resolution: TimerResolutionGuard::new(),
            session_id: String::new(),
            unites_video_ecrites: 0,
            dernier_compte_video: Instant::now(),
            audio_mort_signale: false,
            audio_reconstructeur: None,
            reconstructions_restantes: crate::audio::RECONSTRUCTIONS_MAX,
            prochaine_reconstruction: None,
            audio_reconstruit_sans_preuve: false,
            audio_vivant_a_annoncer: false,
            audio_porteuse: false,
        };

        // `add_local_candidate` est une mutation : on draine avant de rendre
        // la main, pour ne jamais dépendre de ce que l'appelant fera après
        // `new()`.
        session.drain_quietly()?;

        Ok(session)
    }

}
