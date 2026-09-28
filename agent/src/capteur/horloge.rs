//! The clock shared by the sensor and its children.
//!
//! `clock_origin` is a `std::time::Instant`, shared on the child side between
//! video and audio: it is this common origin that makes the two
//! timelines comparable, hence the A/V sync exact. An `Instant` makes no sense
//! in another process — but `QueryPerformanceCounter` is monotonic and
//! **common to the whole machine**. The child therefore sends its origin in QPC ticks,
//! and the sensor rebuilds the equivalent `Instant` on its side.
//!
//! The conversion is PURE and outside `cfg`: it is what can be wrong, not
//! the system call.

use std::time::{Duration, Instant};

/// Rebuilds the child's clock origin in the sensor's `Instant` frame
/// of reference.
///
/// Falls back on `maintenant` in both degenerate cases — origin after
/// the current reading, or zero frequency — rather than panicking: a
/// wrong origin shifts the A/V sync, a panic kills the window.
pub fn origine_depuis_qpc(
    origine_qpc: i64,
    qpc_maintenant: i64,
    frequence: i64,
    maintenant: Instant,
) -> Instant {
    if frequence <= 0 || qpc_maintenant <= origine_qpc {
        return maintenant;
    }
    let tics = (qpc_maintenant - origine_qpc) as u128;
    let nanos = tics * 1_000_000_000u128 / frequence as u128;
    maintenant
        .checked_sub(Duration::from_nanos(nanos.min(u64::MAX as u128) as u64))
        .unwrap_or(maintenant)
}

#[cfg(windows)]
mod systeme {
    use anyhow::{Context, Result};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    pub fn lire_qpc() -> Result<i64> {
        let mut value = 0i64;
        unsafe { QueryPerformanceCounter(&mut value) }.context("QueryPerformanceCounter")?;
        Ok(value)
    }

    pub fn frequence_qpc() -> Result<i64> {
        let mut value = 0i64;
        unsafe { QueryPerformanceFrequency(&mut value) }.context("QueryPerformanceFrequency")?;
        Ok(value)
    }
}

#[cfg(windows)]
pub use systeme::{frequence_qpc, lire_qpc};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn une_origine_anterieure_se_reconstruit_en_arriere() {
        let maintenant = Instant::now();
        // 10 MHz, and 25 million elapsed ticks = 2.5 s.
        let reconstruite = origine_depuis_qpc(1_000_000, 26_000_000, 10_000_000, maintenant);
        let ecart = maintenant.duration_since(reconstruite);
        assert!(
            ecart.abs_diff(Duration::from_millis(2500)) < Duration::from_millis(1),
            "écart reconstruit : {ecart:?}"
        );
    }

    #[test]
    fn une_origine_egale_a_maintenant_ne_recule_pas() {
        let maintenant = Instant::now();
        assert_eq!(
            origine_depuis_qpc(42, 42, 10_000_000, maintenant),
            maintenant
        );
    }

    /// A LATER origin cannot exist, but a clock read
    /// wrongly would produce one: we then fall back on `maintenant` rather than
    /// panicking by subtracting beyond the `Instant`'s origin.
    #[test]
    fn une_origine_posterieure_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(
            origine_depuis_qpc(100, 50, 10_000_000, maintenant),
            maintenant
        );
    }

    #[test]
    fn une_frequence_nulle_retombe_sur_maintenant() {
        let maintenant = Instant::now();
        assert_eq!(origine_depuis_qpc(1, 2, 0, maintenant), maintenant);
    }
}
