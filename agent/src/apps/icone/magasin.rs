//! The agent's icon store: bytes ADDRESSED BY THEIR CONTENT.
//!
//! **PURE, with no `cfg`.** It knows neither Windows, nor COM, nor the file
//! system: it holds a `empreinte -> octets` table for the CURRENT
//! catalogue, and it can tell which of an announced set are missing from
//! another.
//!
//! 🔴 WHY CONTENT ADDRESSING PAYS OFF, AND IT IS NOT A CONJECTURE.
//! Measured on 20 August 2026 on the real corpus of the development VM:
//! **153 applications yield 99 DISTINCT PNGs**, i.e. **54 uploads
//! avoided (35.3 %)**. Twelve fingerprints are shared, one of them by **twenty-seven**
//! applications — the same `runcmdu.exe` targeted by twenty-seven shortcuts
//! with different arguments. Decision D4 of the specification does separate these
//! twenty-seven APPLICATIONS; they share ONE icon, and that is exactly what
//! this module exists to avoid paying for twenty-seven times.
//!
//! ⚠️ **The DEDUPLICATED weight has NOT been measured**: the probe sums the 153 PNGs
//! (4,576,398 bytes, 29,911 B/icon), never the 99 distinct ones. Do not
//! deduce it by the rule of three — icons do not all have the same size.

use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

use crate::apps::sha256;

/// The SHA-256 fingerprint of a PNG, in lowercase hexadecimal.
///
/// 🔴 IT IS THE FINGERPRINT OF THE PNG BYTES, NOT OF THE PIXELS, and the next
/// link is what decides. The platform RECOMPUTES the fingerprint of what
/// it receives ("no hop trusts the previous one"): addressing
/// by PIXELS would force it to DECODE the PNG to check, that is to
/// ship a PNG decoder in TypeScript — a new production
/// dependency, which this sub-block refuses. Addressing by bytes makes the
/// check exact and free: `sha256(corps) === :sha256`.
///
/// ⚠️ **THE PRICE OF THIS CHOICE IS THE ENCODER'S DETERMINISM**, and it is
/// named. If the encoder did not write the same bytes twice for the same
/// image — a `tIME` chunk, a software `tEXt` —, the fingerprint would change at
/// every reconciliation and the agent would re-upload everything, indefinitely. **It
/// is NOT a blocking gate**: the catalogue would stay correct and the
/// icons would still be served; only the cost would rise. The remedy is named
/// in advance and lives **in this very module, which is pure** — fingerprint only the
/// `IHDR`/`PLTE`/`IDAT`/`IEND` chunks, discarding the ancillary ones.
///
/// 🔵 NO NEW LINE OF CRYPTOGRAPHY: `apps::sha256` is already written,
/// pure, and tested against the FIPS 180-4 known-answer vectors.
pub fn empreinte(png: &[u8]) -> String {
    sha256::hex(png)
}

/// The bytes of the CURRENT catalogue, one entry per distinct fingerprint.
#[derive(Debug, Default)]
pub struct Magasin {
    par_empreinte: BTreeMap<String, Vec<u8>>,
}

impl Magasin {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a PNG and returns its fingerprint. **Idempotent.**
    ///
    /// 🔴 TWO ADDITIONS OF THE SAME CONTENT MAKE ONLY ONE ENTRY, and that is
    /// the essential point: on this corpus, accumulating would cost 153 entries where
    /// 99 are enough.
    pub fn add(&mut self, png: Vec<u8>) -> String {
        let e = empreinte(&png);
        self.par_empreinte.entry(e.clone()).or_insert(png);
        e
    }

    #[cfg(test)]
    pub fn contient(&self, empreinte: &str) -> bool {
        self.par_empreinte.contains_key(empreinte)
    }

    pub fn octets(&self, empreinte: &str) -> Option<&[u8]> {
        self.par_empreinte.get(empreinte).map(Vec::as_slice)
    }

    #[cfg(test)]
    pub fn empreintes(&self) -> BTreeSet<String> {
        self.par_empreinte.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.par_empreinte.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.par_empreinte.is_empty()
    }

    /// 🔴 DISCARDS ALL THE PREVIOUS CONTENT. The store carries the CURRENT
    /// catalogue, never the history: merging it would make it grow with no end
    /// from one reconciliation to the next, on a process that lives for days.
    pub fn remplacer(&mut self, neuf: Magasin) {
        self.par_empreinte = neuf.par_empreinte;
    }
}

/// Those of `annoncees` that `connues` does not carry, **in the order
/// of announcement** and without duplicates.
///
/// ⚠️ **IT HAS NO PRODUCTION CALLER IN THE AGENT, AND IT IS DECLARED
/// RATHER THAN HIDDEN.** It is the PLATFORM that decides what it is missing —
/// by querying its DISK, never a table —, and the agent merely
/// honours the list it pushes to it. This function is the
/// HOST-TESTABLE twin of that rule: it exists so that the rule is tested
/// where it is pure, and so that the day the agent has to filter by itself,
/// it does not have to rewrite it.
///
/// ⚠️ This repository has no doctrine on orphan code — sub-block D10
/// DELETED the size-compatibility check and KEPT `refresh_output_size` without
/// stating a rule. The choice is made here in favour of keeping it, and
/// it is written down.
///
/// It is the rule the platform applies too, written once on the side where
/// it is PURE.
///
/// ⚠️ THE ORDER OF ANNOUNCEMENT IS PRESERVED rather than sorted: it is the
/// catalogue's, hence the one in which the user will see the icons arrive.
#[cfg(test)]
pub fn manquantes(annoncees: &[String], connues: &BTreeSet<String>) -> Vec<String> {
    let mut vues = BTreeSet::new();
    annoncees
        .iter()
        .filter(|e| !connues.contains(*e) && vues.insert((*e).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "magasin/tests.rs"]
mod tests;
