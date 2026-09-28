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
/// The old form compared `after == before - removed` and returned an
/// `ERROR` on **any** difference. Measured twice on 30 August 2026:
/// `removed=5 before=7 after=1 expected=2` then `removed=7 before=9 after=1
/// expected=2` (field names as they read today) — `after` **LOWER** than `expected` in both cases, that is
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
pub fn verdict(before: usize, after: usize, removed: usize) -> Verdict {
    let expected = before.saturating_sub(removed);
    match after.cmp(&expected) {
        std::cmp::Ordering::Equal => Verdict::Conforme,
        std::cmp::Ordering::Less => Verdict::UnTiersAAussiRetire,
        std::cmp::Ordering::Greater => Verdict::RetraitsSansEffet,
    }
}

#[cfg(test)]
mod tests_verdict {
    use super::*;

    #[test]
    fn as_many_fewer_as_removals_is_compliant() {
        assert_eq!(verdict(9, 2, 7), Verdict::Conforme);
        assert_eq!(verdict(0, 0, 0), Verdict::Conforme);
    }

    /// 🔴 THE TWO REAL SURVEYS OF 30 AUGUST 2026, which returned an `ERROR`.
    /// They must no longer return one: the purge had worked.
    #[test]
    fn the_two_readings_that_cried_wrongly_no_longer_cry() {
        assert_eq!(verdict(7, 1, 5), Verdict::UnTiersAAussiRetire);
        assert_eq!(verdict(9, 1, 7), Verdict::UnTiersAAussiRetire);
    }

    /// 🔴 THE RED THAT REMAINS, and it is the defect the verdict existed to
    /// catch: the driver declares successful removals, the topology does not
    /// move.
    #[test]
    fn ineffective_removals_stay_denounced() {
        assert_eq!(verdict(9, 9, 7), Verdict::RetraitsSansEffet);
        assert_eq!(verdict(3, 3, 1), Verdict::RetraitsSansEffet);
    }

    /// The case the old form already handled: no removal, nothing
    /// moves, all is well.
    #[test]
    fn no_removal_and_nothing_moving_is_compliant() {
        assert_eq!(verdict(4, 4, 0), Verdict::Conforme);
    }
}
