//! The tests of `opus.rs`, moved to their own file under the
//! 500-line rule: `opus.rs` crossed the ceiling when gaining its
//! DECODER (work stream E, block E1, task 3), and the repository's doctrine requires
//! EXTRACTING, never compressing a comment to get back under the
//! line.
//!
//! Declared in the parent through `#[path]` — it is a usage explicitly OUTSIDE
//! the "Child module convention" of `docs/claude/module-conventions.md`, which only targets the
//! modules taken out of a `#[cfg(windows)]` parent to compile them on
//! the host. Here the parent is already portable; the only motive is size, and
//! the precedent is `superviseur/table.rs`.

use super::*;

/// Energy of the signal at `freq` hertz on the left channel, through the
/// Goertzel algorithm.
///
/// Insensitive to delay: the codec introduces an algorithmic latency
/// (312 samples in `Application::Audio`, see the docstring of
/// `OpusEncoder`), so a sample-by-sample comparison with
/// the input would fail for a reason that has nothing to do with
/// fidelity.
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

/// `n` stereo interleaved samples of a sine wave at `freq` hertz,
/// starting at sample `depuis` to stay continuous from one frame to
/// the next.
fn ton(freq: f64, depuis: usize, n: usize) -> Vec<i16> {
    (0..n)
        .flat_map(|i| {
            let phase =
                (depuis + i) as f64 * 2.0 * std::f64::consts::PI * freq / SAMPLE_RATE_HZ as f64;
            let v = (phase.sin() * 12_000.0) as i16;
            [v, v]
        })
        .collect()
}

#[test]
fn a_10_ms_frame_is_480_samples_per_channel() {
    assert_eq!(FRAME_SAMPLES, 480);
    assert_eq!(FRAME_INTERLEAVED, 960);
}

#[test]
fn refuses_a_frame_of_wrong_size() {
    let mut encodeur = OpusEncoder::new().unwrap();
    let err = encodeur.encode(&vec![0i16; 1000]).unwrap_err();
    assert!(
        err.to_string().contains("960"),
        "the message must name the expected size, got: {err}"
    );
}

#[test]
fn an_encoded_then_decoded_tone_stays_the_same_tone() {
    // Checking that the encoder returns bytes would prove nothing:
    // noise would return just as many. We decode back and check
    // that the energy stays concentrated on the original frequency.
    let mut encodeur = OpusEncoder::new().unwrap();
    // Unlike `use`s, expressions resolve the `opus::` path by first going through
    // the module tree of the current crate. Finding nothing named `opus` there,
    // they go up to the prelude (external crates), hence the `opus` crate. No `::` required.
    let mut decodeur = opus::Decoder::new(SAMPLE_RATE_HZ, opus::Channels::Stereo).unwrap();

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
        "the energy must stay concentrated on 440 Hz: 440 Hz = {a_440:.1}, 1500 Hz = {a_1500:.1}"
    );

    let entree = energie_a(&ton(440.0, 0, 20 * FRAME_SAMPLES), 440.0);
    let rapport = a_440 / entree;
    assert!(
        (0.8..=1.2).contains(&rapport),
        "the restored amplitude must stay close to the original, ratio = {rapport:.3}"
    );
}

#[test]
fn prolonged_silence_falls_back_to_a_few_bytes_per_frame() {
    // DTX takes several frames to converge: the first five are
    // still 217 then 161 bytes. Measuring too early would wrongly conclude that
    // DTX does not work. We therefore look at the TAIL, not the beginning.
    let mut encodeur = OpusEncoder::new().unwrap();
    let silence = vec![0i16; FRAME_INTERLEAVED];
    let sizes: Vec<usize> = (0..40)
        .map(|_| encodeur.encode(&silence).unwrap().len())
        .collect();

    let queue = &sizes[35..];
    assert!(
        queue.iter().all(|&t| t <= 8),
        "in steady state, a silence frame must fit in a few bytes, got: {queue:?}"
    );
}

#[test]
fn the_loss_percentage_is_bounded() {
    let mut enc = OpusEncoder::new().expect("encodeur");

    enc.set_packet_loss_perc(0).expect("0 accepted");
    enc.set_packet_loss_perc(25).expect("25 accepted");

    // Out of bounds: clamped rather than refused. The controller already clamps,
    // but this function is public and must not let through a
    // value libopus would reject with an opaque error.
    enc.set_packet_loss_perc(-5)
        .expect("negative value clamped");
    enc.set_packet_loss_perc(300)
        .expect("excessive value clamped");
}

