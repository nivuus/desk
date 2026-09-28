//! What to do with each ProjFS notification. **PURE** — no `cfg`, no
//! dependency on the `windows` crate, entirely tested on the host.
//!
//! # ❌ THIS MODULE NO LONGER HOLDS READ-ONLY — F2 OPENED IT FOR WRITING
//!
//! **It was F1's SCOPE**: "any write attempt returns
//! `ERROR_WRITE_PROTECT`. It is a scope, not a gap" (spec §8). F1's
//! acceptance run had already refuted this sentence for **creation** (2
//! runs recorded out of 2: it SUCCEEDS, the notification being a POST one).
//! **F2 refutes the other half, deliberately**: `PRE_CONVERT_TO_FULL` is
//! now ALLOWED when the root is writable and the channel open, and
//! the bytes are pushed to the local workstation **after the fact**.
//!
//! ⚠️ **What remains refused, and it is named**: `PRE_RENAME` and `PRE_DELETE`,
//! because `Renommer` and `Delete` are deliverables of **F3**. An
//! application using the *write-temporary / rename / delete* idiom
//! will therefore fail **loudly at the renaming**, rather than succeed on the VM
//! while leaving the local workstation on the old content. **Accepting the renaming
//! without pushing it would produce exactly the silent loss this module
//! exists to forbid.**
//!
//! # 🔴 WHAT F2 CANNOT DO, AND WHICH MUST BE KNOWN BEFORE READING ON
//!
//! **The write path has NO back-pressure.** There is no
//! notification by which one *accepts* a write: one accepts by **not
//! refusing** `PRE_CONVERT_TO_FULL`, and one learns there are bytes to
//! push through `FILE_HANDLE_CLOSED_FILE_MODIFIED` and `FILE_OVERWRITTEN`, both
//! **POST** — that is, **after** the application has closed its handle
//! and believed it had saved. If the push then fails — permission revoked,
//! disk full, tab closed —, **no `HRESULT` can reach anyone
//! any more**.
//!
//! The only two levers left are therefore:
//!
//! 1. a refusal **UPSTREAM**, at `PRE_CONVERT_TO_FULL`, bearing on a **STATE**
//!    (read-only root, closed channel) and **never on the outcome** — it is
//!    the reason to exist of the [`Etat`] parameter of [`decider`];
//! 2. a **DENUNCIATION** after the fact: the resumption journal, the shell page's
//!    due writes counter, and `beforeunload`.
//!
//! # Why this decision is here and not in the callback
//!
//! Left in `pont/projfs/rappels/notification.rs`, it would be
//! `#[cfg(windows)]`, called by the system, and **no test could
//! exercise it** — whereas what it does is a pure mapping of an integer
//! code and a state to a decision. `CLAUDE.md` makes it a review criterion,
//! not a wish: "any decision that could live in a pure module MUST
//! live there".
//!
//! # The values are COPIED, with their source line
//!
//! Same doctrine as [`crate::pont::errors`]: importing the constants of
//! `windows::Win32::Storage::ProjectedFileSystem` would gate this module behind
//! `#[cfg(windows)]` and make it lose its host testability, which is its whole
//! point. Each constant therefore carries the line number of its source,
//! read by command on August 19th, 2026 in
//! `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`.
//!
//! ⚠️ **ProjFS's two families of constants are NOT interchangeable,
//! and they carry names alike enough to mislead.**
//! `PRJ_NOTIFICATION_*` (`i32`) is what the callback RECEIVES;
//! `PRJ_NOTIFY_*` (`u32`) is what the MASK requests. They have
//! the same numeric values here, but they are two distinct types in
//! windows-rs, and nothing guarantees they will stay aligned.

use crate::pont::errors::Error;

// What the callback RECEIVES — `PRJ_NOTIFICATION`, `i32`.
pub const PRE_CONVERT_TO_FULL: i32 = 4096; // mod.rs:340
pub const PRE_RENAME: i32 = 32; // mod.rs:378
pub const PRE_DELETE: i32 = 16; // mod.rs:377
pub const PRE_SET_HARDLINK: i32 = 64; // mod.rs:379
pub const HARDLINK_CREATED: i32 = 256; // mod.rs:342
pub const NEW_FILE_CREATED: i32 = 4; // mod.rs:349
pub const FILE_OVERWRITTEN: i32 = 8; // mod.rs:339
pub const FILE_HANDLE_CLOSED_FILE_MODIFIED: i32 = 1024; // mod.rs:336
                                                        // ── F3'S TWO ──────────────────────────────────────────────────────────────
                                                        // ⚠️ **The VALUE is authoritative, never the line number.** `windows` and
                                                        // `windows-sys` expose TWO `Win32/Storage/ProjectedFileSystem` modules with
                                                        // identical symbols and different lines; the author of F2's plan
                                                        // declared three exact citations false for having forgotten it. The authoritative
                                                        // crate is the one `agent/Cargo.toml` declares — `windows` (0.62.2).
