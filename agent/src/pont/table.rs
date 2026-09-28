//! The commands in flight: correlation, expiry, cancellation, enumeration
//! sessions. **PURE**: no `cfg`, no clock read internally — time
//! is a parameter, which makes expiry testable without sleeping.
//!
//! **The old bridge's defect this module exists not to replay**
//! (spec §4.2): a `message` listener was set **per request**
//! (`src/file.js:155`) and never removed on the error path (`:126-129`).
//! A failed operation therefore left its listener for life, and all the
//! survivors re-parsed each following message — the cost grew with
//! the number of past failures, indefinitely. Here there is **only one** entry per
//! command, removed by the first of the three outcomes: response, cancellation,
//! expiry.
//!
//! **A late response is THROWN AWAY, never applied.** It is the central
//! invariant: [`resoudre`] returns `None` for a cancelled, expired or
//! unknown correlation. Applying a response whose ProjFS command has already been
//! completed would write into a buffer the system has taken back.
//!
//! [`resoudre`]: Table::resoudre

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// ⚠️ **NOT CALIBRATED.** Set, not measured.
///
/// ✅ **F4 GAVE DISTRIBUTIONS, AND TWO OF THE FIVE BUDGETS ACTUALLY
/// BITE.** `DELAI_LISTER` (20 s) fails any listing beyond ~3,150
/// entries, and `DELAI_LIRE` (5 s) fails any read of four chunks or
/// more, `MORCEAUX_EN_VOL = 4` making these four chunks share
/// ~33 KiB/s. `DELAI_ATTRIBUTS` (2 s) has a margin of two orders of magnitude
/// (traversals of 13 to 31 ms), and so does `DELAI_MUTATION` (15 s) (125 to 193 ms).
/// ⚠️ **GIVING WHAT IT TAKES TO CALIBRATE IS NOT CALIBRATING**: F4 publishes
/// distributions, never proposed values — choosing a number requires a
/// judgement in use that no work item of this repository has ever made. They
/// join `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`,
/// `REPIT_REARMEMENT_AUDIO` and `REARMEMENTS_MAX` in the list of this repository's
/// constants that no measurement has judged.
///
/// **Why THREE budgets and not one**: the old bridge had **a single one**,
/// 10 s, for everything (`src/file.js:89`). Hence two symmetric defects — reads
/// of large blocks that expired before completing, and `getattr`s that
/// froze Explorer ten seconds on a nonexistent path. A single
/// budget cannot be right for an operation that must answer in
/// milliseconds and for one that transfers megabytes.
///
/// ✅ **The fourth budget has arrived: it is [`DELAI_ECRIRE`], and F2 sets it.**
/// *(This line announced "it belongs to F2"; it is corrected here
/// rather than left to the future, by the very branch that realises it.)*
pub const DELAI_ATTRIBUTS: Duration = Duration::from_secs(2);
pub const DELAI_LIRE: Duration = Duration::from_secs(5);
pub const DELAI_LISTER: Duration = Duration::from_secs(20);

/// The budget of a write CHUNK — not of a file.
///
/// ⚠️ **NOT CALIBRATED**, like the three above. It is wider than
/// [`DELAI_LIRE`] for a reason of form, not of measurement: the browser must
/// **write** to the local workstation's disk, and the `dernier` chunk also triggers
/// the `close()` of `createWritable()`, that is, the commit —
/// a copy of the swap file to its destination, whose cost grows with
/// the file's size and which no measurement of this repository bounds.
///
/// 🔴 **THIS BUDGET PROTECTS NO ONE, and that is what distinguishes it from the three
/// others.** Theirs bound the wait of an APPLICATION blocked in a
/// ProjFS callback; this one bounds the wait of the **write thread**, which makes
/// no one wait. Exceeding it returns no `HRESULT`: it leaves
/// the entry IN THE JOURNAL and names it.
pub const DELAI_ECRIRE: Duration = Duration::from_secs(30);

