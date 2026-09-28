//! Messages of the platform <-> agent channel (`/agent`).
//!
//! Versioned JSON format: `{"type":"...","v":2,...}` (`type` serves as the internal
//! tag of the enum and is always emitted first by serde). The `v` field
//! is mandatory and checked on deserialization: a message without `v`, or
//! with a `v` other than [`PLATEFORME_VERSION`], is rejected.
//!
//! ⚠️ THIS MODULE IS DISTINCT FROM [`crate::control`], AND IT IS NO ACCIDENT.
//! `control` versions the **agent <-> browser** data channel; this one
//! versions the **agent <-> platform** channel. The two evolve for
//! unrelated reasons, and a shared constant would force each to move
//! when the other changes — which would make any bump unreadable.
//!
//! 🔴 WHAT HAPPENS WHEN THE VERSIONS DIVERGE IS NOT SYMMETRIC, because
//! the two ends do not have the same power:
//!   - the agent sends a version the platform does not know: it
//!     answers `{v, type:"refus", motif:"version"}`, logs WITH the version
//!     received, and closes the socket. No downward negotiation: there is
//!     only one version;
//!   - the platform sends a version the agent does not know:
//!     `serde_json::from_str` fails, the agent logs with the text of
//!     the error and closes the channel.
//!
//! 🔴 A VERSION DIVERGENCE MUST NEVER READ AS A NETWORK FAILURE.
//! A `version` refusal IS NOT RETRIED; a socket drop is. Two
//! behaviours, two distinct traces — otherwise a version incompatibility
//! would disguise itself as an infinite reconnection loop, which is the most
//! costly failure mode to diagnose.
//!
//! ⚠️ **THIS PARAGRAPH WAS REFUTED BY MEASUREMENT, AND IT BECAME TRUE AGAIN THROUGH
//! THE FIX OF 20 AUGUST 2026.** The survey that refuted it (acceptance run of
//! sub-block G1, ONE run, log
//! `docs/superpowers/plans/journaux-gestion-apps/step3-version-v1-contre-v2-plat.log`):
//! a v1 agent facing a v2 platform logged **0** line "the
//! platform REFUSES the version" and **10** pairs "unreadable message from the platform
//! (diverging version?)" / "resuming the /agent channel", up to
//! the 30 s step, with no end.
//!
//! **The cause was in this file**, and it was structural:
//! `check_version` is a `deserialize_with` set on the `v` field of
//! **every** message, and the platform emits its refusal with ITS version —
//! `{"type":"refus","v":2,"motif":"version"}`. An agent of version N
//! could therefore NEVER READ the refusal of a platform of version M ≠ N: it
//! fell into the "unreadable" branch, which is resumable, and the
//! `version` arm of `sur_refus` was reachable only if both ends
//! already agreed on `v` — that is, never in the only case
//! for which it exists.
//!
//! 🔴 **THE PROTOCOL DECISION, AND ITS PRICE.** The refusal is no longer a message
//! versioned like the others: it is a **MINIMAL ENVELOPE OUTSIDE
//! VERSIONING**, and that reads in three clauses.
//!
//!   1. **Its `v` field is TOLERATED, never checked** (`version_toleree`). It
//!      stays MANDATORY and stays an integer — it says who is speaking, and that is
//!      logged — but no value makes it rejected. A refusal is the
//!      only message whose meaning depends on no version: it says "I will not
//!      serve you", and that is understood without negotiation.
//!   2. **Its `motif` field is a FREE WORD on the wire** (`String`), not a
//!      closed enum. Without this second clause the remedy would only hold
//!      until the first reason added by a future version: the refusal
//!      would become unreadable again, in the "unreadable" branch, and the
//!      failure mode would come back identically. The table of known reasons lives in
//!      [`MotifCanal::depuis_mot`]; what it does not recognise is
//!      logged **verbatim** rather than lost.
//!   3. 🔴 **ITS SHAPE IS FROZEN: `type`, `v`, `motif`, AND NOTHING ELSE,
//!      EVER.** This enum carries `deny_unknown_fields`; a field added to the
//!      refusal by a future version would be rejected by the earlier
//!      versions, and would on its own render clauses 1 and 2 ineffective.
//!      That is the price of the decision, and it is written here because nothing in
//!      the type prevents it.
//!
//! **What has NOT changed, and must not change**: all the other
//! messages stay strictly versioned. An `enrole` of an unknown version
//! may give a known field a meaning we are unaware of; accepting it would be
//! worse than rejecting it. Two tests guard each half, on each of the two
//! ends.

use serde::{Deserialize, Serialize};

