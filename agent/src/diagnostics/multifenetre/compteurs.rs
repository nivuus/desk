//! The bench's metrology, shared by its two protocols.
//!
//! Extracted from `banc.rs` when a second protocol appeared (the
//! multi-output set-up of `paralleles.rs`). Sharing is not a writing
//! convenience: it is what makes the figures of both benches comparable. A
//! second bench with its own counting loop would diverge, and we would
//! no longer know whether a frame rate gap comes from the measured path or from the bench that
//! measures it.
//!
//! What is NOT here: the staging of the covering and the elimination
//! gate (specific to the single-output protocol, left in `banc.rs`), and
//! the rotation of the check (specific to the multi-output protocol, in
//! `paralleles.rs`).

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::capture::CapturedFrame;
use crate::mire;

use super::mires::Mires;
use super::voies::VoieDeCapture;
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// Duration of each pass.
pub(super) const DUREE_PASSE: Duration = Duration::from_secs(10);
/// Logging cadence of the counters.
pub(super) const PERIODE_JOURNAL: Duration = Duration::from_secs(1);
/// Beat cadence of the virtual display driver's watchdog.
///
/// One second is a third of `delai = 3` read in seconds, the most
/// unfavourable reading of this field of unknown unit (same reasoning as
/// `montee::CADENCE_PING`). The guard does not change nature depending on the pass that
/// is running: a single value for all.
const CADENCE_PING: Duration = Duration::from_secs(1);

/// Beats the driver's watchdog at a fixed cadence — **and measures what it
/// really beat**.
///
/// The second role is not decorative. Round 1 dated in the log a gap
/// of 11.1 s without a single ping (control pass and opening of the duplications),
/// which nothing in the program reported: pings not being traced, a
/// gap could only be seen by cross-checking by hand the timestamps of neighbouring
/// lines. `intervalle_max` makes this defect OBSERVABLE through a single number, and
/// it is this number that proves — or not — that the gap is closed.
///
/// An aggregated counter, logged once at the end of the measurement: tracing each
/// ping would give one line per second for nothing.
pub(super) struct Garde<'p> {
    pilote: &'p PiloteParIoctl,
    dernier: Instant,
    intervalle_max: Duration,
}

impl<'p> Garde<'p> {
    /// To be built right after the last known ping — typically on return
    /// from `attendre_en_pinguant`.
    ///
    /// **Wording precision, corrected in the final review**: `dernier` is set to
    /// `Instant::now()` HERE, so the seam between the last real ping and
    /// this construction **escapes the counter** — it is not *counted*,
    /// it is made *negligible* by the adjacency of the two calls (66 µs in the
    /// survey of the parallel duplications work stream). Counting this seam
    /// would require `attendre_en_pinguant` to return the instant of its last ping.
    pub(super) fn nouvelle(pilote: &'p PiloteParIoctl) -> Self {
        Self {
            pilote,
            dernier: Instant::now(),
            intervalle_max: Duration::ZERO,
        }
    }

    /// Beats unconditionally, and records the gap since the previous beat.
    pub(super) fn battre(&mut self) -> Result<()> {
        let maintenant = Instant::now();
        self.intervalle_max = self.intervalle_max.max(maintenant - self.dernier);
        self.pilote.pinguer()?;
        self.dernier = maintenant;
        Ok(())
    }

    /// Beats if the cadence requires it, doing nothing otherwise.
    pub(super) fn battre_si_du(&mut self) -> Result<()> {
        if self.dernier.elapsed() >= CADENCE_PING {
            self.battre()?;
        }
        Ok(())
    }

    /// The largest gap between two beats, **including the one running
    /// since the last**: a gap open at the instant of reading counts
    /// as much as a closed gap, otherwise the last segment of the measurement
    /// would escape the check.
    pub(super) fn intervalle_max(&self) -> Duration {
        self.intervalle_max.max(self.dernier.elapsed())
    }
}

