//! The JSON headers of the file bridge frames. **PURE** — no `cfg`,
//! no dependency on `windows`, wire shapes **pinned by shared
//! vectors**.
//!
//! # Why this module lives HERE and not in the agent
//!
//! It lived there, for the span of one commit: `agent/src/pont/entetes.rs` carried these
//! seven structures and itself declared that it was a debt, because the
//! scope of the task that wrote them forbade touching `proto/`.
//! Its wording was exact, and it is what decided the move:
//! **"a field renamed here breaks the bridge without breaking a single test on the
//! client side"**.
//!
//! This repository knows that pattern by heart, and paid for it twice: `TYPES_AGENT`
//! (`proto/ts/control.ts`) is a hand-written set that nothing confronts
//! with the union it mirrors, and a `battement-recu` variant stayed green
//! on fifty tests because nothing pinned its bytes. Spec §4.2
//! moreover names `proto/` as the "single source of truth": the debt was
//! a scope divergence, not a design decision.
//!
//! `proto` therefore now carries both layers: the **frame** (version, type,
//! correlation, header length) and the **headers** it carries.
//!
//! # The check that catches a rename
//!
//! `proto/fichiers-vectors.json` freezes the EXACT JSON string of each shape, and (policy: allow-fr - file name)
//! it is read by **both** implementations:
//!
//! - `proto/src/files/entetes/tests.rs` (Rust);
//! - `proto/ts/fichiers-entetes.test.ts` (TypeScript). (policy: allow-fr - file name)
//!
//! Renaming a field on one side only turns that side RED — measured, see the
//! report of task 15. Both tests also check the `version` key
//! of the file, which `vectors.json` does only on the TypeScript side.
//!
//! **No `#[serde(default)]` anywhere**: an incomplete header is rejected,
//! never silently completed — the version doctrine of
//! [`crate::control`], applied to headers.
//!
//! ⚠️ **The split is the SAME on both sides**: the frame in
//! `files.rs`/`fichiers.ts`, the headers in `files/entetes.rs` and (policy: allow-fr - file name)
//! `fichiers-entetes.ts`. A split asymmetry would make the pairing harder (policy: allow-fr - file name)
//! to reread than to write.

use serde::{Deserialize, Serialize};

/// The header of `TYPE_LISTER` and of `TYPE_ATTRIBUTS`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Chemin {
    /// Logical path, components separated by `/`, **normalised** on the agent side by
    /// `pont::chemins`. Empty = the root.
    pub chemin: String,
}

/// The header of `TYPE_LIRE`. The range is **one chunk**, never the whole
/// file: it is `pont::decoupe` that produces it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Lire {
    pub chemin: String,
    pub position: u64,
    #[serde(rename = "longueur")]
    pub length: u32,
}

/// A directory entry, in the `TYPE_ENTREES` answer.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EntreeJson {
    pub nom: String,
    pub repertoire: bool,
    #[serde(rename = "taille")]
    pub size: u64,
    /// `File.lastModified`: milliseconds since the Unix epoch, **signed** —
    /// a file older than 1970 yields a negative one, and refusing it would make
    /// an enumeration fail over a date.
    #[serde(rename = "modifie")]
    pub modified: i64,
}

/// The header of `TYPE_ENTREES`. **Empty** binary payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Entrees {
    pub entrees: Vec<EntreeJson>,
}

/// The header of `TYPE_META`. **Empty** binary payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// 🔴 **THE CANONICAL NAME — THE ONE STORED ON THE LOCAL MACHINE, never
    /// the one the application typed** (F3).
    ///
    /// It is consequence ① of the case canonicaliser: without it, a
    /// `GROS.BIN` requested over a local `gros.bin` would create a placeholder
    /// named `GROS.BIN` in the virtualisation root. The root being NTFS,
    /// hence case-insensitive, the open would succeed — but **an
    /// enumeration of the parent would return `gros.bin` and the placeholder would carry
    /// `GROS.BIN`**: two names for one file, one of which exists
    /// nowhere.
    ///
    /// ⚠️ **EMPTY for the ROOT itself**, which has no name.
    ///
    /// ⚠️ **REQUIRED FIELD, without `#[serde(default)]`** — the doctrine of this module
    /// carries none. A peer older than F3 therefore cannot produce it:
    /// it is a break, and `FILES_VERSION` **stays 1** because both
    /// ends of this bridge are always deployed together (a single `agent.exe`,
    /// a single shell page). *Saying so rather than suggesting a
    /// compatible addition.*
    pub nom: String,
    pub repertoire: bool,
    #[serde(rename = "taille")]
    pub size: u64,
    #[serde(rename = "modifie")]
    pub modified: i64,
}

