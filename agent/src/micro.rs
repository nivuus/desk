//! The UPSTREAM direction of the microphone, on the agent side: what arrives from the browser in
//! Opus, put back in order, rid of its duplicates, latency-bounded,
//! decoded, and made ready to play.
//!
//! **This module is PURE: it never references the `windows` crate and opens
//! no device.** It is the dividing line of spec §6, and it is
//! what makes testable under Linux everything that can go wrong — ordering,
//! jitter, drift, decoding, silence. Block E2 will only have to
//! wake a WASAPI thread and call `LecteurMicro::remplir`.
//!
//! ⚠️ **It does DECODE, however, and the header said the opposite until the
//! closing of work stream E**: `LecteurMicro` owns the `OpusDecoder` (l. 278).
//! It is not a breach of purity — libopus knows neither Windows nor
//! devices —, and it is even what makes decoding, PLC and FEC
//! testable under Linux. The false sentence dated from task 4, which only had
//! the buffer; task 6 added the decoder without rereading it.
//!
//! **The module's invariant**: `deposer` returns nothing and can therefore never
//! make the transport loop wait. It is the exact mirror of work stream A's
//! rule — the loop drops off, a dedicated thread works.

use std::collections::VecDeque;
use std::time::Duration;

use crate::opus::{OpusDecoder, CHANNELS, SAMPLE_RATE_HZ};

/// Target occupancy of the buffer: the latency / jitter resistance trade-off.
///
/// ⚠️ **NOT CALIBRATED.** It joins `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS` and the others: no listening judgement has ever been
/// made on a constant of this repository.
pub const CIBLE: Duration = Duration::from_millis(40);

/// Occupancy beyond which we throw away rather than accumulate. Without it,
/// a browser that emits faster than the cable consumes would make
/// latency grow without bound — the buffer would become a permanent delay.
pub const PLAFOND: Duration = Duration::from_millis(200);

/// Above, we skip a frame to catch up with drift (task 5).
pub const SEUIL_SAUT: Duration = Duration::from_millis(120);

/// Below, we insert one (task 5).
pub const SEUIL_INSERTION: Duration = Duration::from_millis(20);

/// Une trame Opus telle qu'elle arrive du navigateur.
///
/// `echantillons` is the number of samples PER CHANNEL the packet carries,
/// **read from its header** by `OpusDecoder::echantillons_de` and never assumed
/// (spec §7): Chrome emits 20 ms, work stream A 10 ms, and nothing forces
/// a peer to stick to that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrameMicro {
    pub opus: Vec<u8>,
    pub rtp_48k: u64,
    pub echantillons: usize,
}

/// Everything the buffer has met. **None of these events is
/// silent** (spec §8): each has its counter, and it is what makes an
/// acceptance run readable without instrumenting the code on the fly.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompteursMicro {
    pub deposees: u64,
    pub hors_ordre: u64,
    pub doublons: u64,
    pub jetees_saturation: u64,
    pub jetees_perimees: u64,
    pub sauts: u64,
    pub insertions: u64,
    pub famines: u64,
    pub plc: u64,
    /// Concealments REFUSED because `PLAFOND_DISSIMULATION` was
    /// reached: as many frames returned as SILENCE instead of being
    /// extrapolated.
    ///
    /// ⚠️ **It is disjoint from `plc`, and that is its whole point**: without it,
    /// an acceptance run could not distinguish "concealment is working"
    /// from "the cap has bitten and the sink is silent", and the fix would not
    /// be falsifiable. `plc` counts what was extrapolated,
    /// `plc_plafonnees` what deliberately was not.
    pub plc_plafonnees: u64,
    pub fec: u64,
}