pub const FILE_RENAMED: i32 = 128; // mod.rs:341
pub const FILE_HANDLE_CLOSED_FILE_DELETED: i32 = 2048; // mod.rs:335

// What the MASK requests — `PRJ_NOTIFY_TYPES`, `u32`.
pub const NOTIFY_FILE_PRE_CONVERT_TO_FULL: u32 = 4096; // mod.rs:385
pub const NOTIFY_PRE_RENAME: u32 = 32; // mod.rs:391
pub const NOTIFY_PRE_DELETE: u32 = 16; // mod.rs:390
pub const NOTIFY_PRE_SET_HARDLINK: u32 = 64; // mod.rs:392
pub const NOTIFY_NEW_FILE_CREATED: u32 = 4; // mod.rs:388
pub const NOTIFY_FILE_OVERWRITTEN: u32 = 8; // mod.rs:384
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED: u32 = 1024; // mod.rs:381
pub const NOTIFY_FILE_RENAMED: u32 = 128; // mod.rs:386
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED: u32 = 2048; // mod.rs:380

/// The mask the root requests — **NINE bits**: five in F1, seven after F2,
/// nine since F3.
///
/// - **The four `PRE_`s are REFUSABLE**, and **THREE** of them actually
///   decide something since F3: `PRE_CONVERT_TO_FULL` (writing),
///   `PRE_RENAME` and `PRE_DELETE`. *(These lines said "only one", and
///   "the other three are refused unconditionally — renaming and
///   deletion: F3". F3 has arrived.)* The fourth, `PRE_SET_HARDLINK`,
///   stays refused unconditionally: hard links have **no** equivalent
///   in the File System Access API.
/// - **The five POSTs cannot be refused**: they say what has ALREADY happened
///   on the VM, and that is all one can draw from them.
///
/// 🔵 **WHAT THE `PRE_`s BUY F3, AND WHAT F2 DID NOT HAVE.** F2 declares
/// that the write path has **no** back-pressure: it only learns of a
/// write when the handle closes, through a POST, when the application has
/// already believed it saved. `PRE_RENAME` and `PRE_DELETE`, for their part, are **PRE**s:
/// F3 can refuse a renaming or a deletion **before** they take
/// place, and the application sees it.
///
/// ⚠️ **But the refusal can only bear on a STATE, never on an OUTCOME.**
/// `PRE_`s are **synchronous** and never consult the browser (spec
/// §4.3). We therefore do not know whether the local workstation will accept; we only know
/// whether our side is able to push.
///
/// ⚠️ **`FILE_HANDLE_CLOSED_NO_MODIFICATION` (512, `mod.rs:382`) is
/// DELIBERATELY NOT REQUESTED.** It would arrive at **each closing of a read
/// handle**, that is, on the bridge's hottest path, only to
/// learn what we already know: that there is nothing to push. The cost is
/// certain, the gain zero. *Decision, not oversight.*
///
/// ⚠️ **`HARDLINK_CREATED` (256) stays requested AND refused, as in F1** — but
/// it is **POST**: the refusal prevents nothing, it **logs**. Saying so,
/// rather than letting it be believed that a hard link is prevented.
///
/// ⚠️ **Requesting LESS would lose writes; requesting MORE would make
/// a notification arrive without a decision.** Both are pinned by
/// `le_masque_demande_exactement_les_sept_notifications_de_f2` and
/// `each_mask_bit_has_a_named_decision`.
pub const MASQUE: u32 = NOTIFY_FILE_PRE_CONVERT_TO_FULL
    | NOTIFY_PRE_RENAME
    | NOTIFY_PRE_DELETE
    | NOTIFY_PRE_SET_HARDLINK
    | NOTIFY_NEW_FILE_CREATED
    | NOTIFY_FILE_OVERWRITTEN
    | NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
    | NOTIFY_FILE_RENAMED
    | NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED;

/// What a push carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poussee {
    /// The bytes of a file closed after modification.
    Contenu,
    /// An entry that has just appeared. **A directory carries no
    /// content**; a file, for its part, will be followed by a content push when
    /// its handle closes.
    Creation,
    /// **F3** — the entry was renamed. `de` is `FilePathName`, `vers` is
    /// `destinationFileName`: two DIRECT parameters of the callback, never
    /// members of the `PRJ_NOTIFICATION_PARAMETERS` union, which this bridge
    /// still does not dereference.
    ///
    /// 🔴 **Getting the direction wrong DESTROYS**, and it is F3's most serious
    /// risk. The bridge therefore refuses to push a renaming whose destination is
    /// empty or equal to the source, with a `warn!` naming both raw
    /// fields — a safeguard that depends on no measurement.
    Renommage,
    /// **F3** — the entry was deleted.
    ///
    /// ⚠️ The notification's name is `FILE_HANDLE_CLOSED_FILE_DELETED`: the
    /// deletion only takes effect when the **last** handle closes, which
    /// is Windows semantics and not a ProjFS subtlety.
    Suppression,
}

