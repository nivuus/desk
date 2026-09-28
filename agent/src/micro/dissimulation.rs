//! The concealment CAP: beyond so many milliseconds concealed
//! in a row, we return SILENCE rather than an extrapolated frame.
//!
//! **PURE, no `cfg`, no decoder.** The rule is tested on the host without
//! libopus or device: it only knows durations.
//!
//! # Why this cap exists — a MEASURED defect, not a caveat
//!
//! Acceptance run E1 (task 14, work stream E) noted, during **60 s of silence**
//! from the browser — its nominal DTX, `packetsSent` frozen —, a "micro
//! mesuré" trace that carried second after second:
//!
//! ```text
//! plc = 50/s for 60 s            starvations = 50/s
//! peak        0.53 to 0.67       (not a silence)
//! frequency_hz  wandering between 308 and 393 Hz
//! ```
//!
//! In other words: **concealment produced a continuous drone.** In block
//! E2 this drone would come out on the virtual cable, hence into the Windows
//! application — a user staying silent would make a humming heard.
//! Evidence: `docs/superpowers/plans/journaux-micro/agent-silence-520-plat.log`.
//!
//! # It is NOT a libopus defect, and that is what makes the cap ours
//!
//! Read in the source vendored by `audiopus_sys` 0.2.2, and not from memory:
//!
//! - `opus/celt/celt_decoder.c:537` —
//!   `noise_based = loss_count >= 5 || start != 0 || st->skip_plc;`: from the
//!   **6th consecutive loss**, CELT abandons pitch-based extrapolation and
//!   switches to **noise**. Past that point, what we return is no longer an
//!   extrapolation of the speaker's voice, it is synthesised noise.
//! - `celt_decoder.c:562` and `:566` — energy decreases by 0.5 dB per lost
//!   frame, **but bounded from below**:
//!   `MAX16(backgroundLogE[...], oldBandE[...] - decay)`. It therefore converges
//!   towards the **background noise floor and stays there**. It is
//!   comfort noise generation, and it is deliberate: **libopus NEVER stops
//!   by itself.** It is exactly what the 0.6 peak held for
//!   sixty seconds measures.
//! - `celt_decoder.c:1149-1152` — beyond the **10th** consecutive loss,
//!   libopus changes regime and its comment names the situation: "*when
//!   we're in DTX*". The library itself stops treating the hole as
//!   a loss.
//! - `opus/silk/PLC.h:36` and `silk/PLC.c:250-254` — on the SILK side, the per-frame
//!   attenuation is **saturated** from the 2nd loss (`NB_ATT = 2`,
//!   `silk_min_int(NB_ATT - 1, lossCnt)`).
//!
//! **Bounding the concealed duration is therefore up to US**, and no one else.
//!
//! # Network loss and sender silence: the same bound, for lack of being able
//! # to distinguish them
//!
//! Two causes produce the same missing frame, and they do not have the same
//! meaning:
//!
//! - a **network loss** is short (a few frames), and concealment
//!   exists precisely for it;
//! - a **sender silence** (Chrome's DTX) is potentially
//!   INFINITE: the browser simply stops emitting.
//!
//! ⚠️ **THERE ARE TWO SILENCES, AND ACCEPTANCE RUN E2 MET THE SECOND ONE** (task
//! 12, criterion ④, 20 August 2026). This module describes E1's: the track's SOURCE
//! stops (`OscillatorNode` off), Chrome has nothing left to encode,
//! `packetsSent` freezes — and that is indeed DTX. E2 measured the other: a
//! **live capture device producing digital silence** (the silent WAV
//! of the fake device). There, **Chrome does NOT engage DTX** — it
//! emits **50 packets/s without interruption**, hence `plc=0` **AND**
//! `plc_plafonnees=0`: no frame is missing, and there is nothing to conceal.
//!
//! **This refutes nothing of the above** — the two surveys bear on
//! different situations, and the cap remains necessary for the first.
//! But it **bounds** the scope of the word "silence" in this file, and it
//! refutes the "`plc_plafonnees` runs" half of criterion ④ as plan E2
//! wrote it. **The cap was indeed seen running, on a STOPPED track**
//! (`plc_plafonnees=100/s`), and the cable stayed at 3.1 × 10⁻⁵ there where E1
//! noted a peak of 0.53 to 0.67.
//!
//! **The code cannot distinguish them, and it is structural.** What reaches
//! `TamponGigue` is the absence of an RTP packet; nothing, in what
//! `transport/piste_micro.rs` receives, says "I am going silent" — a sender in
//! DTX does not send a message, it sends *nothing*, and the absence is the same
//! on both sides. An RTP marker for speech resumption, supposing we
//! read it, would only arrive at the END of the silence, never during it.
//!
//! **The same bound therefore serves both, and it is the right trade-off**: under the
//! cap we conceal, which network loss needs; beyond it we go silent,
//! which sender silence requires. The cause does not have to be known —
//! only the duration during which we extrapolate with nothing to lean on
//! matters, and it is the same whatever the reason.

use std::time::Duration;