/// Version of the platform <-> agent channel protocol. Increment on any
/// format change.
///
/// v1 (sub-block P3): enrolment, heartbeat, agent token.
/// v2 (sub-block G1): application catalogue, launch order.
/// v3 (sub-block G2): 256 icons, their provenance, and the inventory of the
///                     missing ones.
/// v4 (sub-block G3): installation — `installer` downstream, `progression`
///                     and `termine` upstream, and the two enumerations
///                     [`Phase`] and [`Issue`] they carry.
///
/// 🔴 THIS SET IS NOT DECORATIVE: WITHOUT ITS LINE, THE CONSTANT LIES. A
/// reader who comes looking for what the current version carries would leave
/// with the one before last, and would believe the protocol smaller than it is.
///
/// 🔴 **EVERY** STEP UP MAKES EVERY ALREADY DEPLOYED AGENT OBSOLETE — the reasoning
/// below is written for the step to 2, it holds word for word for those to
/// 3 and to 4, and it is NOT rewritten at each bump: rereading it in the present is
/// what we want, renumbering it each time would say nothing more.
///
/// 🔴 THE STEP TO 2 MAKES EVERY ALREADY DEPLOYED AGENT OBSOLETE, and it is a
/// decision, not a side effect. A v1 agent receives `refus{motif:version}`
/// and DOES NOT RETRY (header of this module): agent and platform are
/// deployed AT THE SAME COMMIT, otherwise the VM falls silent without looping, which is
/// exactly the intended behaviour — a frank silence rather than an infinite
/// reconnection.
///
/// ❌ **"THE VM FALLS SILENT WITHOUT LOOPING" IS FALSE, MEASURED** — see the box in
/// the header of this module. The VM loops, at 30 s intervals and with no end.
/// The obligation to deploy both ends at the same commit, however, is
/// UNCHANGED and even reinforced: it is the only safeguard that exists today.
///
/// ✅ **THIS BOX IS A DATED SURVEY (G1 acceptance run), AND IT WAS REFUTED ON
/// 20 AUGUST 2026 — it is ANNOTATED rather than erased.** The fix of the same
/// day (commit `457a7f8`, the three clauses at the top of this module) took the
/// refusal out of versioning: an obsolete agent now READS the refusal that
/// tells it so, logs it with both versions, and GIVES UP. It
/// no longer loops. **The break is still a break; it has only
/// become DIAGNOSABLE**, and the step to 3 of sub-block G2 is the first
/// bump since that fix — hence the first able to PROVE it.
/// The obligation to deploy both ends at the same commit is, for its part,
/// strictly unchanged.
pub const PLATEFORME_VERSION: u8 = 5;

/// The three field readers called by `deserialize_with`, extracted so
/// that this file does not cross 500 lines when welcoming sub-block G3.
///
/// 🔴 THE `use` IS NOT COSMETIC: `serde` resolves the path of a
/// `deserialize_with = "check_version"` **in the scope of the module carrying
/// the attribute**. It is what lets the extraction touch NONE of the
/// attributes of the structures below, hence be a pure transposition.
mod champs;
use champs::{check_version, icone_obligatoire, option_obligatoire, version_toleree};

/// The table of refusal reasons, extracted for the same reason.
mod motifs;
pub use motifs::MotifCanal;

/// The APPLICATION MANAGEMENT types live in a child module.
///
/// 🔴 EXTRACTED BECAUSE THIS FILE CROSSED 500 LINES — 588 —, and the
/// doctrine of `CLAUDE.md` is to catch up through an EXTRACTION, never through a
/// compression. ⚠️ **It should have PRECEDED the addition**: the G2 plan had
/// named three extractions to carry out in advance, all three were carried out, and
/// this one was not planned. The crossing is DECLARED.
///
/// ⚠️ It is NOT the "Child module convention" of `CLAUDE.md`, which targets
/// modules extracted from a `#[cfg(windows)]` parent: it is the same mechanism
/// used for the other reason — the 500-line rule.
mod apps;
pub use apps::{Application, IssueLancement, SourceMax};

/// The INSTALLATION payload types, in a sibling module of `apps`,
/// for the same reason and by the same mechanism.
mod installation;
pub use installation::{Issue, Phase};

/// The builders of both enums, extracted so that this file does not exceed
/// 500 lines. See the module header — it is its SECOND crossing.
mod constructeurs;

