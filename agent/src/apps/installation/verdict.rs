//! The outcome of an installation: what actually happened, and nothing more.
//!
//! 🔴 **THE EXIT CODE IS REPORTED, NEVER INTERPRETED, and that is the
//! decision that governs this whole module.** `msiexec` returns **3010**
//! (`ERROR_SUCCESS_REBOOT_REQUIRED`) for a success that requires a
//! reboot, and many graphical installers return **0** after a
//! cancellation by the user. A verdict drawn from the code would therefore be wrong
//! **in both directions**: it would call a successful installation a failure, and
//! a success an installation nobody performed. It is the doctrine that
//! sub-block D8 paid for on another ground — *judge on the RE-READ, never
//! on the return code* —, and the re-read here is the counting window of
//! [`super::fenetre`], which says what the disk actually gained.
//!
//! 🔴 **THE ONLY ROLE OF THE `Option` IS TO DISTINGUISH "CODE COLLECTED" FROM
//! "CODE LOST".** Its VALUE enters no branch, and a test
//! guards it by setting `Some(3010)` on an installation that did indeed place
//! its shortcuts. The code travels to the hub, which shows it to the operator;
//! it decides nothing here.
//!
//! **Pure, no `cfg`, no clock, no input-output** — like
//! `capteur::plein_ecran` and `apps::reconciliation`: its tests run on
//! the Linux host, where the rest of the installation cannot be tested.

/// Why an installation **never started**.
///
/// ⚠️ **`Refusee` IS AN ADDITION TO THE SPECIFICATION, WHICH COUNTS ONLY
/// THREE, and it is justified**: these five cases are neither a success, nor a
/// "no effect", nor ignorance — **they are refusals, and they know
/// why**. Folding them into [`Issue::IssueInconnue`] would read "we do not
/// know" where we know very well, and would deprive the hub of the only message
/// it can usefully show the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motif {
    /// The SHA-256 re-read after writing is not the one that was announced.
    Empreinte,
    /// `CreateProcessW` returned `ERROR_ELEVATION_REQUIRED` (740). It is the
    /// remedy that makes an elevation READABLE instead of a wait that
    /// nobody understands: the consent box opens on the secure
    /// desktop, out of reach of any capture.
    ElevationRequise,
    /// The extension is neither `.exe` nor `.msi` — `.bat` in particular, whose
    /// interpreter, working directory and execution policy
    /// would call for their own decisions.
    Extension,
    /// 🔴 The installing process is assigned to a **job object**, so
    /// the installer would die there with the agent — in the middle of a registry
    /// write, and the machine would stay half-installed. A loud refusal
    /// is better than an installation a redeployment will kill. It is a
    /// GUARD, not the remedy: the remedy is G1's hand-over no. 1, which does not belong
    /// to this sub-block.
    JobObject,
    /// There is no room to write the installer.
    DisquePlein,
    /// The disk refused a write that is NOT the installer's —
    /// the installer's log, for instance.
    ///
    /// ⚠️ DISTINCT FROM [`Self::DisquePlein`], and it is not a nuance of
    /// style: a full disk is fixed by freeing space, a write
    /// refusal is fixed by looking at permissions. Confusing them would send you
    /// looking for the wrong thing.
    Disque,
    /// `CreateProcessW` failed for a reason OTHER than a required
    /// elevation — corrupted executable, antivirus that quarantined it,
    /// incompatible image.
    ///
    /// ⚠️ IT DOES NOT SAY WHICH, and that is honest: the exact Windows error
    /// is in the log, this reason only says "the installer never
    /// started". Inventing a taxonomy here would pretend to know.
    LancementImpossible,
}

impl Motif {
    /// The five variants, in the order the test walks them.
    ///
    /// 🔴 ANTI-FORGETTING: a variant added without its line here would be missing
    /// from the mapping test, which compares this list to a hand-written table
    /// whose count is hardcoded — the compiler requires the branch in
    /// [`Motif::mot`], and the test requires the entry.
    #[cfg(test)]
    pub const ALL: [Self; 7] = [
        Self::Empreinte,
        Self::ElevationRequise,
        Self::Extension,
        Self::JobObject,
        Self::DisquePlein,
        Self::Disque,
        Self::LancementImpossible,
    ];

    /// The exact word that travels on the wire.
    ///
    /// 🔴 WRITTEN BY HAND, NEVER BY A `rename_all`: sub-block G1
    /// MEASURED that a serialisation convention is **unobservable** on an
    /// enum whose variants all fit in one word — switching `kebab-case`
    /// to `snake_case` on `IssueLancement` left `cargo test -p proto` at
    /// 75 passed. `elevation-requise` closes the gap by itself, and the
    /// table below guards it even if a single-word variant were to
    /// remain alone.
    pub fn mot(self) -> &'static str {
        match self {
            Self::Empreinte => "empreinte",
            Self::ElevationRequise => "elevation-requise",
            Self::Extension => "extension",
            Self::JobObject => "job-object",
            Self::DisquePlein => "disque-plein",
            Self::Disque => "disque",
            Self::LancementImpossible => "lancement-impossible",
        }
    }

    /// The reason this word designates, or `None` if we do not know it.
    ///
    /// 🔴 `None` IS NOT AN ERROR: it is a reason from a version that is ahead of
    /// us, and the caller must log it **verbatim** rather than
    /// lose it or replace it with a default.
    #[cfg(test)]
    pub fn depuis_mot(mot: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|candidat| candidat.mot() == mot)
    }
}

