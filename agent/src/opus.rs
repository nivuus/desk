//! Opus codec of both audio tracks: the ENCODER of the downstream track
//! (work stream A, agent → browser) and the DECODER of the upstream track
//! (work stream E, browser → agent).
//!
//! Media Foundation exposes no Opus encoder, and no codec Chrome
//! accepts in WebRTC is available natively under Windows: we therefore go
//! through libopus, whose C source is vendored in `audiopus_sys` and built by
//! cmake at compile time (see `scripts/build-agent.sh`).
//!
//! This module never references the `windows` crate: it compiles and is tested
//! under Linux.

use anyhow::{bail, Context, Result};
// The leading `::` forces resolution of the external `opus` crate. This module is itself
// called `opus` (edition 2021), so without the `::`, a `use opus::...` would designate
// the current module, not the external crate — hence compilation would fail. The leading `::`
// forces the path through the crate root, hence the external crate.
use ::opus::{Application, Bitrate, Channels, Decoder, Encoder};

/// Sampling frequency of the audio track, in hertz. It is also the
/// RTP clock frequency of the Opus payload type.
pub const SAMPLE_RATE_HZ: u32 = 48_000;

/// Number of channels transmitted.
pub const CHANNELS: usize = 2;

/// Duration of a frame, in milliseconds.
pub const FRAME_MS: u64 = 10;

/// Samples per channel in a frame.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as u64 * FRAME_MS / 1000) as usize;

/// Interleaved samples in a frame — the exact size `encode`
/// requires.
pub const FRAME_INTERLEAVED: usize = FRAME_SAMPLES * CHANNELS;

/// Target bitrate, in bits per second.
///
/// `pub` since the final branch review (I5): `transport.rs` references it
/// for the audio budget of the congestion controller (`congestion::Config::
/// audio_bps`), rather than hardcoding a duplicate `128_000` unlinked to this
/// constant.
pub const BITRATE_BPS: i32 = 128_000;

/// Upper bound of an encoded packet. A 10 ms frame at 128 kbps is ~160
/// bytes; 4,000 leaves all the needed margin without ever truncating.
const MAX_PACKET_BYTES: usize = 4_000;

/// Opus encoder configured once and for all.
///
/// **`Application::Audio`** is chosen to make in-band FEC possible.
/// `OPUS_APPLICATION_RESTRICTED_LOWDELAY` forces `MODE_CELT_ONLY`
/// (`opus/src/opus_encoder.c:1349`), in which `decide_fec` returns zero
/// (`opus/src/opus_encoder.c:721`) — LBRR redundancy only exists in SILK.
/// The cost is a pre-delay that goes from 120 samples (2.5 ms) in
/// `RESTRICTED_LOWDELAY` to 312 (6.5 ms) in `Audio`, a measurement established in
/// work stream A. These extra 4 ms compare with the ~48 ms of median video
/// latency of the pipeline — audio stays well ahead of the image.
/// The trade-off was settled in favour of network resilience.
pub struct OpusEncoder {
    inner: Encoder,
}

impl OpusEncoder {
    pub fn new() -> Result<Self> {
        let mut inner = Encoder::new(SAMPLE_RATE_HZ, Channels::Stereo, Application::Audio)
            .context("creating the Opus encoder")?;
        inner
            .set_bitrate(Bitrate::Bits(BITRATE_BPS))
            .context("setting the Opus bitrate")?;
        // In-band FEC: the decoder can reconstruct a lost frame from
        // the next one. On an arbitrary link, it is what avoids
        // audible micro-cuts.
        inner
            .set_inband_fec(true)
            .context("activation du FEC in-band")?;
        // DTX: digital silence falls to 1 byte per frame in steady
        // state (measured). Without it, it would cost 3 bytes — variable bitrate
        // encoding already spends little. The gain is modest, the cost nil.
        inner.set_dtx(true).context("enabling DTX")?;
        Ok(Self { inner })
    }

    /// Declares to the encoder the loss rate observed on the link, in
    /// percent.
    ///
    /// **It is this setting that makes in-band FEC effective.** `set_inband_fec`
    /// alone only allows LBRR redundancy; libopus only emits it if
    /// a non-zero loss is declared. Without this call, the FEC enabled at
    /// construction produces nothing.
    ///
    /// The value is clamped to [0, 100]: libopus refuses the rest with an
    /// opaque error, and the caller does not have to know this bound.
    pub fn set_packet_loss_perc(&mut self, perc: i32) -> Result<()> {
        self.inner
            .set_packet_loss_perc(perc.clamp(0, 100))
            .context("setting the loss rate declared to Opus")
    }

    /// Encodes exactly one 10 ms frame.
    ///
    /// `pcm` must contain `FRAME_INTERLEAVED` interleaved samples
    /// (left, right, left, ...). libopus refuses any other size with
    /// `BadArg`; we refuse here earlier, with a message that names the expected
    /// size rather than letting an opaque error go up.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        if pcm.len() != FRAME_INTERLEAVED {
            bail!(
                "frame of {} interleaved samples, {FRAME_INTERLEAVED} expected",
                pcm.len()
            );
        }
        self.inner
            .encode_vec(pcm, MAX_PACKET_BYTES)
            .context("Opus encoding")
    }
}

