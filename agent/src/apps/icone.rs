//! The icon of an application: extract it, encode it, and PROVE where it
//! comes from.
//!
//! ⚠️ THIS MODULE IS DECLARED WITHOUT `cfg` IN `apps.rs`, AND IT IS ITS WINDOWS
//! PARTS THAT CARRY THEIRS. It is the exact shape of `apps.rs` itself,
//! one level down, and it is what makes `apps::icone::ressource`,
//! `::magasin` and `::source` exist on the Linux host, where their tests run.
//!
//! 🔴 **DIVERGENCE FROM THE G2 PLAN, NOTED AND SETTLED RATHER THAN
//! COPIED.** The plan writes two things that do not hold together: that this
//! very file is `#[cfg(windows)]`, AND that its pure children are declared
//! "in `apps.rs` … and this avoids the `#[path]`". But one cannot declare
//! a GRANDchild from the grandparent without `#[path]`: if `icone` were gated,
//! `apps::icone::ressource` would not exist on the host, and making it live there
//! would require precisely the `#[path]` the plan says it avoids. What is
//! kept is therefore its INTENT — the pure modules exist on the host, and
//! no `#[path]` is used — through the only construction that serves it: the
//! parent is free of `cfg`, its Windows parts are gated inside.
//!
//! The "Child module convention" of `CLAUDE.md` is **not** invoked:
//! no module crosses a `#[cfg(windows)]` boundary here.

/// 🔴 THE PROOF OF THE SUB-BLOCK, AND IT IS PURE: reading a `GRPICONDIR` or an
/// `ICONDIR` from a `&[u8]`.
pub mod ressource;

/// The in-memory store, content-addressed. **PURE.**
pub mod magasin;
/// Where the icon of a shortcut comes from. **PURE.**
pub mod source;

// ---------------------------------------------------------------------------
// What follows is `#[cfg(windows)]`: extraction through the Shell, PNG
// encoding through WIC, and reading the resource. Nothing there DECIDES — the decisions
// live in the three pure modules above.
// ---------------------------------------------------------------------------

/// Les octets bruts d'un `GRPICONDIR`. **`#[cfg(windows)]`.**
#[cfg(windows)]
pub mod lecture_pe;

#[cfg(windows)]
mod extraction;

#[cfg(windows)]
pub use extraction::{armee, extraire, provenance_de};

/// The HTTP upload of icons to the platform.
///
/// ⚠️ WITHOUT `cfg`: its two PURE parts — deriving the authority and
/// reading the status — are tested on the host, and they are what decides the
/// refusal of TLS.
pub mod televersement;
