//! The MEASUREMENT SINK itself: the consumer, its one-second
//! observation window, and what it reports to the log.
//!
//! ⚠️ **EXTRACTED VERBATIM from `demarrage/micro.rs` (task 6 of plan E2), BEFORE
//! the addition that would have made it cross the cap — not after.** The parent
//! file was **452** lines and task 10 must add the
//! "cable or measurement or nothing" routing to it. D9 paid twice for the reverse order
//! (`sommeil.rs` brought down to 499 **by compression**, a gesture `CLAUDE.md`
//! forbids by name, then extracted at the review's demand; `capteur/fenetre.rs`,
//! 508 → 496 → 485); D10 inverted the order three times and compressed not
//! once. **No behaviour changes**, and the transposition check is
//! the test count, announced BEFORE being measured.
//!
//! What REMAINS in the parent is the routing alone: `arme` and `brancher`.
//!
//! ⚠️ **BENCH INSTRUMENT, NEVER A SHIPPED CONFIGURATION** — see the header
//! of `demarrage/micro.rs` for the `MICRO_MESURE=1` convention.
//!
//! **PURE: no `cfg`, no COM object, no device.**

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::micro::{
    frequence_par_passages_a_zero, CompteursMicro, LecteurMicro, PuitsMicro, TrameMicro,
};
use crate::opus::SAMPLE_RATE_HZ;

/// Wake-up period of the consumer. 10 ms is the usual granularity of a
/// shared WASAPI buffer: the sink therefore imitates what block E2 will do.
const PERIODE: Duration = Duration::from_millis(10);

/// Frames per channel consumed at each wake-up. 480 at 48 kHz = 10 ms.
const TRAMES_PAR_REVEIL: usize = 480;

/// The sink, as `transport/piste_micro.rs` sees it.
///
/// ⚠️ `pub(super)`: it is, with that of `consommer`, the ONLY thing
/// the extraction changes in the code — `brancher` stayed in the parent and must
/// be able to name them. No behaviour moves.
///
/// ⚠️ **`deposer` takes a lock, and the work stream's invariant is that it does
/// NOT block.** The lock is only held for the time of `TamponGigue::deposer`
/// (an ordered insertion into a bounded queue) on one side, and of
/// `LecteurMicro::remplir` (decoding at most a handful of Opus frames)
/// on the other: windows of the order of ten microseconds, against
/// a transport loop running at the video's pace. That is acceptable
/// **for a bench**.
///
/// ⚠️ **The rest of this sentence announced that "block E2 will have a real
/// WASAPI thread with a hard deadline and will have to decide otherwise — a
/// lock-free queue, or a double buffer". It was a PREDICTION, and block E2
/// decided the other way**: `windows_micro.rs` keeps the `Mutex`, because
/// the render thread only holds it for the time of `remplir` — of the order of
/// ten microseconds — against a WASAPI deadline of the order of 10 ms,
/// three orders of magnitude above. A lock-free queue would be work
/// written before having observed the need. **And the need is made
/// OBSERVABLE**: the render thread counts its missed deadlines (`retards` in
/// its periodic trace), which makes the question decidable instead of
/// conjectural.
pub(super) struct PuitsDeMesure {
    pub(super) lecteur: Arc<Mutex<LecteurMicro>>,
}

impl PuitsMicro for PuitsDeMesure {
    fn deposer(&mut self, trame: TrameMicro) -> bool {
        // `false` would mean "exclusivity not acquired" (block E2). This sink
        // claims none: it always accepts.
        match self.lecteur.lock() {
            Ok(mut lecteur) => {
                lecteur.deposer(trame);
                true
            }
            // The lock is poisoned: the consumer thread panicked. We
            // refuse rather than propagate the panic into the transport
            // loop — a mic defect never kills a video session.
            Err(_) => false,
        }
    }
}

/// What a one-second observation window reports to the log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Releve {
    /// Samples PER CHANNEL actually consumed in the window.
    pub echantillons: usize,
    /// Absolute peak, all channels combined.
    pub crete: f32,
    /// Dominant frequency, or `None` if the signal is too weak for a
    /// zero crossing to make sense.
    pub frequence_hz: Option<f32>,
}

