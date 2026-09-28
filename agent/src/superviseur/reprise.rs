//! How many times to retry attaching an output, and when to give up.
//!
//! 🔴 **THE FACT GOVERNING THIS RULE, MEASURED ON AUGUST 30TH, 2026.** A HELD
//! virtual output attaches in **less than a second** — recorded at 1 Hz,
//! `seconde=1 attachees=2 presente=Some(true)`. In the same hour, on the same
//! machine, another output was **still not** attached after 3 s, and
//! the product refused it after 5 s. **Attachment is therefore not SLOW, it
//! is INTERMITTENT** — and the disturbance correlates with the encoder probe
//! Apollo restarts at each display topology change.
//!
//! 🔴 **IT IS THEREFORE NOT A CONSTANT TO LENGTHEN.** Lengthening the wait of a
//! single turn only waits longer WITHIN the same disturbance
//! window; what is needed is **several spaced chances**, so that at
//! least one falls in a lull. That is why the rule carries a number
//! of TURNS and a RESPITE, and not a longer delay.
//!
//! 🔴 **AND ABOVE ALL: WE DO NOT RECREATE THE OUTPUT BETWEEN TWO TURNS.** Destroying
//! then recreating changes the topology, hence **re-triggers Apollo's probe** —
//! the retry would feed exactly what it waits for. It is also what
//! consumes the pool: the red arm's survey carries **9 outputs created for
//! 7 refusals**. The driver has already accepted the creation; there is nothing to redo.
//!
//! ⚠️ **WHAT THIS RETRY COSTS, AND IT MUST BE SAID.** `create_output` runs
//! INSIDE the supervisor loop, which is single-threaded: during the wait, nothing
//! else is handled. The worst case goes from 5 s to
//! `TOURS × LIMITE_RATTACHEMENT + (TOURS − 1) × REPIT`. It is PURE waiting
//! time before the user sees their refusal, when the output will never
//! come.
//!
//! ⚠️ **Out of reach of `DELAI_ATTENTE_VIEWPORT_MAX`**: that 30 s
//! bound only filters `Etat::AttendLeViewport`
//! (`superviseur/table/orphelines.rs`), whereas `create_output` runs in
//! `Etat::AttendLaSortie` (`table/attribution.rs`). Checked by reading both,
//! not assumed — a retry cancelled by a neighbouring safeguard would be a remedy
//! that remedies nothing.

use std::time::Duration;

/// The number of chances given to an output to attach.
///
/// **Three, and the value comes from measurement**: attachment succeeds in
/// < 1 s, and Apollo's disturbance comes back at a cadence of the order of 5 s
/// (from its log: one encoder probe per topology change,
/// at the cadence of our own attempts). Three spaced turns therefore cover
/// several disturbance cycles. ⚠️ **It is NOT a calibrated
/// constant** — none is in this repository, and this one is no
/// exception: it is a value DERIVED from a measurement, which is not the
/// same thing.
pub const TOURS: u32 = 3;

/// The time given back to the machine between two turns.
///
/// Without a respite, the three turns would be contiguous and would be worth only one
/// longer turn — exactly what this rule refuses to do.
pub const REPIT: Duration = Duration::from_secs(1);

/// What to do after an unsuccessful waiting turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suite {
    /// Retry, **on the same output**, after this respite.
    Reessayer { next_round: u32, apres: Duration },
    /// Give up: the output is handed back to the driver and the window refused.
    ///
    /// `tours_epuises` serves the LOG: a refusal after N turns does not read
    /// like an immediate refusal, and without this number one would never know which
    /// one is being read.
    Renoncer { tours_epuises: u32 },
}

/// The rule, pure: after turn `tour_acheve` (1-indexed), do we continue?
///
/// 🔴 **BOUNDED BY CONSTRUCTION.** This repository already carries an unbounded restart
/// loop as an open defect; this one cannot become a
/// second: `tour_acheve >= tours` always returns `Renoncer`, and so does `tours == 0`
/// — a zero number of turns must not read as "forever".
pub fn apres_un_tour(tour_acheve: u32, tours: u32, repit: Duration) -> Suite {
    if tours == 0 || tour_acheve >= tours {
        return Suite::Renoncer {
            tours_epuises: tour_acheve.max(1),
        };
    }
    Suite::Reessayer {
        next_round: tour_acheve + 1,
        apres: repit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_tours_intermediaires_reessaient_sur_la_meme_sortie() {
        assert_eq!(
            apres_un_tour(1, 3, REPIT),
            Suite::Reessayer {
                next_round: 2,
                apres: REPIT
            }
        );
        assert_eq!(
            apres_un_tour(2, 3, REPIT),
            Suite::Reessayer {
                next_round: 3,
                apres: REPIT
            }
        );
    }

    /// 🔴 The bound. Without it, the retry would become the second unbounded
    /// restart loop of this repository.
    #[test]
    fn the_last_round_gives_up_and_says_how_many_it_made() {
        assert_eq!(
            apres_un_tour(3, 3, REPIT),
            Suite::Renoncer { tours_epuises: 3 }
        );
    }

    /// A turn beyond the bound gives up too: no arithmetic can
    /// restart the loop.
    #[test]
    fn au_dela_de_la_borne_on_renonce_encore() {
        assert_eq!(
            apres_un_tour(9, 3, REPIT),
            Suite::Renoncer { tours_epuises: 9 }
        );
    }

    /// ⚠️ The degenerate case: `tours = 0` must not read as "unbounded".
    #[test]
    fn zero_tour_renonce_immediatement_et_jamais_a_l_infini() {
        assert_eq!(
            apres_un_tour(1, 0, REPIT),
            Suite::Renoncer { tours_epuises: 1 }
        );
        assert_eq!(
            apres_un_tour(0, 0, REPIT),
            Suite::Renoncer { tours_epuises: 1 }
        );
    }

    /// The product from BEFORE this remedy, expressed in the same rule: a single
    /// turn. It is the WITNESS that makes the retry discriminating — it shows
    /// that the rule also knows how to retry nothing.
    #[test]
    fn a_single_round_is_the_product_before_the_retry() {
        assert_eq!(
            apres_un_tour(1, 1, REPIT),
            Suite::Renoncer { tours_epuises: 1 }
        );
    }

    /// The worst case is BOUNDED and computable: it is what the supervisor
    /// loop can spend without handling anything else.
    #[test]
    fn le_pire_cas_est_borne_et_calculable() {
        let mut tours = 0;
        let mut attente = Duration::ZERO;
        let limite = Duration::from_secs(5);
        let mut tour = 1;
        loop {
            tours += 1;
            attente += limite;
            match apres_un_tour(tour, TOURS, REPIT) {
                Suite::Reessayer { next_round, apres } => {
                    attente += apres;
                    tour = next_round;
                }
                Suite::Renoncer { .. } => break,
            }
            assert!(tours <= TOURS, "la reprise doit être bornée");
        }
        assert_eq!(tours, TOURS);
        assert_eq!(attente, Duration::from_secs(17));
    }
}
