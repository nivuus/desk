//! Les en-têtes JSON des trames du pont. **PUR** — aucun `cfg`, aucune
//! dépendance à `windows`, formes sur le fil **épinglées** par des tests
//! d'hôte.
//!
//! # ⚠️ CE MODULE DEVRAIT VIVRE DANS `proto/`, ET C'EST UNE DETTE DÉCLARÉE
//!
//! La spec §4.2 nomme `proto/src/fichiers.rs` et `proto/ts/fichiers.ts` comme
//! « schéma versionné partagé (**source de vérité unique**) ». `proto` porte
//! bien la TRAME (version, type, corrélation, longueur d'en-tête), mais laisse
//! l'en-tête en `&[u8]` de JSON non analysé : « le décodeur rend les octets,
//! l'appelant les analyse ». Les formes ci-dessous sont donc, aujourd'hui,
//! **définies d'un seul côté** — et le jumeau TypeScript (tâche 15) devra les
//! reproduire à la main.
//!
//! **C'est une divergence forcée par le périmètre de la tâche 14**, qui
//! interdit de toucher `proto/`, et non une décision de conception. Le remède
//! est nommé : porter ces structures dans `proto/src/fichiers.rs`, et leur
//! jumeau dans `proto/ts/fichiers.ts`, avec les tests d'épinglage des deux
//! côtés. **Tant que ce n'est pas fait, un champ renommé ici casse le pont sans
//! casser un seul test côté client.**
//!
//! # Ce que les tests épinglent, et pourquoi
//!
//! Ce dépôt a laissé passer une variante `battement-recu` verte sur cinquante
//! tests parce que rien n'épinglait ses octets ; `proto::fichiers` épingle déjà
//! ses sept codes d'échec pour cette raison. **Aucun `#[serde(default)]` nulle
//! part** : un en-tête incomplet est rejeté, jamais silencieusement complété —
//! la doctrine de version de `proto::control`, appliquée aux en-têtes.

use serde::{Deserialize, Serialize};

/// L'en-tête de `TYPE_LISTER` et de `TYPE_ATTRIBUTS`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Chemin {
    /// Chemin logique, composants séparés par `/`, **normalisé** par
    /// [`crate::pont::chemins`]. Vide = la racine.
    pub chemin: String,
}

/// L'en-tête de `TYPE_LIRE`. La plage est **un morceau**, jamais le fichier
/// entier : c'est [`crate::pont::decoupe`] qui la produit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Lire {
    pub chemin: String,
    pub position: u64,
    pub longueur: u32,
}

/// Une entrée de répertoire, dans la réponse `TYPE_ENTREES`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EntreeJson {
    pub nom: String,
    pub repertoire: bool,
    pub taille: u64,
    /// `File.lastModified` : millisecondes depuis l'époque Unix, **signé** —
    /// un fichier antérieur à 1970 en rend un négatif, et le refuser ferait
    /// échouer une énumération pour une date.
    pub modifie: i64,
}

/// L'en-tête de `TYPE_ENTREES`. Charge binaire **vide**.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Entrees {
    pub entrees: Vec<EntreeJson>,
}

/// L'en-tête de `TYPE_META`. Charge binaire **vide**.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub repertoire: bool,
    pub taille: u64,
    pub modifie: i64,
}

/// L'en-tête de `TYPE_DONNEES`. **La charge porte les octets**, jamais encodés.
///
/// `longueur` est redondante avec la taille de la charge, **et c'est
/// délibéré** : le décodeur peut ainsi refuser une trame dont l'en-tête et la
/// charge se contredisent, plutôt que d'écrire dans le tampon de ProjFS une
/// quantité d'octets que l'émetteur ne croyait pas envoyer.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Donnees {
    pub position: u64,
    pub longueur: u32,
}

/// L'en-tête de `TYPE_ECHEC`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Echec {
    pub code: proto::fichiers::CodeEchec,
}

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
pub fn filetime_depuis_ms(ms: i64) -> i64 {
    /// Millisecondes entre le 1ᵉʳ janvier 1601 et le 1ᵉʳ janvier 1970.
    const DECALAGE_MS: i64 = 11_644_473_600_000;
    /// Unités de 100 ns dans une milliseconde.
    const PAR_MS: i64 = 10_000;
    ms.saturating_add(DECALAGE_MS).saturating_mul(PAR_MS).max(0)
}

#[cfg(test)]
mod tests;
