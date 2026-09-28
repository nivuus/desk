//! The histogram of bridge traversals, **per command family**. **PURE** —
//! no `cfg`, no I/O, **and no clock read internally**: the duration is
//! a parameter, exactly as in [`crate::pont::table`], "which makes
//! expiry testable without sleeping".
//!
//! # 🔴 WHAT THIS MODULE IS NOT
//!
//! **It is not a per-command trace.** *Count or sample, never
//! trace per unit* — the per-packet trace of the TURN work item killed the session
//! it measured with 18,619 lines in a few seconds, written to a
//! CIFS share from the loop. It is the same reason that made
//! [`crate::pont::compteurs`] a counter rather than a per-failure trace.
//!
//! **It is not a measure of what the APPLICATION waits for.** It measures the
//! **bridge → browser → bridge** traversal, and nothing else: neither the entry into
//! the ProjFS callback, nor the table registration, nor the sweep at
//! `PERIODE_BALAYAGE`, nor `PrjCompleteCommand`, nor ProjFS's return to
//! the application. **The names of things say so** — `traversees`, never
//! `latences`. What the application waits for is measured by a stopwatch IN
//! the VM, and the difference between the two is a **named residue, never a
//! measured quantity** (F4 plan, §0.5).
//!
//! **It is not a `Mutex`.** The counters are `AtomicU64`s, like those
//! of [`crate::pont::compteurs`] and **for the same reason**: they are touched
//! from the callback threads the SYSTEM owns, where waiting for a lock
//! would make the application wait.
//!
//! # The family is the BUDGET, not the verb
//!
//! The five families cover **exactly** the five budgets of
//! [`crate::pont::table`], and that is what makes the census readable against
//! them: `Create` shares `WRITE_TIMEOUT` with `Write` (both registered
//! by `ecriture::fil`), `Muter` has `DELAI_MUTATION` to itself. Grouping by
//! verb instead of by budget would produce a distribution no
//! constant frames.
//!
//! # The structural guard, in three stages
//!
//! The twin of the one in [`crate::pont::errors`] and [`crate::pont::compteurs`]:
//! [`COUNT`] forces registration in [`Famille::ALL`], [`nom`] is an
//! **exhaustive** `match` — a new family cannot inherit the name of another
//! one —, and [`Famille::de`] is a second exhaustive `match` that forces
//! classifying any [`Attendue`] variant that would appear.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::pont::table::Attendue;

/// A command family, as read in the census.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Famille {
    Attributs,
    Lister,
    Lire,
    Write,
    Mutation,
}

/// ⚠️ **Adding one more family requires raising `COUNT`**, which makes
/// the compilation of [`Famille::ALL`], typed `[Famille; COUNT]`, fail
/// as long as the new variant is not listed there.
pub const COUNT: usize = 5;

impl Famille {
    pub const ALL: [Famille; COUNT] = [
        Famille::Attributs,
        Famille::Lister,
        Famille::Lire,
        Famille::Write,
        Famille::Mutation,
    ];

    /// The family of a command in flight. **Exhaustive** `match`: a new
    /// [`Attendue`] variant does not compile as long as it is not classified.
    pub fn de(attendue: &Attendue) -> Famille {
        match attendue {
            Attendue::Attributs { .. } => Famille::Attributs,
            Attendue::Lister { .. } => Famille::Lister,
            Attendue::Lire { .. } => Famille::Lire,
            // ⚠️ `Create` IS of the `Write` family: both are registered
            // by `ecriture::fil` under the same `WRITE_TIMEOUT`.
            Attendue::Write { .. } | Attendue::Create { .. } => Famille::Write,
            Attendue::Muter { .. } => Famille::Mutation,
        }
    }
}

/// The name of a family **on the census line**.
///
/// ⚠️ **Written at each field, never derived from a rank**: an acceptance `grep`
/// reads a name, and a census whose order drifted would otherwise read one
/// counter for another.
pub fn nom(f: Famille) -> &'static str {
    match f {
        Famille::Attributs => "attributs",
        Famille::Lister => "lister",
        Famille::Lire => "lire",
        Famille::Write => "ecrire",
        Famille::Mutation => "mutation",
    }
}

/// The UPPER bounds of the buckets, in milliseconds. A thirteenth implicit
/// bucket, `inf`, collects everything beyond them.
///
/// **Chosen to cover the five budgets** (2 s, 5 s, 15 s, 20 s, 30 s) and
/// the bridge's RTT (1 to 3 ms measured in D1). ⚠️ **NOT CALIBRATED** — they
/// join `MAX_FRAME_SIZE`, `SEUIL_TAMPON`, `MORCEAUX_EN_VOL`, `BPP_MIN`
/// and everything this repository has never judged in use.
pub const SEAUX_MS: [u64; 12] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000];

