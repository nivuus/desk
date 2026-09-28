//! The queue of due writes: **what** to push, **in which order**, and what
//! to do with a notification that arrives during a push. **PURE** —
//! no `cfg`, no I/O, no clock.
//!
//! # The five rules, and what each prevents
//!
//! 1. **ONLY ONE PUSH IN FLIGHT AT A TIME.** Two concurrent `createWritable()`
//!    streams on the same file would overwrite one another; two streams
//!    on distinct files would saturate the SCTP queue, which F1 already
//!    decided to avoid ("one chunk in flight at a time").
//! 2. **A NOTIFICATION ARRIVING DURING A PUSH IS REPLAYED AFTER**,
//!    and the file is then **reread from the start**. Throwing it away would lose the
//!    last bytes written by the user, **silently**; pushing
//!    the rest without rereading would mix chunks from two eras of the same
//!    file, which no digest would catch.
//! 3. **FIFO REGISTRATION ORDER between distinct paths.** A `HashSet` would
//!    return a different one at each run, and resuming a batch
//!    would become irreproducible.
//! 4. **A DIRECTORY CREATION CARRIES NO CONTENT.** Making it read a
//!    file would return `IsADirectory` on the most mundane path there is.
//! 5. **A FILE CREATION IS FOLLOWED, IN GENERAL, BY A CONTENT
//!    PUSH** — at handle close. A file created and never written
//!    stays empty on both sides, which is right.
//!
//! # ⚠️ What F3 expects from this module, and which must not be taken away from it
//!
//! F3's plan ("renaming without renaming") sets two rules whose
//! omission produces a data loss, and **both assume a queue
//! indexed by PATH**:
//!
//! - `Renommer { de, vers }` must **push first** the writes due on
//!   `de` — otherwise a late push would arrive **after** the renaming,
//!   on a path that no longer exists, and the browser **would recreate the temporary
//!   file**: the save would be lost;
//! - `Delete { chemin }` must **remove** the writes due on `chemin` —
//!   pushing them **would recreate what the user erases**.
//!
//! **Each of the two sub-blocks is correct alone; it is their interaction that
//! destroys.** F2 does not implement them — it has neither renaming nor deletion —,
//! but it exposes what they require: [`File::en_vol`], [`File::attend`] and
//! [`File::oublier`].

pub mod fil;

/// What triggers a push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evenement {
    /// A file was closed after modification, or truncated at opening.
    Modified { chemin: String },
    /// An entry has just appeared in the root.
    Cree { chemin: String, repertoire: bool },
    /// **F3** — an entry was renamed in the VM.
    ///
    /// 🔴 **`de` IS THE SOURCE, `vers` THE DESTINATION**, and getting it wrong
    /// destroys. The callback refuses to build this variant if `vers` is
    /// empty or equal to `de`.
    Renomme {
        de: String,
        vers: String,
        repertoire: bool,
    },
    /// **F3** — an entry was deleted in the VM.
    Deleted { chemin: String, repertoire: bool },
}

impl Evenement {
    /// The path this event CONCERNS.
    ///
    /// ⚠️ **For a renaming, it is the SOURCE**, and it is what makes the
    /// coalescing of [`File`] hold: two gestures on the same file — writing it
    /// then renaming it — are serialised under the same path, which is
    /// exactly what rule §0.3 of F3's plan requires.
    pub fn chemin(&self) -> &str {
        match self {
            Evenement::Modified { chemin }
            | Evenement::Cree { chemin, .. }
            | Evenement::Deleted { chemin, .. } => chemin,
            Evenement::Renomme { de, .. } => de,
        }
    }

    /// A directory has **no byte** to read.
    ///
    /// ⚠️ **A renaming and a deletion have none either**, whatever
    /// their nature: they are not CONTENT pushes. This predicate
    /// only serves the merging of [`fusionner`], where only directory creation
    /// has a consequence — and widening it to mutations would mean that a
    /// renaming followed by a write would never push the bytes.
    pub fn est_repertoire(&self) -> bool {
        matches!(
            self,
            Evenement::Cree {
                repertoire: true,
                ..
            }
        )
    }

    /// Is this event a **mutation** — a renaming or a deletion?
    ///
    /// 🔴 **A mutation DOES NOT COALESCE with a write**: renaming then
    /// writing, or writing then deleting, are two gestures whose order is the
    /// very meaning. It is [`crate::pont::mutation`] that schedules them, and this
    /// predicate is what makes it possible to distinguish them without reading the variant.
    pub fn est_mutation(&self) -> bool {
        matches!(self, Evenement::Renomme { .. } | Evenement::Deleted { .. })
    }
}

/// The queue of due writes.
///
/// ⚠️ **A `Vec` and not a `HashMap`, for the reason of rule 3**: the order
/// is the only thing that makes resumption deterministic.
#[derive(Debug, Default)]
pub struct File {
    /// The push in progress, if there is one.
    en_vol: Option<Evenement>,
    /// ⚠️ **What the push in progress will have to REPLAY at its end.** A
    /// boolean is not enough: the replayed event can be of another nature than
    /// the one in flight (a creation followed by a modification).
    a_rejouer: Option<Evenement>,
    /// The waiting paths, in their registration order.
    attente: Vec<Evenement>,
}

