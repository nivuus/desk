//! Les en-têtes JSON des trames du pont. **PUR** — aucun `cfg`, aucune
//! dépendance à `windows`.
//!
//! # ✅ LA DETTE DÉCLARÉE ICI EST SOLDÉE : les formes vivent dans `proto/`
//!
//! Ce module portait les sept structures d'en-tête et déclarait en toutes
//! lettres que c'était une dette : « **tant que ce n'est pas fait, un champ
//! renommé ici casse le pont sans casser un seul test côté client** ». Le
//! périmètre de la tâche qui les a écrites interdisait de toucher `proto/` ;
//! la tâche 15, qui pouvait, l'a fait plutôt que de reproduire les formes à la
//! main côté TypeScript.
//!
//! Elles sont désormais dans [`proto::fichiers::entetes`], épinglées par
//! `proto/fichiers-vectors.json`, que **les deux** implémentations lisent —
//! `proto/src/fichiers/entetes/tests.rs` et `proto/ts/fichiers-entetes.test.ts`.
//! Un renommage n'a plus qu'un seul côté à casser pour être vu ROUGE.
//!
//! Ce module ne garde donc que ce qui n'est PAS une forme sur le fil : **ce
//! que l'agent FAIT des en-têtes que `proto` définit**, c'est-à-dire la
//! conversion d'époque, qui est une affaire de Windows et n'a pas de jumeau
//! navigateur.
//!
//! ⚠️ **Il ne RÉ-EXPORTE délibérément pas les sept structures.** Un
//! `pub use proto::fichiers::entetes::*` aurait évité de toucher les trois
//! sites d'appel, au prix de deux choses : un avertissement `unused_imports`
//! sur l'hôte, tous les consommateurs étant `#[cfg(windows)]`, et surtout une
//! indirection qui cacherait au lecteur de `projfs/rappels.rs` l'endroit d'où
//! viennent réellement ces formes. Les trois sites écrivent donc
//! `use proto::fichiers::entetes;`, et le disent.

/// Millisecondes depuis l'époque Unix → unités de 100 ns depuis l'époque
/// FILETIME (1ᵉʳ janvier 1601).
///
/// ⚠️ **Deux erreurs classiques, et une seule des deux se voit :**
///
/// - **se tromper d'ÉPOQUE** rend des dates de 1601 dans l'Explorateur — 369
///   ans d'écart, visible, mais seulement si quelqu'un regarde ;
/// - **se tromper de FACTEUR** (10⁷ au lieu de 10⁶, ou l'inverse) rend des
///   dates plausibles et fausses, que personne ne remarquera jamais.
///
/// Les deux sont épinglées par `l_epoque_unix_devient_l_epoque_filetime`.
///
/// Une date antérieure à 1601 est ramenée à **zéro** : un FILETIME négatif est
/// interprété par Windows comme un temps **relatif**, ce qui donnerait à un
/// fichier une date qui n'a rien à voir avec la sienne. Un débordement est
/// saturé pour la même raison — en `release`, il boucle en silence.
///
/// ⚠️ **Elle reste ICI et non dans `proto/`**, à dessein : ce n'est pas une
/// forme sur le fil mais une conversion propre à Windows. Le navigateur n'a
/// jamais de FILETIME à produire ni à lire ; l'y porter donnerait à `proto` un
/// jumeau TypeScript sans appelant.
pub fn filetime_depuis_ms(ms: i64) -> i64 {
    /// Millisecondes entre le 1ᵉʳ janvier 1601 et le 1ᵉʳ janvier 1970.
    const DECALAGE_MS: i64 = 11_644_473_600_000;
    /// Unités de 100 ns dans une milliseconde.
    const PAR_MS: i64 = 10_000;
    ms.saturating_add(DECALAGE_MS).saturating_mul(PAR_MS).max(0)
}

#[cfg(test)]
mod tests;