/// The budget of a MUTATION — a renaming or a deletion.
///
/// ⚠️ **A FOURTH BUDGET, where spec §5.3 sets three, and it is a
/// DECLARED divergence.** *(The comment above already said "the fourth
/// budget has arrived: it is `DELAI_ECRIRE`" — this one is therefore the
/// FIFTH, and the spec's count has aged by two sub-blocks.)*
///
/// ⚠️ **NOT CALIBRATED**, like the four others.
///
/// **Why it is neither a read's nor a write's**: a
/// mutation carries **no byte** — it is a single round trip —, but
/// its copy fallback is O(size) AND O(number of entries) on the
/// browser side, on a directory that must be recreated leaf by leaf. A
/// read's budget (5 s) would kill the renaming of a deep directory;
/// a write's (30 s) would freeze Explorer for half a minute on a
/// mere refused `ren`.
///
/// 🔴 **THIS ONE PROTECTS SOMEONE, unlike [`DELAI_ECRIRE`].** A
/// mutation arises from a POST notification — the application has already returned
/// control —, **but the `PRE_` preceding it is SYNCHRONOUS**: Explorer
/// waits there. The budget therefore does bound an application's wait, like the
/// three of F1 and unlike the write's.
pub const DELAI_MUTATION: Duration = Duration::from_secs(15);

/// What a command in flight waits for, and as what the response will have to be
/// interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attendue {
    Attributs {
        chemin: String,
    },
    Lire {
        chemin: String,
        position: u64,
        longueur: u32,
    },
    /// A write chunk pushed to the browser.
    ///
    /// ⚠️ **`dernier` is kept here because it is what decides what
    /// the `Fait` means**: on the last chunk, it means "the file is
    /// committed, the entry can leave the journal"; on the others, only
    /// "request the next one". Rereading it from the emitted header would be rereading it
    /// from a source the peer could have distorted.
    Ecrire {
        chemin: String,
        dernier: bool,
    },
    /// An entry creation pushed to the browser.
    Creer {
        chemin: String,
    },
    /// A **mutation** pushed to the browser (F3).
    ///
    /// ⚠️ **`chemin` is the SOURCE**, the one on which writes can
    /// be due.
    ///
    /// ❌ *These lines said: "the destination of a renaming lives in
    /// the emitted header, not here: the table does not need to know it to pair
    /// a response". The premise stays true — **pairing** still does not
    /// need it —, but the conclusion no longer holds: **F5 makes it
    /// carry it**, for what happens AFTER the pairing.*
    Muter {
        chemin: String,
        /// **F5** — the destination of a renaming, `None` for a deletion.
        ///
        /// 🔴 **It is NOT there to pair, but to INVALIDATE**, and it
        /// closes a real window. A ProjFS notification invalidates both
        /// parents as soon as the VM renames; but **the browser has not
        /// renamed yet** — a listing of the destination parent during that
        /// interval would memorise, *legitimately*, a content that does not carry
        /// the new name yet. It is AFTER the `Fait` that this memory becomes
        /// wrong, and it is therefore at the `Fait` that it must be forgotten a second
        /// time. Without this field, that stale content would be served until the end
        /// of `TTL_ENUMERATION`.
        destination: Option<String>,
        /// `true` for a renaming, `false` for a deletion. **What depends on it
        /// is the LOG**, never the pairing — but a log that
        /// would not say which of the two verbs failed would send the reader
        /// back to the source code.
        renommage: bool,
    },
    Lister {
        chemin: String,
        /// ⚠️ **The callback's enumeration GUID, NOT the path** (spec §7.2).
        /// Two applications listing the same directory at the same time
        /// open two distinct sessions on the same path: indexing by
        /// path would make the second overwrite the first, and one of the
        /// two would receive an empty directory.
        enumeration: [u8; 16],
    },
}

#[derive(Debug)]
struct EnVol {
    /// The ProjFS command to complete, **if there is one**.
    ///
    /// 🔴 **`None` FOR A WRITE, and it is not a degenerate case: it is
    /// the nature of write-back.** A write completes NO callback — it
    /// arises from a POST notification, which has already returned control to the application.
    /// There is therefore nothing to complete, and calling `PrjCompleteCommand(0)` on
    /// a nonexistent command would be a system call on an identifier
    /// belonging to someone else.
    command_id: Option<i32>,
    quoi: Attendue,
    echeance: Instant,
    /// L'instant d'inscription.
    ///
    /// 🔴 **It is what makes F1's legacy no. 4 DIAGNOSABLE**, and nothing
    /// else would: F1 measured reads that STALL without ever
    /// expiring — the expired-command count stays at 0 for 540 s — and declares that we
    /// do not know WHERE the blockage happens, "for lack of a trace at
    /// table registration". The deadline alone is not enough: it says
    /// when the command will die, never how long it has been waiting.
    inscrite_a: Instant,
}

