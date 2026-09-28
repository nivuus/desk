//! Renaming and deletion: **what to push, and IN WHICH ORDER relative
//! to due writes**. **PURE** — no `cfg`, no I/O, no
//! clock.
//!
//! # 🔴 THE RULE WHOSE OMISSION PRODUCES DATA LOSS
//!
//! **It is written in no document of the repository before this one**, and it
//! arises from the interaction of two sub-blocks each of which is correct alone.
//!
//! The saving idiom spec §3.5 gives to justify its decision
//! D5 is: *write a temporary file, rename, delete the old one*.
//! LibreOffice, Word and most editors use it, and the three
//! gestures arrive **in a burst**, on the same directory.
//!
//! Yet F2 pushes writes **after the fact**, when the handle closes, in
//! a window it declares itself is not bounded in duration.
//! So, without this module:
//!
//! - **a write still due on `de` when `Renommer` goes out would arrive
//!   AFTER the renaming, on a path that no longer exists** — and
//!   `getFileHandle(…, { create: true })` **WOULD RECREATE the temporary file**:
//!   the save would be lost, and a swap file would remain on the
//!   local workstation;
//! - **a write due on a path just deleted WOULD RECREATE** what
//!   the user erases.
//!
//! **Each of the two sub-blocks is correct alone; it is their interaction that
//! destroys.** It is the exact shape of the defects the cross-cutting reviews of
//! this repository have found since D7, and the only one a per-task review
//! structurally cannot see.
//!
//! # ⚠️ THIS MODULE DOES NOT DECIDE THE `PRE_`, AND IT IS A DECLARED DIVERGENCE
//!
//! §3.2 of F3's plan prescribes a `decider_prealable(etat, quoi)` for it.
//! **It is not written**: [`crate::pont::notifications::decider`] IS that
//! function, it has existed since F1, it is pure, it is swept over the 32
//! bits of the mask and over four states, and F3 has just added to it the four
//! refusals of renaming and deletion. Writing a second one would make two
//! truths nothing confronts — the defect of `TYPES_AGENT`
//! (`proto/ts/control.ts`), a hand-written list nothing compares to
//! the union it reflects.

/// Ce qu'une mutation demande au poste local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    /// 🔴 **`de` IS THE SOURCE, `vers` THE DESTINATION.** Getting the direction wrong would
    /// produce no error: the renaming would happen, backwards, and the
    /// destination would overwrite the source. It is risk R-F3-1 of the plan.
    Renommer {
        de: String,
        vers: String,
        repertoire: bool,
    },
    Supprimer {
        chemin: String,
        repertoire: bool,
    },
}

impl Mutation {
    /// The **SOURCE** path — the one that still exists when the mutation
    /// is decided, and hence the one on which writes can be due.
    pub fn source(&self) -> &str {
        match self {
            Mutation::Renommer { de, .. } => de,
            Mutation::Supprimer { chemin, .. } => chemin,
        }
    }

    /// Is the source a directory?
    ///
    /// 🔴 **It is what decides whether writes due on CHILDREN
    /// count**, and that is why `isdirectory` is carried from
    /// the callback rather than rediscovered by the browser: the browser would
    /// ask again at the cost of a round trip, and would be wrong about an entry
    /// the renaming has precisely just made disappear.
    pub fn repertoire(&self) -> bool {
        match self {
            Mutation::Renommer { repertoire, .. } | Mutation::Supprimer { repertoire, .. } => {
                *repertoire
            }
        }
    }
}

/// What must be done **BEFORE** pushing a mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordonnancement {
    /// Nothing holds back: push now.
    Pousser,
    /// These paths carry due writes, and the mutation **IS NOT
    /// pushed**: they must be drained first, then ask again.
    ///
    /// ⚠️ **Draining is not instantaneous, and its failure must NOT be
    /// treated as a success.** If the writes do not go out, the mutation
    /// stays waiting, the file is NAMED to the user by F2's
    /// counter, and the log carries its reason. Pushing anyway would lose
    /// the save.
    AttendreEcrituresDues { chemins: Vec<String> },
    /// These due writes must be **REMOVED** from the journal, **then the
    /// mutation is pushed right after**.
    ///
    /// 🔴 **The removal comes BEFORE the push, and the order is the very meaning**:
    /// pushing first would recreate on the local workstation what the user has just
    /// erased, and the deletion that follows would not necessarily catch
    /// up — the browser can refuse it (non-empty directory,
    /// permission), and the resurrected file would remain.
    AbandonnerEcrituresDues { chemins: Vec<String> },
}

