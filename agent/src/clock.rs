//! Correspondance entre horodatages de présentation et instants réels.
//!
//! Les deux médias ont des horloges de fréquences différentes — 90 kHz pour la
//! vidéo, 48 kHz pour l'audio — mais **une seule origine**, créée dans `main.rs`
//! et passée aux deux sources. C'est cette origine commune qui rend les RTCP
//! Sender Reports cohérents entre les deux pistes, et donc la synchro A/V
//! exacte par construction plutôt que par chance.

use std::time::{Duration, Instant};

/// Instant réel correspondant à `pts`, exprimé dans une horloge de `rate_hz`
/// graduations par seconde, relativement à `origin`.
///
/// Le calcul passe par 128 bits : `pts * 1_000_000_000` déborderait un `u64`
/// au bout d'environ 57 heures de session à 90 kHz. Même précaution que
/// `WindowsSource::next_pts_90k`.
///
/// Une fréquence nulle rend `origin` : elle n'a pas de sens, mais faire
/// paniquer la boucle de transport pour cela en aurait encore moins.
pub fn instant_from_pts(origin: Instant, pts: u64, rate_hz: u32) -> Instant {
    if rate_hz == 0 {
        return origin;
    }
    let nanos = pts as u128 * 1_000_000_000 / rate_hz as u128;
    origin + Duration::from_nanos(nanos as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn un_horodatage_nul_vaut_l_origine() {
        let origine = Instant::now();
        assert_eq!(instant_from_pts(origine, 0, 90_000), origine);
    }

    #[test]
    fn convertit_depuis_l_horloge_video_a_90_khz() {
        let origine = Instant::now();
        // 90 000 graduations = une seconde.
        assert_eq!(
            instant_from_pts(origine, 90_000, 90_000),
            origine + Duration::from_secs(1)
        );
        // 1 500 graduations = une image à 60 i/s.
        assert_eq!(
            instant_from_pts(origine, 1_500, 90_000),
            origine + Duration::from_nanos(16_666_666)
        );
    }

    #[test]
    fn convertit_depuis_l_horloge_audio_a_48_khz() {
        let origine = Instant::now();
        assert_eq!(
            instant_from_pts(origine, 48_000, 48_000),
            origine + Duration::from_secs(1)
        );
        // 480 échantillons = une trame de 10 ms.
        assert_eq!(
            instant_from_pts(origine, 480, 48_000),
            origine + Duration::from_millis(10)
        );
    }

    #[test]
    fn ne_deborde_pas_sur_une_session_tres_longue() {
        // `pts * 1_000_000_000` déborderait un u64 au bout d'environ 57
        // heures à 90 kHz. Le calcul se fait donc en 128 bits.
        let origine = Instant::now();
        let pts = 90_000u64 * 3600 * 100; // 100 heures
        let obtenu = instant_from_pts(origine, pts, 90_000);
        assert_eq!(obtenu, origine + Duration::from_secs(3600 * 100));
    }

    #[test]
    fn une_frequence_nulle_rend_l_origine_plutot_que_de_paniquer() {
        let origine = Instant::now();
        assert_eq!(instant_from_pts(origine, 1_000, 0), origine);
    }
}
