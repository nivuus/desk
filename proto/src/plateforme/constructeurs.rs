//! Les constructeurs des deux enums de message.
//!
//! 🔴 EXTRAITS PARCE QUE CE FICHIER A FRANCHI 500 LIGNES — 528 —, ET C'EST LA
//! DEUXIÈME FOIS DANS CE SOUS-BLOC QUE CE FICHIER LE FRANCHIT. G3 lui avait
//! rendu 104 lignes AVANT toute addition (`champs.rs`, `motifs.rs`, 468 → 364) ;
//! les cinq variantes en ont repris 92, et la revue transverse 72 de plus.
//!
//! ⚠️ **C'EST LA LEÇON QUE CE DÉPÔT ÉCRIT DEPUIS D6, PAYÉE UNE FOIS DE PLUS :
//! la marge regagnée par une extraction se reperd si on la traite comme
//! acquise.** Elle avait été jouée d'avance, correctement, et elle n'a pas
//! suffi — parce qu'une revue transverse est elle aussi une source de
//! croissance, ce que S2 et S3 ont tous deux mesuré.
//!
//! **Le franchissement est DÉCLARÉ, et rattrapé par une EXTRACTION, jamais par
//! une compression** — ce qui aurait ici voulu dire raccourcir les réfutations
//! que la revue venait d'écrire.
//!
//! ⚠️ Ce n'est PAS la « Convention de module enfant » de `CLAUDE.md`, qui vise
//! les modules extraits d'un parent `#[cfg(windows)]` : c'est le même mécanisme
//! employé pour l'autre raison — la règle des 500 lignes.
//!
//! 🔴 UN `impl` INHÉRENT PEUT VIVRE DANS N'IMPORTE QUEL MODULE DU MÊME CRATE,
//! et c'est ce qui rend cette extraction PUREMENT DE FORME : aucun appelant, ni
//! ici ni dans `agent/` ou `plateforme/`, n'a eu à bouger d'un caractère.

use super::{
    Application, DepuisLaPlateforme, Issue, IssueLancement, MotifCanal, Phase,
    VersLaPlateforme, PLATEFORME_VERSION,
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

    pub fn catalogue(complet: bool, applications: Vec<Application>, disparues: Vec<String>) -> Self {
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

    /// ⚠️ PREND LA VARIANTE TYPÉE, ET NON UN MOT : c'est ce qui garantit que la
    /// plateforme ne peut pas mettre sur le fil un motif que sa propre table
    /// ne connaît pas. La tolérance de la clause 2 est une tolérance de
    /// LECTURE ; en écriture, rien n'est libre.
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

    /// ⚠️ L'APPELANT DOIT VÉRIFIER QUE `empreintes` N'EST PAS VIDE avant
    /// d'émettre : ce constructeur ne le fait pas pour lui, parce qu'il ne
    /// saurait pas quoi rendre à la place. La règle vit du côté qui décide —
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
        taille: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self::Installer {
            version: PLATEFORME_VERSION,
            installation: installation.into(),
            url: url.into(),
            nom: nom.into(),
            taille,
            sha256: sha256.into(),
        }
    }
}
