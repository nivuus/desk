//! `LinkQuality` and `LinkAdaptation` — the two vocabularies of the link state,
//! extracted from `control.rs`.
//!
//! ⚠️ **EXTRACTION, not a rework.** The cross-cutting review of block E3 took
//! `control.rs` to **496** lines, i.e. a margin of **4** — it is the trap this
//! repository has paid since S2: *documenting an extraction takes back part of the
//! margin it frees*, and the round that denounces the drift produces it. The
//! doctrine is to extract, never to compress, and above all never to
//! shorten a rebuttal to reach a line count.
//!
//! ⚠️ **The content is VERBATIM.** Only the `use serde::…` is repeated — a child
//! module does not see its parent's imports —, and both types stay
//! `pub`, re-exported by `control.rs`: no call site of
//! `proto::control::LinkQuality` nor of `proto::control::LinkAdaptation` has
//! moved.

use serde::{Deserialize, Serialize};

/// What the user must understand of the link state.
///
/// Three values and not a boolean: "degraded" and "insufficient" are two
/// distinct situations, and the second is not deduced from the first by
/// a negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkQuality {
    /// Full resolution, comfortable link.
    Bonne,
    /// Resolution reduced to hold the link.
    Degradee,
    /// Floor reached: the link no longer allows fast-paced gaming. It is
    /// the explicit warning required by the gaming framing (§3).
    Insuffisante,
}

/// Does the agent receive something to regulate itself on?
///
/// Independent of `LinkQuality`: a session without a bandwidth
/// estimate can very well run as `Bonne` on a wide link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAdaptation {
    Active,
    /// No estimate reaches the agent: the bitrate stays frozen at the
    /// configured ceiling. To be said, not kept quiet.
    Indisponible,
}
