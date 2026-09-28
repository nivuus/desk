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
        "{frequence} Hz, {canaux} channels, {bits} bits, {}",
        if flottant { "float" } else { "integer" }
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
            "mix format at {frequence} Hz: only {attendu} Hz is supported for mic \
             writing (no resampler is embedded, spec §4) [{description}]"
        );
    }
    if canaux != CANAUX {
        bail!(
            "mix format with {canaux} channels: only stereo ({CANAUX} channels) is supported \
             for mic writing, `LecteurMicro::remplir` producing only that \
             [{description}]"
        );
    }
    if !flottant {
        bail!(
            "integer {bits}-bit mix format not supported for mic writing: \
             `LecteurMicro::remplir` returns normalised floats [{description}]"
        );
    }
    if bits != BITS {
        bail!(
            "{bits}-bit float mix format not supported for mic writing: \
             only {BITS}-bit is [{description}]"
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
    fn the_format_read_on_the_vm_is_accepted() {
        assert!(verify(48_000, 2, 32, true).is_ok());
        assert_eq!(SAMPLE_RATE_HZ, 48_000, "the codec constant changed");
    }

    /// 🔴 The case of CABLE Output, the other end: 44,100 Hz. The message must
    /// name the frequency encountered — without it, an acceptance run reads "format
    /// refused" and does not know what to fix.
    #[test]
    #[allow(non_snake_case)]
    fn another_frequency_is_refused_BY_NAMING_IT() {
        let e = verify(44_100, 2, 32, true).unwrap_err().to_string();
        assert!(
            e.contains("44100"),
            "the encountered frequency must be named: {e}"
        );
        assert!(
            e.contains("48000"),
            "the expected frequency must be named: {e}"
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
    fn a_32_bit_integer_format_is_refused_by_the_float_guard_alone() {
        let e = verify(48_000, 2, 32, false).unwrap_err().to_string();
        assert!(
            e.contains("integer"),
            "the reason must name the integer format: {e}"
        );
        assert!(
            !e.contains("32-bit float"),
            "it is not the bits guard that must refuse this case: {e}"
        );
    }

    /// And 16-bit integer stays refused too.
    #[test]
    fn a_16_bit_integer_format_is_refused() {
        assert!(verify(48_000, 2, 16, false).is_err());
    }

    #[test]
    fn a_float_that_is_not_32_bits_is_refused() {
        assert!(verify(48_000, 2, 64, true).is_err());
    }

    /// 🔴 **Mono is REFUSED, not folded.** Laying an interleaved stereo buffer
    /// on a mono endpoint would play every other channel at double
    /// speed: not degraded sound, wrong sound.
    #[test]
    fn mono_is_refused_rather_than_converted() {
        let e = verify(48_000, 1, 32, true).unwrap_err().to_string();
        assert!(
            e.contains('1'),
            "the encountered channel count must be named: {e}"
        );
    }

    #[test]
    fn multichannel_is_refused() {
        assert!(verify(48_000, 6, 32, true).is_err());
    }

    /// 🔴 The counter that makes the periodic trace READABLE. Without it, "48,000
    /// frames written" does not distinguish a microphone that speaks from a microphone
    /// that is silent — and `remplir` fills with silence without ever saying so.
    #[test]
    fn pure_silence_is_counted_frame_by_frame() {
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
    fn a_frame_where_a_single_channel_speaks_is_not_silence() {
        assert_eq!(trames_de_silence(&[0.0, 0.3, 0.0, 0.0], 2), 1);
    }

    /// Negative zero is silence: `-0.0 == 0.0` in IEEE 754, and a
    /// comparison that distinguished them would make the counter wrong without
    /// any sound changing.
    #[test]
    fn negative_zero_is_silence() {
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
            "48000 Hz, 2 channels, 32 bits, float"
        );
        assert_eq!(
            decrire(44_100, 2, 16, false),
            "44100 Hz, 2 channels, 16 bits, integer"
        );
    }
}