/// The commands in flight, indexed by correlation.
#[derive(Debug, Default)]
pub struct Table {
    en_vol: HashMap<u32, EnVol>,
    /// Next correlation to hand out. Strictly increasing, and **steps over**
    /// any value still in flight at wraparound time.
    prochaine: u32,
}

impl Table {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// Injectable counter start, **for tests only**.
    ///
    /// ⚠️ Without this seam, the `u32` wraparound test would be
    /// **vacuous**: reaching it honestly would require four billion
    /// registrations, and a test that cannot be run is a test that
    /// does not exist. It is the pattern D10 caught four times.
    #[cfg(test)]
    pub fn nouvelle_depuis(prochaine: u32) -> Self {
        Self {
            prochaine,
            ..Self::default()
        }
    }

    /// Registers a ProjFS command and returns its correlation.
    pub fn inscrire(&mut self, command_id: i32, quoi: Attendue, echeance: Instant) -> u32 {
        self.inscrire_interne(Some(command_id), quoi, echeance)
    }

    /// Registers an operation that completes **no** ProjFS callback — a
    /// write — and returns its correlation.
    ///
    /// 🔴 **WHY THE SAME TABLE, AND NOT A SECOND SOURCE OF CORRELATIONS.**
    /// The channel is unique, and the correlation is a monotonic `u32` with
    /// a search for a free one. Two independent counters on the same channel would
    /// collide, and **the collision would be SILENT**: a response
    /// applied to the wrong command. It is exactly the defect
    /// [`Table::corrélation_libre`] already documents against wraparound —
    /// getting the correlation elsewhere would replay it through the back door.
    pub fn inscrire_sans_commande(&mut self, quoi: Attendue, echeance: Instant) -> u32 {
        self.inscrire_interne(None, quoi, echeance)
    }

    fn inscrire_interne(
        &mut self,
        command_id: Option<i32>,
        quoi: Attendue,
        echeance: Instant,
    ) -> u32 {
        let correlation = self.corrélation_libre();
        // ⚠️ **`echeance` is already computed by the caller from ITS clock**,
        // and the registration instant is taken here: both come from the
        // same `Instant::now()` within a few microseconds, and the module
        // stays pure — it does not read the time to DECIDE, only to
        // TIMESTAMP what it keeps. Passing it as a parameter would make a
        // third argument all callers would set to the same
        // value.
        let inscrite_a = Instant::now();
        self.en_vol.insert(
            correlation,
            EnVol {
                command_id,
                quoi,
                echeance,
                inscrite_a,
            },
        );
        correlation
    }

