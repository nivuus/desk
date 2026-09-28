//! What DXGI exposes of a display output, without anything depending on
//! the Windows API itself.
//!
//! Extracted from `capture.rs` (which stays `#![cfg(windows)]` as a whole)
//! so that `superviseur::placement::sortie_par_dimensions`, purely
//! algorithmic, can compile and be tested on the Linux host —
//! `crate::capture` does not exist at all outside Windows, so no
//! type depending on it can cross that boundary. `capture.rs`
//! re-exports this type (`pub use crate::sortie_dxgi::SortieDxgi;`) so that
//! `crate::capture::SortieDxgi` stays the same type on the Windows side — same
//! precedent as `windows_source_sortie::region_de_sortie`, which already moves out the
//! portable part of an otherwise `#[cfg(windows)]` module.

/// What we know of a DXGI output, without duplicating any of it.
#[derive(Debug, Clone)]
pub struct SortieDxgi {
    pub index_adaptateur: u32,
    pub index_sortie: u32,
    pub adaptateur: String,
    pub nom_sortie: String,
    pub attachee_au_bureau: bool,
    /// Position and dimensions in virtual desktop coordinates.
    pub rect: crate::geometry::Rect,
}
