//! The JSON headers of the bridge's frames. **PURE** — no `cfg`, no
//! dependency on `windows`.
//!
//! # ✅ THE DEBT DECLARED HERE IS SETTLED: the shapes live in `proto/`
//!
//! This module carried the seven header structures and declared in so many
//! words that it was a debt: "**as long as this is not done, a field
//! renamed here breaks the bridge without breaking a single client-side test**". The
//! scope of the task that wrote them forbade touching `proto/`;
//! task 15, which could, did it rather than reproduce the shapes by
//! hand on the TypeScript side.
//!
//! They now live in [`proto::files::entetes`], pinned by
//! `proto/fichiers-vectors.json`, which **both** implementations read — (policy: allow-fr, real file path)
//! `proto/src/files/entetes/tests.rs` and `proto/ts/fichiers-entetes.test.ts`. (policy: allow-fr, real file path)
//! A renaming now has only one side to break to be seen RED.
//!
//! This module therefore only keeps what is NOT a wire shape: **what
//! the agent DOES with the headers `proto` defines**, that is the
//! epoch conversion, which is a Windows affair and has no browser
//! twin.
//!
//! ⚠️ **It deliberately does NOT RE-EXPORT the seven structures.** A
//! `pub use proto::files::entetes::*` would have avoided touching the three
//! call sites, at the cost of two things: an `unused_imports` warning
//! on the host, all consumers being `#[cfg(windows)]`, and above all an
//! indirection that would hide from the reader of `projfs/rappels.rs` where
//! these shapes really come from. The three sites therefore write
//! `use proto::files::entetes;`, and say so.

/// Milliseconds since the Unix epoch → 100 ns units since the
/// FILETIME epoch (January 1st, 1601).
///
/// ⚠️ **Two classic mistakes, and only one of the two shows:**
///
/// - **getting the EPOCH wrong** yields dates of 1601 in Explorer — 369
///   years off, visible, but only if someone looks;
/// - **getting the FACTOR wrong** (10⁷ instead of 10⁶, or the reverse) yields
///   plausible and wrong dates, which no one will ever notice.
///
/// Both are pinned by `the_unix_epoch_becomes_the_filetime_epoch`.
///
/// A date before 1601 is brought back to **zero**: a negative FILETIME is
/// interpreted by Windows as a **relative** time, which would give a
/// file a date unrelated to its own. An overflow is
/// saturated for the same reason — in `release`, it wraps silently.
///
/// ⚠️ **It stays HERE and not in `proto/`**, on purpose: it is not a
/// wire shape but a Windows-specific conversion. The browser never
/// has a FILETIME to produce or read; moving it there would give `proto` a
/// TypeScript twin without a caller.
pub fn filetime_depuis_ms(ms: i64) -> i64 {
    /// Milliseconds between January 1st, 1601 and January 1st, 1970.
    const DECALAGE_MS: i64 = 11_644_473_600_000;
    /// 100 ns units in a millisecond.
    const PAR_MS: i64 = 10_000;
    ms.saturating_add(DECALAGE_MS).saturating_mul(PAR_MS).max(0)
}

#[cfg(test)]
mod tests;