/// The accumulator of one second of consumed PCM.
///
/// ⚠️ **The buffer returned by `LecteurMicro::remplir` is INTERLEAVED STEREO**, and
/// `frequence_par_passages_a_zero` expects a MONO signal. It is here, and
/// nowhere else, that de-interleaving must happen.
///
/// ⚠️ **Forgetting it HALVES the frequency — it does not double it**, contrary to
/// what the doc of `micro::frequence_par_passages_a_zero` long claimed.
/// **Measured** by removing the `step_by(2)` below and rerunning
/// `la_frequence_est_celle_du_signal_et_non_son_double`: **219.5 Hz returned for
/// a 440 Hz tone**, identical channels. The mechanism: the function
/// derives the duration from `pcm.len() / hz`, yet an interleaved buffer carries twice
/// as many values as frames — the computed duration doubles, while the
/// number of zero crossings does not move (duplicating each sample
/// adds no sign change).
pub(super) struct Fenetre {
    /// Samples per second and per channel: the size of the window.
    hz: u32,
    /// The LEFT channel alone, accumulated.
    mono: Vec<f32>,
    /// Absolute peak, all channels combined — an imbalance between channels must
    /// not go unnoticed on the pretext that only one channel is analysed.
    crete: f32,
}

impl Fenetre {
    pub(super) fn new(hz: u32) -> Self {
        Self {
            hz,
            mono: Vec::with_capacity(hz as usize),
            crete: 0.0,
        }
    }

    /// Absorbs an interleaved stereo buffer. Returns a reading — and starts over from zero —
    /// as soon as the window is full.
    pub(super) fn absorber(&mut self, stereo: &[f32]) -> Option<Releve> {
        for &e in stereo {
            self.crete = self.crete.max(e.abs());
        }
        // The LEFT channel alone: one sample out of two. See the type's doc
        // for what forgetting this `step_by` costs.
        for &g in stereo.iter().step_by(2) {
            self.mono.push(g);
        }

        if self.mono.len() < self.hz as usize {
            return None;
        }
        let releve = Releve {
            echantillons: self.mono.len(),
            crete: self.crete,
            frequence_hz: frequence_par_passages_a_zero(&self.mono, self.hz),
        };
        self.mono.clear();
        self.crete = 0.0;
        Some(releve)
    }
}