#[test]
fn a_declared_loss_really_changes_the_encoding() {
    // Proof that in-band FEC is no longer inert.
    //
    // CAREFUL with the direction: this test does NOT measure a size
    // increase. Under a fixed target bitrate, LBRR does not add to the bytes,
    // it redistributes them — `compute_silk_rate_for_hybrid`
    // (opus_encoder.c:751) uses different rate tables depending on
    // whether FEC is coded or not. Packets may therefore SHRINK.
    //
    // What makes the proof is that the output DIFFERS: in CELT-only mode
    // (`Application::LowDelay`), it was bit-for-bit identical, because
    // `decide_fec` (opus_encoder.c:721) returns 0 without looking at anything else.
    // In SILK/hybrid mode, the only path through which `packet_loss_perc`
    // influences encoding is `decide_fec` -> `LBRR_coded`.
    //
    // A NON-silent signal is indispensable: under DTX, silence
    // falls to 1 byte per frame whatever is declared.
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut sans = OpusEncoder::new().expect("encodeur");
    let mut with = OpusEncoder::new().expect("encodeur");
    with.set_packet_loss_perc(20).expect("declared loss");

    let mut total_sans = 0usize;
    let mut total_with = 0usize;
    for _ in 0..100 {
        total_sans += sans.encode(&pcm).expect("encoding").len();
        total_with += with.encode(&pcm).expect("encoding").len();
    }

    assert_ne!(
        total_with, total_sans,
        "identical output ({total_sans} bytes on both sides): \
         `decide_fec` took its early return, so no LBRR redundancy \
         is encoded — this is the symptom of the CELT-only mode"
    );
}

#[test]
fn lbrr_is_really_decodable() {
    // Test that the coded LBRR redundancy is really present and
    // decodable. It is the semantic proof that FEC is effective:
    // we encode declaring a loss, take a packet in steady
    // state, and decode this packet with the FEC flag on a fresh decoder
    // (without history). We measure the reconstructed energy.
    //
    // The strategy: encode the same frame 100 times to reach
    // steady state. Take packet #99. Decode this packet with FEC
    // on two fresh decoders: one from the encoder with declared loss
    // (expects LBRR redundancy), one from the encoder without loss
    // (no redundancy, only reconstruction noise).
    //
    // Expected: reconstructed energy(with FEC) >> reconstructed energy(without).
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut enc_sans = OpusEncoder::new().expect("encodeur");
    let mut enc_with = OpusEncoder::new().expect("encodeur");
    enc_with.set_packet_loss_perc(20).expect("declared loss");

    // Encode 100 frames to reach steady state.
    let mut paquets_sans = Vec::new();
    let mut packets_with = Vec::new();
    for _ in 0..100 {
        paquets_sans.push(enc_sans.encode(&pcm).expect("encoding without"));
        packets_with.push(enc_with.encode(&pcm).expect("encoding with"));
    }

    // Take the last packet in steady state.
    let last_without = &paquets_sans[99];
    let last_with = &packets_with[99];

    // Fresh decoder without history to decode the packet as an
    // FEC frame (the decoder reconstructs from the redundancy of the
    // NEXT packet, or simply tries to mask the loss).
    let mut dec_pour_sans =
        ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo).expect("decoder");
    let mut dec_for_with =
        ::opus::Decoder::new(SAMPLE_RATE_HZ, ::opus::Channels::Stereo).expect("decoder");

    let mut sortie_sans = vec![0i16; FRAME_INTERLEAVED];
    let mut output_with = vec![0i16; FRAME_INTERLEAVED];

    // Decode with the FEC flag (simulates a lost frame).
    dec_pour_sans
        .decode(last_without, &mut sortie_sans, true)
        .expect("decoding without, with FEC");
    dec_for_with
        .decode(last_with, &mut output_with, true)
        .expect("decoding with, with FEC");

    // Measure the energy (normalised sum of squares).
    let energie_sans: f64 = sortie_sans
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / (FRAME_INTERLEAVED as f64);
    let energy_with: f64 = output_with
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / (FRAME_INTERLEAVED as f64);

    eprintln!(
        "Reconstructed energy: without FEC = {:.2}, with FEC = {:.2}",
        energie_sans, energy_with
    );

    // Expects the LBRR redundancy to produce a significant signal.
    // If it is present, energy_with >> energie_sans.
    assert!(
        energy_with > energie_sans,
        "no decodable LBRR redundancy: \
         energy without FEC = {:.2}, energy with FEC = {:.2} — \
         the FEC brought nothing to the reconstruction",
        energie_sans,
        energy_with
    );
}

