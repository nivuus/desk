//! Mapping between presentation timestamps and real instants.
//!
//! The two media have clocks of different frequencies — 90 kHz for
//! video, 48 kHz for audio — but **a single origin**, created in `demarrage.rs`
//! and passed to both sources. It is this common origin that makes the RTCP
//! Sender Reports consistent between the two tracks, and hence the A/V sync
//! exact by construction rather than by luck.

use std::time::{Duration, Instant};

/// Real instant corresponding to `pts`, expressed in a clock of `rate_hz`
/// ticks per second, relative to `origin`.
///
/// The computation goes through 128 bits: `pts * 1_000_000_000` would overflow a `u64`
/// after about 57 hours of session at 90 kHz. Same precaution as
/// `WindowsSource::next_pts_90k`.
///
/// A zero frequency returns `origin`: it makes no sense, but making
/// the transport loop panic over it would make even less.
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
        // 1,500 ticks = one frame at 60 fps.
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
        // 480 samples = one 10 ms frame.
        assert_eq!(
            instant_from_pts(origine, 480, 48_000),
            origine + Duration::from_millis(10)
        );
    }

    #[test]
    fn ne_deborde_pas_sur_une_session_tres_longue() {
        // `pts * 1_000_000_000` would overflow a u64 after about 57
        // hours at 90 kHz. The computation is therefore done in 128 bits.
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