/// What to do with `quoi`, knowing that `dues` are the paths whose
/// bytes are still waiting to be pushed.
///
/// ⚠️ **`dues` carries the paths of the write queue — IN FLIGHT INCLUDED.**
/// Only considering the waiting list would let through the most common case: the
/// temporary file whose push has just started, and which the renaming
/// follows by a few milliseconds.
pub fn ordonnancer(dues: &[String], quoi: &Mutation) -> Ordonnancement {
    let concernes: Vec<String> = dues
        .iter()
        .filter(|due| concerne(due, quoi.source(), quoi.repertoire()))
        .cloned()
        .collect();
    if concernes.is_empty() {
        return Ordonnancement::Pousser;
    }
    match quoi {
        Mutation::Renommer { .. } => Ordonnancement::AttendreEcrituresDues { chemins: concernes },
        Mutation::Supprimer { .. } => {
            Ordonnancement::AbandonnerEcrituresDues { chemins: concernes }
        }
    }
}

/// Is a write due on `due` held back by a mutation of `cible`?
///
/// 🔴 **TWO RULES THAT WOULD CONTRADICT EACH OTHER IF FILE AND
/// DIRECTORY WERE CONFUSED**, and it is `repertoire` that decides:
///
/// - a write due on **ANOTHER** path delays nothing — comparing by
///   prefix alone would make every write block every renaming;
/// - a write due on a **CHILD** of a renamed directory does delay —
///   comparing by equality alone would let the directory case slip
///   through, and the child would be recreated under the old path.
///
/// ⚠️ **The prefix's `/` is not decorative**: without it, renaming `a`
/// would hold back a write due on `ab/x`, which is unrelated.
fn concerne(due: &str, cible: &str, repertoire: bool) -> bool {
    if due == cible {
        return true;
    }
    if !repertoire {
        return false;
    }
    // Renaming the ROOT (`cible` empty) does not exist: ProjFS never delivers
    // a notification for it. The case is refused rather than treated
    // as "everything is a child", which would hold back every write forever.
    if cible.is_empty() {
        return false;
    }
    due.starts_with(&format!("{cible}/"))
}

/// The mutations in flight and waiting.
///
/// ⚠️ **Only one in flight at a time, GLOBALLY — it is stricter than what
/// the plan asks, and it is deliberate.** It writes "one mutation per PATH at
/// a time"; F2's write queue serialises globally. Two
/// different disciplines on the same channel read badly together, and the stricter
/// makes the other true *a fortiori*: two renamings of the same path cannot
/// cross if no pair can.
///
/// 🔴 **NO COALESCING, unlike [`crate::pont::ecriture::File`].**
/// Two writes of the same file have only one effect — writing the last
/// content. Two mutations do not: renaming `a`→`b` then `b`→`c` are two
/// gestures whose **order is the meaning**, and merging the second into the first
/// would leave `b` on the local workstation.
#[derive(Debug, Default)]
pub struct FileMutations {
    en_vol: Option<Mutation>,
    attente: Vec<Mutation>,
}

impl FileMutations {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Registers a mutation, and returns **what must be pushed now**.
    ///
    /// `None` means "nothing to start": a mutation is already in flight.
    pub fn signaler(&mut self, quoi: Mutation) -> Option<Mutation> {
        self.attente.push(quoi);
        self.demarrer()
    }

    /// The mutation in flight is over — **whatever its outcome**.
    ///
    /// 🔴 **FAILURE INCLUDED.** A mutation that fails frees the flight: otherwise
    /// a single refusal would block all following mutations. It is what
    /// F2's write queue already does, for the same reason.
    pub fn terminee(&mut self) -> Option<Mutation> {
        self.en_vol = None;
        self.demarrer()
    }

    /// Puts the in-flight mutation back **at the HEAD of the waiting list**, without losing it.
    ///
    /// It is what we do with a mutation [`ordonnancer`] holds back: the
    /// due writes go out, and the mutation goes back **ahead** of those that
    /// followed it — otherwise the order of the user's gestures would be
    /// reversed.
    pub fn differer(&mut self) {
        if let Some(quoi) = self.en_vol.take() {
            self.attente.insert(0, quoi);
        }
    }

    #[cfg(test)]
    pub fn en_vol(&self) -> Option<&Mutation> {
        self.en_vol.as_ref()
    }

    #[cfg(test)]
    pub fn en_attente(&self) -> usize {
        self.attente.len()
    }

    fn demarrer(&mut self) -> Option<Mutation> {
        if self.en_vol.is_some() || self.attente.is_empty() {
            return None;
        }
        let suivante = self.attente.remove(0);
        self.en_vol = Some(suivante.clone());
        Some(suivante)
    }
}

#[cfg(test)]
mod tests;
