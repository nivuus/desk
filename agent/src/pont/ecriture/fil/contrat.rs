//! The write thread's contract: what it **receives** ([`Ordre`]) and what it
//! **needs** ([`Config`]).
//!
//! # What this extraction IS, and what it is NOT
//!
//! **It adds no behaviour.** The two declarations are transposed
//! **VERBATIM** from `fil.rs` (l. 97-127), with their doc comments, and
//! `fil.rs` re-exports them through `pub use contrat::{Config, Ordre};`: **no
//! call site moves.**
//!
//! It comes **BEFORE** the addition it hosts, and not after: `fil.rs`
//! was at **472** lines, margin **28**, and F5 must add `Ordre::Bonjour`
//! to it plus its arm in `Fil::traiter` — which would have brought it between 482 and 490,
//! too tight for the review that follows. It is the gesture D9 invented
//! (`capteur/serveur/instances.rs`, margin returned from 10 to 65) and that D10 played
//! three times. **Never a compression**, which `CLAUDE.md` forbids by name.
//!
//! ⚠️ **Why THESE two and not `Fil` nor `EnCours`.** The two extracted
//! are `pub`: their type follows their visibility. `Fil::en_cours` is `pub(super)`
//! and so is its type `EnCours`; taking them would require hoisting a
//! visibility, and `fil.rs:138-142` already carries the finding that a **strictly
//! verbatim extraction does not compile** when a private item of a child
//! module must become visible to its parent — `rustc` says so through
//! `private_interfaces`, *a warning of a family other than `dead_code`*.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::pont::ecriture::Evenement;
use crate::pont::table::Table;
use crate::pont::transport::VersNavigateur;
use proto::fichiers::CodeEchec;
/// What the write thread receives.
///
/// ⚠️ **A SINGLE CHANNEL, and it is a declared divergence from F2's plan**,
/// whose signature takes **two** `Receiver`s (the events, the acks).
/// Two receivers on a blocking thread would impose alternating polling, hence
/// a latency bounded by yet another arbitrary delay — and a test that depends
/// on a `sleep`. A single channel makes the loop deterministic, hence testable
/// without sleeping: the property `pont::table` gave itself for expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// A ProjFS notification named a path.
    Survenu(Evenement),
    /// The browser acknowledged a chunk.
    Fait { correlation: u32 },
    /// The browser refused, or the command expired.
    Echec { correlation: u32, code: CodeEchec },
    /// **F5** — the browser announced itself, and it says on which root.
    ///
    /// 🔴 **IT IS THE ONLY ORDER THAT TRIGGERS RESUMPTION**, and it is what
    /// closes the thirty-second window F2 measured two times out of two.
    /// Before F5, `Fil::demarrer` called `reprendre()` **at the thread's
    /// start** — that is, at the bridge's start, *without knowing whether a browser
    /// is there, nor which one, nor on which directory*. F2 recorded the replay's
    /// push **0.8 s BEFORE** the browser announced its mount, then
    /// `command expired … correlation=0` **+30.2 s** later: *"the indicator
    /// that exists to denounce the loss is SILENT for thirty seconds."*
    ///
    /// It is **literally the remedy F2 named**: "that the bridge only opens
    /// its write channel after an acknowledgement from the writer".
    ///
    /// ⚠️ **There was NO order matching `CanalOuvert`**, and it is
    /// the structural cause of that window: the thread could not know that
    /// the browser was ready, since no one told it.
    Bonjour {
        /// The name of the root the user chose. **A hint, not a
        /// proof** — see [`crate::pont::bonjour`].
        racine: String,
        /// The user confirmed wanting to push despite a different name.
        forcer: bool,
    },
}

/// What the thread needs to run.
pub struct Config {
    /// The virtualisation root, where hydrated files live.
    pub racine: PathBuf,
    /// The resumption journal, **outside the root**.
    pub chemin_journal: PathBuf,
    /// **The SAME table as reads**: two sources of correlations on a
    /// single channel would collide silently.
    pub table: Arc<Mutex<Table>>,
    pub vers_navigateur: Sender<VersNavigateur>,
    /// `PONT_ECRITURE`: `false` = the disarmed arm of the A/B.
    pub armee: bool,
}
