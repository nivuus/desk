//! `LinkQuality` et `LinkAdaptation` — les deux vocabulaires de l'état du lien,
//! extraits de `control.rs`.
//!
//! ⚠️ **EXTRACTION, pas un remaniement.** La revue transverse du bloc E3 a porté
//! `control.rs` à **496** lignes, soit une marge de **4** — c'est le piège que ce
//! dépôt paie depuis S2 : *documenter une extraction reprend une part de la
//! marge qu'elle rend*, et la ronde qui dénonce la dérive la produit. La
//! doctrine est d'extraire, jamais de comprimer, et surtout jamais de
//! raccourcir une réfutation pour atteindre un compte de lignes.
//!
//! ⚠️ **Le contenu est VERBATIM.** Seul le `use serde::…` est répété — un module
//! enfant ne voit pas les imports de son parent —, et les deux types restent
//! `pub`, ré-exportés par `control.rs` : aucun site d'appel de
//! `proto::control::LinkQuality` ni de `proto::control::LinkAdaptation` n'a
//! bougé.

use serde::{Deserialize, Serialize};

/// Ce que l'utilisateur doit comprendre de l'état du lien.
///
/// Trois valeurs et non un booléen : « dégradé » et « insuffisant » sont deux
/// situations distinctes, et la seconde ne se déduit pas de la première par
/// une négation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkQuality {
    /// Pleine résolution, lien confortable.
    Bonne,
    /// Résolution réduite pour tenir le lien.
    Degradee,
    /// Plancher atteint : le lien ne permet plus le jeu nerveux. C'est
    /// l'avertissement explicite exigé par le cadrage jeu (§3).
    Insuffisante,
}

/// L'agent reçoit-il de quoi s'asservir ?
///
/// Indépendant de `LinkQuality` : une session sans estimation de bande
/// passante peut très bien tourner en `Bonne` sur un lien large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAdaptation {
    Active,
    /// Aucune estimation ne parvient à l'agent : le débit reste figé au
    /// plafond configuré. À dire, pas à taire.
    Indisponible,
}
