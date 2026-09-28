//! What comparing the two topology surveys ACTUALLY allows
//! to establish after a purge.
//!
//! Outside `#[cfg(windows)]`, like the parent module and for the same reason as
//! `superviseur::designation` and `superviseur::reprise`: **the rule must
//! have tests, and they would not run under `#[cfg(windows)]`.**
//! `purge.rs`, for its part, is gated — the first draft of this verdict lived there,
//! and `cargo test --workspace` filtered out its four tests without saying anything.

/// What comparing the two surveys ACTUALLY allows to establish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// As many fewer outputs as successful removals.
    Conforme,
    /// **FEWER** outputs than expected: someone else removed one.
    /// It is **not** our failure, and denouncing it drowns the real ones.
    UnTiersAAussiRetire,
    /// **MORE** outputs than expected: removals the driver declared
    /// successful removed nothing. It is the only anomaly this
    /// comparison can attribute to the purge.
    RetraitsSansEffet,
}

/// 🔴 **THIS VERDICT CRIED WRONGLY AT EACH START-UP, AND THAT IS WHAT WE FIX.**
/// The old form compared `apres == avant - retirees` and returned an
/// `ERROR` on **any** difference. Measured twice on 30 August 2026:
/// `retirees=5 avant=7 apres=1 attendu=2` then `retirees=7 avant=9 apres=1
/// attendu=2` — `apres` **LOWER** than `attendu` in both cases, that is
/// MORE outputs had disappeared than we had removed. The purge
/// had nevertheless worked perfectly (SudoVDA monitors: 8 → 0).
///
/// **The assumed equality is false as soon as a third party touches the topology**, and
/// it is the nominal case on this VM: Apollo creates then destroys a temporary
/// virtual output to probe its encoders.
///
/// ⚠️ **An `ERROR` that cries for no reason at each start-up is an `ERROR`
/// no one reads anymore.** This repository lost several rounds because the
/// trace that told the truth was drowned.
///
/// **What the comparison can still establish, and which we keep**: if there remain
/// MORE outputs than expected, then removals declared successful removed
/// nothing — a broken handle or IOCTL, that is the defect this verdict
/// existed to catch.
pub fn verdict(avant: usize, apres: usize, retirees: usize) -> Verdict {
    let attendu = avant.saturating_sub(retirees);
    match apres.cmp(&attendu) {
        std::cmp::Ordering::Equal => Verdict::Conforme,
        std::cmp::Ordering::Less => Verdict::UnTiersAAussiRetire,
        std::cmp::Ordering::Greater => Verdict::RetraitsSansEffet,
    }
}

#[cfg(test)]
mod tests_verdict {
    use super::*;

    #[test]
    fn autant_de_moins_que_de_retraits_est_conforme() {
        assert_eq!(verdict(9, 2, 7), Verdict::Conforme);
        assert_eq!(verdict(0, 0, 0), Verdict::Conforme);
    }

    /// 🔴 THE TWO REAL SURVEYS OF 30 AUGUST 2026, which returned an `ERROR`.
    /// They must no longer return one: the purge had worked.
    #[test]
    fn les_deux_releves_qui_criaient_a_tort_ne_crient_plus() {
        assert_eq!(verdict(7, 1, 5), Verdict::UnTiersAAussiRetire);
        assert_eq!(verdict(9, 1, 7), Verdict::UnTiersAAussiRetire);
    }

    /// 🔴 THE RED THAT REMAINS, and it is the defect the verdict existed to
    /// catch: the driver declares successful removals, the topology does not
    /// move.
    #[test]
    fn des_retraits_sans_effet_restent_denonces() {
        assert_eq!(verdict(9, 9, 7), Verdict::RetraitsSansEffet);
        assert_eq!(verdict(3, 3, 1), Verdict::RetraitsSansEffet);
    }

    /// The case the old form already handled: no removal, nothing
    /// moves, all is well.
    #[test]
    fn aucun_retrait_et_rien_ne_bouge_est_conforme() {
        assert_eq!(verdict(4, 4, 0), Verdict::Conforme);
    }
}
