//! Les en-têtes JSON des trames du pont fichiers. **PUR** — aucun `cfg`,
//! aucune dépendance à `windows`, formes sur le fil **épinglées par des
//! vecteurs partagés**.
//!
//! # Pourquoi ce module vit ICI et non dans l'agent
//!
//! Il y a vécu, le temps d'un commit : `agent/src/pont/entetes.rs` portait ces
//! sept structures et déclarait lui-même que c'était une dette, parce que le
//! périmètre de la tâche qui les a écrites interdisait de toucher `proto/`.
//! Sa formulation était exacte, et c'est elle qui a décidé du déplacement :
//! **« un champ renommé ici casse le pont sans casser un seul test côté
//! client »**.
//!
//! Ce dépôt connaît ce patron par cœur, et l'a payé deux fois : `TYPES_AGENT`
//! (`proto/ts/control.ts`) est une liste écrite à la main que rien ne confronte
//! à l'union qu'elle reflète, et une variante `battement-recu` est restée verte
//! sur cinquante tests parce que rien n'épinglait ses octets. La spec §4.2
//! nomme d'ailleurs `proto/` comme « source de vérité unique » : la dette était
//! une divergence de périmètre, pas une décision de conception.
//!
//! `proto` porte donc désormais les deux couches : la **trame** (version, type,
//! corrélation, longueur d'en-tête) et les **en-têtes** qu'elle transporte.
//!
//! # Le contrôle qui rattrape un renommage
//!
//! `proto/fichiers-vectors.json` fige la chaîne JSON EXACTE de chaque forme, et
//! il est lu par **les deux** implémentations :
//!
//! - `proto/src/fichiers/entetes/tests.rs` (Rust) ;
//! - `proto/ts/fichiers-entetes.test.ts` (TypeScript).
//!
//! Renommer un champ d'un seul côté rend ce côté-là ROUGE — mesuré, voir le
//! rapport de la tâche 15. Les deux tests vérifient en outre la clé `version`
//! du fichier, ce que `vectors.json` ne fait que côté TypeScript.
//!
//! **Aucun `#[serde(default)]` nulle part** : un en-tête incomplet est rejeté,
//! jamais silencieusement complété — la doctrine de version de
//! [`crate::control`], appliquée aux en-têtes.
//!
//! ⚠️ **Le découpage est le MÊME des deux côtés** : la trame dans
//! `fichiers.{rs,ts}`, les en-têtes dans `fichiers/entetes.rs` et
//! `fichiers-entetes.ts`. Une asymétrie de découpage rendrait le jumelage plus
//! difficile à relire qu'à écrire.

use serde::{Deserialize, Serialize};

/// L'en-tête de `TYPE_LISTER` et de `TYPE_ATTRIBUTS`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Chemin {
    /// Chemin logique, composants séparés par `/`, **normalisé** côté agent par
    /// `pont::chemins`. Vide = la racine.
    pub chemin: String,
}

/// L'en-tête de `TYPE_LIRE`. La plage est **un morceau**, jamais le fichier
/// entier : c'est `pont::decoupe` qui la produit.
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
    pub code: super::CodeEchec,
}

#[cfg(test)]
mod tests;