/// Samples PER CHANNEL an Opus packet carries, read from its header (TOC).
///
/// FREE function, and it is deliberate: the transport loop needs this
/// reading to build a `TrameMicro`, and it has no reason to
/// own a decoder for that — the decoder lives in `LecteurMicro`, on the
/// thread that decodes. `OpusDecoder::echantillons_de` delegates to it.
///
/// **Never assumed** (spec §7): Chrome emits 20 ms, work stream A
/// 10 ms, and nothing forces a peer to stick to that.
pub fn echantillons_de(paquet: &[u8]) -> Result<usize> {
    ::opus::packet::get_nb_samples(paquet, SAMPLE_RATE_HZ)
        .context("reading the duration of an Opus packet")
}

/// Opus decoder of the upstream track (work stream E).
///
/// **Always STEREO**, whatever the number of channels the peer
/// actually encoded: Chrome encodes the microphone in mono, and libopus then duplicates
/// the single channel onto both outputs. Spec §7 described this
/// conversion as work to be written; it is done by the library,
/// and `a_mono_stream_comes_out_stereo_by_duplication` CHECKS it rather than
/// assuming it.
///
/// **No frame duration is assumed** (spec §7). Chrome emits 20 ms,
/// work stream A 10 ms, and nothing forces a peer to stick to that: the duration is
/// READ from the packet (`echantillons_de`) before any allocation, and that of the PLC
/// is read from the last decoded frame (`derniere_duree`).
pub struct OpusDecoder {
    inner: Decoder,
}

impl OpusDecoder {
    pub fn new() -> Result<Self> {
        let inner =
            Decoder::new(SAMPLE_RATE_HZ, Channels::Stereo).context("creating the Opus decoder")?;
        Ok(Self { inner })
    }

    /// Samples PER CHANNEL this packet carries, read from its header (TOC).
    ///
    /// **Never assumed**: it is this function that sizes the output
    /// buffer, and a constant in its place would truncate any frame
    /// longer than the one we would have guessed.
    pub fn echantillons_de(&self, paquet: &[u8]) -> Result<usize> {
        echantillons_de(paquet)
    }

    /// Decodes a normal frame. Returns the number of samples PER CHANNEL
    /// written; `sortie` must hold at least that many times `CHANNELS`.
    pub fn decoder(&mut self, paquet: &[u8], sortie: &mut [i16]) -> Result<usize> {
        self.inner
            .decode(paquet, sortie, false)
            .context("Opus decoding")
    }

    /// Reconstructs the PREVIOUS frame from the LBRR redundancy carried
    /// by `suivante`.
    ///
    /// It is the direction of in-band FEC, and it is counter-intuitive: a frame is
    /// never reconstructed from itself — it is reconstructed
    /// from the one that FOLLOWS it. Hence the jitter buffer's rule
    /// (`micro.rs`): FEC is only useful if the next one has ALREADY arrived.
    pub fn decoder_fec(&mut self, suivante: &[u8], sortie: &mut [i16]) -> Result<usize> {
        self.inner
            .decode(suivante, sortie, true)
            .context("Opus decoding through FEC reconstruction")
    }

    /// Loss concealment: no packet is available, not even its
    /// successor. libopus extrapolates from its internal state.
    ///
    /// **The duration produced is that of the LAST DECODED FRAME, and it is
    /// WE who impose it — not libopus.** The gesture is not cosmetic:
    /// `opus_decode` called with an empty packet takes as `frame_size` the
    /// SIZE OF THE BUFFER handed to it, and therefore produces as much PLC as
    /// the room offered, unrelated to what was decoded before.
    /// A caller handing a 40 ms buffer after a 10 ms frame
    /// would get 40 ms of concealment, and `micro.rs`'s timeline
    /// would drift by 30 ms at each loss.
    ///
    /// This defect was found by MUTATING
    /// `concealment_returns_the_duration_of_the_last_frame` (task 3,
    /// step 2): the first draft delegated the duration to libopus, and the
    /// test still passed when the PLC was preceded by a 10 ms frame
    /// instead of 40. It measured nothing.
    ///
    /// Returns `Ok(0)` without writing anything as long as no frame has been decoded:
    /// there is then no duration to conceal, and the caller must return
    /// silence.
    pub fn dissimuler(&mut self, sortie: &mut [i16]) -> Result<usize> {
        let par_canal = self.derniere_duree()?;
        if par_canal == 0 {
            return Ok(0);
        }
        let voulu = par_canal * CHANNELS;
        if sortie.len() < voulu {
            bail!(
                "concealment buffer of {} samples, {voulu} expected                  (duration of the last decoded frame: {par_canal} per channel)",
                sortie.len()
            );
        }
        self.inner
            .decode(&[], &mut sortie[..voulu], false)
            .context("dissimulation de perte Opus")
    }

    /// Duration of the last decoded frame, in samples PER CHANNEL.
    ///
    /// Is 0 as long as nothing has been decoded: there is then no duration to
    /// conceal, and the caller must return silence rather than calling
    /// `dissimuler`.
    pub fn derniere_duree(&mut self) -> Result<usize> {
        let n = self
            .inner
            .get_last_packet_duration()
            .context("reading the duration of the last Opus frame")?;
        Ok(n as usize)
    }
}

#[cfg(test)]
#[path = "opus/tests.rs"]
mod tests;