/// The state the decision depends on.
///
/// 🔴 **THE DECISION BEARS ON A STATE, NEVER ON AN OUTCOME**, and it is the
/// only form of refusal still possible: at `PRE_CONVERT_TO_FULL` we know
/// nothing of what the push will become, and when we know it will be too late
/// to tell anyone (see the header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etat {
    /// Does the root accept writing? `false` = F1's behaviour.
    pub inscriptible: bool,
    /// Is the bridge's channel open? Refusing here is the only moment when
    /// the application can still learn it.
    pub canal_ouvert: bool,
    /// **F3** — are mutations armed? `PONT_MUTATION=0` disarms them.
    ///
    /// 🔴 **BENCH VARIABLE, never a shipped configuration.** It exists
    /// to make criteria ① and ② of the acceptance run RED: disarmed, the
    /// `PRE_` refuses, the application sees `ERROR_WRITE_PROTECT`, and **the local
    /// workstation is unchanged**. It is a red of the MECHANISM — the refusal is
    /// logged and the `protege-en-ecriture` counter rises —, never a vacuous
    /// red.
    ///
    /// ⚠️ **It does NOT touch writing**: `PONT_ECRITURE` has its own. Two
    /// distinct mechanisms, two distinct switches — confusing them
    /// would mean an acceptance run of renaming would also cut the
    /// temp+rename idiom it wants to exercise.
    pub mutations_armees: bool,
}

/// What we know of a notification's DESTINATION.
///
/// ⚠️ **An `enum` and not a `bool`, because a deletion has no
/// destination at all.** Passing `false` there would suggest "the destination is
/// in the root", which means nothing, and a test writing it
/// would exercise nothing.
///
/// ⚠️ **`chemins::normaliser` decides, never [`decider`].** Spec §4.3 says
/// "accept, unless the target leaves the root", and `pont::chemins` is already
/// the module that refuses `..`, `:` and reserved names. Duplicating it
/// here would make two truths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    /// The notification carries no destination — all codes except
    /// `PRE_RENAME`.
    SansObjet,
    /// The destination is acceptable: in the root, and normalised.
    InRoot,
    /// The destination leaves the root, or `chemins::normaliser` refused it.
    HorsRacine,
}

/// What the notification callback must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reponse {
    /// Refuse, with the cause. The callback returns `hresult(cause)`.
    Refuser(Error),
    /// **Allow a write.** It is this module's ONLY explicit acceptance,
    /// and the only line of F2 that changes what an application gets.
    ///
    /// 🔴 **IT IS DISTINCT FROM THE CATCH-ALL, AND IT IS NOT A LUXURY.** F2's
    /// plan prescribed `AccepterSansAttendre` for this case; that would have
    /// made a bit **REQUESTED by the mask** fall back into the catch-all arm,
    /// hence made `each_mask_bit_has_a_named_decision` fail — this
    /// module's exhaustiveness guard. The obvious remedy would have been
    /// to exclude `PRE_CONVERT_TO_FULL` from the sweep, which would have **emptied the
    /// guard** instead of satisfying it. *A plan does not immunise against the
    /// vacuous check: it is a source of it.*
    Autoriser,
    /// The notification triggers a **write-back**.
    Pousser(Poussee),
    /// Accept without saying more than an unexpected-mask `warn!`.
    ///
    /// ⚠️ **It is the CATCH-ALL arm, and it is named as such.** No bit
    /// requested by [`MASQUE`] must fall back into it.
    AccepterSansAttendre,
}

// ⚠️ **`AccepterEnSignalant` HAS DISAPPEARED, and it is a declared divergence from
// F2's plan**, which kept it in its enumeration. It had only one
// producer left in F1 — `NEW_FILE_CREATED` —, which F2 moves to
// `Pousser(Creation)`: keeping it would make it a variant without any
// construction site, that is, dead code in a module whose whole point
// is to be exhaustively swept.

