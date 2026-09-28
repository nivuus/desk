//! The debounce: a train of notifications produces only ONE reconciliation.
//!
//! **PURE**: no `cfg`, no clock of its own, no input-output.
//! The instant is a **parameter**, exactly as in
//! `agent/src/pont/table.rs` and `agent/src/apps/installation/fenetre.rs` — and
//! that is why **no test in this file sleeps**.

use std::time::{Duration, Instant};

/// Every notification pushes the deadline back by this delay.
///
/// ⚠️ **NOT CALIBRATED.** No usage judgement has been made on the experience
/// it produces. What bounds it from below is measured, on the other hand: the
/// polling in `apps::boucle` has a granularity of **200 ms**
/// (`std::thread::sleep(reste.min(200 ms))`), so a debounce shorter
/// than that would not be observable — it would be absorbed by the polling.
pub const DELAI_ANTI_REBOND: Duration = Duration::from_millis(750);

/// The deadline can never exceed `first notification of the train + this`.
///
/// 🔴 **THIS CAP IS NOT A COMFORT, IT IS THE SAFEGUARD OF CRITERION ①** —
/// same role, and same precedent, as `REPLI_MAX_MS` in
/// `agent/src/plateforme/repli.rs`. Without it, a CONTINUOUS stream of notifications
/// postpones the reconciliation **without end**: an installer writing for
/// two minutes would produce no catalogue before it finished, and a noisy
/// root would **never** produce one.
///
/// 🔴 **FOUR SECONDS, AND NOT THE SPECIFICATION'S FIVE: it is DERIVED, not
/// copied.** The worst case of criterion ① — "a created shortcut appears in less
/// than five seconds" — is the sum of four terms:
///
/// ```text
/// worst case = DELAI_ANTI_REBOND_MAX
///            + polling granularity     (200 ms, READ in apps/boucle.rs)
///            + cost of a reconciliation (≈ 70 ms at rest, MEASURED)
///            + 10 ms per new icon       (MEASURED)
/// ```
///
/// At **5 s**: `5,000 + 200 + 70 + 10 = 5,280 ms` — **above** the five
/// seconds the criterion requires, in the case where the shortcut is born *inside*
/// a continuous stream. At **4 s**: `4,000 + 200 + 70 + 10 =
/// 4,280 ms`, and **720 ms** of margin remain, that is **seventy-two new
/// icons** before the criterion is threatened.
///
/// ✅ **THE DERIVATION IS CORROBORATED BY MEASUREMENT, AND IT WAS NOT SOUGHT**
/// (acceptance criterion ①, two runs): a created shortcut appears in
/// **960 ms** then **999 ms**, when the sum predicts `750 + ≤200 + ~70` =
/// **960 to 1,020 ms**. Both measurements fall in the interval.
///
/// 🔵 **DERIVED IS NOT CALIBRATED, and the two are not the same thing.**
/// Corroborating a sum is not judging an experience: nobody said that
/// 960 ms "feels right".
/// Nobody judged that four seconds "feel right": it therefore joins
/// this repository's list of non-calibrated values — `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `TAILLE_MAX_SORTIE`,
/// `SEUIL_INJOIGNABLE_MS`, `PERIODE_RECONCILIATION`, `DELAI_LANCEMENT_MS`.
///
/// ⚠️ **THE WORST CASE REMAINS OPEN IN ONE PLACE, NAMED AND NOT BOUNDED**: if *k*
/// applications appear at once, the icon term is `k × 10 ms` and
/// criterion ① fails as soon as `k > 72`. That is the price of the "a single
/// reconciliation for a whole train" path, and it is accepted.
pub const DELAI_ANTI_REBOND_MAX: Duration = Duration::from_secs(4);

/// The state of a train of notifications in progress.
///
/// Two instants are enough: that of the **first** notification of the train
/// — which bounds the postponement — and that of the **last** — which pushes it back.
#[derive(Default, Debug)]
pub struct Rebond {
    premiere: Option<Instant>,
    derniere: Option<Instant>,
}

impl Rebond {
    /// Une notification vient d'arriver.
    ///
    /// The first of the train sets the cap's anchor point; the following ones
    /// only push back the short deadline.
    pub fn notifier(&mut self, maintenant: Instant) {
        if self.premiere.is_none() {
            self.premiere = Some(maintenant);
        }
        self.derniere = Some(maintenant);
    }

    /// The instant at which the reconciliation must start, or `None` if no train
    /// is in progress.
    ///
    /// 🔴 `min` OF THE TWO, AND THAT IS THE WHOLE DEBOUNCE: the short branch
    /// absorbs bursts, the long branch guarantees that a continuous stream
    /// still ends up producing a catalogue.
    pub fn echeance(&self) -> Option<Instant> {
        let premiere = self.premiere?;
        let derniere = self.derniere?;
        Some((derniere + DELAI_ANTI_REBOND).min(premiere + DELAI_ANTI_REBOND_MAX))
    }