impl File {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals an event, and returns **what must be pushed NOW**.
    ///
    /// `None` means "nothing to start": either a push is already in
    /// flight, or the path was already waiting its turn.
    ///
    /// ⚠️ **DECLARED DIVERGENCE FROM F2'S PLAN.** Its signature announces
    /// "`None` if already in flight → marks `a_rejouer`", and gives
    /// [`File::terminee`] the sole role of "returning the event to replay".
    /// Taken literally, **a distinct path queued during a
    /// push would never be started**: nothing would take it out of the queue. The
    /// two methods therefore return *the event to push now*, which
    /// covers the replay AND the waiting queue.
    pub fn signaler(&mut self, evenement: Evenement) -> Option<Evenement> {
        if self.en_vol.as_ref().map(Evenement::chemin) == Some(evenement.chemin()) {
            // Rule 2: never two pushes of the same path, never a
            // lost notification.
            //
            // 🔴 **THE BASE OF THE MERGE IS THE EVENT IN FLIGHT when no
            // replay is set yet, and a test caught it RED.** Taking
            // `a_rejouer` alone — which is `None` the first time — lost the
            // fact that a DIRECTORY was in flight: a modification arriving
            // during its creation would have replaced it, and the thread would have tried to
            // READ a directory.
            let base = self.a_rejouer.take().or_else(|| self.en_vol.clone());
            self.a_rejouer = Some(fusionner(base, evenement));
            return None;
        }
        match self
            .attente
            .iter()
            .position(|e| e.chemin() == evenement.chemin())
        {
            // Coalescing **in place**: the path keeps its rank. Moving it
            // back to the tail would let younger entries pass ahead of it,
            // whereas it has been waiting longer.
            Some(i) => {
                let ancien = self.attente.remove(i);
                self.attente.insert(i, fusionner(Some(ancien), evenement));
                None
            }
            None => {
                self.attente.push(evenement);
                self.start()
            }
        }
    }

    /// The path's push is over — **whatever its outcome**.
    ///
    /// 🔴 **FAILURE INCLUDED, and it is deliberate.** A push that fails frees
    /// the flight slot: the entry stays due IN THE JOURNAL, but the queue must be able to
    /// advance, otherwise a single failure would block all following
    /// writes. It is the journal that does not forget, not this queue.
    pub fn terminee(&mut self, chemin: &str) -> Option<Evenement> {
        if self.en_vol.as_ref().map(Evenement::chemin) != Some(chemin) {
            return None;
        }
        self.en_vol = None;
        if let Some(rejeu) = self.a_rejouer.take() {
            // The replay goes AHEAD of the queue: the file has just been rewritten,
            // and its bytes are the most recent anyone is waiting for.
            self.attente.insert(0, rejeu);
        }
        self.start()
    }

    /// The path of the push in progress.
    pub fn en_vol(&self) -> Option<&str> {
        self.en_vol.as_ref().map(Evenement::chemin)
    }

    /// Is this path in flight or waiting? **What F3 will read** before
    /// pushing a renaming.
    #[cfg(test)]
    pub fn attend(&self, chemin: &str) -> bool {
        self.en_vol() == Some(chemin) || self.attente.iter().any(|e| e.chemin() == chemin)
    }

    /// Removes a path from the WAITING list. **What F3 will call** on a
    /// deletion: pushing a due write on a deleted path
    /// would recreate what the user erases.
    ///
    /// ⚠️ **Does not touch the push IN FLIGHT**: its frames have already
    /// gone, and the thread finishes them. Deleting it from here would mean its `Fait`
    /// would no longer have a recipient.
    pub fn oublier(&mut self, chemin: &str) {
        self.attente.retain(|e| e.chemin() != chemin);
        if self.a_rejouer.as_ref().map(Evenement::chemin) == Some(chemin) {
            self.a_rejouer = None;
        }
    }

    #[cfg(test)]
    pub fn en_attente(&self) -> usize {
        self.attente.len()
    }

    /// All the paths this queue holds — **IN FLIGHT INCLUDED**.
    ///
    /// 🔴 **It is what `pont::mutation::ordonnancer` reads**, and including the
    /// flight is the point: only considering the waiting list would let through the
    /// most common case of the temp+rename idiom — the temporary file whose
    /// push has just started, and which the renaming follows by a few
    /// milliseconds.
    pub fn chemins_dus(&self) -> Vec<String> {
        self.en_vol()
            .into_iter()
            .map(str::to_string)
            .chain(self.attente.iter().map(|e| e.chemin().to_string()))
            .collect()
    }

    fn start(&mut self) -> Option<Evenement> {
        if self.en_vol.is_some() || self.attente.is_empty() {
            return None;
        }
        let next = self.attente.remove(0);
        self.en_vol = Some(next.clone());
        Some(next)
    }
}

/// Merges two events of the **same** path.
///
/// 🔴 **A DIRECTORY CREATION IS NEVER REPLACED.** A directory is not
/// "modified": letting it become a `Modified` would make a
/// directory be read like a file, and the local workstation would receive `IsADirectory` on
/// the most mundane path there is.
///
/// Everywhere else, **the most recent wins**: a creation then a
/// modification of the same file have only one effect, writing the content — and
/// the browser's writer creates the file along the way.
fn fusionner(ancien: Option<Evenement>, neuf: Evenement) -> Evenement {
    match ancien {
        Some(a) if a.est_repertoire() => a,
        _ => neuf,
    }
}

#[cfg(test)]
mod tests;
