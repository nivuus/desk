//! The tests of `LecteurMicro` — the residue, the frequency instrument, the
//! pure end-to-end, and the concealment cap.
//!
//! **Moved out of `micro/tests.rs` under the 500-line rule**, and
//! moved out BEFORE the addition they were to host (the concealment
//! cap), not after having crossed the ceiling: it is the repository's
//! doctrine, which requires extracting and forbids compressing a comment to
//! get back under the line. The cut follows the banner that already separated the
//! two halves of the file — the tests of `TamponGigue` stay with the
//! neighbour, the helpers `trames_d_un_ton` and `gauche` come here with the
//! only tests that use them.
//!
//! Declared in the parent through `#[path]`, like its neighbour, and for the same
//! reason: the usage is OUTSIDE the "Child module convention" of
//! `CLAUDE.md`, which only targets modules taken out of a
//! `#[cfg(windows)]` parent. Here the parent is pure; the only motive is size, and
//! the precedent is `superviseur/table.rs`.

use super::*;

/// Encodes `n` 10 ms frames of a pure tone at `f` hertz, in stereo, as
/// work stream A produces them — that is by the SAME encoder as the
/// product's, not by a more lenient ad hoc encoder.
fn trames_d_un_ton(f: f32, n: usize) -> Vec<TrameMicro> {
    use crate::opus::{OpusEncoder, FRAME_SAMPLES};
    let mut enc = OpusEncoder::new().expect("encodeur");
    let mut out = Vec::new();
    let mut phase = 0u64;
    for i in 0..n {
        let pcm: Vec<i16> = (0..FRAME_SAMPLES)
            .flat_map(|k| {
                let t = (phase + k as u64) as f32 / 48_000.0;
                let e = ((2.0 * std::f32::consts::PI * f * t).sin() * 12_000.0) as i16;
                [e, e]
            })
            .collect();
        phase += FRAME_SAMPLES as u64;
        out.push(TrameMicro {
            opus: enc.encode(&pcm).expect("encoding"),
            rtp_48k: (i * FRAME_SAMPLES) as u64,
            echantillons: FRAME_SAMPLES,
        });
    }
    out
}

/// The left channel of an interleaved stereo buffer.
fn gauche(entrelace: &[f32]) -> Vec<f32> {
    entrelace.iter().step_by(2).copied().collect()
}

/// The WASAPI packet is almost never the size of an Opus frame: a
/// 10 ms frame yields 480 samples per channel, and the claimed buffer may
/// want 441, 480 or 1024 of them. The residue is the piece that avoids throwing away the
/// tail of each frame.
///
/// **The criterion is a SAMPLE-BY-SAMPLE EQUALITY**, between a
/// player filled by small slices and an identical player filled in a
/// single go. It is exact, and it cannot be confused with anything.
///
/// ⚠️ **The first draft judged on FREQUENCY over 83 ms, and it
/// measured something else**: it returned 402 Hz for a 440 tone on a
/// perfectly nominal regime (`sauts`, `insertions`, `plc`,
/// `famines` all zero, surveyed). The gap came from the warm-up of the
/// Opus decoder, whose first frames weigh a quarter of such a short
/// window — the same test over 1 s returns 437 Hz. **A correct instrument
/// applied to the wrong window remains a bad instrument.**
#[test]
fn the_residue_survives_from_one_fill_to_the_next() {
    let trames = trames_d_un_ton(440.0, 12);

    // Two players fed identically, read differently.
    let mut par_tranches = LecteurMicro::new().unwrap();
    let mut d_un_coup = LecteurMicro::new().unwrap();
    for t in &trames {
        par_tranches.deposer(t.clone());
        d_un_coup.deposer(t.clone());
    }

    // 100 INTERLEAVED samples per slice, that is 50 per channel: never a
    // divisor of a frame's 480, so each slice cuts a frame right in the
    // middle. Without residue, the tail would be lost every time.
    const TRANCHE: usize = 100;
    const TOURS: usize = 80;
    let mut recolte = Vec::new();
    for _ in 0..TOURS {
        let mut tranche = vec![0.0f32; TRANCHE];
        par_tranches.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }

    let mut reference = vec![0.0f32; TRANCHE * TOURS];
    d_un_coup.remplir(&mut reference);

    // The test must not compare two silences: without this guard, a
    // player that decoded NOTHING would pass the equality below.
    assert!(
        reference.iter().any(|&e| e.abs() > 0.01),
        "the reference is silent: there is nothing to compare"
    );
    assert_eq!(
        recolte.len(),
        reference.len(),
        "the two splittings do not return the same number of samples"
    );
    if let Some(i) = recolte.iter().zip(&reference).position(|(a, b)| a != b) {
        panic!(
            "divergence at sample {i} ({} versus {}): the residue loses the tail \
             of the frames that the splitting cuts in two",
            recolte[i], reference[i]
        );
    }

    // And the regime is indeed nominal: no correction has made up the equality.
    let c = par_tranches.compteurs();
    assert_eq!(
        (c.sauts, c.insertions, c.plc, c.famines),
        (0, 0, 0, 0),
        "{c:?}"
    );
}

