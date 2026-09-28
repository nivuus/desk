use super::*;

// ─ what MIGRATED, and where to find it ────────────────────────────────────
// The three wire SHAPE tests that lived here — pinned serialisation
// of request headers, re-reading of response headers, rejection of an
// incomplete header — left with the structures they pinned, for
// `proto/src/files/entetes/tests.rs`. They are stronger there than here: their
// vector is a SHARED FILE that the TypeScript twin reads too, whereas these
// three only pinned the shapes from one side.
//
// Only the tests of `filetime_depuis_ms` therefore remain here, which has no
// browser twin.

/// 🔴 **The FILETIME epoch is not the Unix one, and the gap is 369
/// years.** Getting the epoch or the unit wrong yields dates of 1601 in
/// Explorer — visible, but only if someone looks. Getting the
/// FACTOR wrong (10⁷ versus 10⁶) is barely visible.
#[test]
fn l_epoque_unix_devient_l_epoque_filetime() {
    // January 1st, 1970, 00:00:00 UTC = 116,444,736,000,000,000 100 ns units
    // since January 1st, 1601.
    assert_eq!(filetime_depuis_ms(0), 116_444_736_000_000_000);
    // One millisecond is worth 10,000 100 ns units.
    assert_eq!(filetime_depuis_ms(1), 116_444_736_000_010_000);
    assert_eq!(filetime_depuis_ms(1000), 116_444_736_010_000_000);
}

/// A date before 1970 is legal (`lastModified` can be negative);
/// a date before **1601** is not, and would yield a negative FILETIME
/// that Windows interprets as a relative time. It is brought back to zero.
#[test]
fn une_date_anterieure_a_1601_est_ramenee_a_zero() {
    assert_eq!(filetime_depuis_ms(-11_644_473_600_000), 0);
    assert_eq!(filetime_depuis_ms(-11_644_473_600_001), 0);
    assert_eq!(filetime_depuis_ms(i64::MIN), 0);
    // Just above the FILETIME epoch: still positive.
    assert_eq!(filetime_depuis_ms(-11_644_473_599_999), 10_000);
}

/// An absurd `lastModified` must not overflow the computation: an
/// overflow in `release` wraps silently and would yield an arbitrary date.
#[test]
fn une_date_absurde_ne_deborde_pas() {
    assert_eq!(filetime_depuis_ms(i64::MAX), i64::MAX);
}
