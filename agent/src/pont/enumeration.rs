//! Enumeration sessions: what a directory contains, in which order,
//! and where we are. **PURE** — no `cfg`, no dependency on `windows`,
//! entirely tested on the host.
//!
//! # ProjFS's two traps, both SILENT
//!
//! 1. **The order is IMPOSED.** Entries must be filled in the order of
//!    `PrjFileNameCompare` — which is **neither** `OsStr`'s lexicographic order,
//!    **nor** `Ordering::cmp`. Yet `dir.values()` of the File System Access API
//!    guarantees **no** order. The bridge therefore sorts itself, and
//!    `PrjFileNameCompare` is loaded (task 12) precisely for that.
//! 2. **The `searchExpression` filter is OPTIONAL and it is PROVIDED**
//!    (`PRJ_GET_DIRECTORY_ENUMERATION_CB`, `mod.rs:315`). Ignoring it is a
//!    silent fault: a `dir /b *.txt` would return everything. It is applied through
//!    `PrjFileNameMatch`.
//!
//! **Both comparators are INJECTED**, and that is what makes this module
//! testable: the logic — filter then sort, and where the cursor is — is
//! pure; only the two comparison functions come from ProjFS.
//!
//! # What this module is NOT: an enumeration cache
//!
//! ⚠️ A [`Session`] holds the entries of **one** enumeration, between the
//! successive `GetDirectoryEnumeration` calls that serve it, and dies with
//! `EndDirectoryEnumeration`. **It is not the enumeration cache
//! (`TTL_ENUMERATION`) of spec §7.4, which is NOT delivered in F1**: that one
//! would outlive the session, would be indexed by PATH, and could only be emptied
//! by `Rafraichir` — a deliverable of F5. Setting up a cache whose content nothing can
//! empty would mean a file added on the local workstation would
//! **never** appear: the exact defect of the old bridge, whose data cache
//! had no TTL (`src/file.js:232-241`).
//!
//! ✅ **THIS PROGNOSIS WAS VERIFIED, AND F5 EXISTS (August 21st, 2026).** *These lines
//! announced: "F5's RED criterion — the file appears WITHOUT
//! `Rafraichir` — will by construction be red as long as F5 does not exist".*
//! **It was, and it is measured**: on F5's binary with `PONT_CACHE=0`,
//! which reproduces exactly the earlier product, the file added on the local
//! workstation appears **without** `Rafraichir` — 2 runs. With the cache armed, it
//! only appears **after** — 3 runs.
//!
//! ⚠️ **THE CACHE NOW LIVES IN [`crate::pont::cache`], NOT HERE**, and the
//! distinction this module states remains whole: a [`Session`] dies with
//! `EndDirectoryEnumeration`, the cache outlives it and is indexed only by
//! PATH. **`preparer` runs at each session load, including on a
//! cache hit** — the cache memorises the RAW entries, precisely so
//! that a `dir *.txt` does not poison the next `dir`.

use std::cmp::Ordering;

/// A directory entry, as the browser reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entree {
    pub nom: String,
    pub repertoire: bool,
    pub size: u64,
    /// `File.lastModified`, in milliseconds since the Unix epoch.
    ///
    /// ⚠️ **The File System Access API gives only ONE**, and the four time
    /// fields of `PRJ_FILE_BASIC_INFO` all carry it. It is an accepted
    /// divergence (spec §3.5.2), not an oversight.
    pub modified_ms: i64,
}

/// Filters then sorts, with ProjFS's two functions **injected**.
///
/// `expression` absent: the matcher is **never** consulted. ProjFS does not
/// always provide one, and forging one (`*`) would make the result depend
/// on `PrjFileNameMatch`'s behaviour on a pattern we invented.
pub fn preparer(
    entrees: Vec<Entree>,
    expression: Option<&str>,
    mut apparier: impl FnMut(&str, &str) -> bool,
    mut comparer: impl FnMut(&str, &str) -> Ordering,
) -> Vec<Entree> {
    let mut retenues: Vec<Entree> = match expression {
        Some(motif) => entrees
            .into_iter()
            .filter(|e| apparier(&e.nom, motif))
            .collect(),
        None => entrees,
    };
    // `sort_by` and not `sort_unstable_by`: the comparator comes from ProjFS and
    // can declare two names equal (case, notably). An unstable sort
    // would then give a different order from one call to the next on the same
    // input, which is exactly what ProjFS enumeration forbids between
    // two `GetDirectoryEnumeration` of the same session.
    retenues.sort_by(|a, b| comparer(&a.nom, &b.nom));
    retenues
}

/// An enumeration session: the prepared entries, and where we are.
///
/// ⚠️ **Indexed by the ENUMERATION GUID, never by path** (spec §7.2) —
/// it is the caller that holds the index, but the reason lives here: two
/// applications listing the same directory at the same time open two
/// distinct sessions, and indexing by path would make the second overwrite
/// the first; one of the two would receive an empty directory.
#[derive(Debug, Default)]
pub struct Session {
    entrees: Option<Vec<Entree>>,
    curseur: usize,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// True as soon as the browser has answered **once** for this session.
    ///
    /// An empty directory is indeed "loaded": without this distinction, a
    /// session on an empty directory would request the list again at each
    /// `GetDirectoryEnumeration` call, indefinitely.
    pub fn chargee(&self) -> bool {
        self.entrees.is_some()
    }

    /// Sets the entries **and resets the cursor to zero**.
    ///
    /// The reset is not a convenience: without it, a second
    /// `Entrees` response would leave the cursor beyond the new list, and
    /// the enumeration would return empty.
    pub fn poser(&mut self, entrees: Vec<Entree>) {
        self.entrees = Some(entrees);
        self.curseur = 0;
    }

    /// Honours `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (`mod.rs:177`, value
    /// `1i32`): the cursor goes back to the start, **and the entries are
    /// kept**.
    ///
    /// Throwing them away would force asking the browser for the list again, which is a
    /// round trip for nothing — and above all would return `S_OK` with an empty
    /// buffer in the meantime, that is, an empty directory, silently.
    pub fn redemarrer(&mut self) {
        self.curseur = 0;
    }

    /// The current entry, or `None` if the session is exhausted or not loaded.
    pub fn prochaine(&self) -> Option<&Entree> {
        self.entrees.as_ref()?.get(self.curseur)
    }

    /// Moves to the next. Called **after** a fill accepted by
    /// `PrjFillDirEntryBuffer`, never before: advancing on a full buffer
    /// would lose the entry forever.
    pub fn avancer(&mut self) {
        self.curseur += 1;
    }
}

#[cfg(test)]
mod tests;
