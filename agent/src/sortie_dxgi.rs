//! Ce que DXGI expose d'une sortie d'affichage, sans rien qui dépende de
//! l'API Windows elle-même.
//!
//! Extrait de `capture.rs` (qui reste `#![cfg(windows)]` dans son ensemble)
//! pour que `superviseur::placement::sortie_par_dimensions`, purement
//! algorithmique, puisse se compiler et se tester sur l'hôte Linux —
//! `crate::capture` n'existe pas du tout en dehors de Windows, donc aucun
//! type qui en dépend ne peut traverser cette frontière. `capture.rs`
//! réexporte ce type (`pub use crate::sortie_dxgi::SortieDxgi;`) pour que
//! `crate::capture::SortieDxgi` reste le même type côté Windows — même
//! précédent que `windows_source_sortie::region_de_sortie`, qui sort déjà la
//! part portable d'un module par ailleurs `#[cfg(windows)]`.

/// Ce qu'on sait d'une sortie DXGI, sans en dupliquer quoi que ce soit.
#[derive(Debug, Clone)]
pub struct SortieDxgi {
    pub index_adaptateur: u32,
    pub index_sortie: u32,
    pub adaptateur: String,
    pub nom_sortie: String,
    pub attachee_au_bureau: bool,
    /// Position et dimensions dans les coordonnées du bureau virtuel.
    pub rect: crate::geometry::Rect,
}
