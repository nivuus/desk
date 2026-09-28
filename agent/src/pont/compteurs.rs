//! The counter of the twelve failure causes, and **the ONLY point where a cause
//! becomes an `HRESULT`**. **PURE** — no `cfg`, no clock, no I/O.
//!
//! # What this module delivers, and what it does NOT deliver
//!
//! The table of twelve `HRESULT`s **exists since F1** ([`crate::pont::errors`],
//! twelve variants, twelve codes, `COUNT = 12` and a two-stage structural
//! guard). Spec §8 F3 nevertheless writes that F3 "delivers the HRESULT table of
//! §5 in full": **that is stale, and F3's plan notes it**.
//! What F3 really delivers is criterion (4) — *each of the twelve is
//! observed at least once* —, and it is this module that makes it decidable.
//!
//! # 🔴 WHY A COUNTER, AND NOT A TRACE PER FAILURE
//!
//! The only trace that named the cause of a refusal was a `debug!`
//! (`pont::service`), whereas `scripts/run-agent.sh` sets `RUST_LOG=info` by
//! default and this repository's doctrine is that operation runs at
//! `info`: **in an ordinary acceptance run, no cause was observable**, and
//! criterion (4) was therefore UNSATISFIABLE.
//!
//! Raising the whole bridge to `debug` would flood the log with one line per callback
//! — it is the "never trace per packet" trap the TURN work stream paid for with
//! 18,619 lines in a few seconds, written to a CIFS share from the
//! loop: **the measurement destroyed what it measured**. We count, and we
//! take a census once per period.
//!
//! # The third stage of the structural guard
//!
//! [`crate::pont::errors`] carries two: an exhaustive `match` forces
//! classifying any new variant, and `COUNT` forces registering it in `ALL`.
//! [`nom`] is a **third**: without a name, a new variant
//! would not appear in the census, and criterion (4) would declare it held
//! without ever having seen it.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::pont::errors::{hresult, Error, COUNT};

/// The name of a cause **on the census line**.
///
/// ⚠️ **Kebab-case, like `proto::files::CodeEchec` on the wire**, and for the
/// same reason: it is what an acceptance `grep` will write. The `match` is
/// **exhaustive** — a new variant cannot inherit another's name.
///
/// ⚠️ **IT IS NOT `CodeEchec`, and the two do not overlap.**
/// `CodeEchec` is what the BROWSER says; `Error` is what the BRIDGE returns
/// to Windows. `casse-ambigue` exists on the browser side and not here;
/// `delai-depasse` exists here and not there. Confusing them would make one look in
/// the census for a name that will never be there.
pub fn nom(e: Error) -> &'static str {
    match e {
        Error::Introuvable => "introuvable",
        Error::CheminIntrouvable => "chemin-introuvable",
        Error::AccesRefuse => "acces-refuse",
        Error::CanalFerme => "canal-ferme",
        Error::DelaiDepasse => "delai-depasse",
        Error::Abandonnee => "abandonnee",
        Error::DisquePlein => "disque-plein",
        Error::NonSupporte => "non-supporte",
        Error::RepertoireNonVide => "repertoire-non-vide",
        Error::DejaPresent => "deja-present",
        Error::ProtegeEnEcriture => "protege-en-ecriture",
        Error::Inattendue => "inattendue",
    }
}

/// One counter per variant of [`Error`].
///
/// ⚠️ **`AtomicU64`s and not a bare field under a lock**: the counters are
/// incremented from **the callback threads the system owns**, where
/// thread discipline forbids waiting for a lock. A `Mutex` there would make
/// the application reading the file wait.
#[derive(Debug, Default)]
pub struct Compteurs {
    cases: [AtomicU64; COUNT],
}

impl Compteurs {
    pub fn nouveaux() -> Self {
        Self::default()
    }

    /// 🔴 **THE ONLY POINT WHERE A CAUSE BECOMES AN `HRESULT`.**
    ///
    /// The invariant F3 delivers is that `errors::hresult` has only ONE
    /// caller outside tests: this one. The check that establishes it lives in the
    /// log of task 5, and it was seen RED **before** the task
    /// started — 26 lines of code called `hresult` directly.
    ///
    /// ⚠️ **Counting AND translating in the same call is deliberate.** Two
    /// functions — one that counts, the other that translates — could be
    /// separated by a hurried caller, and the counter would stop counting without
    /// anything saying so. Here, obtaining the code IS incrementing it.
    pub fn rendre(&self, cause: Error) -> i32 {
        self.cases[rang(cause)].fetch_add(1, Ordering::Relaxed);
        hresult(cause)
    }

    /// Le compte d'une cause.
    pub fn compte(&self, cause: Error) -> u64 {
        self.cases[rang(cause)].load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        Error::ALL.iter().map(|e| self.compte(*e)).sum()
    }

    /// The causes still at **zero** — that is what is missing for criterion (4).
    ///
    /// 🔴 **It is the most important red of this module**: an always empty
    /// `manquants()` would make criterion (4) be declared HELD on a run where
    /// nothing was exercised. The test `missing_returns_exactly_the_causes_at_zero`
    /// catches it.
    pub fn manquants(&self) -> Vec<Error> {
        Error::ALL
            .into_iter()
            .filter(|e| self.compte(*e) == 0)
            .collect()
    }

    /// The census line, **in the order of `Error::ALL`**.
    ///
    /// ⚠️ **The order is pinned by a test.** A census whose order
    /// drifted would make one counter be read for another — it is the trap of
    /// "two messages sharing a substring" in another form.
    /// The names are written out in full at each field, so an acceptance `grep`
    /// reads a name and never a rank.
    pub fn recensement(&self) -> String {
        let mut ligne = format!("total={}", self.total());
        for e in Error::ALL {
            ligne.push_str(&format!(" {}={}", nom(e), self.compte(e)));
        }
        ligne
    }
}

/// The rank of a variant in [`Error::ALL`].
///
/// ⚠️ **Derived from `ALL` and not written by hand.** `errors::index` already exists
/// and is authoritative, but it is **private** — and duplicating it here would make
/// two truths nothing confronts, exactly the defect of `TYPES_AGENT`
/// (`proto/ts/control.ts`), a hand-written list nothing compares with
/// the union it reflects. Searching in `ALL` is O(12) on a failure
/// path: the cost is nil, and the uniqueness of the source is not.
fn rang(e: Error) -> usize {
    Error::ALL
        .iter()
        .position(|c| *c == e)
        .expect("every Error variant appears in ALL — COUNT enforces it")
}

#[cfg(test)]
mod tests;