/// Maximum duration of CONSECUTIVE concealment. Beyond it, the sink returns
/// silence until a real frame comes back.
///
/// ⚠️ **NOT CALIBRATED.** No listening judgement has been made on it, and
/// in that it joins `CIBLE`, `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `MAX_OUTPUT_SIZE` and `AUDIO_PERIPHERIQUE`: this repository
/// names its uncalibrated constants rather than suggesting a
/// measured setting.
///
/// **What the ORDER OF MAGNITUDE rests on**, for lack of calibration: the
/// marker `st->loss_count < 10` of `celt_decoder.c:1152`, where libopus
/// itself names the situation "DTX". Chrome emits 20 ms frames; ten
/// of them make **200 ms**, and that is the value retained. With
/// 10 ms frames — work stream A's cadence — the same cap lets twenty through:
/// that is why the constant is a **DURATION and not a frame count**,
/// the duration of a frame being read from the packet and never assumed (spec §7).
///
/// ⚠️ **It is the same number as `micro::PLAFOND`, and it is a
/// COINCIDENCE, not a derivation.** The two answer different
/// questions — how much audio we accept to accumulate, how long we
/// accept to extrapolate — and **they must not be made to follow one
/// another**. The repository already wrote this rule for `PERIODE_STYLE` and
/// `PERIODE_REARBITRAGE`.
///
/// **The direction a future calibration should explore is
/// TIGHTENING**: the marker of `celt_decoder.c:537` — the 6th loss, that is
/// 120 ms at 20 ms per frame — is the one where concealment stops being an
/// extrapolation of the signal and becomes noise. Going silent from there is
/// defensible; it is a listening judgement that must decide, not reasoning.
pub const PLAFOND_DISSIMULATION: Duration = Duration::from_millis(200);

/// What remains of the concealment budget before we go silent.
///
/// **A DURATION counter, reset to zero by any real frame.** It is the
/// whole rule, and it fits in two methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetDissimulation {
    plafond: Duration,
    consecutif: Duration,
}

impl BudgetDissimulation {
    pub fn new(plafond: Duration) -> Self {
        Self {
            plafond,
            consecutif: Duration::ZERO,
        }
    }

    /// Can we still conceal `duree`? Returns `false` when the cap is
    /// reached — up to the caller to return silence.
    ///
    /// The comparison is made **before** the addition: the budget therefore allows
    /// exactly `plafond` of cumulated concealment, never one frame more.
    ///
    /// ⚠️ **A ZERO duration is refused**, and it is not a textbook case: an
    /// accepted zero duration would never advance the counter and this budget
    /// could never again reach its cap — precisely the unbounded defect
    /// it exists to close.
    pub fn consommer(&mut self, duree: Duration) -> bool {
        if duree.is_zero() || self.consecutif >= self.plafond {
            return false;
        }
        self.consecutif = self.consecutif.saturating_add(duree);
        true
    }

    /// A real frame has been returned: the budget starts whole again.
    pub fn trame_reelle(&mut self) {
        self.consecutif = Duration::ZERO;
    }

    /// Duration concealed in a row since the last real frame.
    #[cfg(test)]
    pub fn consecutif(&self) -> Duration {
        self.consecutif
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRAME: Duration = Duration::from_millis(20);

    /// The NOMINAL case, and it must not change: a short network loss
    /// stays concealed exactly as before the cap.
    #[test]
    fn une_perte_courte_est_dissimulee_sans_reserve() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        for i in 0..3 {
            assert!(
                b.consommer(TRAME),
                "la {}ᵉ trame perdue a été refusée",
                i + 1
            );
        }
        assert_eq!(b.consecutif(), Duration::from_millis(60));
    }

    /// The cap bites, and it bites EXACTLY at its value: ten frames of
    /// 20 ms make the 200 ms allowed, the eleventh is refused.
    #[test]
    fn au_dela_du_plafond_la_dissimulation_est_refusee() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        for i in 0..10 {
            assert!(
                b.consommer(TRAME),
                "la {}ᵉ trame a été refusée trop tôt",
                i + 1
            );
        }
        assert_eq!(b.consecutif(), PLAFOND_DISSIMULATION);
        assert!(
            !b.consommer(TRAME),
            "la 11ᵉ trame a été dissimulée : le plafond ne mord pas"
        );
        // And the refusal is LASTING: it does not lift by itself at the next
        // round. Without this line, a cap that only forbade one frame
        // out of two would pass the assertion above.
        for _ in 0..100 {
            assert!(!b.consommer(TRAME), "le refus s'est levé tout seul");
        }
    }

    /// ⚠️ **THE reset test.** Without it, a cap reached
    /// once would condemn the session to silence forever: the sink would
    /// never again conceal the slightest network loss.
    #[test]
    fn une_vraie_trame_rend_le_budget_entier() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        while b.consommer(TRAME) {}
        assert!(!b.consommer(TRAME));

        b.trame_reelle();
        assert_eq!(b.consecutif(), Duration::ZERO);
        assert!(
            b.consommer(TRAME),
            "après le retour d'une vraie trame, le budget reste fermé"
        );
    }

    /// The bound is a DURATION, not a frame count: at 10 ms per frame
    /// twice as many get through as at 20 ms, for the same concealed duration.
    ///
    /// It is what makes the constant right whatever the peer's cadence
    /// — Chrome emits 20 ms, work stream A 10 ms, and nothing forces a
    /// peer to stick to that (spec §7).
    #[test]
    fn le_plafond_est_une_duree_et_non_un_compte_de_trames() {
        let compte = |trame: Duration| {
            let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
            let mut n = 0;
            while b.consommer(trame) {
                n += 1;
            }
            (n, b.consecutif())
        };
        assert_eq!(
            compte(Duration::from_millis(20)),
            (10, PLAFOND_DISSIMULATION)
        );
        assert_eq!(
            compte(Duration::from_millis(10)),
            (20, PLAFOND_DISSIMULATION)
        );
    }

    /// A zero duration cannot make the budget spin endlessly — that
    /// would be the unbounded defect replayed inside its own remedy.
    #[test]
    fn une_duree_nulle_est_refusee_plutot_que_de_ne_jamais_epuiser_le_budget() {
        let mut b = BudgetDissimulation::new(PLAFOND_DISSIMULATION);
        assert!(!b.consommer(Duration::ZERO));
        assert_eq!(b.consecutif(), Duration::ZERO);
    }
}