/// The header of `TYPE_DATA`. **The payload carries the bytes**, never encoded.
///
/// `length` is redundant with the payload size, **and that is
/// deliberate**: the decoder can thus refuse a frame whose header and
/// payload contradict each other, rather than writing into the ProjFS buffer a
/// quantity of bytes the sender did not believe it was sending.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Data {
    pub position: u64,
    #[serde(rename = "longueur")]
    pub length: u32,
}

/// The header of `TYPE_WRITE`. **The payload carries the bytes**, never encoded.
///
/// ⚠️ **`premier` and `last` are NOT deducible from `position` and
/// `length`.** A single-chunk file carries both at `true`;
/// a file of ZERO size has no chunk at all and goes through
/// [`Create`]. Above all, `position == 0` is not enough to say "first" the day
/// a partial write exists: it is the flag that decides, and it
/// alone, because it is what commands opening the stream **without**
/// `keepExistingData`.
///
/// 🔵 **`last` IS THE COMMIT.** The browser's `createWritable()` writes
/// into a swap file and commits only on `close()`: it is the
/// `last` chunk that triggers that `close()`, and hence the only instant the
/// local machine's file changes. A push interrupted before it leaves the local
/// file **unchanged** — not half written. ⚠️ *Inference from the specification
/// of the File System Access API, not measured by this sub-block.*
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub chemin: String,
    pub position: u64,
    #[serde(rename = "longueur")]
    pub length: u32,
    /// Premier morceau : le flux s'ouvre **sans** `keepExistingData`.
    pub premier: bool,
    /// Last chunk: the stream closes, and **that is the commit**.
    #[serde(rename = "dernier")]
    pub last: bool,
}

/// The header of `TYPE_CREATE`. **Empty** binary payload.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Create {
    pub chemin: String,
    pub repertoire: bool,
}

/// The header of `TYPE_RENOMMER`. **Empty** binary payload.
///
/// 🔴 **THE ORDER OF THE TWO FIELDS IS THE DIRECTION OF THE OPERATION, and getting it wrong
/// DESTROYS.** `de` is the source, `vers` the destination — that is, on the
/// ProjFS side, `FilePathName` then `destinationFileName`. Swapping the two would
/// produce no error: the rename would happen, backwards, and the
/// destination file would overwrite the source. It is risk R-F3-1 of the plan,
/// and it carries **two** safeguards that do not depend on each other:
///
/// 1. probe S1 records on evidence which ProjFS field carries what, **before**
///    any acceptance run;
/// 2. the bridge **refuses to push** a rename whose `vers` is empty or equal to
///    `de`, with a `warn!` naming both raw fields. That one depends
///    on no measurement.
///
/// ⚠️ **`repertoire` is CARRIED rather than rediscovered.** It is
/// the `isdirectory` that the notification callback receives from the system; the
/// browser would ask for it again at the cost of a round trip, and would be wrong about
/// an entry that vanished in between. It decides two things: the recursion of the
/// copy fallback, and the way the pending write of a CHILD delays the
/// rename of a directory (`agent/src/pont/mutation.rs`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Renommer {
    /// The source, as it exists today on the local machine.
    pub de: String,
    /// The destination. **Never empty, never equal to `de`** — the bridge refuses to
    /// push otherwise.
    pub vers: String,
    pub repertoire: bool,
}

/// The header of `TYPE_DELETE`. **Empty** binary payload.
///
/// ⚠️ **Deletion is NOT recursive on the browser side**, against the letter
/// of spec §3.5 (`dir.removeEntry(nom, { recursive })`). A gesture in the VM
/// must not trigger a recursive destruction on the disk of the local
/// machine, on the strength of a mirror that no proof says is up to date. The refusal
/// then climbs up as [`super::CodeEchec::RepertoireNonVide`], which thereby
/// becomes **diagnostic** instead of being a code never produced.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Delete {
    pub chemin: String,
    pub repertoire: bool,
}

