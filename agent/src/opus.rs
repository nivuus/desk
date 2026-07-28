//! Encodeur Opus pour la piste audio.
//!
//! Media Foundation n'expose aucun encodeur Opus, et aucun codec que Chrome
//! accepte en WebRTC n'est disponible nativement sous Windows : on passe donc
//! par libopus, dont la source C est vendorée dans `audiopus_sys` et bâtie par
//! cmake à la compilation (voir `scripts/build-agent.sh`).
//!
//! Ce module ne référence jamais le crate `windows` : il se compile et se teste
//! sous Linux.

use anyhow::{bail, Context, Result};
use ::opus::{Application, Bitrate, Channels, Encoder};

/// Fréquence d'échantillonnage de la piste audio, en hertz. C'est aussi la
/// fréquence d'horloge RTP du type de charge utile Opus.
pub const SAMPLE_RATE_HZ: u32 = 48_000;

/// Nombre de canaux transmis.
pub const CHANNELS: usize = 2;

/// Durée d'une trame, en millisecondes.
pub const FRAME_MS: u64 = 10;

/// Échantillons par canal dans une trame.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as u64 * FRAME_MS / 1000) as usize;

/// Échantillons entrelacés dans une trame — la taille exacte que `encode`
/// exige.
pub const FRAME_INTERLEAVED: usize = FRAME_SAMPLES * CHANNELS;

/// Débit cible, en bits par seconde.
const BITRATE_BPS: i32 = 128_000;

/// Borne haute d'un paquet encodé. Une trame de 10 ms à 128 kbps fait ~160
/// octets ; 4 000 laisse toute la marge nécessaire sans jamais tronquer.
const MAX_PACKET_BYTES: usize = 4_000;

/// Encodeur Opus configuré une fois pour toutes.
///
/// **`Application::LowDelay`** (`OPUS_APPLICATION_RESTRICTED_LOWDELAY`) plutôt
/// que `Audio` ou `Voip` : mesuré sur cette configuration, la latence
/// algorithmique tombe à 120 échantillons (2,5 ms) contre 312 (6,5 ms) pour
/// les deux autres modes, **sans rien coûter sur le silence** — DTX y converge
/// vers 1 octet par trame exactement comme ailleurs.
pub struct OpusEncoder {
    inner: Encoder,
}

impl OpusEncoder {
    pub fn new() -> Result<Self> {
        let mut inner = Encoder::new(SAMPLE_RATE_HZ, Channels::Stereo, Application::LowDelay)
            .context("création de l'encodeur Opus")?;
        inner
            .set_bitrate(Bitrate::Bits(BITRATE_BPS))
            .context("réglage du débit Opus")?;
        // FEC in-band : le décodeur peut reconstruire une trame perdue à
        // partir de la suivante. Sur un lien quelconque, c'est ce qui évite
        // les micro-coupures audibles.
        inner.set_inband_fec(true).context("activation du FEC in-band")?;
        // DTX : le silence numérique retombe à 1 octet par trame en régime
        // établi (mesuré). Sans lui, il coûterait 3 octets — l'encodage à
        // débit variable dépense déjà peu. Le gain est modeste, le coût nul.
        inner.set_dtx(true).context("activation du DTX")?;
        Ok(Self { inner })
    }

