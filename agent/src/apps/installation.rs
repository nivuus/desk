//! Installing software dropped by the user: download,
//! run, and say what really happened.
//!
//! ⚠️ THIS MODULE IS DECLARED WITHOUT `cfg` from `apps.rs`, exactly as
//! `apps` itself is from `main.rs`, and it is its Windows children that
//! carry theirs. That is what makes `verdict`, `fenetre`, `reponse`,
//! `depot`, `cadence` and `telechargement` exist on the Linux host, where their tests
//! run — the "Child module convention" of `docs/claude/module-conventions.md` is therefore not
//! invoked: no module crosses a `#[cfg(windows)]` boundary here.
//!
//! 🔴 **ONLY `execution` CARRIES THE `cfg`.** Everything that could leave it has
//! left — the verdict, the counting window, the paths, the extensions, the
//! cadence, parsing HTTP headers and the download itself —, and
//! that is where the coverage is: `execution.rs` has, on the host, no
//! possible test outside `cargo check --target x86_64-pc-windows-gnu`.

pub mod cadence;
pub mod depot;
pub mod fenetre;
pub mod journal;
pub mod partage;
pub mod reponse;
pub mod telechargement;
pub mod verdict;

#[cfg(windows)]
pub mod execution;
#[cfg(windows)]
pub mod fil;
#[cfg(windows)]
pub mod peripherique_audio;
