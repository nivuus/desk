//! Congestion controller: decides the encoding bitrate and resolution
//! from what the peer reports.
//!
//! No dependency on Windows, str0m or the socket — that is what makes
//! the whole policy testable on Linux, without a VM and without a network. Same
//! reason to be as `geometry.rs`, `rebuild.rs` and `clock.rs`.
//!
//! Split into four sub-modules: `echelle` (the available resolutions),
//! `hysteresis` (the delays before a change), `controleur` (the continuous
//! control loop) and `reconfiguration` (the source size change).
//!
//! Visibility among the four: what a sub-module must expose to its
//! siblings (fields of `Controleur`, hysteresis constants, test
//! utility functions) is marked `pub(super)`, never `pub` — bounded to the
//! `congestion` module alone, the outside only seeing what
//! `pub use controleur::Controleur` re-exports. A helper nested in a private `mod
//! tests` would only have been visible to its own subtree, hence this
//! choice whenever a detail is shared between two sibling
//! sub-modules.

mod controleur;
mod echelle;
mod hysteresis;
mod reconfiguration;

pub use controleur::Controleur;

use std::time::{Duration, Instant};

/// Share of the estimate we allow ourselves to consume.
///
/// The remaining 10 % leave room for RTX retransmissions and the probing packets
/// the BWE subsystem emits to test upwards. Aiming at
/// 100 % of the estimate guarantees exceeding it.
const MARGE: f32 = 0.9;

/// Relative gap below which the bitrate is not reconfigured. Without it,
/// a quivering estimate would make the encoder be written every second.
const ECART_MINIMAL_DEBIT: f32 = 0.10;

/// Cap on the loss percentage declared to Opus. Beyond it, LBRR
/// redundancy costs more bitrate than it saves.
const PERTE_MAX_OPUS: i32 = 25;

/// Fixed settings of a session.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Video bitrate ceiling, in bits per second (variable `BITRATE`).
    /// Also serves as the fallback value when no estimate arrives.
    pub plafond_bps: u32,
    /// Budget reserved for the audio track, taken out of the estimate.
    pub audio_bps: u32,
    /// Size of the captured source, top of the ladder.
    pub source: (u32, u32),
    pub fps: u32,
}

/// What the transport observes, once per second.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    /// Outgoing bandwidth estimate. `None` as long as none has
    /// arrived — normal case at start-up, permanent case if TWCC is not
    /// negotiated.
    pub estimate_bps: Option<u32>,
    /// Not consumed by the decision: logged by `transport/evenements.rs` so that
    /// the acceptance run has the RTT seen by the agent, to compare with the one the
    /// browser reports.
    pub rtt: Option<Duration>,
    /// Fraction of packets lost, between 0 and 1.
    pub loss: Option<f32>,
    pub at: Instant,
}

/// State of the link as announced to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualite {
    /// Highest rung.
    Bonne,
    /// Reduced resolution: the user must know why the picture softened.
    Degradee,
    /// Floor reached. We no longer degrade — we say so.
    Insuffisante,
}

/// Is the controller receiving anything to control on?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adaptation {
    Active,
    /// No estimate has ever arrived. The bitrate stays at the ceiling, and
    /// this fact must be announced — silence would look like "all is well".
    Indisponible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub video_bitrate_bps: u32,
    pub encode_size: (u32, u32),
    pub opus_loss_perc: i32,
    pub qualite: Qualite,
    pub adaptation: Adaptation,
}