    /// Encode exactement une trame de 10 ms.
    ///
    /// `pcm` doit contenir `FRAME_INTERLEAVED` échantillons entrelacés
    /// (gauche, droite, gauche, ...). libopus refuse toute autre taille avec
    /// `BadArg` ; on refuse ici plus tôt, avec un message qui nomme la taille
    /// attendue plutôt que de laisser remonter une erreur opaque.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        if pcm.len() != FRAME_INTERLEAVED {
            bail!(
                "trame de {} échantillons entrelacés, {FRAME_INTERLEAVED} attendus",
                pcm.len()
            );
        }
        self.inner
            .encode_vec(pcm, MAX_PACKET_BYTES)
            .context("encodage Opus")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Énergie du signal à `freq` hertz sur le canal gauche, par l'algorithme
    /// de Goertzel.
    ///
    /// Insensible au retard : le codec introduit une latence algorithmique
    /// (120 échantillons mesurés en `LowDelay`), donc une comparaison
    /// échantillon par échantillon avec l'entrée échouerait pour une raison
    /// qui n'a rien à voir avec la fidélité.
    fn energie_a(pcm: &[i16], freq: f64) -> f64 {
        let n = pcm.len() / CHANNELS;
        let w = 2.0 * std::f64::consts::PI * freq / SAMPLE_RATE_HZ as f64;
        let coeff = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for i in 0..n {
            let s0 = pcm[i * CHANNELS] as f64 + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt() / n as f64
    }

    /// `n` échantillons entrelacés stéréo d'une sinusoïde à `freq` hertz,
    /// démarrant à l'échantillon `depuis` pour rester continue d'une trame à
    /// la suivante.
    fn ton(freq: f64, depuis: usize, n: usize) -> Vec<i16> {
        (0..n)
            .flat_map(|i| {
                let phase = (depuis + i) as f64 * 2.0 * std::f64::consts::PI * freq
                    / SAMPLE_RATE_HZ as f64;
                let v = (phase.sin() * 12_000.0) as i16;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn une_trame_de_10_ms_vaut_480_echantillons_par_canal() {
        assert_eq!(FRAME_SAMPLES, 480);
        assert_eq!(FRAME_INTERLEAVED, 960);
    }

    #[test]
    fn refuse_une_trame_de_mauvaise_taille() {
        let mut encodeur = OpusEncoder::new().unwrap();
        let err = encodeur.encode(&vec![0i16; 1000]).unwrap_err();
        assert!(
            err.to_string().contains("960"),
            "le message doit nommer la taille attendue, obtenu : {err}"
        );
    }

    #[test]
    fn un_ton_encode_puis_decode_reste_le_meme_ton() {
        // Vérifier que l'encodeur rend des octets ne prouverait rien : du
        // bruit en rendrait tout autant. On décode en retour et on vérifie
        // que l'énergie reste concentrée sur la fréquence d'origine.
        let mut encodeur = OpusEncoder::new().unwrap();
        let mut decodeur =
            opus::Decoder::new(SAMPLE_RATE_HZ, opus::Channels::Stereo).unwrap();

        let mut sortie: Vec<i16> = Vec::new();
        for t in 0..20 {
            let paquet = encodeur
                .encode(&ton(440.0, t * FRAME_SAMPLES, FRAME_SAMPLES))
                .unwrap();
            let mut trame = vec![0i16; FRAME_INTERLEAVED];
            decodeur.decode(&paquet, &mut trame, false).unwrap();
            sortie.extend_from_slice(&trame);
        }

        let a_440 = energie_a(&sortie, 440.0);
        let a_1500 = energie_a(&sortie, 1500.0);
        assert!(
            a_440 > 100.0 * a_1500,
            "l'énergie doit rester concentrée sur 440 Hz : 440 Hz = {a_440:.1}, 1500 Hz = {a_1500:.1}"
        );

        let entree = energie_a(&ton(440.0, 0, 20 * FRAME_SAMPLES), 440.0);
        let rapport = a_440 / entree;
        assert!(
            (0.8..=1.2).contains(&rapport),
            "l'amplitude restituée doit rester proche de l'originale, rapport = {rapport:.3}"
        );
    }

    #[test]
    fn le_silence_prolonge_retombe_a_quelques_octets_par_trame() {
        // DTX met plusieurs trames à converger : les cinq premières valent
        // encore 217 puis 161 octets. Mesurer trop tôt conclurait à tort que
        // DTX ne fonctionne pas. On regarde donc la QUEUE, pas le début.
        let mut encodeur = OpusEncoder::new().unwrap();
        let silence = vec![0i16; FRAME_INTERLEAVED];
        let tailles: Vec<usize> = (0..40)
            .map(|_| encodeur.encode(&silence).unwrap().len())
            .collect();

        let queue = &tailles[35..];
        assert!(
            queue.iter().all(|&t| t <= 8),
            "en régime établi, une trame de silence doit tenir en quelques octets, obtenu : {queue:?}"
        );
    }
}