/// Message from the agent to the platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VersLaPlateforme {
    /// Enrol: present the VM name and the enrolment secret.
    Enroler {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        vm: String,
        secret: String,
    },
    /// Heartbeat. Advances `vu_a`, and returns a fresh token.
    Battement {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
    },
    /// The VM's application catalogue, as a DIFF.
    ///
    /// 🔴 `complet` HAS A NAMED SEMANTICS, and it is the only one that makes
    /// the platform state rebuildable: at `true`, the platform
    /// marks as gone EVERY row of this VM absent from `applications` and
    /// ignores `disparues`; at `false`, it applies the delta.
    ///
    /// The agent emits `complet = true` at each (re)enrolment. That is what
    /// makes the loss of an upstream message harmless: this channel is a
    /// WebSocket `push`, with no delivery guarantee, and without this complete
    /// resend a `Catalogue` lost during an outage would leave the
    /// platform divergent WITH NO END.
    Catalogue {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        complet: bool,
        applications: Vec<Application>,
        /// KEYS, never objects: the platform only needs
        /// the identity to mark a disappearance.
        disparues: Vec<String>,
    },
    /// The outcome of a launch order, matched by `demande`.
    Lancee {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        demande: String,
        issue: IssueLancement,
    },
    /// Where a running installation stands.
    ///
    /// 🔴 **SAMPLED, AND IT IS A SAFETY CONSTRAINT, NOT A
    /// CONVENIENCE ONE.** The agent's upstream queue is bounded to `FILE_EMISSION`
    /// (32) and **drops whatever overflows**. A progress update emitted per slice
    /// of 64 KiB would saturate it and drown the shared log — it is the
    /// doctrine this repository paid for in the TURN workstream: *count or
    /// sample, never trace per packet*. The rule lives in
    /// `agent/src/apps/installation/cadence.rs`, clock as a parameter.
    ///
    /// ⚠️ `octets_total` IS ZERO IN THE `Execution` PHASE, where there is nothing to
    /// total: it is `ecoule_ms` that carries the information, and the interface
    /// shows an indeterminate state.
    Progression {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        installation: String,
        phase: Phase,
        octets_faits: u64,
        octets_total: u64,
        ecoule_ms: u64,
    },
    /// The installation is finished, and here is what really happened.
    ///
    /// 🔴 **`code_sortie` IS AN `Option`, NEVER AN `i32` WITH A `-1`
    /// SENTINEL**: "no code" and "code −1" are two different
    /// facts, and a sentinel would conflate them exactly as a
    /// `source_max_px` at `0` would conflate "unknown" and "zero". It is
    /// REPORTED, never interpreted — see [`Issue`].
    ///
    /// ⚠️ **AN EMPTY `journal` IS THE NORMAL CASE**, not a failure: most
    /// Windows installers are graphical and write nothing on the standard
    /// streams. The interface must not present it as a failure.
    ///
    /// ⚠️ `journal_tronque` SAYS THE TAIL WAS CUT, and it is distinct
    /// from an empty log: without it, a user would read the last
    /// 64 KiB believing they read everything.
    Termine {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        installation: String,
        issue: Issue,
        /// Mandatory on the wire — see `champs::option_obligatoire`.
        #[serde(deserialize_with = "option_obligatoire")]
        motif: Option<String>,
        /// Likewise. `None` = "the code could not be collected".
        #[serde(deserialize_with = "option_obligatoire")]
        code_sortie: Option<i32>,
        journal: String,
        journal_tronque: bool,
    },
}

