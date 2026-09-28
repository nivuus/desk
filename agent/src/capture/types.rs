//! The pure types of `capture.rs` that touch no private field of
//! `DesktopCapture`: why an acquisition fails, what a duplication
//! covers, and how to read a duplication that may be absent.
//!
//! Extracted in task 6 quater of sub-block D2: `EchecAcquisition` now
//! carrying the bare HRESULT of the failure that motivated the reopening (and its
//! associated documentation), the parent file exceeded the 500-line
//! cap (`CLAUDE.md`). Same extraction reason as `capture/ouverture.rs`:
//! none of these types needs to be in the file that defines
//! `DesktopCapture`. Renamed `types.rs` (and no longer `echec.rs`) in review
//! of the same task: `CibleCapture` is not a failure.

use windows::Win32::Graphics::Dxgi::IDXGIOutputDuplication;

/// Why a frame acquisition failed, once the resumptions are exhausted.
///
/// `AccesPerdu` does not come up at the first loss: `next_frame` tries to
/// reopen first (see `FenetreDeReprise`). Receiving it means "I could
/// not come back within the allotted time", not "access has just been lost".
///
/// Carries the bare HRESULT (`e.code().0`) that motivated the reopening: without it,
/// diagnosing an exhaustion of the resumption window forces re-reading the
/// code to guess the error code, as the report of the
/// measurement of 1 August 2026 had to do.
pub enum EchecAcquisition {
    AccesPerdu(i32),
    Panne(anyhow::Error),
}

impl std::fmt::Display for EchecAcquisition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccesPerdu(code) => write!(
                f,
                "accès à la duplication perdu (hresult={code:#010x}) et non repris en {:?}",
                crate::capture_reprise::DUREE_FENETRE_REPRISE
            ),
            Self::Panne(e) => write!(f, "{e:#}"),
        }
    }
}

/// What this duplication covers — and hence what must be reopened after an
/// access loss.
///
/// **An output name, never an index.** `(index_adaptateur, index_sortie)`
/// is positional: it changes as soon as an output appears or disappears. Yet
/// that is exactly what has just happened when we reopen. `\\.\DISPLAYn`
/// is stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CibleCapture {
    /// The output that composes the desktop, whichever it is — the behaviour of
    /// `DesktopCapture::new()`. Resolved at each opening, so a
    /// reopening may legitimately land on another output.
    Bureau,
    /// A specific output, designated by its name.
    Sortie(String),
}

/// Reads the duplication if it is present, or the failure corresponding to its
/// absence.
///
/// **Re-read fix, task 6 quater.** `rouvrir()` sets `None` then
/// may exit with an error on one of its three external calls (DXGI factory,
/// output resolution, the duplication itself): in that case `None`
/// PERSISTS beyond `rouvrir()`, until the next attempt — an
/// earlier comment wrongly claimed the opposite. Returning
/// `AccesPerdu(last_lost_code)` rather than a `Panne` is what lets
/// `next_frame` retry the reopening instead of declaring the source
/// exhausted on a failure that is in no way definitive: an absence of duplication
/// outside `rouvrir()` is therefore, since this fix, no longer a
/// programming defect — it is a normal state of the resumption window.
pub(super) fn lire(
    duplication: &Option<IDXGIOutputDuplication>,
    last_lost_code: i32,
) -> std::result::Result<&IDXGIOutputDuplication, EchecAcquisition> {
    duplication
        .as_ref()
        .ok_or(EchecAcquisition::AccesPerdu(last_lost_code))
}
