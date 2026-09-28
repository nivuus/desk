//! From the cause of a failure to the `HRESULT` ProjFS expects. **PURE**: no
//! `cfg`, and above all **no dependency on the `windows` crate**.
//!
//! ⚠️ **Why the twelve values are COPIED and not imported**: importing
//! `windows::Win32::Foundation::ERROR_FILE_NOT_FOUND` would gate this module behind
//! `#[cfg(windows)]` and make it lose its host testability, which is its whole
//! point. Each constant therefore carries **the line number of its source**
//! alongside — it is the review check spec §3.1 imposes on
//! transcriptions, applied here too. Read by command on August 19th, 2026
//! in `windows-0.62.2/src/Windows/Win32/Foundation/mod.rs`.
//!
//! **The counter-example this module exists not to replay**: the old
//! bridge returned `cb(-1)` — `EPERM` — at **nine** distinct sites
//! (`src/file.js:189,203,245,267,277,288,299,311,323`). An absent file, a
//! full disk, an exceeded delay and an internal error were
//! indistinguishable there, on the Windows application side as in the log.

/// `HRESULT_FROM_WIN32(x)` for a Win32 error code: the severity bit,
/// the `FACILITY_WIN32` facility (7), then the code.
const FACILITE_WIN32: u32 = 0x8007_0000;