/// What the agent declares to the platform when an installation ends.
///
/// ⚠️ **LOCAL TYPE, WAITING FOR ITS PROTOCOL TWIN.** The wire
/// vocabulary lives in `proto::plateforme`; as long as it is not there, this module does not
/// import it and the caller will do the conversion. The names are the same on
/// both sides, on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// ⚠️ `IssueInconnue` STARTS WITH THE NAME OF ITS ENUM, AND CLIPPY FLAGS IT.
// It is DELIBERATE and it is not fixed: sub-block G1 MEASURED that a
// `rename_all` is **unobservable** on an enum whose variants all
// fit in one word — switching `kebab-case` to `snake_case` on `IssueLancement`
// leaves `cargo test -p proto` entirely green. `SansEffet` and `IssueInconnue`
// are the two two-word variants that close that gap, and the test
// `les_deux_variantes_de_deux_mots_d_issue_voyagent_en_kebab_case` depends on them.
// Renaming it would reopen G1's hand-over no. 9 to satisfy a style lint.
#[allow(clippy::enum_variant_names)]
pub enum Issue {
    /// The counting window saw **at least one** application appear.
    Reussie,
    /// The window closed at **zero**: the installer ran, the
    /// catalogue did not move. That is the case of a cancellation.
    SansEffet,
    /// The exit code could not be collected, or the installation
    /// timed out. **We do not know**, and saying so is the only honest statement.
    IssueInconnue,
    /// The installation **never started**, and we know why.
    Refusee,
}

impl Issue {
    /// The outcome, in the priority order the specification sets.
    ///
    /// **The order is not indifferent, and each step has its reason:**
    ///
    /// 1. **`Refusee` wins over everything.** Nothing was launched, so `apparues`
    ///    only counts what OTHER reconciliations found — a
    ///    concurrent installation, or an application placed by hand during
    ///    the window. Drawing a success from it would **invent** an effect for a
    ///    process that did not exist.
    /// 2. **`IssueInconnue` wins next**, over a missing code as well as
    ///    over a timeout. A timed-out installer may be **still
    ///    working**: its appearances are then partial, and announcing
    ///    `Reussie` would amount to declaring finished what nobody saw
    ///    finish. ⚠️ This step therefore costs a few false `IssueInconnue`
    ///    on installations that actually completed after the timeout —
    ///    **that is accepted**: "I do not know" is corrected by a second
    ///    read of the catalogue, "it succeeded" is not.
    /// 3. **`Reussie` if the window counted something**, `SansEffet`
    ///    otherwise.
    ///
    /// ⚠️ `code_sortie` is READ only through `is_none()`. If one day a branch
    /// starts comparing its value, the decision at the head of the module has
    /// been lost.
    /// The PROTOCOL variant this outcome designates.
    ///
    /// 🔴 TWO TYPES RATHER THAN ONE, AND IT IS DELIBERATE. `proto::plateforme::Issue`
    /// is a **wire shape**: it carries its `rename_all`, its `Serialize`
    /// and its version compatibility. This one is a **decision**, and it
    /// is tested on the host without knowing anything about serde. Merging them would bring
    /// a serialisation constraint into a module whose whole point is
    /// to have none — and the day the wire changed words, the
    /// rule would change with it.
    ///
    /// ⚠️ THE `match` IS EXHAUSTIVE: a variant added on one side does not compile
    /// until it has its counterpart. It is the only thing that keeps the
    /// two types aligned, and it is free.
    pub fn vers_protocole(self) -> proto::plateforme::Issue {
        match self {
            Self::Reussie => proto::plateforme::Issue::Reussie,
            Self::SansEffet => proto::plateforme::Issue::SansEffet,
            Self::IssueInconnue => proto::plateforme::Issue::IssueInconnue,
            Self::Refusee => proto::plateforme::Issue::Refusee,
        }
    }

