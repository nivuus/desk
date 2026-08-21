//! L'installation d'un logiciel déposé par l'utilisateur : télécharger,
//! exécuter, et dire ce qui s'est réellement passé.
//!
//! ⚠️ CE MODULE EST DÉCLARÉ SANS `cfg` depuis `apps.rs`, exactement comme
//! `apps` lui-même l'est depuis `main.rs`, et ce sont ses enfants Windows qui
//! portent le leur. C'est ce qui fait exister `verdict`, `fenetre`, `reponse`,
//! `depot`, `cadence` et `telechargement` sur l'hôte Linux, où leurs tests
//! courent — la « Convention de module enfant » de `CLAUDE.md` n'est donc pas
//! mobilisée : aucun module ne franchit ici de frontière `#[cfg(windows)]`.
//!
//! 🔴 **SEUL `execution` PORTE LE `cfg`.** Tout ce qui pouvait en sortir en est
//! sorti — le verdict, la fenêtre de comptage, les chemins, les extensions, la
//! cadence, l'analyse des en-têtes HTTP et le téléchargement lui-même —, et
//! c'est là qu'est la couverture : `execution.rs` n'a, sur l'hôte, aucune
//! épreuve possible hors `cargo check --target x86_64-pc-windows-gnu`.

pub mod cadence;
pub mod depot;
pub mod fenetre;
pub mod partage;
pub mod reponse;
pub mod telechargement;
pub mod verdict;

#[cfg(windows)]
pub mod execution;
#[cfg(windows)]
pub mod fil;
#[cfg(windows)]
pub mod peripherique_audio;