/// The decision, for a `PRJ_NOTIFICATION` notification code and an [`Etat`].
///
/// The `match` is **not** exhaustive in the compiler's sense — `PRJ_NOTIFICATION`
/// is an integer, not a Rust enum —, hence the test guard
/// `each_mask_bit_has_a_named_decision`, which sweeps the 32 bits and
/// checks that no REQUESTED bit falls back into the catch-all arm.
pub fn decider(code: i32, etat: Etat, cible: Cible) -> Reponse {
    match code {
        // 🔵 **THE ONLY REFUSAL GATE FOR A WRITE.** Beyond it, nothing more
        // can be said to the application: it will close its handle believing
        // it has saved.
        //
        // ⚠️ **TWO CAUSES, TWO CODES**, and spec §5.1 requires it: a read-only
        // root returns `ERROR_WRITE_PROTECT`, a closed channel returns
        // `ERROR_IO_DEVICE`. Making them share a code would make
        // "this share is read-only" and "the tab is
        // closed" indistinguishable — two situations that do not call for the same gesture.
        PRE_CONVERT_TO_FULL if !etat.inscriptible => Reponse::Refuser(Error::ProtegeEnEcriture),
        PRE_CONVERT_TO_FULL if !etat.canal_ouvert => Reponse::Refuser(Error::CanalFerme),
        PRE_CONVERT_TO_FULL => Reponse::Autoriser,
        // ❌ **THESE TWO WERE NOT REFUSED BECAUSE THEY HAD TO BE,
        // BUT BECAUSE F3 DID NOT EXIST YET.** *(This arm said:
        // "REFUSED UNCONDITIONALLY, AND IT IS DELIBERATE. `Renommer` and
        // `Delete` are deliverables of F3: accepting them without being able to
        // push them would leave the local workstation on the old content." The reason
        // was right, and it stopped being so: F3 knows how to push them.)*
        //
        // 🔴 **FOUR STATES REFUSE, AND NO OUTCOME DOES.** A `PRE_`
        // is synchronous: we cannot ask the browser what it
        // thinks of the operation, only observe that our side is not
        // able to push it.
        //
        // ⚠️ **THE ORDER IS THAT OF `PRE_CONVERT_TO_FULL`, by symmetry.** It
        // only decides the case of an out-of-root renaming on a root
        // already read-only, which does not happen — but leaving it to chance
        // would make the module's two refusal gates diverge.
        PRE_RENAME | PRE_DELETE if !etat.mutations_armees => {
            Reponse::Refuser(Error::ProtegeEnEcriture)
        }
        PRE_RENAME | PRE_DELETE if !etat.inscriptible => Reponse::Refuser(Error::ProtegeEnEcriture),
        PRE_RENAME | PRE_DELETE if !etat.canal_ouvert => Reponse::Refuser(Error::CanalFerme),
        // ⚠️ **`NonSupporte` AND NOT `ProtegeEnEcriture`**: leaving the root
        // is not a rights refusal, it is an operation the other end cannot
        // do — it has no handle outside the directory the
        // user chose. Making them share a code would violate §5.1
        // of the spec, and would send one looking for a permission where there is none.
        PRE_RENAME if cible == Cible::HorsRacine => Reponse::Refuser(Error::NonSupporte),
        PRE_RENAME | PRE_DELETE => Reponse::Autoriser,
        // Hard links have no equivalent in the File System Access
        // API: it is not a read-only refusal, it is an operation that
        // does not exist on the other side (spec §3.5.2). The distinction is
        // visible on the application side AND in the log — it is the whole purpose of
        // `pont::errors`, whose counter-example is the old bridge, which
        // returned `EPERM` at nine distinct sites.
        PRE_SET_HARDLINK | HARDLINK_CREATED => Reponse::Refuser(Error::NonSupporte),
        // ⚠️ **POST: it cannot be refused** — but F2 PUSHES it, which
        // closes the divergence F1 declared its own ("a file created from
        // scratch lives on the VM and is NEVER pushed").
        NEW_FILE_CREATED => Reponse::Pousser(Poussee::Creation),
        // The two content POSTs. **`FILE_OVERWRITTEN` is not redundant
        // with the handle closing**: it signals a truncation at
        // opening (`CREATE_ALWAYS`, `TRUNCATE_EXISTING`), which an
        // "in place" save commonly produces.
        FILE_OVERWRITTEN | FILE_HANDLE_CLOSED_FILE_MODIFIED => Reponse::Pousser(Poussee::Contenu),
        // ── F3'S TWO POSTS ──────────────────────────────────────────────────────
        // ⚠️ **They are NOT refused**, like all POSTs: the gesture has
        // already happened in the VM. What allowed them is the corresponding
        // `PRE_`, a few microseconds earlier.
        //
        // 🔴 **AND THE STATE DOES NOT CHANGE THEM EITHER.** A push goes out even
        // with the channel closed: the thread serving it will journal it and hold it back.
        // Subordinating them to the state would silently lose exactly what
        // the refusal at the `PRE_` had let through.
        FILE_RENAMED => Reponse::Pousser(Poussee::Renommage),
        FILE_HANDLE_CLOSED_FILE_DELETED => Reponse::Pousser(Poussee::Suppression),
        _ => Reponse::AccepterSansAttendre,
    }
}

#[cfg(test)]
mod tests;