    /// Has the deadline been reached?
    ///
    /// ⚠️ **Returns `false` at rest**, and it is deliberate: a debounce that
    /// fell due without a notification would trigger phantom reconciliations,
    /// that is precisely the cost this module exists to avoid.
    pub fn du(&self, maintenant: Instant) -> bool {
        self.echeance()
            .is_some_and(|echeance| maintenant >= echeance)
    }

    /// The train is consumed: the reconciliation starts.
    ///
    /// 🔴 WITHOUT THIS RESET, THE UPPER BOUND WOULD BITE FOREVER after
    /// the first train — `premiere` would stay at the very first instant, and
    /// `premiere + MAX` would already be exceeded for ever: every later
    /// notification would trigger immediately, that is, the debounce
    /// would cease to exist after one train.
    pub fn consommer(&mut self) {
        self.premiere = None;
        self.derniere = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠️ `Instant` IS NOT FABRICATED, IT IS OFFSET. All tests start
    /// from an `Instant::now()` and add `Duration`s to it: **none sleeps.**
    fn base() -> Instant {
        Instant::now()
    }

    #[test]
    fn une_notification_isolee_echoit_apres_le_delai_court() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        assert_eq!(r.echeance(), Some(t + DELAI_ANTI_REBOND));
        assert!(!r.du(t + DELAI_ANTI_REBOND - Duration::from_millis(1)));
        assert!(r.du(t + DELAI_ANTI_REBOND));
    }

    /// 🔴 IT IS *THE DEBOUNCE*, NOT A MERE DELAY: the second notification
    /// PUSHES BACK the deadline instead of leaving it where the first had set it.
    #[test]
    fn une_seconde_notification_repousse_l_echeance() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        r.notifier(t + Duration::from_millis(500));
        assert_eq!(
            r.echeance(),
            Some(t + Duration::from_millis(500) + DELAI_ANTI_REBOND)
        );
        assert!(
            !r.du(t + DELAI_ANTI_REBOND),
            "l'échéance de la PREMIÈRE ne doit plus valoir"
        );
    }

    /// 🔴 **THE TEST THAT MATTERS.** Without it, the upper bound could be
    /// missing and the other four would pass: a train that never stops
    /// would make the deadline recede indefinitely, and no reconciliation would
    /// start — the failure `DELAI_ANTI_REBOND_MAX` exists to close.
    #[test]
    fn un_train_continu_echoit_quand_meme_au_plafond() {
        let t = base();
        let mut r = Rebond::default();
        // One notification every 100 ms for ten seconds: the short branch
        // can NEVER fall due, `derniere + 750 ms` always being
        // pushed back before being reached.
        for i in 0..100u32 {
            r.notifier(t + Duration::from_millis(100 * u64::from(i)));
        }
        assert_eq!(r.echeance(), Some(t + DELAI_ANTI_REBOND_MAX));
        assert!(r.du(t + DELAI_ANTI_REBOND_MAX));
    }

    #[test]
    fn consommer_remet_le_train_a_zero_et_le_suivant_repart_de_sa_premiere() {
        let t = base();
        let mut r = Rebond::default();
        r.notifier(t);
        r.consommer();
        assert_eq!(r.echeance(), None, "un train consommé n'a plus d'échéance");
        let t2 = t + Duration::from_secs(60);
        r.notifier(t2);
        assert_eq!(
            r.echeance(),
            Some(t2 + DELAI_ANTI_REBOND),
            "le train suivant repart de SA première, jamais de l'ancienne"
        );
    }

    #[test]
    fn aucune_notification_ne_produit_aucune_echeance() {
        let r = Rebond::default();
        assert_eq!(r.echeance(), None);
        assert!(!r.du(base()));
        assert!(!r.du(base() + Duration::from_secs(3600)));
    }

    /// D6's derivation is checked by the test, not only written in
    /// the comment: the worst case must hold UNDER the five seconds
    /// criterion ① requires, icon margin included.
    #[test]
    fn le_pire_cas_derive_tient_sous_les_cinq_secondes_du_critere() {
        let granularite_sondage = Duration::from_millis(200);
        let cout_reconciliation = Duration::from_millis(70);
        let une_icone_neuve = Duration::from_millis(10);
        let pire =
            DELAI_ANTI_REBOND_MAX + granularite_sondage + cout_reconciliation + une_icone_neuve;
        assert!(
            pire < Duration::from_secs(5),
            "pire cas {pire:?} : le critère ① exige moins de cinq secondes"
        );
    }
}