    /// The next correlation that collides with no command in flight.
    ///
    /// The counter wraps after `u32::MAX`: without this search, the
    /// wrapped correlation would overwrite a command still in flight, and its
    /// response would be applied to the wrong one. The loop terminates because
    /// the table is bounded by memory, hence far below 2^32 entries.
    fn corrélation_libre(&mut self) -> u32 {
        loop {
            let candidate = self.prochaine;
            self.prochaine = self.prochaine.wrapping_add(1);
            if !self.en_vol.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// Returns the command of a correlation, or `None` if it was cancelled,
    /// expired, or never existed — the late response is then **thrown away**.
    ///
    /// **The third term is the command's AGE**: the time elapsed between
    /// its registration and this instant, that is, **the
    /// bridge → browser → bridge traversal**. It is what [`crate::pont::latence`]
    /// observes, and it is all the bridge can measure — neither the entry into the
    /// callback, nor the sweep, nor `PrjCompleteCommand` are in it.
    ///
    /// ⚠️ **`maintenant` is a PARAMETER**, as everywhere in this module: the
    /// time is never read there, which makes the age testable without sleeping. It is
    /// the same discipline as [`Table::plus_ancienne`] and
    /// [`Table::expirees`].
    ///
    /// ⚠️ **An EXPIRED command does not go through here**: `expirees` removes it
    /// itself. The returned age is therefore that of a traversal that **completed**,
    /// and never that of a failure — the two are read on two distinct census
    /// lines, never one for the other.
    pub fn resoudre(
        &mut self,
        correlation: u32,
        maintenant: Instant,
    ) -> Option<(Option<i32>, Attendue, Duration)> {
        self.en_vol.remove(&correlation).map(|e| {
            (
                e.command_id,
                e.quoi,
                maintenant.saturating_duration_since(e.inscrite_a),
            )
        })
    }

    /// Cancels the ProjFS command `command_id`, and returns **ALL** its
    /// correlations.
    ///
    /// 🔴 **ALL, AND IT IS F3'S READ WINDOW THAT REQUIRES IT.** Until
    /// F2, a command had only ONE correlation in flight — chunk *n+1*
    /// being requested only on receipt of *n* —, and this function returned
    /// only one. Since F3, a read can have up to
    /// `pont::lecture::MORCEAUX_EN_VOL`.
    ///
    /// **What a version returning only one would produce:** the *N−1*
    /// others would stay in flight, would expire at the budget, and
    /// `service::balayer` would then call `PrjCompleteCommand` on an
    /// **ALREADY COMPLETED** command — that is, a system call on an
    /// identifier that now belongs to someone else. *The worst of
    /// failure modes: mute, deferred, and outside our process.*
    ///
    /// The order of returned correlations is **deterministic**: a `HashMap` has
    /// none, and a caller logging them would produce a
    /// different order at each run.
    pub fn annuler(&mut self, command_id: i32) -> Vec<u32> {
        let mut correlations: Vec<u32> = self
            .en_vol
            .iter()
            .filter(|(_, e)| e.command_id == Some(command_id))
            .map(|(c, _)| *c)
            .collect();
        correlations.sort_unstable();
        for c in &correlations {
            self.en_vol.remove(c);
        }
        correlations
    }

    /// Removes and returns everything expired at `maintenant`.
    ///
    /// The deadline is **reached**, not exceeded: a command whose
    /// deadline is exactly `maintenant` is expired. The opposite would make
    /// expiry depend on the clock's granularity.
    pub fn expirees(&mut self, maintenant: Instant) -> Vec<(Option<i32>, u32)> {
        let echues: Vec<u32> = self
            .en_vol
            .iter()
            .filter(|(_, e)| e.echeance <= maintenant)
            .map(|(c, _)| *c)
            .collect();
        echues
            .into_iter()
            .map(|c| {
                (
                    self.en_vol
                        .remove(&c)
                        .expect("relevée à l'instant")
                        .command_id,
                    c,
                )
            })
            .collect()
    }

    /// Removes and returns EVERYTHING. Called **before** `PrjStopVirtualizing`: a
    /// command left in flight would wait there for a response nothing can
    /// deliver any more, and ProjFS would wait for its completion indefinitely.
    pub fn vider(&mut self) -> Vec<(Option<i32>, u32)> {
        let mut tout: Vec<(Option<i32>, u32)> = self
            .en_vol
            .drain()
            .map(|(c, e)| (e.command_id, c))
            .collect();
        // Deterministic order: a `HashMap` has none, and a caller that
        // logged this list would produce a different order at each
        // run.
        tout.sort_unstable_by_key(|(_, c)| *c);
        tout
    }

    pub fn en_vol(&self) -> usize {
        self.en_vol.len()
    }

    /// How long the OLDEST command in flight has been waiting.
    ///
    /// 🔴 **IT IS WHAT DECIDES BETWEEN THE FOUR HYPOTHESES OF F1'S LEGACY NO. 4**,
    /// and none was decidable until now:
    ///
    /// | What the census shows | What it says about the blockage |
    /// | --- | --- |
    /// | `en vol=0` while the application is frozen | **nothing was ever registered**: the blockage is in the callback, or before it |
    /// | `en vol=N` and this duration growing **beyond the budget** | the table no longer sweeps: the bridge thread has left its loop |
    /// | `en vol=N` and this duration bounded by the budget | registration and expiry work: the blockage is elsewhere |
    /// | no census line any more | **the bridge thread is dead**, which nothing said |
    ///
    /// `None` when nothing is in flight — and it is the first line of the table.
    pub fn plus_ancienne(&self, maintenant: Instant) -> Option<Duration> {
        self.en_vol
            .values()
            .map(|e| maintenant.saturating_duration_since(e.inscrite_a))
            .max()
    }

    /// How many commands in flight have **no** ProjFS callback to complete —
    /// that is, writes and mutations.
    ///
    /// ⚠️ **Distinguishing it from the total is not an affectation**: a
    /// frozen application with `en vol=3` and `sans_commande=3` waits for NOTHING from the
    /// bridge — all three are pushes, and its blockage is elsewhere.
    pub fn sans_commande(&self) -> usize {
        self.en_vol
            .values()
            .filter(|e| e.command_id.is_none())
            .count()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_commandes;
