//! **PURE — no `cfg`, no Windows object.** The cable exclusivity
//! policy: who is allowed to write, when we retry, and what gets logged.
//!
//! Only one child process can write to CABLE Input at a time. With two
//! writers, Windows would mix two shifted copies of the same voice — a
//! comb filter. First come wins (Decision 2 of plan E2).
//!
//! ⚠️ **This module holds NO lock.** It arbitrates, and it says what must be
//! logged. The lock itself is a Windows named mutex, set by
//! `windows_micro.rs` behind the `Verrou` trait; the tests set a fake one
//! that **counts its calls**.

/// Ce qu'un verrou inter-processus doit savoir faire.
pub trait Verrou {
    /// **NON-BLOCKING.** Returns `true` if this process holds the lock after
    /// the call — including if it already held it.
    ///
    /// ⚠️ **It is called AT EACH drop-off, hence about 50 times per second,
    /// including when the lock is ALREADY held.** The implementation must therefore
    /// be cheap *and* idempotent: a `WaitForSingleObject` on an
    /// already owned mutex would increment its recursion count, and as many
    /// `ReleaseMutex` would be needed. It is up to the Windows half to short-
    /// circuit this case, not up to `Exclusivite`.
    fn tenter(&mut self) -> bool;
}

/// What the arbitration commands, log included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// The drop-off is accepted, and nothing is to be logged.
    Accepte,
    /// The drop-off is accepted, and it is the FIRST time after a refusal: one
    /// line, otherwise the refusal's `warn!` would stay true forever in
    /// the reader's eyes.
    AccepteApresRefus,
    /// Refused, and it is the first time: one line.
    RefusePremierement,
    /// Refused, and it has already been said. Silence.
    RefuseDejaDit,
}

/// The arbiter. **The attempt is redone at each drop-off; only the LOG is
/// unique.**
pub struct Exclusivite<V: Verrou> {
    verrou: V,
    tenue: bool,
    refus_dit: bool,
}

impl<V: Verrou> Exclusivite<V> {
    pub fn new(verrou: V) -> Self {
        Self {
            verrou,
            tenue: false,
            refus_dit: false,
        }
    }

    /// ⚠️ **ATTEMPTS AT EACH CALL — Decision 2 of plan E2.** Only the LOG
    /// is unique, never the attempt.
    ///
    /// E1 prescribed a *sticky* refusal: log once and do
    /// not insist anymore. That condemned the following case — windows A and B
    /// are alive, A holds the cable, B is refused; **A dies**, Windows abandons
    /// the mutex, and **B never tries again**, without any line saying
    /// so. A `WaitForSingleObject(handle, 0)` costs a few microseconds
    /// against 50 drop-offs per second: the attempt is redone.
    ///
    /// A second refusal episode reopens its own log: losing the cable
    /// a second time is a new fact, not the repetition of the first.
    pub fn arbitrer(&mut self) -> Issue {
        if self.verrou.tenter() {
            self.tenue = true;
            if std::mem::take(&mut self.refus_dit) {
                Issue::AccepteApresRefus
            } else {
                Issue::Accepte
            }
        } else {
            self.tenue = false;
            if self.refus_dit {
                Issue::RefuseDejaDit
            } else {
                self.refus_dit = true;
                Issue::RefusePremierement
            }
        }
    }

    /// True if the last arbitration left this process as owner.
    #[cfg(test)]
    pub fn tenue(&self) -> bool {
        self.tenue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake lock that **counts its calls**. A lock that only returned
    /// a boolean would let a sticky implementation through: the test
    /// would be vacuous. It is the pattern of E1's `journaux_micro` field.
    struct VerrouFactice {
        /// The answers to return, in order; the last one repeats.
        reponses: Vec<bool>,
        appels: usize,
    }

    impl VerrouFactice {
        fn nouveau(reponses: &[bool]) -> Self {
            Self {
                reponses: reponses.to_vec(),
                appels: 0,
            }
        }
    }

    impl Verrou for VerrouFactice {
        fn tenter(&mut self) -> bool {
            let i = self.appels.min(self.reponses.len() - 1);
            self.appels += 1;
            self.reponses[i]
        }
    }

    #[test]
    fn un_verrou_libre_accepte_sans_rien_journaliser() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[true]));
        assert_eq!(e.arbitrer(), Issue::Accepte);
        assert!(e.tenue());
    }

    #[test]
    fn un_refus_ne_se_journalise_qu_une_fois() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false]));
        assert_eq!(e.arbitrer(), Issue::RefusePremierement);
        for _ in 0..10 {
            assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        }
        assert!(!e.tenue());
    }

    /// 🔴 **THE test of the latent defect of E1's Decision 4.** A sticky
    /// refusal would never call `tenter` again, and window B would stay without
    /// a microphone for the life of its process after window A's death.
    #[test]
    #[allow(non_snake_case)]
    fn la_tentative_est_REFAITE_apres_un_refus() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false]));
        for _ in 0..20 {
            e.arbitrer();
        }
        assert_eq!(
            e.verrou.appels, 20,
            "la tentative doit être refaite à CHAQUE dépôt, refus compris"
        );
    }

    /// 🔴 Without this announcement, the refusal's `warn!` would stay true forever.
    #[test]
    #[allow(non_snake_case)]
    fn une_acquisition_tardive_est_ANNONCEE() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false, false, false, true]));
        assert_eq!(e.arbitrer(), Issue::RefusePremierement);
        assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        assert_eq!(e.arbitrer(), Issue::AccepteApresRefus);
        assert!(e.tenue());
        for _ in 0..5 {
            assert_eq!(e.arbitrer(), Issue::Accepte);
        }
    }

    #[test]
    fn une_acquisition_deja_tenue_ne_reannonce_rien() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[true]));
        for _ in 0..10 {
            assert_eq!(e.arbitrer(), Issue::Accepte);
        }
        assert!(e.tenue());
    }
}
