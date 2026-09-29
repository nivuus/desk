//! The builders of the two message enums.
//!
//! 🔴 EXTRACTED BECAUSE THIS FILE CROSSED 500 LINES — 528 —, AND IT IS THE
//! SECOND TIME IN THIS SUB-BLOCK THAT THIS FILE CROSSES IT. G3 had given it back
//! 104 lines BEFORE any addition (`champs.rs`, `motifs.rs`, 468 → 364);
//! the five variants took back 92 of them, and the cross-cutting review 72 more.
//!
//! ⚠️ **IT IS THE LESSON THIS REPOSITORY HAS BEEN WRITING SINCE D6, PAID ONCE MORE:
//! the margin regained by an extraction is lost again if treated as
//! settled.** It had been carried out in advance, correctly, and it was not
//! enough — because a cross-cutting review is also a source of
//! growth, which S2 and S3 both measured.
//!
//! **The crossing is DECLARED, and caught up through an EXTRACTION, never through
//! a compression** — which here would have meant shortening the rebuttals
//! the review had just written.
//!
//! ⚠️ It is NOT the "Child module convention" of `docs/claude/module-conventions.md`, which targets
//! modules extracted from a `#[cfg(windows)]` parent: it is the same mechanism
//! used for the other reason — the 500-line rule.
//!
//! 🔴 AN INHERENT `impl` CAN LIVE IN ANY MODULE OF THE SAME CRATE,
//! and that is what makes this extraction PURELY FORMAL: no caller, neither
//! here nor in `agent/` or `plateforme/`, had to move by one character.

use super::{
    Application, DepuisLaPlateforme, Issue, IssueLancement, MotifCanal, Phase, VersLaPlateforme,
    PLATEFORME_VERSION,
};

impl VersLaPlateforme {
    pub fn enroler(vm: impl Into<String>, secret: impl Into<String>) -> Self {
        Self::Enroler {
            version: PLATEFORME_VERSION,
            vm: vm.into(),
            secret: secret.into(),
        }
    }

    pub fn battement() -> Self {
        Self::Battement {
            version: PLATEFORME_VERSION,
        }
    }

    pub fn catalogue(
        complet: bool,
        applications: Vec<Application>,
        disparues: Vec<String>,
    ) -> Self {
        Self::Catalogue {
            version: PLATEFORME_VERSION,
            complet,
            applications,
            disparues,
        }
    }

    pub fn lancee(demande: impl Into<String>, issue: IssueLancement) -> Self {
        Self::Lancee {
            version: PLATEFORME_VERSION,
            demande: demande.into(),
            issue,
        }
    }

    pub fn progression(
        installation: impl Into<String>,
        phase: Phase,
        octets_faits: u64,
        octets_total: u64,
        ecoule_ms: u64,
    ) -> Self {
        Self::Progression {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            phase,
            octets_faits,
            octets_total,
            ecoule_ms,
        }
    }

    pub fn termine(
        installation: impl Into<String>,
        issue: Issue,
        motif: Option<String>,
        code_sortie: Option<i32>,
        journal: impl Into<String>,
        journal_tronque: bool,
    ) -> Self {
        Self::Termine {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            issue,
            motif,
            code_sortie,
            journal: journal.into(),
            journal_tronque,
        }
    }
}

impl DepuisLaPlateforme {
    pub fn enrole(prefixe: impl Into<String>, jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::Enrole {
            version: PLATEFORME_VERSION,
            prefixe: prefixe.into(),
            jeton: jeton.into(),
            expire_a,
        }
    }

    pub fn battement_recu(jeton: impl Into<String>, expire_a: i64) -> Self {
        Self::BattementRecu {
            version: PLATEFORME_VERSION,
            jeton: jeton.into(),
            expire_a,
        }
    }

    /// ⚠️ TAKES THE TYPED VARIANT, AND NOT A WORD: that is what guarantees that the
    /// platform cannot put on the wire a reason its own table
    /// does not know. The tolerance of clause 2 is a READ
    /// tolerance; on write, nothing is free.
    pub fn refus(motif: MotifCanal) -> Self {
        Self::Refus {
            version: PLATEFORME_VERSION,
            motif: motif.mot().to_string(),
        }
    }

    pub fn lancer(demande: impl Into<String>, cle: impl Into<String>) -> Self {
        Self::Lancer {
            version: PLATEFORME_VERSION,
            demande: demande.into(),
            cle: cle.into(),
        }
    }

    /// ⚠️ THE CALLER MUST CHECK THAT `empreintes` IS NOT EMPTY before
    /// emitting: this builder does not do it for them, because it would not
    /// know what to return in its place. The rule lives on the side that decides —
    /// `plateforme/src/agents/canal.ts`.
    pub fn icones_manquantes(empreintes: Vec<String>) -> Self {
        Self::IconesManquantes {
            version: PLATEFORME_VERSION,
            empreintes,
        }
    }

    pub fn installer(
        installation: impl Into<String>,
        url: impl Into<String>,
        nom: impl Into<String>,
        size: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self::Installer {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            url: url.into(),
            nom: nom.into(),
            size,
            sha256: sha256.into(),
        }
    }
}