/// What a reading round asks of the decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retrait {
    /// The due frame is there: normal decoding.
    Trame(TrameMicro),
    /// The due frame is missing but the NEXT one is there: decode with FEC.
    ///
    /// The direction is counter-intuitive and worth repeating: the LBRR redundancy
    /// of a packet reconstructs the frame that PRECEDES it. Hence the rule — FEC
    /// is only useful if the next one has already arrived, and never otherwise.
    Reconstruire { suivante: Vec<u8> },
    /// Nothing to play. **THREE outcomes, not two**: concealment if a frame has
    /// already been decoded AND the budget of `micro/dissimulation.rs` is not
    /// exhausted; silence in the two other cases — nothing has ever been decoded,
    /// or `PLAFOND_DISSIMULATION` is reached (counter `plc_plafonnees`).
    /// ⚠️ The third is NEW: without it, a prolonged starvation concealed
    /// endlessly and produced a drone, measured over 60 s by acceptance run E1.
    Manquante,
}

/// Converts a number of samples per channel into a duration at 48 kHz.
fn duree_de(echantillons: usize) -> Duration {
    Duration::from_nanos(echantillons as u64 * 1_000_000_000 / SAMPLE_RATE_HZ as u64)
}

/// The jitter buffer: it orders, deduplicates, bounds, and returns what is due.
pub struct TamponGigue {
    file: VecDeque<TrameMicro>,
    /// RTP timestamp of the frame expected at the next removal. `None` as long
    /// as no frame has been played: the first one that shows up serves as
    /// reference, rather than an arbitrary zero a browser has no
    /// reason to use (Chrome picks its initial RTP timestamp at random).
    prochain_du: Option<u64>,
    /// TARGET occupancy. Kept for the reading of a future fine
    /// control loop; today's drift correction only uses the two
    /// thresholds, which bound the dead band around it.
    #[allow(dead_code)]
    cible: Duration,
    plafond: Duration,
    compteurs: CompteursMicro,
}

impl TamponGigue {
    pub fn new(cible: Duration, plafond: Duration) -> Self {
        Self {
            file: VecDeque::new(),
            prochain_du: None,
            cible,
            plafond,
            compteurs: CompteursMicro::default(),
        }
    }

    /// **NON-BLOCKING, and it is the invariant of this module.** No return
    /// value, no `Result`: the transport loop structurally cannot
    /// wait here, nor have to decide anything at all.
    pub fn deposer(&mut self, trame: TrameMicro) {
        self.compteurs.deposees += 1;

        // Stale: its place has already passed. Playing it out of turn
        // would disorder the timeline we are precisely here to keep.
        if let Some(du) = self.prochain_du {
            if trame.rtp_48k < du {
                self.compteurs.jetees_perimees += 1;
                return;
            }
        }

        // Duplicate: the same RTP timestamp is already queued. A
        // retransmission or a replay must not be played twice.
        if self.file.iter().any(|t| t.rtp_48k == trame.rtp_48k) {
            self.compteurs.doublons += 1;
            return;
        }

        // ORDERED insertion. A simple `push_back` would be enough for the nominal case —
        // and that is exactly what decision 2 of the plan forbids us to
        // assume: by bringing `reordering_size_audio` from 15 down to 2, we take away
        // from str0m the ordering guarantee it offered, and it is here that it is
        // made up for.
        let place = self
            .file
            .iter()
            .position(|t| t.rtp_48k > trame.rtp_48k)
            .unwrap_or(self.file.len());
        if place != self.file.len() {
            self.compteurs.hors_ordre += 1;
        }
        self.file.insert(place, trame);

        // Saturation: it is the OLDEST that goes, not the most recent.
        // The reverse of work stream A's emission, and for an exact reason —
        // there we choose what to send, here we undergo a remote timeline
        // that must be followed by moving forward, never backward.
        while self.occupation() > self.plafond {
            let Some(partie) = self.file.pop_front() else {
                break;
            };
            self.compteurs.jetees_saturation += 1;
            // We jumped over it: the next due becomes what remains, otherwise
            // the next round would claim through FEC the frame we just
            // deliberately threw away.
            if self.prochain_du.is_none_or(|du| du <= partie.rtp_48k) {
                self.prochain_du = self.file.front().map(|t| t.rtp_48k);
            }
        }
    }