/// Spec §8 "Silence": the cable must be fed CONTINUOUSLY. An
/// application listening to an empty buffer does not perceive silence, it sees
/// a stream that gets interrupted.
#[test]
fn an_empty_reader_returns_silence_and_never_an_error() {
    let mut l = LecteurMicro::new().unwrap();
    let mut sortie = vec![42.0f32; 480 * 2];
    l.remplir(&mut sortie);
    assert!(
        sortie.iter().all(|&e| e == 0.0),
        "the silence was not written"
    );
}

/// The acceptance run's instrument (decision 8), tested on a SYNTHETIC signal
/// before serving to judge anything at all.
#[test]
fn a_pure_tone_frequency_is_found_within_one_percent() {
    for cible in [220.0f32, 440.0, 1000.0] {
        let pcm: Vec<f32> = (0..48_000)
            .map(|n| (2.0 * std::f32::consts::PI * cible * n as f32 / 48_000.0).sin())
            .collect();
        let f = frequence_par_passages_a_zero(&pcm, 48_000).expect("ton mesurable");
        assert!(
            (f - cible).abs() / cible < 0.01,
            "target {cible}, measured {f}"
        );
    }
}

/// ⚠️ THE test that makes the instrument credible: it must REFUSE what is
/// not a tone. Without it, "the frequency is 440" would prove nothing more
/// than a byte count — and that is exactly the doctrine this repository
/// paid for in D7.
#[test]
fn silence_and_noise_do_not_return_a_credible_frequency() {
    assert!(
        frequence_par_passages_a_zero(&vec![0.0; 48_000], 48_000).is_none(),
        "the silence returned a frequency"
    );
    // Deterministic noise (congruential generator, no new dependency).
    let mut x = 12_345u32;
    let bruit: Vec<f32> = (0..48_000)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (x >> 16) as f32 / 32_768.0 - 1.0
        })
        .collect();
    let f = frequence_par_passages_a_zero(&bruit, 48_000);
    assert!(
        f.is_none_or(|f| (f - 440.0).abs() > 100.0),
        "noise was taken for a 440 Hz tone: {f:?}"
    );
}

/// ⚠️ **THE test that exercises the DEAD BAND**, and it took a mutation to
/// discover that no other did: removing the dead band leaves
/// `silence_and_noise_do_not_return_a_credible_frequency` green (pure noise
/// returns ~12,000 Hz with or without it, hence always far from 440).
///
/// What the dead band really avoids is the MIXED case: a real tone, with
/// low-amplitude noise crossing zero between two legitimate
/// crossings. Each parasitic crossing adds two sign changes there, and
/// the measured frequency explodes while the signal, for its part, is indeed a 440 Hz.
#[test]
fn a_noisy_tone_stays_measured_at_its_frequency() {
    let mut x = 987_654u32;
    let pcm: Vec<f32> = (0..48_000)
        .map(|n| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let bruit = ((x >> 16) as f32 / 32_768.0 - 1.0) * 0.05;
            (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48_000.0).sin() + bruit
        })
        .collect();
    let f = frequence_par_passages_a_zero(&pcm, 48_000).expect("ton mesurable");
    assert!(
        (f - 440.0).abs() / 440.0 < 0.01,
        "a slightly noisy 440 Hz tone is measured at {f} Hz: the spurious \
         crossings around zero are counted as passages"
    );
}

