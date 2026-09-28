//! **PURE — no `cfg`.** The mix format that writing the microphone to
//! the cable requires, and the **named** refusal when it differs.
//!
//! ⚠️ **Why this rule lives HERE and not in `wasapi/ecriture.rs`.** Plan
//! E2 (task 7, step 1) asks for a rule "PURE and tested on the host",
//! and writes twenty lines further down that `wasapi/ecriture.rs` "is NOT tested
//! on the host" (`#![cfg(windows)]`). Both cannot be true of the
//! same file. The precedent that settles it is already in this directory:
//! `wasapi/peripherique.rs`, pure, hoisted to the crate root through `#[path]`
//! (`main.rs`) precisely to escape the `#![cfg(windows)]` of `wasapi.rs`.
//! This rule follows the same setup, under the name `wasapi_format`.
//!
//! ## What is accepted, and what is NOT
//!
//! `LecteurMicro::remplir` returns **normalised interleaved stereo `f32`** at
//! 48 kHz. The mix format noted on the VM on August 20th, 2026 for the cable's render
//! endpoint is `48000 Hz, 2 channels, 32-bit
//! float`: **no conversion is therefore needed on this machine**,
//! and we write none. Writing a converter no machine
//! exercises would be work laid down before having observed the need — which
//! this repository refuses, and which no test would keep honest.
//!
//! The price of this choice is that another format must be **refused
//! explicitly**, never endured: writing stereo `f32`s into a buffer that
//! expects something else does not produce degraded sound, it produces full-level
//! noise in someone's ear. The refusal names the value
//! encountered, on the pattern of the `bail!` of `LoopbackCapture::open`
//! (`agent/src/wasapi.rs`, frequency check).

use anyhow::{bail, Result};

/// The number of channels `LecteurMicro::remplir` produces, and the only one
/// the write knows how to lay down as is.
pub const CANAUX: usize = crate::opus::CHANNELS;

/// The expected sample width, in bits.
pub const BITS: u16 = 32;

/// Describes a mix format as `LoopbackCapture::open` already does, so
/// that both halves of the sound read the same way in `agent.log`.
pub fn decrire(frequence: u32, canaux: usize, bits: u16, flottant: bool) -> String {
    format!(
        "{frequence} Hz, {canaux} canaux, {bits} bits, {}",
        if flottant { "flottant" } else { "entier" }
    )
}

/// Accepts the mix format, or says **precisely** what is wrong.
///
/// The four quantities are those `IAudioClient::GetMixFormat` returns, already
/// unfolded by the caller (the `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT` subformat
/// of a `WAVEFORMATEXTENSIBLE` is reduced to the `flottant` boolean): this
/// function knows no Windows type, that is what makes it testable here.
///
/// ⚠️ **No fallback, no conversion.** See the module header: the refusal
/// is the intended behaviour, and the message is the only thing that allows
/// fixing it.
pub fn verify(frequence: u32, canaux: usize, bits: u16, flottant: bool) -> Result<()> {
    let description = decrire(frequence, canaux, bits, flottant);
    let attendu = crate::opus::SAMPLE_RATE_HZ;
    if frequence != attendu {
        bail!(
            "format de mixage a {frequence} Hz : seul {attendu} Hz est supporte pour l'ecriture \
             du micro (aucun reechantillonneur n'est embarque, spec §4) [{description}]"
        );
    }
    if canaux != CANAUX {
        bail!(
            "format de mixage a {canaux} canaux : seul le stereo ({CANAUX} canaux) est supporte \
             pour l'ecriture du micro, `LecteurMicro::remplir` ne produisant que cela \
             [{description}]"
        );
    }
    if !flottant {
        bail!(
            "format de mixage entier {bits} bits non supporte pour l'ecriture du micro : \
             `LecteurMicro::remplir` rend des flottants normalises [{description}]"
        );
    }
    if bits != BITS {
        bail!(
            "format de mixage flottant {bits} bits non supporte pour l'ecriture du micro : \
             seul le {BITS} bits l'est [{description}]"
        );
    }
    Ok(())
}