    /// What is due now. Never blocks and always returns something:
    /// failing a frame, a concealment instruction.
    pub fn retirer(&mut self) -> Retrait {
        // `_tete`: the `let … else` only exists for its `else` arm — the
        // value is read again below, after drift correction. The
        // underscore closes legacy item no. 4 of E1, the only warning of the crate that was
        // not a `dead_code`.
        let Some(_tete) = self.file.front().map(|t| t.rtp_48k) else {
            self.compteurs.famines += 1;
            return Retrait::Manquante;
        };

        // --- drift correction (spec §8) ---------------------------------
        //
        // Two free clocks cross: that of the browser that encodes and
        // that of the cable that consumes. Nothing slaves one to the other, and
        // the gap, however small, accumulates endlessly.
        //
        // The remedy is CRUDE and assumed: we skip a frame when we are too
        // late, we insert one when we are too early. "Audible once
        // every several minutes" — adaptive resampling would be
        // work written before having observed the need.
        //
        // ⚠️ **The two thresholds form a HYSTERESIS, and its absence would make
        // the buffer oscillate at each frame**: without a dead band between them, the
        // correction that catches up a delay would immediately create the advance that
        // the other correction would come to undo.
        let occupation = self.occupation();
        if occupation > SEUIL_SAUT {
            let saute = self.file.pop_front().expect("tête relue");
            self.compteurs.sauts += 1;
            // Same reason as for saturation: without advancing the due, the next
            // round would claim through FEC the frame we just deliberately
            // skipped, and the skip would be a disguised no-op.
            if self.prochain_du.is_none_or(|du| du <= saute.rtp_48k) {
                self.prochain_du = self.file.front().map(|t| t.rtp_48k);
            }
            let Some(_) = self.file.front() else {
                self.compteurs.famines += 1;
                return Retrait::Manquante;
            };
        } else if occupation < SEUIL_INSERTION {
            // ⚠️ **An insertion does NOT CONSUME a frame.** It returns
            // `Manquante` without popping, which lets the occupancy grow
            // up to the dead band. A `pop` accompanied by an insertion
            // would be a disguised no-op — the occupancy would not move by one
            // sample, and the hysteresis test would not even see it.
            self.compteurs.insertions += 1;
            return Retrait::Manquante;
        }

        let tete = self.file.front().expect("tête relue").rtp_48k;

        let du = *self.prochain_du.get_or_insert(tete);

        if tete > du {
            // The due frame is missing, but the next one is there: that is exactly
            // the condition — and the only one — where in-band FEC can work.
            self.compteurs.fec += 1;
            let suivante = self.file.front().expect("tête relue").opus.clone();
            // We advance to the next one WITHOUT consuming it: it plays in
            // its turn. The hole is filled by its redundancy, not by it.
            self.prochain_du = Some(tete);
            return Retrait::Reconstruire { suivante };
        }

        let trame = self.file.pop_front().expect("tête relue");
        self.prochain_du = Some(trame.rtp_48k + trame.echantillons as u64);
        Retrait::Trame(trame)
    }

    /// Duration of audio currently waiting.
    pub fn occupation(&self) -> Duration {
        duree_de(self.file.iter().map(|t| t.echantillons).sum())
    }

    pub fn compteurs(&self) -> CompteursMicro {
        self.compteurs
    }
}

/// Puits d'un flux montant.
///
/// `deposer` returns `false` when the sink REFUSES — exclusivity not acquired
/// (block E2, a machine-wide named mutex). The caller logs
/// once and does not insist. **The pure layer knows no mutex**: this
/// trait is the seam, and nothing more.
pub trait PuitsMicro {
    fn deposer(&mut self, trame: TrameMicro) -> bool;
}