/// The PURE end-to-end: encode a 440 Hz, pass it through the jitter buffer,
/// decode it, and find it again at its frequency. **It is the criterion of
/// E1's acceptance run, played without VM and without browser** — what will remain for the
/// acceptance run is the WebRTC path, not the processing chain.
#[test]
fn an_encoded_tone_crosses_the_buffer_and_comes_out_at_its_frequency() {
    let trames = trames_d_un_ton(440.0, 106);
    let mut l = LecteurMicro::new().unwrap();

    // Prime with 6 frames (60 ms): squarely INSIDE the dead band of the
    // drift correction. With a single frame in reserve we would be under
    // SEUIL_INSERTION and the player would insert silence at each round; with
    // more than twelve we would be above SEUIL_SAUT and it would skip some.
    let mut it = trames.into_iter();
    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }

    let mut recolte = Vec::new();
    for t in it {
        l.deposer(t);
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }

    let c = l.compteurs();
    assert_eq!(
        (c.sauts, c.insertions, c.plc, c.famines),
        (0, 0, 0, 0),
        "the regime is not nominal: the frequency measurement would bear on \
         a corrected signal, not on the transmitted signal — {c:?}"
    );

    let g = gauche(&recolte);
    let f = frequence_par_passages_a_zero(&g, 48_000).expect("ton mesurable");
    eprintln!("pure end to end: {} samples, {f} Hz measured", g.len());
    assert!(
        (f - 440.0).abs() / 440.0 < 0.02,
        "the tone did not get through: measured {f} Hz instead of 440"
    );
}

// ----------------------------------------------------------------------
// The concealment cap (work stream E fix)
// ----------------------------------------------------------------------

/// Feeds a player in the nominal regime, then cuts the sender and returns what
/// the sink heard during `reveils` wake-ups of 10 ms of starvation.
///
/// **The set-up imitates the real consumer** (`demarrage/micro.rs`): one
/// wake-up every 10 ms, 480 frames per channel. A starvation measured with a
/// single giant `remplir` would not be the same path.
fn famine_apres_un_ton(reveils: usize) -> (Vec<f32>, LecteurMicro) {
    let mut it = trames_d_un_ton(440.0, 30).into_iter();
    let mut l = LecteurMicro::new().unwrap();
    // Prime with 6 frames (60 ms): squarely INSIDE the dead band of the
    // drift correction, like the pure end-to-end above.
    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }
    for t in it {
        l.deposer(t);
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
    }

    // The sender goes silent. Not a single frame more will be dropped off.
    let mut recolte = Vec::with_capacity(reveils * 480 * 2);
    for _ in 0..reveils {
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        recolte.extend_from_slice(&tranche);
    }
    (recolte, l)
}

/// ⚠️ **THE test of the defect measured by acceptance run E1.** During 60 s of silence
/// from the browser — nominal DTX, `packetsSent` frozen —, the player called
/// `dissimuler()` at each missing frame without any bound, and the log
/// noted `plc = 50/s`, `crete` between 0.53 and 0.67 and a `frequence_hz`
/// wandering between 308 and 393 Hz. **Concealment produced a continuous
/// drone**, which in block E2 would come out on the virtual cable, hence into the
/// Windows application.
///
/// The cause is in libopus and is not a defect: its CELT concealment
/// switches to noise from the 6th consecutive loss, then makes energy
/// decrease **down to the background noise floor and keeps it there**
/// (`celt/celt_decoder.c:537` and `:562`, `MAX16(backgroundLogE, ...)`). It
/// therefore converges towards comfort noise and NEVER stops by itself:
/// bounding the concealed duration is up to US.
#[test]
fn prolonged_starvation_stops_concealing_and_returns_silence() {
    const REVEILS: usize = 100; // 1 s de famine
    const QUEUE_DEPUIS: usize = 50; // we judge the second half
    let (recolte, lecteur) = famine_apres_un_ton(REVEILS);

    // Anti-vacuity guard: without it, a player that NEVER concealed
    // anything would pass this test without having exercised anything at all.
    let c = lecteur.compteurs();
    assert!(
        c.plc > 0,
        "no concealment took place: the test measures nothing — {c:?}"
    );

    let queue = &recolte[QUEUE_DEPUIS * 480 * 2..];
    let crete = queue.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    let f = frequence_par_passages_a_zero(&gauche(queue), 48_000);
    assert_eq!(
        crete, 0.0,
        "after 500 ms of starvation the sink still returns signal (peak {crete:.3}, \
         frequency {f:?}): Opus concealment is bounded by nothing and \
         produces a continuous drone"
    );
    assert_eq!(
        f, None,
        "after 500 ms of starvation the sink still returns a dominant frequency"
    );
}