/// How many **frames per channel**, in `pcm` (interleaved), are entirely
/// silent.
///
/// ⚠️ **Why this counter exists, and why it lives in THIS module.**
/// `LecteurMicro::remplir` never blocks and fills with silence (spec §8):
/// a trace that only said "48,000 frames were written in the second" would not
/// distinguish a microphone that speaks from a microphone that is silent, and both
/// are written exactly the same on the cable. It is the only place on the path
/// that can measure it, and it is a measurement of the buffer ACTUALLY laid down, not a
/// report from `remplir` — which makes none.
///
/// As for its place: `wasapi/ecriture.rs`, its only caller, is
/// `#![cfg(windows)]` and is not tested on the host; `micro.rs` is at 483
/// lines (margin 17, the narrowest on the path) and this repository extracts before
/// adding rather than compressing afterwards. This module is pure, hoisted, already
/// tested, and it already talks about what the write lays down on the cable.
pub fn trames_de_silence(pcm: &[f32], canaux: usize) -> usize {
    if canaux == 0 {
        return 0;
    }
    pcm.chunks_exact(canaux)
        .filter(|trame| trame.iter().all(|e| *e == 0.0))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opus::SAMPLE_RATE_HZ;

    /// The format NOTED on the VM on August 20th, 2026 for
    /// "Haut-parleurs (VB-Audio Virtual Cable)".
    #[test]
    fn le_format_releve_sur_la_vm_est_accepte() {
        assert!(verify(48_000, 2, 32, true).is_ok());
        assert_eq!(SAMPLE_RATE_HZ, 48_000, "la constante du codec a changé");
    }

    /// 🔴 The case of CABLE Output, the other end: 44,100 Hz. The message must
    /// name the frequency encountered — without it, an acceptance run reads "format
    /// refused" and does not know what to fix.
    #[test]
    #[allow(non_snake_case)]
    fn une_frequence_autre_est_refusee_EN_LA_NOMMANT() {
        let e = verify(44_100, 2, 32, true).unwrap_err().to_string();
        assert!(
            e.contains("44100"),
            "la fréquence rencontrée doit être nommée : {e}"
        );
        assert!(
            e.contains("48000"),
            "la fréquence attendue doit être nommée : {e}"
        );
        // ⚠️ Mutation M4 ("the reason no longer names the frequency
        // encountered") first SURVIVED: `[{description}]` carried it
        // anyway, and the two `contains` above went through it. Noting
        // it here rather than hardening the assertion, because
        // the tested claim — "the message names the frequency encountered"
        // — is true both ways, and the operator reads it in both
        // cases. What would NOT be true is a message that carries it
        // nowhere: it is that state the two `contains` forbid,
        // and mutation M4-bis (neither name nor description) kills them.
    }

    /// An integer mix is a perfectly legal WASAPI format, and it is
    /// refused anyway: `remplir` returns `f32`s.
    ///
    /// ⚠️ **The width is 32 bits, and that is THE WHOLE POINT of the case.** A
    /// first draft exercised `(48 000, 2, 16, integer)` — refused by
    /// TWO independent guards, the float one and the 32-bit one — and
    /// mutation M3 ("integer passes") **survived**: the bits guard
    /// caught the case, and the word "integer" the test looked for came from
    /// the description, not from the reason. `(48 000, 2, 32, integer)` can only be
    /// refused by the float guard.
    #[test]
    fn un_format_entier_32_bits_est_refuse_par_la_seule_garde_du_flottant() {
        let e = verify(48_000, 2, 32, false).unwrap_err().to_string();
        assert!(
            e.contains("entier"),
            "le motif doit nommer le format entier : {e}"
        );
        assert!(
            !e.contains("flottant 32"),
            "ce n'est pas la garde des bits qui doit refuser ce cas : {e}"
        );
    }

    /// And 16-bit integer stays refused too.
    #[test]
    fn un_format_entier_16_bits_est_refuse() {
        assert!(verify(48_000, 2, 16, false).is_err());
    }

    #[test]
    fn un_flottant_qui_n_est_pas_32_bits_est_refuse() {
        assert!(verify(48_000, 2, 64, true).is_err());
    }

    /// 🔴 **Mono is REFUSED, not folded.** Laying an interleaved stereo buffer
    /// on a mono endpoint would play every other channel at double
    /// speed: not degraded sound, wrong sound.
    #[test]
    fn le_mono_est_refuse_plutot_que_converti() {
        let e = verify(48_000, 1, 32, true).unwrap_err().to_string();
        assert!(
            e.contains('1'),
            "le nombre de canaux rencontré doit être nommé : {e}"
        );
    }

    #[test]
    fn le_multicanal_est_refuse() {
        assert!(verify(48_000, 6, 32, true).is_err());
    }

    /// 🔴 The counter that makes the periodic trace READABLE. Without it, "48,000
    /// frames written" does not distinguish a microphone that speaks from a microphone
    /// that is silent — and `remplir` fills with silence without ever saying so.
    #[test]
    fn le_silence_pur_est_compte_trame_par_trame() {
        assert_eq!(trames_de_silence(&[0.0; 8], 2), 4);
    }

    #[test]
    fn a_full_signal_counts_no_silence_frame() {
        assert_eq!(trames_de_silence(&[0.5, -0.5, 0.25, -0.25], 2), 2 - 2);
    }

    /// ⚠️ **A frame is only silent if ALL its channels are.** A
    /// silent right channel on a speaking left channel is a mixing defect,
    /// not silence, and counting it as such would mask exactly
    /// that defect.
    #[test]
    fn une_trame_dont_un_seul_canal_parle_n_est_pas_du_silence() {
        assert_eq!(trames_de_silence(&[0.0, 0.3, 0.0, 0.0], 2), 1);
    }

    /// Negative zero is silence: `-0.0 == 0.0` in IEEE 754, and a
    /// comparison that distinguished them would make the counter wrong without
    /// any sound changing.
    #[test]
    fn le_zero_negatif_est_du_silence() {
        assert_eq!(trames_de_silence(&[-0.0, -0.0], 2), 1);
    }

    /// An incomplete tail — fewer samples than a whole frame — is
    /// ignored rather than counted as a frame. The case does not happen
    /// (the write always lays down whole frames), and it is precisely
    /// for that reason that it must be defined here rather than discovered elsewhere.
    #[test]
    fn an_incomplete_tail_counts_for_no_frame() {
        assert_eq!(trames_de_silence(&[0.0, 0.0, 0.0], 2), 1);
    }

    #[test]
    fn the_description_reads_like_the_loopback_one() {
        assert_eq!(
            decrire(48_000, 2, 32, true),
            "48000 Hz, 2 canaux, 32 bits, flottant"
        );
        assert_eq!(
            decrire(44_100, 2, 16, false),
            "44100 Hz, 2 canaux, 16 bits, entier"
        );
    }
}
