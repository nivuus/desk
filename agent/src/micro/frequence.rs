//! The frequency instrument of the microphone acceptance run: the dominant frequency
//! of a signal assumed periodic, by counting zero crossings.
//!
//! **PURE, no `cfg`.** Extracted from `micro.rs` under the 500-line
//! rule, and EXTRACTED RATHER THAN COMPRESSED — the parent had to host the
//! concealment cap (`micro/dissimulation.rs`) and no longer had the
//! margin. The repository's doctrine is to extract BEFORE the addition, not
//! after having crossed it; that is what is done here.
//!
//! Nothing in its content changed: it is moved word for word, and
//! `micro.rs` re-exports it, so that no call site moves.
//!
//! ⚠️ **It is NOT the repository's only frequency instrument**: `spectre.rs`
//! carries another, through a Goertzel filter, which looks for THE dominant line
//! on a grid known in advance. The two coexist on purpose — this one
//! assumes no grid and returns `None` on what is not periodic,
//! which a Goertzel does not do by itself.

/// Fraction of the peak below which a sample does not count as a
/// crossing: the dead band.
///
/// **Without it, the instrument would take noise for a tone.** Quantisation
/// noise around zero multiplies the sign changes, and it is
/// exactly what
/// `silence_and_noise_do_not_return_a_credible_frequency` sanctions.
const BANDE_MORTE: f32 = 0.25;

/// Peak amplitude below which the signal has no frequency at all.
const CRETE_MINIMALE: f32 = 1.0 / 512.0;

/// Dominant frequency of a signal assumed PERIODIC, by zero crossings.
///
/// ⚠️ **`pcm` is a MONO signal at `hz` samples per second.** An interleaved
/// stereo buffer must be deinterleaved by the caller (`step_by(2)`).
///
/// ❌ **THIS DOC CARRIED A FALSEHOOD, and it is the MEASUREMENT that refuted it** (task 13,
/// work stream E). It said that analysing it as is "would double the apparent
/// cadence". **It is the reverse: the frequency is HALVED** —
/// noted **219.5 Hz for a 440 Hz tone**, identical channels, by
/// removing the `step_by(2)` from `demarrage::micro::Fenetre` and rerunning its
/// test. The mechanism is in the computation below: `duree` is
/// `pcm.len() / hz`, and an interleaved buffer carries twice as many values as
/// frames — the computed duration doubles, while the number of zero crossings
/// does not move (duplicating each sample adds no sign
/// change). The obligation to deinterleave is UNCHANGED; only the direction of the
/// error made by forgetting it was wrong, and a reader who would have
/// looked for an "x2" in a log would have found nothing. *(The plan described the implementation as operating "on the left
/// channel" while writing its tests on a mono buffer: the two cannot
/// both be true, and it is the test's semantics that was retained,
/// because it is what makes the function usable both ways.)*
///
/// Returns `None` when the signal is too weak for a crossing to make sense:
/// **a signal too weak has no frequency**, and returning a number for
/// silence would make this instrument the byte counter it exists to
/// replace (repository doctrine, paid for in D7).
pub fn frequence_par_passages_a_zero(pcm: &[f32], hz: u32) -> Option<f32> {
    if pcm.len() < 2 || hz == 0 {
        return None;
    }
    let crete = pcm.iter().fold(0.0f32, |m, e| m.max(e.abs()));
    if crete < CRETE_MINIMALE {
        return None;
    }
    let seuil = crete * BANDE_MORTE;

    let mut passages = 0u64;
    let mut signe: Option<bool> = None;
    for &e in pcm {
        if e.abs() <= seuil {
            continue;
        }
        let positif = e > 0.0;
        match signe {
            Some(precedent) if precedent != positif => {
                passages += 1;
                signe = Some(positif);
            }
            None => signe = Some(positif),
            _ => {}
        }
    }
    if passages == 0 {
        return None;
    }
    let duree = pcm.len() as f32 / hz as f32;
    Some(passages as f32 / (2.0 * duree))
}
