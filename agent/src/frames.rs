//! Splitting the captured PCM stream into 10 ms frames, on a real clock.
//!
//! WASAPI delivers blocks of any length, at irregular instants,
//! and **nothing at all** when no application is playing. Opus, for its part, requires
//! frames of exact size, at a regular cadence. This module bridges the two.
//!
//! The principle: the wall clock dictates the cadence, the buffer provides the
//! content when it has some, and silence otherwise. No frame is ever
//! skipped — it is this continuity that makes the RTCP Sender Reports
//! usable and prevents the browser's jitter buffer from starving.
//!
//! This module never references the `windows` crate: it compiles and is tested
//! under Linux.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::opus::{FRAME_INTERLEAVED, FRAME_SAMPLES, SAMPLE_RATE_HZ};

/// Maximum delay tolerated in the buffer, in frames. Beyond it, the oldest
/// samples are thrown away: if WASAPI durably delivers faster than
/// real time, the buffer would grow endlessly and latency with it.
const MAX_BACKLOG_FRAMES: usize = 3;

/// A PCM frame ready to encode.
#[derive(Debug, Clone)]
pub struct Frame {
    /// `FRAME_INTERLEAVED` interleaved samples (left, right, ...).
    pub pcm: Vec<i16>,
    /// Presentation timestamp, in samples since the origin.
    pub pts_48k: u64,
    /// Real instant corresponding to `pts_48k`.
    pub captured_at: Instant,
}

/// Assembles irregular PCM blocks into regular 10 ms frames.
pub struct FrameAssembler {
    origin: Instant,
    tampon: VecDeque<i16>,
    /// Samples **per channel** already emitted since the origin. `None` as long as
    /// the assembler has not anchored itself (see `drain_due`).
    emis_par_canal: Option<u64>,
    complements: u64,
    echantillons_jetes: u64,
}

