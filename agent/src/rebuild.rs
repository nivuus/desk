//! Rebuild-with-fallback of a resource whose successful construction
//! requires first releasing any existing instance.
//!
//! Motivated by `WindowsSource::resize` (see its comment): DXGI
//! only allows a single live `IDXGIOutputDuplication` instance per
//! output and per process at a time, so the old capture must be
//! released BEFORE trying to build a new one. If that attempt
//! fails, the caller must not be left with nothing — a `next_frame` called
//! right after must never hit an absent state (it is exactly the
//! bug reported in review on the first version of the fix: the previous
//! round emptied the field, tried a rebuild, and if that one
//! failed via `?`, the field stayed `None` for good, making
//! the next call to `next_frame` panic).
//!
//! Extracted into a module without a Windows dependency to stay testable on
//! Linux (see the tests below): the decision logic — try,
//! fall back on a recovery, or declare a definitive failure — depends
//! on no type specific to `windows-rs`, `DesktopCapture` or
//! `H264Encoder`.

use anyhow::{Error, Result};

/// Outcome of a rebuild-with-fallback attempt (see the module
/// comment).
pub enum RebuildOutcome<T, C> {
    /// The primary factory succeeded: the caller adopts `T` entirely.
    Rebuilt(T),
    /// The primary factory failed, but the recovery factory
    /// succeeded: the caller must adopt `C` (typically a subset of
    /// what the primary factory produces — the capture alone, not the
    /// region nor the encoder, which stay the previous ones and remain valid)
    /// and keep the rest of its previous state unchanged. The original error
    /// is kept: the fallback restores a usable state, it must not
    /// make the error the caller must log/return disappear.
    Recovered(C, Error),
    /// Both factories failed: no usable state could be
    /// obtained. The caller must keep no partial resource and
    /// must declare itself definitively exhausted rather than let a following
    /// call hit an absent resource.
    Fatal(Error),
}

/// Tries `primary`. On failure, tries `recovery` to fall back on a
/// usable state rather than leave the caller with nothing.
///
/// `recovery` is called ONLY if `primary` fails: at no moment do the
/// two factories produce a live resource simultaneously, which
/// is precisely the constraint that motivates this mechanism.
pub fn rebuild_or_recover<T, C>(
    primary: impl FnOnce() -> Result<T>,
    recovery: impl FnOnce() -> Result<C>,
) -> RebuildOutcome<T, C> {
    match primary() {
        Ok(value) => RebuildOutcome::Rebuilt(value),
        Err(primary_error) => match recovery() {
            Ok(recovered) => RebuildOutcome::Recovered(recovered, primary_error),
            Err(_recovery_error) => RebuildOutcome::Fatal(primary_error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn success_of_the_primary_factory_produces_rebuilt() {
        let outcome = rebuild_or_recover(|| Ok::<_, Error>(42), || Ok::<_, Error>(0));
        match outcome {
            RebuildOutcome::Rebuilt(v) => assert_eq!(v, 42),
            _ => panic!("expected Rebuilt"),
        }
    }

    #[test]
    fn primary_failure_with_successful_fallback_produces_recovered() {
        // The motivating case: the primary factory (new capture +
        // region + encoder) fails, but a recovery capture alone
        // succeeds — the caller must be able to keep producing frames
        // with its old parameters rather than stay without capture.
        let outcome = rebuild_or_recover(
            || Err::<i32, _>(anyhow!("primary failure")),
            || Ok::<_, Error>("secours"),
        );
        match outcome {
            RebuildOutcome::Recovered(v, e) => {
                assert_eq!(v, "secours");
                assert!(e.to_string().contains("primary failure"));
            }
            _ => panic!("expected Recovered"),
        }
    }

    #[test]
    fn failure_of_both_factories_produces_fatal() {
        let outcome = rebuild_or_recover(
            || Err::<i32, _>(anyhow!("primary failure")),
            || Err::<i32, _>(anyhow!("fallback failure")),
        );
        match outcome {
            RebuildOutcome::Fatal(e) => assert!(e.to_string().contains("primary failure")),
            _ => panic!("expected Fatal"),
        }
    }

    #[test]
    fn the_fallback_factory_is_never_called_if_the_primary_succeeds() {
        // Direct proof of the constraint that motivates this mechanism: never
        // build both resources at the same time (DXGI
        // only allows a single live instance at a time for the duplicated
        // output).
        let mut recovery_called = false;
        let outcome = rebuild_or_recover(
            || Ok::<_, Error>(1),
            || {
                recovery_called = true;
                Ok::<_, Error>(2)
            },
        );
        assert!(matches!(outcome, RebuildOutcome::Rebuilt(1)));
        assert!(!recovery_called);
    }
}