// The THIRTEEN Win32 codes, transcribed with their source line. (Twelve failure
// causes, plus `ERROR_IO_PENDING`, which is not one — see `EN_COURS`.)
const ERROR_FILE_NOT_FOUND: u32 = 2; // Foundation/mod.rs:2355
const ERROR_PATH_NOT_FOUND: u32 = 3; // Foundation/mod.rs:3689
const ERROR_ACCESS_DENIED: u32 = 5; // Foundation/mod.rs:1143
const ERROR_WRITE_PROTECT: u32 = 19; // Foundation/mod.rs:4657
const ERROR_GEN_FAILURE: u32 = 31; // Foundation/mod.rs:2435
const ERROR_NOT_SUPPORTED: u32 = 50; // Foundation/mod.rs:3498
const ERROR_FILE_EXISTS: u32 = 80; // Foundation/mod.rs:2347
const ERROR_DISK_FULL: u32 = 112; // Foundation/mod.rs:1781
const ERROR_SEM_TIMEOUT: u32 = 121; // Foundation/mod.rs:3943
const ERROR_DIR_NOT_EMPTY: u32 = 145; // Foundation/mod.rs:1776
const ERROR_OPERATION_ABORTED: u32 = 995; // Foundation/mod.rs:3632
const ERROR_IO_DEVICE: u32 = 1117; // Foundation/mod.rs:3003
const ERROR_IO_PENDING: u32 = 997; // Foundation/mod.rs:3005

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` — **the value EVERY asynchronous
/// callback returns**, and it is not an [`Error`].
///
/// ⚠️ **It deliberately has NO [`Error`] variant**, and it is not
/// an oversight: `Error` enumerates the causes of a FAILURE, and its `COUNT` carries a
/// structural guard that forces classifying any new variant. "The operation
/// is in progress" is not a failure — giving it a variant would mean that an
/// exhaustive sweep of error causes would include a deferred success, and that
/// `hresult` could return `EN_COURS` where a caller expects a refusal
/// code.
///
/// It is the value that makes the threading discipline hold: a callback registers
/// in the table, returns it, and **returns control immediately**. Waiting there for a
/// browser round trip would freeze the application reading the file.
pub const EN_COURS: i32 = (FACILITE_WIN32 | ERROR_IO_PENDING) as i32;

/// What prevented an operation from completing.
///
/// **One variant per CAUSE**, never for code convenience: it is what
/// allows the Windows application to distinguish "this file does not exist" from
/// "the browser closed the tab", and the operator to read it in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The requested file does not exist in the shared directory.
    Introuvable,
    /// An intermediate component of the path does not exist.
    CheminIntrouvable,
    /// The File System Access API refused access (permission revoked).
    AccesRefuse,
    /// The data channel is closed: tab closed, page reloaded, WebRTC
    /// down. It is "the standard I/O error" of framing §7.
    CanalFerme,
    /// The browser did not answer within the allotted budget.
    DelaiDepasse,
    /// The command was cancelled, by ProjFS or by the bridge's shutdown.
    Abandonnee,
    /// The local workstation's disk is full.
    ///
    /// ✅ **BUILT SINCE F2** *(this line said "(F2 and beyond)")*:
    /// `service::cause_de` produces it on a `CodeEchec::DisquePlein`.
    ///
    /// 🔴 **AND ITS `HRESULT` REACHES NO ONE.** It is born from a WRITE
    /// push, that is AFTER the application has closed its handle and
    /// believed it had saved: there is **no longer any ProjFS command to
    /// complete**. `ERROR_DISK_FULL` is a LOG value here, and nothing
    /// else. Without this sentence, a successor would believe the error code
    /// does something.
    DisquePlein,
    /// The operation has no equivalent in the File System Access API.
    NonSupporte,
    /// Deletion of a non-empty directory.
    ///
    /// ✅ **BUILT SINCE F3** *(this line said "(F3)")*:
    /// `service::cause_de` produces it on a `CodeEchec::RepertoireNonVide`.
    ///
    /// 🔵 **AND ITS `HRESULT` DOES REACH SOMEONE, unlike
    /// [`Error::DisquePlein`] and [`Error::DejaPresent`].** It is born from a
    /// deletion, whose `PRE_DELETE` is **SYNCHRONOUS**: Explorer waits on
    /// it. It is also the only **DIAGNOSTIC** cause in the table — receiving
    /// it means the local workstation carries entries the VM does not
    /// know, that is, **the mirror has drifted**.
    ///
    /// ⚠️ **It exists ONLY because F3 deletes WITHOUT `recursive`.** With
    /// `recursive: true`, the local workstation's subtree would disappear
    /// silently, and this code would never be produced — a code never produced
    /// is exactly what criterion (4) exists to forbid.
    RepertoireNonVide,
    /// Creation of an entry that already exists.
    ///
    /// ✅ **BUILT SINCE F2** *(this line said "(F2)")*. Same caveat
    /// as [`Error::DisquePlein`]: its `HRESULT` reaches no one.
    DejaPresent,
    /// ❌ **"F1 LIVES ENTIRELY IN THIS STATE" IS NO LONGER TRUE.** F2 opened
    /// writing: `PRE_CONVERT_TO_FULL` is **allowed** when the root is
    /// writable and the channel open, and the bytes are pushed to the
    /// local workstation after the fact (`pont::notifications`).
    ///
    /// **What this variant means NOW**, and it is narrower:
    /// - the root is mounted read-only (`PONT_ECRITURE=0` does NOT produce
    ///   it — it is a bench variable of the thread, not of the root);
    /// - or the operation is a **renaming** or a **deletion**, which F2
    ///   refuses unconditionally because `Renommer` and `Delete` are
    ///   deliverables of **F3**;
    /// - or the browser answered `protege-en-ecriture`.
    ///
    /// ⚠️ *F1's caveat holds: a file created FROM SCRATCH is
    /// still not refusable, the notification being a POST one — but F2 now
    /// PUSHES it, which closes the divergence it described.*
    ProtegeEnEcriture,
    /// Everything else. A single catch-all variant, and it is named as
    /// such — it is what prevents it from swallowing the eleven others.
    Inattendue,
}

/// The number of [`Error`] variants.
///
/// ⚠️ **It is not a convenience: it is the second stage of the structural
/// guard.** Adding a variant first breaks the compilation of [`index`]
/// and [`hresult`] (two exhaustive `match`es), which forces giving it an
/// index; the next index requires raising `COUNT` to 13, which makes
/// the compilation of [`Error::ALL`], typed `[Error; COUNT]`, fail as long
/// as the variant is not listed there. **The sweep therefore cannot become
/// decorative silently** — same intention as F3's criterion (4).
pub const COUNT: usize = 12;

impl Error {
    /// All variants. The tests' exhaustive sweep relies on it.
    pub const ALL: [Error; COUNT] = [
        Error::Introuvable,
        Error::CheminIntrouvable,
        Error::AccesRefuse,
        Error::CanalFerme,
        Error::DelaiDepasse,
        Error::Abandonnee,
        Error::DisquePlein,
        Error::NonSupporte,
        Error::RepertoireNonVide,
        Error::DejaPresent,
        Error::ProtegeEnEcriture,
        Error::Inattendue,
    ];
}

/// The rank of a variant. **Exhaustive** `match`: it is what makes it
/// impossible to add a variant without being forced to classify it.
#[cfg(test)]
const fn index(e: Error) -> usize {
    match e {
        Error::Introuvable => 0,
        Error::CheminIntrouvable => 1,
        Error::AccesRefuse => 2,
        Error::CanalFerme => 3,
        Error::DelaiDepasse => 4,
        Error::Abandonnee => 5,
        Error::DisquePlein => 6,
        Error::NonSupporte => 7,
        Error::RepertoireNonVide => 8,
        Error::DejaPresent => 9,
        Error::ProtegeEnEcriture => 10,
        Error::Inattendue => 11,
    }
}

/// `HRESULT_FROM_WIN32(x) = 0x8007_0000 | x`.
///
/// Returns an `i32`: it is what `windows_core::HRESULT` wraps, and the module
/// stays PURE. The `match` is **exhaustive** — a new variant cannot
/// fall into a catch-all arm and silently inherit another's code.
pub fn hresult(e: Error) -> i32 {
    let code = match e {
        Error::Introuvable => ERROR_FILE_NOT_FOUND,
        Error::CheminIntrouvable => ERROR_PATH_NOT_FOUND,
        Error::AccesRefuse => ERROR_ACCESS_DENIED,
        Error::CanalFerme => ERROR_IO_DEVICE,
        Error::DelaiDepasse => ERROR_SEM_TIMEOUT,
        Error::Abandonnee => ERROR_OPERATION_ABORTED,
        Error::DisquePlein => ERROR_DISK_FULL,
        Error::NonSupporte => ERROR_NOT_SUPPORTED,
        Error::RepertoireNonVide => ERROR_DIR_NOT_EMPTY,
        Error::DejaPresent => ERROR_FILE_EXISTS,
        Error::ProtegeEnEcriture => ERROR_WRITE_PROTECT,
        Error::Inattendue => ERROR_GEN_FAILURE,
    };
    (FACILITE_WIN32 | code) as i32
}

#[cfg(test)]
mod tests;
