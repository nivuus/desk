//! The rule of the `Bonjour` handshake: **push the due writes, or
//! HOLD them BACK**. **PURE** — no `cfg`, no I/O, fully tested on
//! the host.
//!
//! # The danger this module exists to prevent
//!
//! The journal of due writes (F2) survives the bridge's stop. On resumption, it carries
//! paths **relative to a root** — and nothing, in the journal, says
//! WHICH. If the user comes back choosing **another** directory,
//! replaying blindly would write *the files of one session into the folder
//! of another* (spec §6.4 case 2).
//!
//! ⚠️ **And the journal is NOT EMPTIED when holding back**: throwing it away would lose the
//! data, pushing would put it in the wrong place. **We do neither one nor
//! the other: we NAME it**, and the browser displays "Resume
//! saving".
//!
//! # 🔴 WHAT THIS MODULE CANNOT DO, AND IT MUST BE READ THAT WAY
//!
//! It compares a **NAME**. `isSameEntry()` compares two **live** handles,
//! never a handle with a memory (spec §6.4 case 2): there is no way
//! to recognise a directory from one visit to the next. **The name is a clue,
//! not a proof** — two same-named directories on two different disks
//! would defeat this rule, and nothing here would say so.
//!
//! ⚠️ **And the PERMISSION MODEL is tested by nothing**:
//! `showDirectoryPicker()`, `queryPermission`, `requestPermission` and
//! transient user activation are called nowhere in this
//! repository — legacy of F1, carried over by F2, F3 and F4. The OPFS set-up of the
//! acceptance run creates two directories with different names and mounts one then the other:
//! it tests **the rule**, never **that it is enough**.

/// What the thread must do with its due writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Push, and remember the announced name.
    Pousser,
    /// Push nothing, and announce it to the browser through `Dues.retenues`.
    ///
    /// ⚠️ **The announced name is NOT remembered**: remembering it would mean that a
    /// second `Bonjour` on this same directory would push, whereas
    /// the user confirmed nothing. **Only an explicit confirmation
    /// (`forcer`) changes the remembered name.**
    Retenir,
}

/// Decides, from the remembered name and what the browser announces.
///
/// | Remembered name | Announced name | `forcer` | Decision |
/// | --- | --- | --- | --- |
/// | absent | any | — | **Push**, and remember |
/// | `X` | `X` | — | **Push** |
/// | `X` | `Y ≠ X` | `false` | **Hold back**, and remember nothing |
/// | `X` | `Y ≠ X` | `true` | **Push**, and remember `Y` |
///
/// ⚠️ **The first mount PUSHES, and it is not a hole**: nothing there can
/// be misplaced, the journal being empty or born from this same mount. Holding back at the
/// first mount would make any resumption impossible without a click, including
/// after a simple restart on the same directory.
pub fn decider(memorise: Option<&str>, annonce: &str, forcer: bool) -> Decision {
    match memorise {
        None => Decision::Pousser,
        Some(x) if x == annonce => Decision::Pousser,
        Some(_) if forcer => Decision::Pousser,
        Some(_) => Decision::Retenir,
    }
}

/// The name to remember after this decision, if one must be remembered.
///
/// 🔴 **`None` on `Retenir`, and it is the half that matters.** Remembering the name
/// just refused would mean that the next `Bonjour` — a simple
/// page reload — would find it "known" and would push. *Holding back would
/// only last one visit, and the second attempt would do the damage the first
/// avoided.*
pub fn a_memoriser(decision: Decision, annonce: &str) -> Option<&str> {
    match decision {
        Decision::Pousser => Some(annonce),
        Decision::Retenir => None,
    }
}

#[cfg(test)]
mod tests;
