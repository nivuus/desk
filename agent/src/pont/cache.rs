//! The enumeration cache: what a directory contained, and since when.
//! **PURE** — no `cfg`, no dependency on `windows`, fully tested on
//! the host, **clock injected**.
//!
//! # What it is, and what it is NOT
//!
//! A [`crate::pont::enumeration::Session`] keeps the entries of **one**
//! enumeration, between the successive `GetDirectoryEnumeration` calls that
//! serve it, and dies with `EndDirectoryEnumeration`. **This cache
//! outlives it**: it is indexed by **PATH**, it expires by `TTL_ENUMERATION`, and
//! it can only be forcibly emptied by the `Rafraichir` announcement.
//!
//! # 🔴 THE OLD BRIDGE'S DEFECT, AND WHY THIS MODULE IS RISK NO. 1
//!
//! `src/file.js` served bytes from a cache **without any TTL**, invalidated
//! only by a write going through this same bridge. Spec §7.4 draws from it the
//! sentence that governs this file: *"A modification made on the local workstation
//! was therefore never seen, forever."*
//!
//! **It is the only addition of the whole sub-project ③ that can make WRONG a
//! behaviour already accepted by F2 and F3**: a file created, renamed or
//! deleted that would stop being seen. Hence [`CacheEnumeration::invalider`], and
//! hence criterion ④ of the acceptance run, whose red removes the invalidation.
//!
//! # Why entries are stored RAW
//!
//! Filtering by `searchExpression` and sorting by `PrjFileNameCompare`
//! depend on the **request** (`dir *.txt` and `dir` do not have the same
//! result) and on comparators only ProjFS provides. Storing the *prepared*
//! result would mean a `dir *.txt` **would poison** the cache for the next
//! `dir`. The cache therefore lives **upstream** of
//! [`crate::pont::enumeration::preparer`], which runs at each session
//! load.
//!
//! # The clock is a PARAMETER
//!
//! Never `Instant::now()` inside: it is what makes expiry
//! testable **without sleeping**, exactly as `pont::table` gave itself for
//! its delays.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::pont::enumeration::Entree;

/// How long a memorised enumeration keeps being served.
///
/// ⚠️ **NOT CALIBRATED.** It joins `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `MAX_OUTPUT_SIZE`,
/// `REPIT_REARMEMENT_AUDIO`, `REARMEMENTS_MAX`, `MAX_FRAME_SIZE`,
/// `DELAI_LISTER`, `ATTENTE_MAX` and the thirteen buckets of `pont::latence` in the
/// list of this repository's constants that **no measurement has judged**.
///
/// What governed the choice, and which is NOT a calibration:
///
/// - it must be **longer** than a complete acceptance gesture — F4 measures
///   a listing of a thousand entries at ~6 s, and the protocol of criterion ① chains
///   two listings plus a file addition;
/// - it must be **shorter** than the time a user accepts to
///   see stale content without anything telling them;
/// - ⚠️ **it must NOT be 60 s**, which is `PERIODE_HYDRATATION`
///   (`service.rs`). *Two constants that would be recalibrated separately and that
///   carry the same number end up believing they are linked* — this repository already
///   writes this about `PLAFOND_DISSIMULATION` and `micro::PLAFOND`.
pub const TTL_ENUMERATION: Duration = Duration::from_secs(30);

/// What a directory contained, and when it was learnt.
struct Memoire {
    entrees: Vec<Entree>,
    pose_a: Instant,
}

/// The enumeration cache, indexed by directory path.
#[derive(Default)]
pub struct CacheEnumeration {
    par_chemin: HashMap<String, Memoire>,
}

impl CacheEnumeration {
    pub fn new() -> Self {
        Self::default()
    }

    /// The entries memorised for this directory, if they have not expired.
    ///
    /// **The expired entry is REMOVED, not merely ignored.** Leaving it
    /// would make the cache grow endlessly on a tree traversed
    /// once — and this module has **no** eviction policy (spec §10 R4).
    pub fn lire(&mut self, chemin: &str, maintenant: Instant) -> Option<&[Entree]> {
        let perime = {
            let m = self.par_chemin.get(chemin)?;
            maintenant.duration_since(m.pose_a) >= TTL_ENUMERATION
        };
        if perime {
            self.par_chemin.remove(chemin);
            return None;
        }
        self.par_chemin.get(chemin).map(|m| m.entrees.as_slice())
    }

    /// Memorises what a directory contains. Overwrites any earlier memory.
    pub fn poser(&mut self, chemin: String, entrees: Vec<Entree>, maintenant: Instant) {
        self.par_chemin.insert(
            chemin,
            Memoire {
                entrees,
                pose_a: maintenant,
            },
        );
    }

    /// Forgets what the **PARENT** directory of the mutated path contained.
    ///
    /// 🔴 **THE PARENT, NEVER THE PATH ITSELF, and getting it wrong there is
    /// SILENT.** An enumeration cache is indexed by **directory**:
    /// invalidating `dossier/note.txt` would touch no key, the listing of
    /// `dossier` would keep being served from memory, and **the created
    /// file would never appear**. Nothing would say so. A host test
    /// locks it, and its red is to make this function take the path
    /// itself.
    ///
    /// ⚠️ **The parent of `"note.txt"` is the ROOT, `""`** — and the root is
    /// a key like any other, the one a `Get-ChildItem` on the mounted
    /// drive solicits. Forgetting it would make any creation at the root
    /// invisible, which is exactly the gesture of criterion ① of the acceptance run.
    pub fn invalider(&mut self, chemin: &str) {
        self.par_chemin.remove(parent_de(chemin));
    }

    /// Forgets everything. It is what the `Rafraichir` announcement does.
    pub fn drain(&mut self) {
        self.par_chemin.clear();
    }

    /// How many directories are memorised. **For the trace and the tests.**
    pub fn size(&self) -> usize {
        self.par_chemin.len()
    }
}

/// The directory that contains `chemin`, in the bridge's convention:
/// `/` separators, and the **root is the empty string**.
///
/// ⚠️ **This convention is that of `pont::chemins`**, and it comes from the
/// File System Access API, which does not know `\`. A ProjFS path arrives with
/// `\`s and it is converted **before** reaching this module.
fn parent_de(chemin: &str) -> &str {
    match chemin.rfind('/') {
        Some(i) => &chemin[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests;
