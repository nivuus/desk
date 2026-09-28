//! `CursorShape` — the cursor shape, extracted from `control.rs`.
//!
//! ⚠️ **PRIOR EXTRACTION, not a rework.** Sub-block E3 adds the
//! `AgentControl::MicState` variant to `control.rs`, which was at **444** lines
//! for a gate at 500: the addition documented at the density of `Accent` would have
//! taken it beyond 490. The repository's doctrine is to **extract BEFORE
//! adding**, never to compress after crossing — this repository crossed that
//! ceiling five times and caught up twice by a compression it
//! forbids itself.
//!
//! ⚠️ **The content is VERBATIM.** Only one thing changed, and it is the only one
//! that had the right to change: the `use serde::…` the parent already carried
//! is repeated here, a child module not seeing its parent's imports. The
//! type stays `pub`, and `control.rs` re-exports it, so that no call
//! site of `proto::control::CursorShape` has moved.

use serde::{Deserialize, Serialize};

/// Cursor shape, expressed directly in the vocabulary of the
/// CSS `cursor` property: the client sets it as is, with no mapping
/// table to maintain on its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    Default,
    Text,
    Wait,
    Progress,
    Crosshair,
    Pointer,
    Move,
    NotAllowed,
    Help,
    NsResize,
    EwResize,
    NwseResize,
    NeswResize,
}