/// Message from the platform to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DepuisLaPlateforme {
    /// The enrolment is accepted: here is the session prefix and the token.
    Enrole {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        prefixe: String,
        jeton: String,
        /// In MILLISECONDS, like every timestamp of this service.
        expire_a: i64,
    },
    /// The heartbeat is recorded: here is a FRESH token.
    ///
    /// ⚠️ A fresh token AT EACH heartbeat, and not the same one: the access token
    /// lasts ten minutes, and an agent that kept the first would fall at its
    /// expiry without seeing it coming.
    BattementRecu {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        jeton: String,
        expire_a: i64,
    },
    /// Refusal, with its reason. The socket then closes.
    ///
    /// 🔴 **THE ONLY VARIANT OUTSIDE VERSIONING IN THIS WHOLE PROTOCOL**, and
    /// the three clauses that govern it are at the top of the module. In two
    /// words: `v` is tolerated, `motif` is a free word, and **the shape is
    /// frozen — no field must ever be added to it**.
    Refus {
        /// The SENDER's version, as it arrives. May differ from
        /// [`PLATEFORME_VERSION`]: it is even the only case for which this
        /// variant exists.
        #[serde(rename = "v", deserialize_with = "version_toleree")]
        version: u8,
        /// The raw word. [`MotifCanal::depuis_mot`] interprets it when it
        /// can; the caller logs the word itself when it
        /// cannot.
        motif: String,
    },
    /// Launch an application of the VM.
    ///
    /// ⚠️ THE ORDER DOES NOT CARRY THE SHORTCUT PATH, it carries the key, and
    /// the agent resolves it in ITS OWN catalogue — the one it just
    /// read from disk. The platform's copy may be one reconciliation
    /// old; the agent's never is.
    ///
    /// `demande` matches the order to its [`VersLaPlateforme::Lancee`], the HTTP
    /// route awaiting that answer.
    Lancer {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        demande: String,
        cle: String,
    },
    /// The fingerprints the platform does NOT have, among those the last
    /// [`VersLaPlateforme::Catalogue`] announced.
    ///
    /// 🔴 IT IS NOT EMITTED WHEN THE SET IS EMPTY: an empty set
    /// would cost one message per reconciliation on an idle disk, which is what
    /// the diff of sub-block G1 exists precisely to avoid.
    ///
    /// ⚠️ **THE BYTES NEVER TAKE IT**: this message only carries an
    /// inventory. The images go through `PUT /icone/:sha256`, exactly
    /// like the installers of G3 (spec D7) — the channel is JSON, it carries
    /// the heartbeat, and 4.4 MB in base64 would cost +33 % there and
    /// would block that heartbeat.
    ///
    /// ⚠️ **A lost `IconesManquantes` breaks nothing**: the channel is a
    /// `push` without delivery guarantee, and the next reconciliation
    /// replays the announcement. It is the same net as `complet = true` at each
    /// re-enrolment (decision D3 of G1), and the G1 acceptance run saw it
    /// work on the real path.
    IconesManquantes {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        empreintes: Vec<String>,
    },
    /// Install a piece of software the user uploaded.
    ///
    /// 🔴 **THE BYTES NEVER TAKE THIS MESSAGE**, and it is the same
    /// rule as for [`Self::IconesManquantes`]: this channel is JSON, it
    /// carries the heartbeat, and an 8 MiB slice would cost +33 % there
    /// in base64 while blocking that heartbeat. The order carries a **URL**, and
    /// the agent pulls the bytes over HTTP, with its agent token.
    ///
    /// ⚠️ `sha256` IS THE FINGERPRINT OF THE WHOLE FILE, the same value the
    /// browser announced and the platform recomputed at sealing.
    /// **One single value, comparable everywhere** — including by a human with
    /// a `sha256sum`. That is why it is NOT a tree fingerprint over
    /// the slices, which would have been native and free on the browser side but
    /// incomparable everywhere else.
    ///
    /// 🔴 **THE AGENT RECOMPUTES THIS FINGERPRINT AFTER WRITING**, and it is the
    /// THIRD of the three checks: the browser can lie, the
    /// platform's disk can get corrupted, the transfer can truncate.
    /// **No hop trusts the previous one.**
    ///
    /// ⚠️ **THIS MESSAGE IS RE-EMITTED AT EACH ENROLMENT** as long as the installation
    /// is pending: a WebSocket `push` has no delivery guarantee,
    /// and without this re-emission an order emitted during an outage would be lost
    /// WITH NO END. It is the same net as the catalogue's `complet = true`.
    /// **The agent therefore deduplicates by `installation`, and its memory is ON
    /// DISK** — see `agent/src/apps/installation/depot.rs`.
    Installer {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        installation: String,
        url: String,
        nom: String,
        #[serde(rename = "taille")]
        size: u64,
        sha256: String,
    },
}

#[cfg(test)]
#[path = "plateforme/tests.rs"]
mod tests;

// 🔴 THE SECOND TEST FILE WAS BORN FROM A RECORDED DEBT, NOT FROM A TASTE.
// `plateforme/tests.rs` was at 561 lines — above the 500 ceiling of
// `CLAUDE.md`, which recorded it in the debt table WITH NO landing point. The
// sub-block G2 works in it, so it split it: lifecycle here,
// app management there. It is the same `#[path]` mechanism as the line above,
// used for the same reason — the 500-line rule —, and NOT the
// "Child module convention" of `CLAUDE.md`, which targets modules extracted
// from a `#[cfg(windows)]` parent.
#[cfg(test)]
#[path = "plateforme/tests_apps.rs"]
mod tests_apps;

// 🔴 A THIRD TEST FILE, AND IT HAS ITS OWN REASON: the shared
// vectors are a set of ROUND-TRIPS, which says nothing of what must be
// REFUSED. The most fragile guard of v4 — a `termine` whose optional
// key is MISSING — therefore cannot be tested there, and it lives here.
#[cfg(test)]
#[path = "plateforme/tests_installation.rs"]
mod tests_installation;

// 🔴 A FOURTH TEST FILE, BORN FROM A SECOND CROSSING. `tests.rs`
// went back above 500 under the additions of G3, and the two tests that
// `plateforme-vectors.json` DRIVES left it — it is the boundary of the
// source of truth, not a convenience split.
#[cfg(test)]
#[path = "plateforme/tests_vecteurs.rs"]
mod tests_vecteurs;