/// The jitter buffer, the decoder, and the residue: everything a WASAPI thread
/// will need to call, and nothing more.
///
/// **Pure, although it serves a WASAPI thread** — it is the dividing line of
/// spec §6, and it paid off: block E2 calls exactly
/// `remplir(&mut [f32])` from the thread WASAPI wakes, and **everything that
/// can go wrong — ordering, jitter, drift, decoding, residue,
/// silence — is tested under Linux.**
pub struct LecteurMicro {
    tampon: TamponGigue,
    decodeur: OpusDecoder,
    /// Decoded PCM not yet handed to the caller, interleaved stereo.
    ///
    /// **Without it, the tail of each frame would be thrown away.** The packet
    /// WASAPI claims is almost never the size of an Opus frame: a
    /// 20 ms frame yields 960 samples per channel, and the claimed buffer
    /// may want 441, 480 or 1024 of them. The residue is the piece that glues
    /// two unrelated splittings back together.
    residu: VecDeque<f32>,
    /// What remains to be concealed before we go silent. See
    /// `micro/dissimulation.rs` for the measured defect that made it
    /// necessary, and for what libopus does — and does not do — on its side.
    budget: BudgetDissimulation,
}

impl LecteurMicro {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            tampon: TamponGigue::new(CIBLE, PLAFOND),
            decodeur: OpusDecoder::new()?,
            residu: VecDeque::new(),
            budget: BudgetDissimulation::new(PLAFOND_DISSIMULATION),
        })
    }

    pub fn deposer(&mut self, trame: TrameMicro) {
        self.tampon.deposer(trame);
    }

    pub fn compteurs(&self) -> CompteursMicro {
        self.tampon.compteurs()
    }

    /// Duration of audio dropped off and not yet returned — that is, the latency
    /// the jitter buffer ADDS, by itself.
    ///
    /// ⚠️ **It is NOT the end-to-end latency**, which this work stream does not measure
    /// any more than the previous ones: the network path, the browser's encoding
    /// and — in block E2 — writing to the cable are absent from it. It is the
    /// "drop-off → removal" bound of acceptance run E1, and nothing more. Bounded by
    /// construction to `PLAFOND` (see `TamponGigue::deposer`).
    pub fn occupation(&self) -> Duration {
        self.tampon.occupation()
    }

    /// Fills `sortie` (interleaved stereo, `f32`) with what is due, completes
    /// with silence, and **NEVER blocks**.
    ///
    /// Spec §8 "Silence": the cable must be fed CONTINUOUSLY. An
    /// application listening to an empty buffer does not perceive silence — it
    /// sees a stream that gets interrupted, which is not the same thing and
    /// can be heard.
    pub fn remplir(&mut self, sortie: &mut [f32]) {
        let mut ecrit = 0;
        while ecrit < sortie.len() {
            // The residue first: it is what glues the splittings back together.
            while ecrit < sortie.len() {
                let Some(e) = self.residu.pop_front() else {
                    break;
                };
                sortie[ecrit] = e;
                ecrit += 1;
            }
            if ecrit == sortie.len() {
                return;
            }

            // Nothing in reserve: ask the buffer for something to continue with.
            if !self.produire_une_trame() {
                // Nothing more to produce, not even a concealment. We
                // complete with silence and return — **the loop MUST
                // stop here**: without this exit, a player that has
                // never decoded anything would spin endlessly, `dissimuler` returning
                // zero samples at each round.
                sortie[ecrit..].fill(0.0);
                return;
            }
        }
    }

    /// Decodes what is due into the residue. Returns `false` when nothing could
    /// be produced — up to the caller to complete with silence.
    fn produire_une_trame(&mut self) -> bool {
        let (paquet, echantillons, fec) = match self.tampon.retirer() {
            Retrait::Trame(t) => (t.opus, t.echantillons, false),
            Retrait::Reconstruire { suivante } => {
                // The reconstructed duration is that of the MISSING frame, which we
                // do not know. That of the next one is the best available
                // witness of it, and it is READ from the packet, never assumed.
                let n = self.decodeur.echantillons_de(&suivante).unwrap_or(0);
                if n == 0 {
                    return false;
                }
                (suivante, n, true)
            }
            Retrait::Manquante => {
                // Concealment: the duration comes from the last decoded frame,
                // and is zero as long as nothing has been decoded — in which case there
                // is nothing to conceal, and silence is the right answer.
                let n = self.decodeur.derniere_duree().unwrap_or(0);
                if n == 0 {
                    return false;
                }
                // ⚠️ **THE CAP.** Beyond `PLAFOND_DISSIMULATION`
                // concealed in a row, we return SILENCE: libopus never
                // stops by itself and converges towards comfort noise
                // that it maintains endlessly — measured as a continuous
                // drone over 60 s of browser silence. `false` makes
                // `remplir` complete with silence, which it already knows how to do.
                if !self.budget.consommer(duree_de(n)) {
                    self.tampon.compteurs.plc_plafonnees += 1;
                    return false;
                }
                let mut pcm = vec![0i16; n * CHANNELS];
                let Ok(rendus) = self.decodeur.dissimuler(&mut pcm) else {
                    return false;
                };
                self.pousser(&pcm[..rendus * CHANNELS]);
                self.tampon.compteurs.plc += 1;
                return rendus > 0;
            }
        };

        let mut pcm = vec![0i16; echantillons * CHANNELS];
        let rendus = if fec {
            self.decodeur.decoder_fec(&paquet, &mut pcm)
        } else {
            self.decodeur.decoder(&paquet, &mut pcm)
        };
        let Ok(rendus) = rendus else {
            return false;
        };
        // Real audio has come back — reconstructed by FEC or decoded as
        // is: the concealment budget starts whole again. Without this line,
        // a cap reached once would condemn the session to permanent
        // silence.
        if rendus > 0 {
            self.budget.trame_reelle();
        }
        self.pousser(&pcm[..rendus * CHANNELS]);
        rendus > 0
    }

    fn pousser(&mut self, pcm: &[i16]) {
        self.residu.extend(pcm.iter().map(|&e| e as f32 / 32_768.0));
    }
}