/// Tally of the verdicts given on test pattern 0, by NATURE and not as a lump.
///
/// A mere count of "wrong" is not enough for measurement ③: a BLACK image
/// and an image carrying the pattern ON TOP are two opposite results. The
/// first would say that Windows does not compose a virtual output without an attached
/// screen — and the founding hypothesis of the "one virtual monitor per
/// window" path would fall. The second would say that it composes it perfectly, and that
/// it is the duplication that cannot undo a covering — which we
/// already knew from the physical desktop. Merging them under one counter would make
/// the measurement uninterpretable.
#[derive(Default, Debug)]
pub(super) struct Verdicts {
    pub(super) justes: u64,
    pub(super) voisines: u64,
    pub(super) noires: u64,
    pub(super) inconnues: u64,
}

impl Verdicts {
    pub(super) fn compter(&mut self, verdict: mire::Verdict) {
        match verdict {
            mire::Verdict::Juste => self.justes += 1,
            mire::Verdict::Voisine(_) => self.voisines += 1,
            mire::Verdict::Noire => self.noires += 1,
            mire::Verdict::Inconnue => self.inconnues += 1,
        }
    }

    pub(super) fn faux(&self) -> u64 {
        self.voisines + self.noires + self.inconnues
    }
}

pub(super) struct Compteurs {
    pub(super) images: Vec<u64>,
    pub(super) unites: Vec<u64>,
    /// Verdicts given BEFORE the covering is in place: test pattern 0 is
    /// then unobstructed, and it is the only window of the bench where one reads "does this path
    /// simply capture this window". On the physical desktop the
    /// answer went without saying; on a virtual output, it is the question.
    pub(super) avant_recouvrement: Verdicts,
    /// Verdicts given once test pattern 0 is covered: the elimination gate.
    pub(super) apres_recouvrement: Verdicts,
}

impl Compteurs {
    pub(super) fn nouveaux(nombre: usize) -> Self {
        Self {
            images: vec![0; nombre],
            unites: vec![0; nombre],
            avant_recouvrement: Verdicts::default(),
            apres_recouvrement: Verdicts::default(),
        }
    }
}

/// Control pass: the test patterns paint, nothing captures.
///
/// `garde` beats the driver's watchdog during the pass. `None` for the
/// single-output protocol (`banc.rs`), which holds no driver; `Some` for
/// the multi-output protocol (`paralleles.rs`), whose N outputs live under
/// a watchdog of UNKNOWN unit — **the second is not ruled out**. Without
/// this beat, this pass is a ten-second gap during which the
/// driver can take back its outputs, and the next measurement would capture black
/// without anything saying why (11.1 s gap dated in the log of
/// round 1).
pub(super) fn passe_temoin(mires: &mut Mires, mut garde: Option<&mut Garde<'_>>) -> Result<()> {
    let debut = Instant::now();
    let mut trames = 0u64;
    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        trames += 1;
        if let Some(garde) = garde.as_mut() {
            garde.battre_si_du()?;
        }
    }
    let secondes = debut.elapsed().as_secs_f64();
    tracing::info!(
        mires = mires.nombre(),
        trames,
        cadence = trames as f64 / secondes,
        "passe TÉMOIN — cadence de peinture sans capture"
    );
    Ok(())
}

pub(super) fn journaliser(passe: &str, voie: &str, nombre: u8, compteurs: &Compteurs) {
    let secondes = DUREE_PASSE.as_secs_f64();
    let cadences: Vec<f64> = compteurs
        .images
        .iter()
        .map(|n| *n as f64 / secondes)
        .collect();
    tracing::info!(
        passe,
        voie,
        nombre,
        ?cadences,
        unites = ?compteurs.unites,
        verdicts_faux = compteurs.apres_recouvrement.faux(),
        mire0_avant_recouvrement = ?compteurs.avant_recouvrement,
        mire0_apres_recouvrement = ?compteurs.apres_recouvrement,
        "passe terminée"
    );
}

/// Reads the centre of the image and returns the corresponding verdict.
///
/// `attendu` is the identity of the test pattern that MUST be there. The reading is
/// done on the path's device: a texture cannot be read from a
/// device other than its own.
pub(super) fn lire_verdict(
    voie: &mut dyn VoieDeCapture,
    image: &CapturedFrame,
    attendu: u8,
) -> Result<mire::Verdict> {
    let appareil = voie.device();
    let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
        &appareil,
        &image.texture,
        image.width,
        image.height,
        image.width / 2,
        image.height / 2,
    )?;
    Ok(mire::verdict(attendu, (r, g, b)))
}
