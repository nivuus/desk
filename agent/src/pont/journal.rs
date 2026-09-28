//! The resumption journal of due writes. **PURE** — no `cfg`, no
//! input-output: it returns the LINES to append, and it is the caller that
//! writes them.
//!
//! # 🔴 What it is, and why it cannot be anything else
//!
//! **ProjFS NEVER puts the provider on the write path** (spec
//! §6.1). When we learn that a file was modified, the application has
//! already closed its handle and **believed it had saved**. Between that moment and
//! the arrival of the bytes on the local workstation opens a **loss window** that
//! nothing can close.
//!
//! This module does not close it either. It does the only thing left:
//! **write on the VM's disk, OUTSIDE the root, the list of what has not
//! arrived yet** — so that a restarted bridge pushes it again, and so that a
//! user who closes their tab learns what they risk losing.
//!
//! *Knowing what we lost is not having it* (spec §6.4). This module holds the
//! first term.
//!
//! # The file's shape, and the four reasons for it
//!
//! ```text
//! +<octets> <chemin JSON>\n     inscription
//! -<chemin JSON>\n              retrait
//! ```
//!
//! 1. **APPEND-ONLY, never rewritten in place.** An interrupted
//!    rewrite would lose the **EARLIER** entries — that is, the
//!    oldest, hence those that have been waiting the longest. Spec
//!    §4.4 names this case explicitly as this module's red.
//! 2. **The path is encoded as JSON.** It can carry spaces,
//!    diacritics, and — the local workstation possibly being macOS or Linux — **a line
//!    break**, which is a perfectly legal file-name character there.
//!    A naive separator would cut an entry in two. F1 already measures an
//!    accented name with a space; the line break is the case
//!    no one tries and everyone breaks.
//! 3. **The last line may be TRUNCATED, and [`Journal::relire`] throws it away
//!    while counting it.** A partial line is the only damage an abrupt
//!    stop can cause to an append-only file — and raising rather than throwing it away
//!    would lose ALL the earlier entries, which are intact.
//! 4. **Compaction happens ONLY on an empty journal.** Truncating a file
//!    that still carries a due entry would lose the data **exactly when it
//!    matters**.
//!
//! ⚠️ **This module knows neither `%LOCALAPPDATA%` nor ProjFS**: it is
//! `pont::executer` — already `#[cfg(windows)]` — that resolves the path. The
//! journal lives **outside the root** (`projfs/racine.rs`), for the reason
//! that file states: a state that lived IN the root would itself be a
//! projected object, hence dependent on the bridge to be read — circular — and it
//! would disappear with the root the day it had to be recreated, that is
//! **exactly the day it matters**.

/// Beyond this size, an **empty** journal is truncated to zero.
///
/// ⚠️ **NOT CALIBRATED.** It joins `WRITE_TIMEOUT`, `MAX_FRAME_SIZE`,
/// `DELAI_ATTRIBUTS`, `DELAI_LIRE`, `DELAI_LISTER`, `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `MAX_OUTPUT_SIZE`,
/// `REPIT_REARMEMENT_AUDIO` and `REARMEMENTS_MAX` in the list of this repository's
/// constants that no measurement has judged.
pub const JOURNAL_COMPACTION_SIZE: u64 = 256 * 1024;

/// The set of due writes, in their registration order.
///
/// ⚠️ **A `Vec` and not a `HashMap`, and it is not a convenience**: the
/// registration order is the only thing that makes resumption deterministic, and a
/// `HashMap` would give a different one at each run. The cost is a
/// linear sweep per operation, over a set counting the writes
/// **not yet acknowledged** — a handful in nominal operation.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Journal {
    dues: Vec<(String, u64)>,
}

impl Journal {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rereads a journal, **TOLERATING a truncated last line**.
    ///
    /// Returns the journal and the **number of ignored lines**: the caller
    /// logs them, it does not guess them.
    ///
    /// 🔴 **An unreadable line is THROWN AWAY, never fatal.** Raising would lose
    /// all earlier entries, which are nevertheless intact — and the
    /// journal exists precisely to lose nothing.
    pub fn relire(contenu: &str) -> (Self, usize) {
        let mut journal = Self::new();
        let mut ignorees = 0usize;
        for ligne in contenu.split('\n') {
            if ligne.is_empty() {
                // The cut after the last `\n`: it is not a line.
                continue;
            }
            match analyser(ligne) {
                Some(Entree::Inscription { chemin, octets }) => journal.poser(&chemin, octets),
                Some(Entree::Retrait { chemin }) => journal.oter(&chemin),
                None => ignorees += 1,
            }
        }
        (journal, ignorees)
    }

    /// Registers a due write, and returns **the line to append to the file**.
    ///
    /// A path already present has its bytes updated **without changing
    /// place**: the replay of a write is not a new write, and
    /// moving it back to the tail would let younger entries pass ahead
    /// of it.
    pub fn inscrire(&mut self, chemin: &str, octets: u64) -> String {
        self.poser(chemin, octets);
        format!("+{octets} {}\n", encoder(chemin))
    }

    /// Removes a due write, and returns **the line to append to the file**.
    ///
    /// ⚠️ **The removal is WRITTEN even if the path was absent.** The file
    /// is an event log, not a state: keeping quiet about a removal would make it
    /// dependent on what memory thinks it knows.
    pub fn retirer(&mut self, chemin: &str) -> String {
        self.oter(chemin);
        format!("-{}\n", encoder(chemin))
    }

    /// The due writes, **in registration order**.
    pub fn dues(&self) -> &[(String, u64)] {
        &self.dues
    }

    pub fn compte(&self) -> usize {
        self.dues.len()
    }

    pub fn est_vide(&self) -> bool {
        self.dues.is_empty()
    }

    /// Can the journal be compacted?
    ///
    /// 🔴 **`est_vide()` AND the size, NEVER the size alone.** Truncating a
    /// file that still carries a due entry would lose the data exactly when
    /// it matters.
    pub fn compactable(&self, file_size: u64) -> bool {
        self.est_vide() && file_size > JOURNAL_COMPACTION_SIZE
    }

    fn poser(&mut self, chemin: &str, octets: u64) {
        match self.dues.iter_mut().find(|(c, _)| c == chemin) {
            Some((_, o)) => *o = octets,
            None => self.dues.push((chemin.to_string(), octets)),
        }
    }

    fn oter(&mut self, chemin: &str) {
        // 🔴 **EXACT EQUALITY, never a prefix.** Removing by prefix would mean
        // that `note.txt` would erase `note.txt.bak`, and that a folder would erase
        // everything it contains.
        self.dues.retain(|(c, _)| c != chemin);
    }
}

enum Entree {
    Inscription { chemin: String, octets: u64 },
    Retrait { chemin: String },
}

fn encoder(chemin: &str) -> String {
    serde_json::to_string(chemin).expect("une chaîne se sérialise toujours en JSON")
}

fn decoder(brut: &str) -> Option<String> {
    serde_json::from_str::<String>(brut).ok()
}

fn analyser(ligne: &str) -> Option<Entree> {
    let (marque, reste) = ligne.split_at_checked(1)?;
    match marque {
        "+" => {
            // `+<bytes> <JSON path>`: the first space separates, and there
            // cannot be one in a number.
            let (octets, chemin) = reste.split_once(' ')?;
            Some(Entree::Inscription {
                chemin: decoder(chemin)?,
                octets: octets.parse().ok()?,
            })
        }
        "-" => Some(Entree::Retrait {
            chemin: decoder(reste)?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
