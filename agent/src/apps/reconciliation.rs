//! The diff from one reconciliation to the next.
//!
//! 🔴 THIS MODULE IS PURE, and it is the heart of the sub-block: it is what makes
//! the catalogue travel as a DELTA and not as 154 lines every
//! 30 seconds.
//!
//! ⚠️ IT KEEPS NO MEMORY OF DISAPPEARANCES. An application that comes back
//! after disappearing shows up again in `apparues`, because "a disappearance
//! is not a deletion" and it is the PLATFORM that knows it already
//! knows it — it carries `disparue_a`, the agent does not. Giving it that
//! memory would duplicate a state that already lives elsewhere, and the two copies
//! would diverge at the first agent restart.

use std::collections::BTreeMap;

use proto::plateforme::Application;

/// What changed between two reads of the disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    pub apparues: Vec<Application>,
    pub modifiees: Vec<Application>,
    /// KEYS, never objects: the platform only needs the
    /// identity to mark a disappearance, and the object would inflate the
    /// message without carrying anything useful.
    pub disparues: Vec<String>,
}

impl Diff {
    /// Nothing moved — the nominal state, round after round, on an idle
    /// disk. It is what decides whether anything needs to be emitted at all.
    pub fn est_vide(&self) -> bool {
        self.apparues.is_empty() && self.modifiees.is_empty() && self.disparues.is_empty()
    }
}

/// Compares two catalogues BY KEY, and returns a SORTED result.
///
/// 🔴 BY KEY, NEVER BY ORDER: walking a directory guarantees
/// no order, and comparing positionally would produce a full diff on
/// every round for a disk that has not moved.
///
/// 🔴 SORTED, AND IT IS A PROPERTY OF THE PROTOCOL, NOT A CONVENIENCE: without
/// it, two successive reconciliations would emit different messages
/// for an identical state. The log would show a catalogue moving without
/// cause, and any acceptance run comparing two rounds would be undecidable. The
/// `BTreeMap` gives it by construction — that is why it is not a
/// `HashMap` followed by a `sort`.
///
/// ⚠️ A DUPLICATE KEY IN `aujourdhui` COUNTS ONLY ONCE: two `.lnk`
/// with the same triple are a single application, and it is the measured case — 167
/// kept shortcuts yield 154 keys on the VM. The last one read wins; the
/// fields that distinguish them (name, `.lnk` path) do not take part in
/// the identity, so neither of the two is "the right one".
pub fn diff(hier: &[Application], aujourdhui: &[Application]) -> Diff {
    let anciennes: BTreeMap<&str, &Application> =
        hier.iter().map(|a| (a.cle.as_str(), a)).collect();
    let nouvelles: BTreeMap<&str, &Application> =
        aujourdhui.iter().map(|a| (a.cle.as_str(), a)).collect();

    let mut sortie = Diff::default();
    for (cle, neuve) in &nouvelles {
        match anciennes.get(cle) {
            None => sortie.apparues.push((*neuve).clone()),
            // Equality covers ALL fields, not only the key: the
            // name and the `.lnk` path have no part in the identity,
            // but they must follow — and it is that path the
            // launch uses.
            Some(ancienne) if ancienne != neuve => sortie.modifiees.push((*neuve).clone()),
            Some(_) => {}
        }
    }
    for cle in anciennes.keys() {
        if !nouvelles.contains_key(cle) {
            sortie.disparues.push((*cle).to_string());
        }
    }
    sortie
}

#[cfg(test)]
#[path = "reconciliation/tests.rs"]
mod tests;