impl FrameAssembler {
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            tampon: VecDeque::new(),
            emis_par_canal: None,
            complements: 0,
            echantillons_jetes: 0,
        }
    }

    /// Adds stereo interleaved samples.
    ///
    /// Bounds the accumulated delay: beyond `MAX_BACKLOG_FRAMES` pending
    /// frames, the oldest samples are thrown away rather than letting
    /// latency grow indefinitely.
    pub fn push(&mut self, pcm: &[i16]) {
        self.tampon.extend(pcm.iter().copied());

        let plafond = MAX_BACKLOG_FRAMES * FRAME_INTERLEAVED;
        if self.tampon.len() > plafond {
            let excedent = self.tampon.len() - plafond;
            self.tampon.drain(..excedent);
            self.echantillons_jetes += excedent as u64;
        }
    }

    /// Returns all the frames due at instant `maintenant`.
    ///
    /// The **first** call returns nothing: it anchors the assembler on the time
    /// already elapsed since the origin. Without this anchoring, a 500 ms WASAPI
    /// initialisation would produce 50 frames of silence at once.
    pub fn drain_due(&mut self, maintenant: Instant) -> Vec<Frame> {
        let ecoules = Self::echantillons_ecoules(self.origin, maintenant);

        let emis = match self.emis_par_canal {
            Some(emis) => emis,
            None => {
                self.emis_par_canal = Some(ecoules);
                return Vec::new();
            }
        };

        let mut sorties = Vec::new();
        let mut position = emis;
        while position + FRAME_SAMPLES as u64 <= ecoules {
            sorties.push(self.former_trame(position));
            position += FRAME_SAMPLES as u64;
        }
        self.emis_par_canal = Some(position);
        sorties
    }

    /// Re-anchors the assembler: the next `drain_due` will start again with a
    /// silent anchoring, exactly as on the very first call.
    ///
    /// **To be called at each resumption after a feed interruption**, for
    /// example a disabling then re-enabling of the capture (per-window audio,
    /// sub-block D7). Without it, `emis_par_canal` stays frozen at the
    /// position from before the interruption while the wall clock, for its part,
    /// keeps advancing: the first `drain_due` following the resumption
    /// would then return one frame per 10 ms slice of the WHOLE interruption
    /// all at once — silence-completed since nothing was pushed
    /// during the interruption — instead of a single due frame as in the
    /// nominal case. It is the same burst as the one the lazy anchoring
    /// of the first call already protects against (see
    /// `s_ancre_sur_le_temps_ecoule_plutot_que_d_emettre_une_rafale`); it
    /// is simply not the same occasion to trigger it.
    pub fn reancrer(&mut self) {
        self.emis_par_canal = None;
    }

    /// Number of frames that had to be completed with silence.
    pub fn complements(&self) -> u64 {
        self.complements
    }

    /// Number of interleaved samples thrown away for excessive delay.
    pub fn echantillons_jetes(&self) -> u64 {
        self.echantillons_jetes
    }

    /// Forms a frame from the buffer, completed with silence if the latter
    /// does not have enough to fill it.
    ///
    /// The silence is added **at the end** of the frame; the samples that
    /// arrive afterwards resume at the next frame. The result is a
    /// tiny discontinuity, far preferable to a hole in the
    /// timeline.
    fn former_trame(&mut self, pts_par_canal: u64) -> Frame {
        let disponibles = self.tampon.len().min(FRAME_INTERLEAVED);
        let mut pcm: Vec<i16> = self.tampon.drain(..disponibles).collect();
        if pcm.len() < FRAME_INTERLEAVED {
            pcm.resize(FRAME_INTERLEAVED, 0);
            self.complements += 1;
        }
        Frame {
            pcm,
            pts_48k: pts_par_canal,
            captured_at: self.origin + Self::duree_de(pts_par_canal),
        }
    }

    /// Samples per channel elapsed between `origin` and `maintenant`.
    fn echantillons_ecoules(origin: Instant, maintenant: Instant) -> u64 {
        let ecoule = maintenant.saturating_duration_since(origin);
        // In 128 bits: `as_nanos() * 48_000` would overflow a u64 after
        // ~4 days of session. Same precaution as `next_pts_90k` on the video side.
        (ecoule.as_nanos() * SAMPLE_RATE_HZ as u128 / 1_000_000_000) as u64
    }

    /// Duration corresponding to `echantillons` samples per channel.
    fn duree_de(echantillons: u64) -> Duration {
        Duration::from_nanos((echantillons as u128 * 1_000_000_000 / SAMPLE_RATE_HZ as u128) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::opus::CHANNELS;

    /// Duration corresponding to `n` 10 ms frames.
    fn trames(n: u64) -> Duration {
        Duration::from_millis(n * 10)
    }

    /// `n` interleaved samples all equal to `v`.
    fn bloc(v: i16, n: usize) -> Vec<i16> {
        vec![v; n * CHANNELS]
    }

    #[test]
    fn emits_nothing_before_the_first_frame_is_due() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.push(&bloc(100, FRAME_SAMPLES));
        // Anchoring at the first call, then 5 ms later: half a frame.
        assert!(a.drain_due(origine + trames(1)).is_empty());
        assert!(a
            .drain_due(origine + trames(1) + Duration::from_millis(5))
            .is_empty());
    }

    #[test]
    fn emet_une_trame_pleine_quand_les_echantillons_sont_la() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine); // anchoring at 0
        a.push(&bloc(100, FRAME_SAMPLES));

        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1);
        assert_eq!(sorties[0].pcm.len(), FRAME_INTERLEAVED);
        assert!(sorties[0].pcm.iter().all(|&v| v == 100));
        assert_eq!(sorties[0].pts_48k, 0);
        assert_eq!(a.complements(), 0);
    }

    #[test]
    fn les_horodatages_avancent_de_480_sans_trou() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, FRAME_SAMPLES * 3));

        let sorties = a.drain_due(origine + trames(3));
        let pts: Vec<u64> = sorties.iter().map(|f| f.pts_48k).collect();
        assert_eq!(pts, vec![0, 480, 960]);
    }

    #[test]
    fn fills_with_silence_when_nothing_arrives_and_skips_no_frame() {
        // It is the central property: without it, a silent desktop
        // would dig a hole in the RTP timeline, and the browser's jitter
        // buffer would starve before brutally
        // resynchronising — the exact signature of the defect noted in the
        // milestone 1 acceptance run on video.
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);

        let sorties = a.drain_due(origine + trames(3));
        assert_eq!(sorties.len(), 3);
        assert!(sorties.iter().all(|f| f.pcm.iter().all(|&v| v == 0)));
        assert_eq!(
            sorties.iter().map(|f| f.pts_48k).collect::<Vec<_>>(),
            vec![0, 480, 960]
        );
        assert_eq!(a.complements(), 3);
    }

    #[test]
    fn une_trame_partielle_est_completee_par_du_silence_en_fin() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, 200)); // 200 samples out of 480

        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1);
        let pcm = &sorties[0].pcm;
        assert!(pcm[..200 * CHANNELS].iter().all(|&v| v == 100));
        assert!(pcm[200 * CHANNELS..].iter().all(|&v| v == 0));
        assert_eq!(a.complements(), 1);
    }

    #[test]
    fn l_instant_de_capture_se_deduit_de_l_horodatage() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);
        a.push(&bloc(100, FRAME_SAMPLES * 2));

        let sorties = a.drain_due(origine + trames(2));
        assert_eq!(sorties[0].captured_at, origine);
        assert_eq!(sorties[1].captured_at, origine + Duration::from_millis(10));
    }

    #[test]
    fn s_ancre_sur_le_temps_ecoule_plutot_que_d_emettre_une_rafale() {
        // The clock origin is created before the initialisation of WASAPI, which
        // takes a non-zero time. Without lazy anchoring, the first call
        // would produce at once all the frames elapsed since the origin.
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);

        let sorties = a.drain_due(origine + Duration::from_millis(500));
        assert!(
            sorties.is_empty(),
            "le premier appel ancre, il ne rattrape pas : {} trames émises",
            sorties.len()
        );

        // And the timeline does restart from the elapsed position, not from zero.
        let suivantes = a.drain_due(origine + Duration::from_millis(510));
        assert_eq!(suivantes.len(), 1);
        assert_eq!(suivantes[0].pts_48k, 50 * FRAME_SAMPLES as u64);
    }

    #[test]
    fn se_reancre_sur_le_temps_ecoule_plutot_que_de_rattraper_une_coupure() {
        // The counterpart of `s_ancre_sur_le_temps_ecoule_plutot_que_d_emettre_une_rafale`
        // for an interruption DURING THE LIFETIME rather than at construction: a
        // disabling then re-enabling of the capture (per-window audio,
        // sub-block D7) lets the wall clock advance without anything being
        // pushed. Without `reancrer()`, the first `drain_due` following the
        // resumption would return one silence frame per 10 ms slice of the whole
        // interruption, in a single burst.
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine); // ancrage initial

        // Long interruption: far more than a single 10 ms frame, and nothing
        // is pushed during this period.
        let longue_coupure = origine + Duration::from_secs(30);
        a.reancrer();
        let sorties = a.drain_due(longue_coupure);
        assert!(
            sorties.is_empty(),
            "le premier drain_due après reancrer() ancre, il ne rattrape pas : {} trames émises",
            sorties.len()
        );

        // And the timeline restarts from the elapsed position at resumption,
        // not from zero nor from the cumulated interruption.
        let suivante = a.drain_due(longue_coupure + trames(1));
        assert_eq!(suivante.len(), 1);
    }

    #[test]
    fn jette_les_echantillons_les_plus_anciens_au_dela_du_retard_tolere() {
        let origine = Instant::now();
        let mut a = FrameAssembler::new(origine);
        a.drain_due(origine);

        // MAX_BACKLOG_FRAMES = 3, FRAME_INTERLEAVED = 960
        // Plafond = 2880

        // First push: well beyond the cap
        a.push(&bloc(100, 2000));
        // Buffer: 4000, cap: 2880, excess: 1120
        // Thrown away: 1120
        assert_eq!(
            a.echantillons_jetes(),
            1120,
            "première poussée jette 1120 échantillons (4000 - 2880)"
        );

        // Second push: beyond again, with a different value
        a.push(&bloc(-100, 1600));
        // Buffer before: 2880 (100s), after extend: 6080
        // Excess: 3200, thrown away cumulatively: 1120 + 3200 = 4320
        // Buffer remains: 2880 (the most recent -100s)
        assert_eq!(
            a.echantillons_jetes(),
            4320,
            "deuxième poussée jette les 3200 anciens (100s) du tampon"
        );

        // The emitted frame must contain the -100s (the most recent kept),
        // not the 100s (the oldest, now thrown away).
        let sorties = a.drain_due(origine + trames(1));
        assert_eq!(sorties.len(), 1, "une seule trame est due après 10 ms");
        assert!(
            sorties[0].pcm.iter().all(|&v| v == -100),
            "la trame doit contenir les données les plus récentes (-100), pas les anciennes (100)"
        );
    }
}