// ------------------------------------------------------------------
// The DECODER (work stream E, block E1, task 3)
// ------------------------------------------------------------------

/// The least obvious design fact of decoding: Chrome encodes the
/// microphone in MONO, and the cable expects stereo. We do NOT write it
/// ourselves: a decoder created for two channels duplicates a mono stream.
/// This test is here so that this property is CHECKED and not assumed
/// — spec §7 described it as work to be done ("mono → stereo
/// by duplication").
#[test]
fn a_mono_stream_comes_out_stereo_by_duplication() {
    let mut enc = ::opus::Encoder::new(
        SAMPLE_RATE_HZ,
        ::opus::Channels::Mono,
        ::opus::Application::Audio,
    )
    .unwrap();
    let mono: Vec<i16> = (0..960)
        .map(|n| ((n as f32 * 0.1).sin() * 8000.0) as i16)
        .collect();
    let paquet = enc.encode_vec(&mono, 4000).unwrap();

    let mut dec = OpusDecoder::new().unwrap();
    assert_eq!(dec.echantillons_de(&paquet).unwrap(), 960);
    let mut sortie = vec![0i16; 960 * CHANNELS];
    assert_eq!(dec.decoder(&paquet, &mut sortie).unwrap(), 960);
    // Interleaved: both channels are IDENTICAL sample by sample.
    let (paires, _) = sortie.as_chunks::<2>();
    for paire in paires {
        assert_eq!(paire[0], paire[1], "dissimilar left and right channels");
    }
    // …and the signal is not zero: a decoder that returned silence
    // would pass the equality above without decoding anything.
    assert!(
        sortie.iter().any(|&e| e.abs() > 500),
        "decoded signal is zero"
    );
}

/// "No frame duration is assumed" (spec §7). Chrome emits
/// 20 ms by default, work stream A produces 10 ms, and nothing guarantees
/// they stick to that. It is the test that forbids assuming a
/// constant again.
#[test]
fn frames_of_ten_twenty_and_forty_milliseconds_all_pass() {
    for ms in [10u32, 20, 40] {
        let par_canal = (SAMPLE_RATE_HZ / 1000 * ms) as usize;
        let mut enc = ::opus::Encoder::new(
            SAMPLE_RATE_HZ,
            ::opus::Channels::Stereo,
            ::opus::Application::Audio,
        )
        .unwrap();
        // A REAL signal, not silence: with the DTX an encoder
        // might carry, a mute frame can be coded in one byte and
        // `get_nb_samples` would no longer be representative of anything.
        let pcm: Vec<i16> = (0..par_canal * CHANNELS)
            .map(|n| ((n as f32 * 0.03).sin() * 6000.0) as i16)
            .collect();
        let paquet = enc.encode_vec(&pcm, 4000).unwrap();
        let mut dec = OpusDecoder::new().unwrap();
        assert_eq!(
            dec.echantillons_de(&paquet).unwrap(),
            par_canal,
            "duration {ms} ms"
        );
        let mut sortie = vec![0i16; par_canal * CHANNELS];
        assert_eq!(
            dec.decoder(&paquet, &mut sortie).unwrap(),
            par_canal,
            "duration {ms} ms"
        );
    }
}