/// Douze bornes, plus `inf`.
pub const SEAUX: usize = SEAUX_MS.len() + 1;

/// The rank of the bucket that **contains** `duree`.
///
/// 🔴 **The bound is INCLUSIVE at the top**: a traversal of exactly 5 ms
/// falls into bucket `5`, never into bucket `10`. A `<` instead of a `<=`
/// would shift the whole distribution by one bucket, silently.
fn seau_de(duree: Duration) -> usize {
    let us = duree.as_micros();
    SEAUX_MS
        .iter()
        .position(|ms| us <= u128::from(*ms) * 1_000)
        .unwrap_or(SEAUX_MS.len())
}

/// Compte, somme, maximum et distribution, par famille.
#[derive(Debug, Default)]
pub struct Histogramme {
    compte: [AtomicU64; COUNT],
    somme_us: [AtomicU64; COUNT],
    max_us: [AtomicU64; COUNT],
    seaux: [[AtomicU64; SEAUX]; COUNT],
}

impl Histogramme {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a **COMPLETED** traversal. `duree` is computed by
    /// the caller, from ITS clock.
    ///
    /// ⚠️ **A traversal that does not complete is NOT observed**: an expired
    /// or cancelled command never goes through `Table::resoudre`. The
    /// census therefore measures what **completed**, and failures are read
    /// on the line of the twelve codes — the two are read together, never one
    /// for the other.
    pub fn observer(&self, famille: Famille, duree: Duration) {
        let r = rang(famille);
        let us = u64::try_from(duree.as_micros()).unwrap_or(u64::MAX);
        self.compte[r].fetch_add(1, Ordering::Relaxed);
        self.somme_us[r].fetch_add(us, Ordering::Relaxed);
        self.max_us[r].fetch_max(us, Ordering::Relaxed);
        self.seaux[r][seau_de(duree)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn compte(&self, f: Famille) -> u64 {
        self.compte[rang(f)].load(Ordering::Relaxed)
    }

    pub fn max_us(&self, f: Famille) -> u64 {
        self.max_us[rang(f)].load(Ordering::Relaxed)
    }

    /// The mean, **zero when nothing has been observed** — and not a division
    /// by zero.
    pub fn moyenne_us(&self, f: Famille) -> u64 {
        let n = self.compte(f);
        if n == 0 {
            return 0;
        }
        self.somme_us[rang(f)].load(Ordering::Relaxed) / n
    }

    pub fn seau(&self, f: Famille, rang_seau: usize) -> u64 {
        self.seaux[rang(f)][rang_seau].load(Ordering::Relaxed)
    }

    /// The census line, **in the order of [`Famille::ALL`]**, as a
    /// **single string** `name=value`.
    ///
    /// ⚠️ **Never as `tracing` fields**: those would carry ANSI
    /// sequences between the name and the value in a RAW log — the trap that
    /// D8's input acceptance run paid for and that F1's `grep` replayed three
    /// times. It is read **without `sed`**.
    ///
    /// ⚠️ **The counters are CUMULATIVE since the bridge started**: a
    /// measurement is read by DIFFERENCE between two censuses, never on an
    /// isolated line.
    ///
    /// ⚠️ **The buckets are emitted for the FIVE families**, and not only for
    /// `lire`: emitting only one would make that choice a hidden decision, and
    /// a successor measuring `lister` would find no
    /// distribution there. Divergence E13 of the plan, declared.
    pub fn recensement(&self) -> String {
        let mut ligne = String::from("traversees");
        for f in Famille::ALL {
            ligne.push_str(&format!(
                " {}=n:{} moy_us:{} max_us:{}",
                nom(f),
                self.compte(f),
                self.moyenne_us(f),
                self.max_us(f)
            ));
        }
        ligne.push_str(" | seaux_ms");
        for f in Famille::ALL {
            ligne.push_str(&format!(" {}=", nom(f)));
            for (i, borne) in SEAUX_MS.iter().enumerate() {
                ligne.push_str(&format!("{}:{},", borne, self.seau(f, i)));
            }
            ligne.push_str(&format!("inf:{}", self.seau(f, SEAUX_MS.len())));
        }
        ligne
    }
}

/// The rank of a family in [`Famille::ALL`].
///
/// ⚠️ **Derived from `ALL` and not written by hand**, like
/// `compteurs::rang`: two truths nothing confronts would diverge
/// silently.
fn rang(f: Famille) -> usize {
    Famille::ALL
        .iter()
        .position(|c| *c == f)
        .expect("every family appears in ALL — COUNT enforces it")
}

#[cfg(test)]
mod tests;
