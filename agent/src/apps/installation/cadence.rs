//! Sampling the progress: at what rate an installation is
//! allowed to speak.
//!
//! 🔴 **IT IS A SAFETY CONSTRAINT, NOT A COMFORT ONE.** The upstream queue from
//! the agent to the platform is **bounded to 32 messages**
//! (`crate::plateforme`, `FILE_EMISSION`) and **drops what overflows while
//! logging it**. Progress emitted for every 64 KiB slice would
//! saturate it — an 800 MB file would produce more than twelve thousand — and
//! would drown in passing the log shared by the supervisor and all its
//! children. It is the doctrine the TURN work stream paid for on 30 July 2026,
//! when a trace per `Transmit` wrote 18,619 lines in a few seconds
//! to a CIFS share and **made the session it was measuring fail**:
//! *count or sample, never trace per packet.*
//!
//! 🔴 **THE LAST PROGRESS OF A PHASE IS ALWAYS EMITTED**, whatever
//! the pacing. Without this clause, a phase ending 3 ms after the
//! previous emission would lose its last point, and the hub's bar would
//! stay at 97 % **forever** — a silent freeze, that is the
//! failure mode this repository fights everywhere else. It is therefore a
//! **parameter of [`doit_emettre`]**, and not a second function: a
//! caller can forget to call one more function, it cannot
//! forget to fill in an argument the compiler requires.
//!
//! **Pure, no `cfg`, and the clock is a PARAMETER** — as
//! `agents/fraicheur.ts` and `identite/jeton.ts` already do on the platform side,
//! and for the same reason: it is what makes the decision boundary
//! observable **to the millisecond** on the host, instead of depending on the
//! time a test takes to run.

use std::time::Duration;

/// Between two progress reports of the same phase.
///
/// **NOT CALIBRATED.** What bounds it from below is measured — the queue of 32 and
/// the duration of a real installation —, but **nothing has been measured of the comfort
/// it gives**: nobody has watched a bar move at this rate. It
/// joins the list this repository has kept since `BPP_MIN`: no constant
/// there has ever been calibrated by a usage judgement.
pub const PERIODE_PROGRESSION: Duration = Duration::from_secs(1);

/// The same, in milliseconds, DERIVED and not copied: two numbers written
/// separately would diverge at the first change of either of them.
const PERIODE_MS: u64 = PERIODE_PROGRESSION.as_millis() as u64;

/// Must this progress report go on the wire?
///
/// - `last_ms` is `None` as long as the phase has emitted nothing: **the first
///   progress of a phase always goes through**, otherwise a short phase
///   would exist for nobody and the hub would show a jump from `transfert` to
///   `reconciliation` with nothing in between;
/// - `derniere_de_la_phase` forces the emission — see the header of this module;
/// - otherwise, at least [`PERIODE_PROGRESSION`] must have passed since the last one.
///
/// ⚠️ **A CLOCK GOING BACKWARDS ALLOWS NOTHING.** The gap is computed with
/// `saturating_sub`, so a `maintenant_ms` earlier than `last_ms` gives
/// zero and **refuses** the emission instead of granting it. It is the safe direction: what
/// this module protects is a bounded queue, and the price of refusing — a bar
/// that freezes for a moment — is itself bounded by the end-of-phase clause,
/// which goes through whatever happens.
pub fn doit_emettre(last_ms: Option<u64>, maintenant_ms: u64, derniere_de_la_phase: bool) -> bool {
    if derniere_de_la_phase {
        return true;
    }
    match last_ms {
        None => true,
        Some(last) => maintenant_ms.saturating_sub(last) >= PERIODE_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 THE PACING RED. A download loop calls this
    /// predicate on every slice; a hundred slices written in the same
    /// millisecond must produce **only one** emission, otherwise the queue
    /// of 32 overflows in the blink of an eye.
    #[test]
    fn a_hundred_calls_in_the_same_millisecond_produce_a_single_emission() {
        let mut last = None;
        let mut emissions = 0;
        for _ in 0..100 {
            if doit_emettre(last, 1_000, false) {
                emissions += 1;
                last = Some(1_000);
            }
        }
        assert_eq!(emissions, 1);
    }

    /// 🔴 THE END-OF-PHASE CLAUSE, TESTED ALONE — otherwise it would be true
    /// by chance. The same instant, the same `last`, and **only the flag
    /// changes**: that is what proves it is the flag that decides, and not the
    /// elapsed time.
    #[test]
    fn la_derniere_progression_d_une_phase_passe_meme_a_zero_milliseconde() {
        assert!(
            !doit_emettre(Some(1_000), 1_000, false),
            "the control sample must refuse"
        );
        assert!(doit_emettre(Some(1_000), 1_000, true));
        // And even a clock going backwards does not hold it back: a finished phase
        // must be announced whatever happens.
        assert!(doit_emettre(Some(9_000), 1_000, true));
    }

    #[test]
    fn the_first_progress_of_a_phase_always_passes() {
        assert!(doit_emettre(None, 0, false));
        assert!(doit_emettre(None, u64::MAX, false));
    }

    /// The boundary is besieged from both sides, to the millisecond: that is
    /// why the clock is a parameter.
    #[test]
    fn la_frontiere_de_la_periode_est_assiegee_des_deux_cotes() {
        let last = Some(5_000);
        assert!(!doit_emettre(last, 5_000 + PERIODE_MS - 1, false));
        assert!(doit_emettre(last, 5_000 + PERIODE_MS, false));
        assert!(doit_emettre(last, 5_000 + PERIODE_MS + 1, false));
    }

    #[test]
    fn une_horloge_qui_recule_ne_fait_pas_emettre() {
        assert!(!doit_emettre(Some(5_000), 4_999, false));
        assert!(!doit_emettre(Some(5_000), 0, false));
    }

    /// A long phase emits at a regular pace, and only once per
    /// period: the count is hard-coded, otherwise the loop would pass
    /// without testing anything.
    #[test]
    fn une_phase_longue_emet_une_fois_par_periode() {
        let mut last = None;
        let mut emissions = 0;
        // Ten seconds, sampled every 10 ms as a slice-by-slice
        // write loop would.
        for tour in 0..1_000_u64 {
            let maintenant = tour * 10;
            if doit_emettre(last, maintenant, false) {
                emissions += 1;
                last = Some(maintenant);
            }
        }
        assert_eq!(emissions, 10);
    }

    #[test]
    fn la_periode_declaree_et_sa_forme_en_millisecondes_ne_divergent_pas() {
        assert_eq!(PERIODE_MS, 1_000);
        assert_eq!(PERIODE_PROGRESSION, Duration::from_millis(PERIODE_MS));
    }
}