/// "Checking only that it returns bytes would prove it returns
/// noise just as well" (spec §11). The PLC must return the RIGHT NUMBER
/// of samples, and this number comes from the LAST DECODED FRAME — not
/// from a constant.
#[test]
fn concealment_returns_the_duration_of_the_last_frame() {
    let par_canal = (SAMPLE_RATE_HZ / 1000 * 40) as usize; // 40 ms → 1920
    let mut enc = ::opus::Encoder::new(
        SAMPLE_RATE_HZ,
        ::opus::Channels::Stereo,
        ::opus::Application::Audio,
    )
    .unwrap();
    let pcm: Vec<i16> = (0..par_canal * CHANNELS)
        .map(|n| ((n as f32 * 0.02).sin() * 7000.0) as i16)
        .collect();
    let paquet = enc.encode_vec(&pcm, 4000).unwrap();

    let mut dec = OpusDecoder::new().unwrap();
    let mut sortie = vec![0i16; par_canal * CHANNELS];
    assert_eq!(dec.decoder(&paquet, &mut sortie).unwrap(), par_canal);
    assert_eq!(
        dec.derniere_duree().unwrap(),
        par_canal,
        "the duration read back is not that of the frame just decoded"
    );

    // ⚠️ The buffer is DELIBERATELY larger than the frame — 60 ms for
    // a frame of 40. Without this gap, the test would only measure the size
    // of the buffer: `opus_decode` on an empty packet takes `frame_size` from
    // the length handed to it. It is exactly what the mutation of
    // task 3 revealed.
    let mut plc = vec![0i16; (SAMPLE_RATE_HZ / 1000 * 60) as usize * CHANNELS];
    assert_eq!(
        dec.dissimuler(&mut plc).unwrap(),
        par_canal,
        "the concealment did not return the duration of the last decoded frame"
    );

    // And a FRESH decoder has no duration to conceal: it returns 0, up to
    // the caller to write silence.
    let mut neuf = OpusDecoder::new().unwrap();
    assert_eq!(neuf.dissimuler(&mut plc).unwrap(), 0);
}

/// The exact counterpart of the ENCODER's FEC energy test
/// (`lbrr_is_really_decodable`), taken the other way round: the NEXT frame
/// restores a signal CORRELATED with the original, not only bytes.
///
/// The criterion is an energy RATIO between two streams that only differ in the loss
/// declared to the encoder: without it, libopus emits no
/// LBRR redundancy and the FEC decoder can only mask.
#[test]
fn fec_reconstruction_restores_a_signal_correlated_with_the_original() {
    let pcm: Vec<i16> = (0..FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();

    let mut enc_sans = OpusEncoder::new().expect("encodeur");
    let mut enc_with = OpusEncoder::new().expect("encodeur");
    enc_with.set_packet_loss_perc(20).expect("declared loss");

    let mut paquets_sans = Vec::new();
    let mut packets_with = Vec::new();
    for _ in 0..100 {
        paquets_sans.push(enc_sans.encode(&pcm).expect("encoding without"));
        packets_with.push(enc_with.encode(&pcm).expect("encoding with"));
    }

    let energie = |paquet: &[u8]| -> f64 {
        // FRESH decoder, hence without history: what comes out can only come
        // from the redundancy carried by this very packet.
        let mut dec = OpusDecoder::new().expect("decoder");
        let mut sortie = vec![0i16; FRAME_INTERLEAVED];
        let n = dec.decoder_fec(paquet, &mut sortie).expect("FEC decoding");
        sortie[..n * CHANNELS]
            .iter()
            .map(|&e| (e as f64) * (e as f64))
            .sum::<f64>()
            / (n * CHANNELS).max(1) as f64
    };

    let with = energie(&packets_with[99]);
    let sans = energie(&paquets_sans[99]);
    eprintln!("FEC decoded: energy with declared loss={with:.2}, without={sans:.2}");
    // ⚠️ TWO assertions, and the second is the one that makes the test
    // discriminating. The energy ratio alone does NOT prove that the FEC
    // path was taken: decoding these two packets NORMALLY (fec
    // = false) also returns a ratio above 10, and this mutation
    // passed — noted at task 3, step 2.
    //
    // What really distinguishes the FEC path is that without LBRR
    // redundancy it returns SILENCE: libopus has nothing to reconstruct and
    // invents nothing. A normal decoding, for its part, returns the packet's signal.
    assert!(
        with > sans * 10.0 + 1.0,
        "FEC reconstruction restores nothing correlated: with={with:.2}, without={sans:.2}"
    );
    assert!(
        sans < 1.0,
        "without LBRR redundancy, FEC decoding should return SILENCE and returns {sans:.2}: \
         the FEC flag was not honoured, and this packet was decoded normally"
    );
}