    pub fn depuis(
        code_sortie: Option<i32>,
        apparues: usize,
        expire: bool,
        refus: Option<Motif>,
    ) -> Self {
        if refus.is_some() {
            return Self::Refusee;
        }
        if code_sortie.is_none() || expire {
            return Self::IssueInconnue;
        }
        if apparues > 0 {
            Self::Reussie
        } else {
            Self::SansEffet
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 THE WITNESS OF THE DECISION AT THE HEAD OF THE MODULE. `msiexec` returns 3010 for
    /// a success that requires a reboot: a "non-zero code ⇒ failure"
    /// interpretation would return something other than `Reussie` here, on an
    /// installation that placed two shortcuts before our eyes.
    #[test]
    fn un_code_de_sortie_non_nul_ne_defait_pas_une_fenetre_qui_a_compte() {
        assert_eq!(Issue::depuis(Some(3010), 2, false, None), Issue::Reussie);
        // The same, so that the first does not pass by chance: none of
        // these codes must weigh, whatever its sign or magnitude.
        for code in [1_i32, 1603, 259, -1, i32::MIN, i32::MAX] {
            assert_eq!(
                Issue::depuis(Some(code), 1, false, None),
                Issue::Reussie,
                "le code {code} a été interprété"
            );
        }
    }

    /// 🔴 THE OTHER DIRECTION OF THE SAME ERROR, and it is the witness of spec ⑥:
    /// a cancelled graphical installer exits with **0** without having placed anything.
    #[test]
    fn un_installeur_annule_sort_en_zero_et_reste_sans_effet() {
        assert_eq!(Issue::depuis(Some(0), 0, false, None), Issue::SansEffet);
    }

    #[test]
    fn un_refus_l_emporte_sur_tout_le_reste() {
        // Including on a full window and a success code: what
        // appeared during the window comes from elsewhere, since nothing started.
        assert_eq!(
            Issue::depuis(Some(0), 7, false, Some(Motif::ElevationRequise)),
            Issue::Refusee
        );
        assert_eq!(
            Issue::depuis(None, 0, true, Some(Motif::JobObject)),
            Issue::Refusee
        );
    }

    #[test]
    fn l_ignorance_l_emporte_sur_une_fenetre_pleine() {
        // Code lost: the agent died before collecting it.
        assert_eq!(Issue::depuis(None, 3, false, None), Issue::IssueInconnue);
        // Timed out: the installer may still be working, its appearances are
        // perhaps partial.
        assert_eq!(Issue::depuis(Some(0), 3, true, None), Issue::IssueInconnue);
        assert_eq!(Issue::depuis(None, 0, true, None), Issue::IssueInconnue);
    }

    #[test]
    fn le_cas_nominal_ne_tient_qu_a_la_fenetre() {
        assert_eq!(Issue::depuis(Some(0), 1, false, None), Issue::Reussie);
        assert_eq!(Issue::depuis(Some(0), 0, false, None), Issue::SansEffet);
    }

    /// The table of reasons, walked in BOTH directions — a `rename_all`
    /// would not have let it turn red.
    #[test]
    fn la_table_des_motifs_fait_l_aller_retour_sur_les_sept() {
        // ⚠️ THIS TEST HAS ALREADY SERVED, AND THAT IS ITS REASON TO BE. Writing
        // `installation/execution.rs` needed two reasons this
        // table did not have — `disque` and `lancement-impossible` —, and the
        // hardcoded count demanded them HERE before letting the code compile. A
        // `ALL` derived from an exhaustive `match` would not have done so: the
        // compiler would have accepted the variant, and only the word would have stayed
        // missing from the wire table.
        let attendus = [
            (Motif::Empreinte, "empreinte"),
            (Motif::ElevationRequise, "elevation-requise"),
            (Motif::Extension, "extension"),
            (Motif::JobObject, "job-object"),
            (Motif::DisquePlein, "disque-plein"),
            (Motif::Disque, "disque"),
            (Motif::LancementImpossible, "lancement-impossible"),
        ];
        // 🔴 ANTI-FORGETTING: `ALL` must cover exactly the enumeration
        // above. A variant added without its line here skews this count.
        assert_eq!(Motif::ALL.len(), attendus.len());
        for (motif, mot) in attendus {
            assert!(Motif::ALL.contains(&motif), "{mot} absent de TOUS");
            assert_eq!(motif.mot(), mot);
            assert_eq!(Motif::depuis_mot(mot), Some(motif));
        }
        // An unknown word NEVER becomes a default reason.
        assert_eq!(Motif::depuis_mot("quota-depasse"), None);
        assert_eq!(Motif::depuis_mot(""), None);
        assert_eq!(Motif::depuis_mot("Empreinte"), None);
        assert_eq!(Motif::depuis_mot("elevation_requise"), None);
        // 🔴 `disque` AND `disque-plein` SHARE A PREFIX, and it is
        // exactly the case where a `starts_with` match would return the
        // wrong reason. G1 measured that too broad a prefix left
        // SEVENTEEN tests green; this one is tiny, and it checks that the
        // match is EXACT in both directions.
        assert_eq!(Motif::depuis_mot("disque"), Some(Motif::Disque));
        assert_eq!(Motif::depuis_mot("disque-plein"), Some(Motif::DisquePlein));
        assert_eq!(Motif::depuis_mot("disque-pleine"), None);
    }
}