/// A PENDING write: bytes that live on the VM and not yet on the
/// local machine.
///
/// 🔴 **It is the loss window, made NAMEABLE.** ProjFS never puts the
/// provider on the write path: by the time we learn of it,
/// the application has already closed its handle and believed it had saved. What
/// this structure carries is therefore what the user risks losing if they
/// close their tab now — and naming it is all we can do.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub chemin: String,
    pub octets: u64,
}

/// The header of `TYPE_DUES`. **Empty** binary payload.
///
/// ⚠️ **It is an ANNOUNCEMENT: it awaits no answer**, and the browser must
/// send nothing back. See the comment on `TYPE_DUES` in
/// [`crate::files`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Dues {
    pub dues: Vec<Due>,
    /// **F5** — true when the bridge has pending writes and **refuses to
    /// push them**, the browser having announced a root whose name
    /// differs from the remembered one (`Bonjour`, spec §6.4 case 2).
    ///
    /// 🔴 **REQUIRED FIELD, without `#[serde(default)]`, and it is a DECLARED
    /// DIVERGENCE from decision D7 of the F5 plan**, which announced it as additive.
    /// Three reasons, in this order:
    ///
    /// 1. **the default would go in the dangerous direction.** `#[serde(default)]`
    ///    would yield `false` = "push" for a header whose field had
    ///    been lost — and pushing into the wrong folder is precisely the
    ///    damage `Bonjour` exists to prevent. A default must fall on the
    ///    safe side or not exist;
    /// 2. **this module's header forbids it**: "No `#[serde(default)]`
    ///    anywhere: an incomplete header is rejected, never silently
    ///    completed." Setting one here would make the first, and a doctrine that
    ///    tolerates an exception is no longer one;
    /// 3. **F3 settled the same trade-off in the same direction** for
    ///    `Meta::nom`: accepted break, `FILES_VERSION` **stays 1**, because
    ///    both ends of this bridge are always deployed together — a
    ///    single `agent.exe`, a single shell page.
    ///
    /// ⚠️ **Consequence for the vectors**: `fichiers-vectors.json` does NOT carry (policy: allow-fr - file name)
    /// "both shapes" as D7 planned. It carries the complete shape, and
    /// a test **confronts** the parser with the incomplete shape to check that it
    /// is REFUSED — which pins the **absence** of a default, stronger than a
    /// pinned default.
    pub retenues: bool,
}

/// The header of `TYPE_BONJOUR`. **Empty** binary payload.
///
/// ⚠️ **It is an ANNOUNCEMENT, and it goes UPSTREAM**: from the browser to the bridge, without
/// the bridge having asked for it, and **its correlation is ignored**. See the
/// comment on the fourth family in [`crate::files`], which says
/// why it must be routed before any correlation resolution.
///
/// It carries what only the browser knows: **the name of the root the
/// user chose**. The bridge compares it to the one it remembered to
/// decide whether it pushes its pending writes or **holds them back**.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Bonjour {
    /// `FileSystemDirectoryHandle.name` of the mounted root.
    ///
    /// 🔴 **It is a HINT, never a proof**, and spec §6.4 case 2 says so:
    /// `isSameEntry()` compares two **live** handles, never a handle with
    /// a memory. Two namesake directories on two different disks
    /// would pass for one. **v1 compares the name for lack of anything better**, and it
    /// must be read that way.
    pub racine: String,
    /// The user confirmed they want to push despite the different name.
    ///
    /// ⚠️ **REQUIRED, like every field of this module.** A default at `false`
    /// would look safe, but it would mean a browser that forgot the field
    /// could **never again** resume its saving, without any
    /// trace saying so.
    pub forcer: bool,
}

// ⚠️ **`TYPE_RAFRAICHIR` has NO header of its own: its frame carries `{}`**, and
// it is the precedent of `TYPE_FAIT`, written at the bottom of this file: "giving it
// an empty structure would make a shape to pin that pins nothing, and a
// shared vector that cannot break". **Declared divergence from
// task 8 of the F5 plan**, which named `Rafraichir` among the structures to
// write. What identifies a refresh is its TYPE, not its shape.

/// The header of `TYPE_ECHEC`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Echec {
    pub code: super::CodeEchec,
}

// ⚠️ **`TYPE_FAIT` has NO header of its own: its frame carries `{}`.** Giving it
// an empty structure would make a shape to pin that pins nothing, and a
// shared vector that cannot break. What identifies the acknowledged
// write is the CORRELATION of the frame, not its header.

#[cfg(test)]
mod tests;
