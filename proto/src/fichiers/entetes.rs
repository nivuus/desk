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

/// L'en-tête de `TYPE_ECRIRE`. **La charge porte les octets**, jamais encodés.
///
/// ⚠️ **`premier` et `dernier` ne sont PAS déductibles de `position` et
/// `longueur`.** Un fichier d'un seul morceau les porte tous deux à `true` ;
/// un fichier de taille NULLE n'a aucun morceau du tout et passe par
/// [`Creer`]. Surtout, `position == 0` ne suffit pas à dire « premier » le jour
/// où une écriture partielle existera : c'est le drapeau qui décide, et lui
/// seul, parce que c'est lui qui commande l'ouverture du flux **sans**
/// `keepExistingData`.
///
/// 🔵 **`dernier` EST LA COMMITTAISON.** `createWritable()` du navigateur écrit
/// dans un fichier d'échange et ne commet qu'au `close()` : c'est le morceau
/// `dernier` qui déclenche ce `close()`, et donc le seul instant où le fichier
/// du poste local change. Une poussée interrompue avant lui laisse le fichier
/// local **inchangé** — pas à moitié écrit. ⚠️ *Inférence de la spécification
/// de la File System Access API, non mesurée par ce sous-bloc.*
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Ecrire {
    pub chemin: String,
    pub position: u64,
    pub longueur: u32,
    /// Premier morceau : le flux s'ouvre **sans** `keepExistingData`.
    pub premier: bool,
    /// Dernier morceau : le flux se ferme, et **c'est la committaison**.
    pub dernier: bool,
}

/// L'en-tête de `TYPE_CREER`. Charge binaire **vide**.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Creer {
    pub chemin: String,
    pub repertoire: bool,
}

/// L'en-tête de `TYPE_RENOMMER`. Charge binaire **vide**.
///
/// 🔴 **L'ORDRE DES DEUX CHAMPS EST LE SENS DE L'OPÉRATION, et s'y tromper
/// DÉTRUIT.** `de` est la source, `vers` la destination — c'est-à-dire, côté
/// ProjFS, `FilePathName` puis `destinationFileName`. Inverser les deux ne
/// produirait aucune erreur : le renommage aurait lieu, à l'envers, et le
/// fichier de destination écraserait la source. C'est le risque R-F3-1 du plan,
/// et il porte **deux** parades qui ne dépendent pas l'une de l'autre :
///
/// 1. la sonde S1 relève sur pièces quel champ ProjFS porte quoi, **avant**
///    toute recette ;
/// 2. le pont **refuse de pousser** un renommage dont `vers` est vide ou égal à
///    `de`, avec un `warn!` qui nomme les deux champs bruts. Celle-ci ne dépend
///    d'aucune mesure.
///
/// ⚠️ **`repertoire` est TRANSPORTÉ plutôt que redécouvert.** C'est
/// l'`isdirectory` que le rappel de notification reçoit du système ; le
/// navigateur le redemanderait au prix d'un aller-retour, et se tromperait sur
/// une entrée disparue entre-temps. Il décide de deux choses : la récursion du
/// repli de copie, et la façon dont l'écriture due d'un ENFANT retarde le
/// renommage d'un répertoire (`agent/src/pont/mutation.rs`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Renommer {
    /// La source, telle qu'elle existe aujourd'hui sur le poste local.
    pub de: String,
    /// La destination. **Jamais vide, jamais égale à `de`** — le pont refuse de
    /// pousser autrement.
    pub vers: String,
    pub repertoire: bool,
}

/// L'en-tête de `TYPE_SUPPRIMER`. Charge binaire **vide**.
///
/// ⚠️ **La suppression n'est PAS récursive côté navigateur**, contre la lettre
/// de la spec §3.5 (`dir.removeEntry(nom, { recursive })`). Un geste dans la VM
/// ne doit pas déclencher une destruction récursive sur le disque du poste
/// local, sur la foi d'un miroir qu'aucune preuve ne dit à jour. Le refus
/// remonte alors sous [`super::CodeEchec::RepertoireNonVide`], qui devient de
/// ce fait **diagnostique** au lieu d'être un code jamais produit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Supprimer {
    pub chemin: String,
    pub repertoire: bool,
}

/// Une écriture DUE : des octets qui vivent sur la VM et pas encore sur le
/// poste local.
///
/// 🔴 **C'est la fenêtre de perte, rendue NOMMABLE.** ProjFS ne met jamais le
/// fournisseur sur le chemin de l'écriture : quand nous l'apprenons,
/// l'application a déjà refermé son handle et cru avoir enregistré. Ce que
/// cette structure porte est donc ce que l'utilisateur risque de perdre s'il
/// referme son onglet maintenant — et le nommer est tout ce qu'on peut faire.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub chemin: String,
    pub octets: u64,
}

/// L'en-tête de `TYPE_DUES`. Charge binaire **vide**.
///
/// ⚠️ **C'est une ANNONCE : elle n'attend aucune réponse**, et le navigateur ne
/// doit rien renvoyer. Voir le commentaire de `TYPE_DUES` dans
/// [`crate::fichiers`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Dues {
    pub dues: Vec<Due>,
}

/// L'en-tête de `TYPE_ECHEC`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Echec {
    pub code: super::CodeEchec,
}

// ⚠️ **`TYPE_FAIT` n'a PAS d'en-tête propre : sa trame porte `{}`.** Lui donner
// une structure vide ferait une forme à épingler qui n'épingle rien, et un
// vecteur partagé qui ne peut pas casser. Ce qui identifie l'écriture
// acquittée est la CORRÉLATION de la trame, pas son en-tête.

#[cfg(test)]
mod tests;
