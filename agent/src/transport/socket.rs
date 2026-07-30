//! Le socket et l'attente : classification des erreurs de réception, recul
//! exponentiel, résolution du timer Windows, et calcul de l'attente bornée
//! entre deux tours de boucle.

use std::time::{Duration, Instant};

/// Tranche maximale d'une attente sans donnée sur le socket, dans la boucle
/// de sondage non bloquant d'`act_on_timeout` (branche c).
///
/// Remplace `UdpSocket::set_read_timeout`, dont le délai déborde massivement
/// sous Windows (mesure indépendante : dépassement moyen +12,7 ms, jusqu'à
/// +37 ms ; un délai demandé de 617 µs a été honoré après 31 758 µs — cinq
/// fois le budget d'une image entière à 60 Hz). `recv_from` consommait ainsi
/// jusqu'à ~90 % du temps de boucle disponible à chaque tour, du temps qui
/// aurait dû revenir à la capture et à l'encodage.
///
/// **Chiffre de comparaison retiré (28/07) :** cette mesure citait à l'origine
/// un plafond capture/encodage isolé de 47-51 im/s contre un débit de bout en
/// bout observé de 23-25 im/s. La recette du jalon 1
/// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, « Ce qui a été
/// appris ») établit que ce chiffre de 47-51 im/s provenait d'un harnais
/// isolé (`ENCODER_THROUGHPUT_TEST`), qui ne passe pas par `Session::run` et
/// n'est donc pas comparable à une mesure de bout en bout — et qu'il était de
/// toute façon périmé : la capture, remesurée depuis en isolation
/// (`CAPTURE_TEST`), soutient ~90 im/s sur la même VM. Le plafond de débit
/// réellement établi par la recette se situe côté encodeur matériel
/// (`METransformNeedInput` n'est accepté qu'à ~30 Hz, voir
/// `windows_source.rs` et `encode.rs`), sans lien démontré avec l'imprécision
/// de `recv_from` documentée ci-dessus, qui reste une mesure valide en soi.
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
pub(super) const RECV_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Issue de la classification d'une erreur de réception UDP (voir
/// `classify_recv_error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecvErrorAction {
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
pub(super) fn classify_recv_error(kind: std::io::ErrorKind) -> RecvErrorAction {
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

pub(super) fn recv_error_backoff(consecutive_errors: u32) -> Duration {
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
pub(super) struct TimerResolutionGuard;

#[cfg(windows)]
impl TimerResolutionGuard {
    pub(super) fn new() -> Self {
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
pub(super) struct TimerResolutionGuard;

#[cfg(not(windows))]
impl TimerResolutionGuard {
    pub(super) fn new() -> Self {
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
pub(super) fn bounded_wait(
    now: Instant,
    rtc_deadline: Instant,
    next_frame_at: Option<Instant>,
    cap: Option<Duration>,
) -> Duration {
    let mut wait = rtc_deadline.saturating_duration_since(now);
    if let Some(next_frame_at) = next_frame_at {
        wait = wait.min(next_frame_at.saturating_duration_since(now));
    }
    // Plafond audio : les paquets arrivent d'un AUTRE fil, sans échéance que
    // cette boucle puisse prévoir. Seul un réveil régulier permet de les
    // relever à temps. Un plafond ne fait que RACCOURCIR l'attente, jamais
    // l'allonger.
    if let Some(cap) = cap {
        wait = wait.min(cap);
    }
    wait
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn attente_bornee_par_l_echeance_d_image_la_plus_proche() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_secs(1);
        let next_frame_at = now + Duration::from_micros(5_000);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at), None);
        assert_eq!(wait, Duration::from_micros(5_000));
    }

    #[test]
    fn attente_bornee_par_l_echeance_rtc_si_plus_proche() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_micros(2_000);
        let next_frame_at = now + Duration::from_secs(1);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at), None);
        assert_eq!(wait, Duration::from_micros(2_000));
    }

    #[test]
    fn attente_dictee_par_rtc_seul_sans_piste_video() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_millis(10);
        assert_eq!(bounded_wait(now, rtc_deadline, None, None), Duration::from_millis(10));
    }
}