/// ⚠️ **OBSERVABILITY, without which the fix would not be
/// falsifiable.** An acceptance run must be able to distinguish "concealment
/// is working" from "the cap has bitten and the sink is silent": both
/// return starvations, and without two DISJOINT counters they read
/// identically in the log.
///
/// The test checks the three properties that make the trace readable:
/// `plc` stops, `plc_plafonnees` takes over, and the sum of the two
/// does cover all the missing frames returned.
#[test]
fn the_ceiling_is_counted_apart_from_concealment() {
    const REVEILS: usize = 100;
    let (_, lecteur) = famine_apres_un_ton(REVEILS);
    let c = lecteur.compteurs();

    assert!(
        c.plc_plafonnees > 0,
        "the ceiling bit (the sink goes silent) but nothing counts it: an \
         acceptance run cannot tell that silence from a concealment that \
         is working — {c:?}"
    );
    // Concealment did take place BEFORE the cap, and it is bounded by
    // it: 200 ms of 10 ms frames make at most 20 concealments.
    assert!(
        (1..=20).contains(&c.plc),
        "the concealments actually produced should fit within the \
         200 ms of the ceiling (at most 20 frames of 10 ms) — {c:?}"
    );
    // And the two counters share EXACTLY the missing frames:
    // a due frame is either concealed or returned as silence, never
    // both nor neither.
    assert_eq!(
        c.plc + c.plc_plafonnees,
        c.famines + c.insertions,
        "missing frames are counted neither as concealed nor as \
         capped — {c:?}"
    );
}

/// ⚠️ **THE test that forbids the cap from condemning the session.** Once the
/// cap is reached, the sink goes silent — but the return of speech must give back
/// the whole budget, otherwise the NEXT network loss, however
/// short, would never be concealed again. It is the only test of this
/// file that exercises the reset on the PRODUCTION path: the unit
/// tests of `micro/dissimulation.rs` cover the rule, never its wiring.
#[test]
fn after_the_ceiling_returning_speech_restores_the_whole_budget() {
    let all = trames_d_un_ton(440.0, 70);
    let mut it = all.into_iter();
    let mut l = LecteurMicro::new().unwrap();
    let reveil = |l: &mut LecteurMicro| {
        let mut tranche = vec![0.0f32; 480 * 2];
        l.remplir(&mut tranche);
        tranche
    };

    for _ in 0..6 {
        l.deposer(it.next().unwrap());
    }
    for _ in 0..24 {
        l.deposer(it.next().unwrap());
        reveil(&mut l);
    }

    // First starvation, long enough to exhaust the cap.
    for _ in 0..60 {
        reveil(&mut l);
    }
    let apres_famine_1 = l.compteurs();
    assert!(
        apres_famine_1.plc_plafonnees > 0,
        "the ceiling did not bite: the rest of the test would prove nothing — {apres_famine_1:?}"
    );

    // Speech comes back: thirty frames, one per wake-up.
    let mut entendu = Vec::new();
    for _ in 0..30 {
        l.deposer(it.next().unwrap());
        entendu.extend_from_slice(&reveil(&mut l));
    }

    // The tone has indeed come back — otherwise "the budget is given back" would be read on
    // a sink that returns nothing at all anymore.
    let crete = entendu.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    assert!(
        crete > 0.05,
        "the tone did not come back after the ceiling (peak {crete:.3}): the sink \
         stays silent while frames arrive"
    );

    // Second starvation, SHORT: three wake-ups, the order of magnitude of a network
    // loss. It must be concealed as before, hence make `plc` grow.
    for _ in 0..3 {
        reveil(&mut l);
    }
    let apres_famine_2 = l.compteurs();
    assert!(
        apres_famine_2.plc > apres_famine_1.plc,
        "a short loss occurring AFTER the ceiling bit is no longer \
         concealed: the budget is not given back when speech returns — \
         {apres_famine_1:?} then {apres_famine_2:?}"
    );
}