/// The consumer thread: it keeps the deadline, fills, and reports.
pub(super) fn consommer(lecteur: Arc<Mutex<LecteurMicro>>, session_id: String) {
    let mut tampon = vec![0.0f32; TRAMES_PAR_REVEIL * 2];
    let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
    let mut precedents = CompteursMicro::default();
    let mut echeance = Instant::now();
    // The maximum occupancy over the observation window. ⚠️ A maximum, not
    // an average: criterion ④ of acceptance run E1 is a BOUND, and an average
    // would drown the only spike that would cross it.
    let mut occupation_max = Duration::ZERO;

    loop {
        // ⚠️ DEADLINE, NOT `sleep(PERIODE)`. A fixed-period `sleep` drifts
        // by the work time at every round: the sink would consume more slowly
        // than 48 kHz, the jitter buffer would saturate, and we would measure the drift
        // of the instrument while believing we measure that of the network.
        echeance += PERIODE;
        let maintenant = Instant::now();
        if echeance > maintenant {
            std::thread::sleep(echeance - maintenant);
        } else {
            // Clear lateness (loaded machine): we restart from now rather
            // than catching up in a burst, which would empty the buffer at once
            // and count starvations that are not.
            echeance = maintenant;
        }

        let (compteurs, occupation) = {
            let Ok(mut lecteur) = lecteur.lock() else {
                tracing::warn!("measurement sink lock poisoned, measurement thread stopped");
                return;
            };
            // ⚠️ READ BEFORE `remplir`, never after: it is what the buffer
            // made wait at the moment the consumer showed up.
            // After, the queue has just been emptied and we would always read
            // roughly zero — a check unable to cross its bound.
            let occupation = lecteur.occupation();
            lecteur.remplir(&mut tampon);
            (lecteur.compteurs(), occupation)
        };
        occupation_max = occupation_max.max(occupation);

        let Some(releve) = fenetre.absorber(&tampon) else {
            continue;
        };

        // ⚠️ `crete` and `frequence_hz` are SIDE BY SIDE, and that is the substance of
        // this trace: a frequency returned on a near-zero signal means
        // nothing. `frequence_par_passages_a_zero` then returns `None`, which
        // is written "none" — a reader of the log cannot read one without
        // the other, nor take a number for proof of sound.
        let frequence = match releve.frequence_hz {
            Some(f) => format!("{f:.1}"),
            None => "none".to_string(),
        };
        // The counters are DELTAS of the elapsed second, not cumulative values.
        // `CompteursMicro` is cumulative; a cumulative value would drag a single
        // incident for the rest of the session, and the plan's example
        // (`deposees=50`, that is one second of 20 ms frames) is a delta.
        // `deposees_total` is added so that the cumulative value stays readable.
        let d = |maintenant: u64, before: u64| maintenant.saturating_sub(before);
        tracing::info!(
            // ⚠️ MANDATORY: `agent.log` mixes the supervisor and all its
            // children since D4. A trace without `session` is a number in an
            // anonymous multiset, and D6 had to re-attribute two traces in the middle of an
            // acceptance run for lack of this field.
            session = %session_id,
            echantillons = releve.echantillons,
            crete = format!("{:.3}", releve.crete),
            frequence_hz = %frequence,
            deposees = d(compteurs.deposees, precedents.deposees),
            sauts = d(compteurs.sauts, precedents.sauts),
            insertions = d(compteurs.insertions, precedents.insertions),
            plc = d(compteurs.plc, precedents.plc),
            // ⚠️ **`plc` and `plc_plafonnees` are SIDE BY SIDE, and that is the substance
            // of this pair**: both are born from a missing frame, and
            // without the second we cannot distinguish "concealment
            // is working" from "the cap bit and the sink is silent". That is
            // what makes the cap fix FALSIFIABLE — acceptance run E1
            // recorded `plc = 50/s` during sixty seconds of silence, and
            // what must now be read there is `plc = 0` with
            // `plc_plafonnees = 50/s`, `crete = 0.000` and `frequence_hz =
            // none`. See `micro/dissimulation.rs`.
            plc_plafonnees = d(compteurs.plc_plafonnees, precedents.plc_plafonnees),
            fec = d(compteurs.fec, precedents.fec),
            famines = d(compteurs.famines, precedents.famines),
            hors_ordre = d(compteurs.hors_ordre, precedents.hors_ordre),
            doublons = d(compteurs.doublons, precedents.doublons),
            jetees_saturation = d(compteurs.jetees_saturation, precedents.jetees_saturation),
            jetees_perimees = d(compteurs.jetees_perimees, precedents.jetees_perimees),
            deposees_total = compteurs.deposees,
            // Criterion ④ of acceptance run E1 (spec §13): the latency the
            // jitter buffer adds on its own, bounded by `micro::PLAFOND`.
            // ⚠️ It is NOT the end-to-end latency — see the doc of
            // `LecteurMicro::occupation`.
            occupation_ms = occupation.as_millis() as u64,
            occupation_max_ms = occupation_max.as_millis() as u64,
            "mic measured"
        );
        precedents = compteurs;
        occupation_max = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `arme` stayed in the parent (the routing); its tests, however,
    // leave with the module. The only line added by the extraction.
    use crate::demarrage::micro::arme;

    /// A CONTINUOUS tone, interleaved stereo, both channels identical.
    ///
    /// ⚠️ **Phase continuity is essential, and its absence first
    /// made the test fail for a wrong reason.** A first draft
    /// built ONE 10 ms block and repeated it a hundred times: the phase restarted
    /// from zero at each block, 440 Hz not fitting a whole number of cycles in
    /// 480 samples, and the splice discontinuity made it read 400 Hz.
    /// The implementation was right; it was the instrument that lied.
    fn tonalite_continue(hz_signal: f32, trames: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(trames * 2);
        for n in 0..trames {
            let e = (2.0 * std::f32::consts::PI * hz_signal * n as f32 / SAMPLE_RATE_HZ as f32)
                .sin()
                * 0.5;
            v.push(e);
            v.push(e);
        }
        v
    }

    /// Cuts an interleaved buffer into wake-ups of `TRAMES_PAR_REVEIL` frames.
    fn reveils(entrelace: &[f32]) -> impl Iterator<Item = &[f32]> {
        entrelace.chunks(TRAMES_PAR_REVEIL * 2)
    }

    #[test]
    fn only_the_value_1_arms_the_sink() {
        assert!(arme(Some("1")));

        // ⚠️ The four cases that follow are the substance of the test. A reading through
        // `is_ok()` — the form the neighbouring variables have — would return TRUE on
        // `Some("0")`, `Some("")` and `Some("true")`: an operator writing
        // `MICRO_MESURE=0` to be certain to turn the instrument off
        // would turn it on.
        assert!(!arme(None), "absent: disarmed");
        assert!(!arme(Some("0")), "\"0\": disarmed, like absence");
        assert!(!arme(Some("")), "empty: disarmed");
        assert!(!arme(Some("true")), "\"true\" is not \"1\"");
    }

    #[test]
    fn une_fenetre_ne_se_cloture_qu_a_la_seconde_pleine() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let seconde = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let mut blocs = reveils(&seconde);

        // 99 wake-ups of 10 ms: 990 ms, the window is not full.
        for _ in 0..99 {
            assert_eq!(fenetre.absorber(blocs.next().unwrap()), None);
        }
        let releve = fenetre
            .absorber(blocs.next().unwrap())
            .expect("the hundredth closes the second");
        assert_eq!(releve.echantillons, SAMPLE_RATE_HZ as usize);
    }

    #[test]
    fn la_frequence_est_celle_du_signal_et_non_son_double() {
        // ⚠️ THE TEST THAT MATTERS. The buffer is INTERLEAVED STEREO: analysed as
        // is, it counts the zero crossings of BOTH channels and returns 880 Hz
        // for a 440 tone. That is what the first draft of
        // `Fenetre::absorber` did, and that is what this test caught.
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let seconde = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let mut last = None;
        for bloc in reveils(&seconde) {
            if let Some(r) = fenetre.absorber(bloc) {
                last = Some(r);
            }
        }
        let f = last
            .expect("one second absorbed")
            .frequence_hz
            .expect("a strong signal has a frequency");
        assert!(
            (f - 440.0).abs() < 5.0,
            "measured frequency {f:.1} Hz, expected ≈ 440 Hz (220 would reveal a non-deinterleaved buffer)"
        );
    }

    #[test]
    fn silence_returns_no_frequency_but_returns_its_peak() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let silence = vec![0.0f32; TRAMES_PAR_REVEIL * 2];
        let mut last = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&silence) {
                last = Some(r);
            }
        }
        let releve = last.expect("one second absorbed");
        // A signal too weak has NO frequency: returning a number for
        // silence would make this instrument the byte counter it exists
        // to replace (doctrine paid for in D7).
        assert_eq!(releve.frequence_hz, None);
        assert_eq!(releve.crete, 0.0);
    }

    #[test]
    fn the_peak_sees_both_channels_not_only_the_analysed_one() {
        // Left channel silent, right channel at 0.8: the peak must see it, even
        // though the frequency is analysed on the left. An imbalance between
        // channels must not go unnoticed.
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let mut bloc = Vec::with_capacity(TRAMES_PAR_REVEIL * 2);
        for _ in 0..TRAMES_PAR_REVEIL {
            bloc.push(0.0);
            bloc.push(0.8);
        }
        let mut last = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&bloc) {
                last = Some(r);
            }
        }
        assert_eq!(last.expect("one second absorbed").crete, 0.8);
    }

    #[test]
    fn une_fenetre_repart_a_zero_apres_sa_cloture() {
        let mut fenetre = Fenetre::new(SAMPLE_RATE_HZ);
        let tonalite = tonalite_continue(440.0, SAMPLE_RATE_HZ as usize);
        let silence = vec![0.0f32; TRAMES_PAR_REVEIL * 2];

        for bloc in reveils(&tonalite) {
            fenetre.absorber(bloc);
        }
        // Second window, silent: without a reset, the peak and the
        // frequency of the first would leak into the second.
        let mut last = None;
        for _ in 0..100 {
            if let Some(r) = fenetre.absorber(&silence) {
                last = Some(r);
            }
        }
        let releve = last.expect("one second absorbed");
        assert_eq!(releve.crete, 0.0);
        assert_eq!(releve.frequence_hz, None);
    }

    /// Criterion ④ of acceptance run E1 is read on `occupation_ms`, and nothing
    /// else carries it. This test guards the instrument itself: a
    /// forgotten delegation — `Duration::ZERO` returned without looking at the queue —
    /// would read a zero buffer latency on a full buffer, and the
    /// criterion would pass without having measured anything.
    #[test]
    fn l_occupation_du_lecteur_est_celle_des_trames_en_attente() {
        let mut lecteur = LecteurMicro::new().expect("Opus decoder");
        assert_eq!(lecteur.occupation(), Duration::ZERO, "when empty");

        // Three 20 ms frames: 960 samples per channel each.
        for i in 0..3u64 {
            lecteur.deposer(TrameMicro {
                opus: vec![0u8; 8],
                rtp_48k: i * 960,
                echantillons: 960,
            });
        }
        assert_eq!(
            lecteur.occupation(),
            Duration::from_millis(60),
            "three 20 ms frames make 60 ms of waiting"
        );
    }
}
