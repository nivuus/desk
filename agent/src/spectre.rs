//! Dominant frequency of a block of samples — **pure, exercised on the host**.
//!
//! ## Pourquoi ce module existe
//!
//! This repository established, in sub-block D7, that **the right instrument to judge a
//! sound is its dominant frequency, never a byte count**: a track
//! whose `bytesReceived` grows can carry a spectrum at −1000 dB. The
//! `AUDIO_PROBE` probe (`diagnostics/audio.rs`) only returned a **peak**, which
//! distinguishes "some sound" from "nothing" but never "MY sound" from "another
//! sound". It is exactly the distinction the "A-bis" fix
//! needs: it must show that a NAMED device carries the tone
//! played on it, while another does not.
//!
//! ## Goertzel, not an FFT
//!
//! We are not looking for a full spectrum: we are looking for **the** dominant line
//! of a pure tone, on a frequency grid known in advance. The
//! Goertzel filter evaluates a single frequency in O(n) without allocating, and
//! sweeping a few dozen points costs less than an FFT — and above all
//! brings in no dependency. The survey's resolution is the grid's,
//! and it is **returned with the result** rather than assumed.

/// Squared magnitude of a given frequency, through the Goertzel filter.
///
/// `echantillons` is MONO (see `mono` below). Returns 0 on an empty block,
/// which spares the caller a special case: a zero magnitude can
/// never become a strict maximum.
pub fn magnitude(echantillons: &[f32], frequence_hz: f32, taux_hz: f32) -> f32 {
    if echantillons.is_empty() || taux_hz <= 0.0 {
        return 0.0;
    }
    let omega = 2.0 * std::f32::consts::PI * frequence_hz / taux_hz;
    let coefficient = 2.0 * omega.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &x in echantillons {
        let s0 = x + coefficient * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    // |X(k)|² without the phase term, enough to compare lines with each
    // other — it is an ARGMAX we are looking for, not a physical value.
    s1 * s1 + s2 * s2 - coefficient * s1 * s2
}

/// Reduces an interleaved stereo block (what `LoopbackCapture::read` returns) to
/// floating mono. The average of the channels, and not a single one: a tone played
/// on a single channel must not disappear.
pub fn mono(entrelace: &[i16], canaux: usize) -> Vec<f32> {
    if canaux == 0 {
        return Vec::new();
    }
    entrelace
        .chunks_exact(canaux)
        .map(|trame| {
            let somme: f32 = trame.iter().map(|&v| v as f32).sum();
            somme / canaux as f32 / i16::MAX as f32
        })
        .collect()
}

/// What a dominant frequency survey returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dominante {
    /// The grid frequency whose magnitude is maximal.
    pub frequence_hz: f32,
    /// Its magnitude, so that the caller can judge the ratio to the rest.
    pub magnitude: f32,
    /// The grid step: the survey's resolution, **returned rather than
    /// assumed**. Without it, "441 Hz" does not say whether to read 440 or 441.
    pub resolution_hz: f32,
}