/// The dominant frequency of a periodic signal, through zero crossings.
///
/// ⚠️ **Extracted to `micro/frequence.rs`** under the 500-line
/// rule, and re-exported here so that no call site moves: it remains
/// `crate::micro::frequence_par_passages_a_zero` for `demarrage/micro.rs`
/// as for the tests.
pub use frequence::frequence_par_passages_a_zero;

mod frequence;

/// The cable exclusivity policy: a single writer, the attempt
/// redone at each drop-off, and a log that does not repeat itself.
///
/// ⚠️ **`pub mod`, and NOT the `pub use` + `mod` that plan E2 prescribes
/// (task 3, "Files"), and it is an assumed divergence.** Re-exporting three
/// names no one consumes yet — tasks 7 to 10 will wire them —
/// raises an `unused_imports`, that is a warning of a **new
/// category**, whereas the check of this same task requires all
/// remaining warnings to be `dead_code`s. The plan contradicts itself on this
/// point; we keep its check and follow the pattern of `wasapi.rs`
/// (`pub mod rendu;`, `pub mod process_loopback;`). Call sites will write
/// `crate::micro::exclusivite::Exclusivite`.
pub mod exclusivite;

/// The local loop guard: is the cable what the loopback captures?
/// **Measured necessary on 20 August 2026** — spec §3 asserted the opposite.
/// `pub mod` for the reason written just above.
pub mod boucle_locale;

/// The concealment cap, and the pure rule that holds it.
pub use dissimulation::{BudgetDissimulation, PLAFOND_DISSIMULATION};

mod dissimulation;

#[cfg(test)]
#[path = "micro/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "micro/tests_lecteur.rs"]
mod tests_lecteur;