/// Looks for the dominant frequency between `min_hz` and `max_hz`, on a grid
/// of step `pas_hz`.
///
/// Returns `None` when the block is empty, the grid degenerate, or **the whole**
/// band is at zero magnitude — a silence has no dominant, and
/// inventing one would be exactly the kind of verdict that cannot fail.
pub fn dominante(
    mono: &[f32],
    taux_hz: f32,
    min_hz: f32,
    max_hz: f32,
    pas_hz: f32,
) -> Option<Dominante> {
    if mono.is_empty() || pas_hz <= 0.0 || min_hz > max_hz || taux_hz <= 0.0 {
        return None;
    }
    let mut meilleure: Option<Dominante> = None;
    let mut frequence = min_hz;
    while frequence <= max_hz {
        let m = magnitude(mono, frequence, taux_hz);
        if m > meilleure.map_or(0.0, |d: Dominante| d.magnitude) {
            meilleure = Some(Dominante {
                frequence_hz: frequence,
                magnitude: m,
                resolution_hz: pas_hz,
            });
        }
        frequence += pas_hz;
    }
    meilleure
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an interleaved stereo sine wave of `frequence` Hz.
    fn sinus(frequence: f32, taux: f32, echantillons: usize, amplitude: f32) -> Vec<i16> {
        (0..echantillons)
            .flat_map(|n| {
                let v = (2.0 * std::f32::consts::PI * frequence * n as f32 / taux).sin();
                let e = (v * amplitude * i16::MAX as f32) as i16;
                [e, e]
            })
            .collect()
    }

    #[test]
    fn une_tonalite_pure_est_retrouvee_a_la_resolution_de_la_grille() {
        let bloc = sinus(440.0, 48_000.0, 24_000, 0.5);
        let d = dominante(&mono(&bloc, 2), 48_000.0, 100.0, 2_000.0, 1.0)
            .expect("une tonalité pure a une dominante");
        assert!(
            (d.frequence_hz - 440.0).abs() <= d.resolution_hz,
            "dominante relevée {} Hz, attendue 440 Hz à ±{} Hz",
            d.frequence_hz,
            d.resolution_hz
        );
    }

    /// **The test that makes the instrument discriminating**: two different
    /// tones must not return the same verdict. Without it, an
    /// implementation always returning `min_hz` would pass the test
    /// above if the bound were 440.
    #[test]
    fn deux_tonalites_differentes_rendent_deux_dominantes_differentes() {
        let grave = dominante(
            &mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2),
            48_000.0,
            100.0,
            2_000.0,
            1.0,
        )
        .unwrap();
        let aigu = dominante(
            &mono(&sinus(1_000.0, 48_000.0, 24_000, 0.5), 2),
            48_000.0,
            100.0,
            2_000.0,
            1.0,
        )
        .unwrap();
        assert!((grave.frequence_hz - 440.0).abs() <= 1.0);
        assert!((aigu.frequence_hz - 1_000.0).abs() <= 1.0);
        assert_ne!(grave.frequence_hz, aigu.frequence_hz);
    }

    /// A silence has no dominant. It is THE case the "A-bis"
    /// fix must be able to distinguish: a device not being played.
    #[test]
    fn a_silence_has_no_dominant() {
        assert_eq!(
            dominante(&vec![0.0; 4_800], 48_000.0, 100.0, 2_000.0, 1.0),
            None
        );
        assert_eq!(dominante(&[], 48_000.0, 100.0, 2_000.0, 1.0), None);
    }

    /// The magnitude of the carrier line must CLEARLY dominate a neighbouring
    /// line: without a gap, "dominant" would mean nothing.
    #[test]
    fn la_raie_porteuse_domine_nettement_ses_voisines() {
        let m = mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2);
        let porteuse = magnitude(&m, 440.0, 48_000.0);
        let voisine = magnitude(&m, 700.0, 48_000.0);
        assert!(
            porteuse > voisine * 100.0,
            "porteuse {porteuse} contre voisine {voisine} : rapport insuffisant"
        );
    }

    #[test]
    fn le_mono_moyenne_les_canaux_et_ignore_une_trame_incomplete() {
        // Three values for two channels: the third is incomplete.
        let m = mono(&[i16::MAX, 0, 1234], 2);
        assert_eq!(m.len(), 1);
        assert!((m[0] - 0.5).abs() < 1e-3, "moyenne obtenue {}", m[0]);
        assert!(mono(&[1, 2, 3], 0).is_empty());
    }

    /// A tone present on a SINGLE channel must not be lost by the
    /// fold into mono — it is only half as strong.
    #[test]
    fn une_tonalite_sur_un_seul_canal_survit_au_repliement() {
        let bloc: Vec<i16> = (0..24_000)
            .flat_map(|n| {
                let v = (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48_000.0).sin();
                [(v * 0.5 * i16::MAX as f32) as i16, 0]
            })
            .collect();
        let d = dominante(&mono(&bloc, 2), 48_000.0, 100.0, 2_000.0, 1.0).unwrap();
        assert!((d.frequence_hz - 440.0).abs() <= d.resolution_hz);
    }

    /// **Pins the FORMULA, not only the argmax.** A mutation that
    /// cut Goertzel's cross term (`- coefficient·s1·s2`) stayed
    /// GREEN on all other tests: the argmax survives a
    /// wrong magnitude, because it only depends on the order of the lines. The
    /// magnitude being PUBLISHED in `Dominante`, a caller that
    /// compared it to a threshold would read a wrong number — hence this test.
    ///
    /// Analytical value: for a real sine wave of amplitude `A`
    /// exactly on a line, over `N` samples, Goertzel returns
    /// `|X(k)|² = (A·N/2)²`. Here `A = 0.5`, `N = 24,000` — i.e. 220 whole
    /// periods of 440 Hz at 48 kHz, hence no spectral leakage.
    #[test]
    fn the_magnitude_follows_the_goertzel_analytic_value() {
        let m = mono(&sinus(440.0, 48_000.0, 24_000, 0.5), 2);
        let mesuree = magnitude(&m, 440.0, 48_000.0);
        let attendue = (0.5 * 24_000.0 / 2.0f32).powi(2);
        let ecart = (mesuree - attendue).abs() / attendue;
        assert!(
            ecart < 0.02,
            "magnitude mesurée {mesuree}, analytique {attendue}, écart relatif {ecart}"
        );
    }

    #[test]
    fn une_grille_degeneree_ne_rend_rien_plutot_qu_un_verdict_invente() {
        let m = mono(&sinus(440.0, 48_000.0, 4_800, 0.5), 2);
        assert_eq!(dominante(&m, 48_000.0, 100.0, 2_000.0, 0.0), None);
        assert_eq!(dominante(&m, 48_000.0, 2_000.0, 100.0, 1.0), None);
        assert_eq!(dominante(&m, 0.0, 100.0, 2_000.0, 1.0), None);
    }
}
