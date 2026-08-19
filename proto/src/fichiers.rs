//! Trame binaire du pont fichiers (canal fiable, ordonné).
//!
//! Format : `version: u8 | type: u8 | correlation: u32 | longueur_entete: u32 |
//! entete | charge`, entiers en **petit-boutiste** — la convention d'[`input`]
//! (`proto/src/input.rs`), et celle de `DataView.setUint32(…, true)` côté
//! navigateur.
//!
//! **Pourquoi binaire, et non JSON comme [`control`]** — la raison est un
//! chiffre relevé sur l'ancien pont, pas une préférence : `src/file.js:264`
//! sérialise les octets d'une écriture par `Array.from(buffer.slice(0, length))`,
//! soit ~4 octets transmis par octet utile, et `web/index.js:653-657` fait pire
//! au retour. Un protocole de fichiers qui encode les octets en JSON paie cet
//! ordre de grandeur **sur chaque octet de chaque lecture**. Ici la charge est
//! transportée telle quelle, jamais encodée ; seul l'en-tête, petit et
//! structuré, est du JSON.
//!
//! De [`control`] on reprend la doctrine de version **et sa note** : pas de
//! valeur par défaut sur la version. Une trame trop courte pour porter sa
//! version est **rejetée**, jamais silencieusement complétée.
//!
//! [`input`]: crate::input
//! [`control`]: crate::control

use serde::{Deserialize, Serialize};

/// Version du protocole de fichiers. Incrémenter à tout changement de format.
pub const FICHIERS_VERSION: u8 = 1;

/// Taille maximale de la **charge** d'une trame, en octets.
///
/// ⚠️ **NON CALIBRÉE.** Posée, pas mesurée — c'est le sous-bloc F4 (le banc de
/// latence) qui donnera de quoi la juger. Elle rejoint `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
/// `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO` et `REARMEMENTS_MAX` dans la
/// liste des constantes de ce dépôt qu'aucune mesure n'a jugées.
///
/// ⚠️ **Le nom dit « trame », la valeur borne la CHARGE** — divergence relevée
/// dans le plan, qui nomme la constante `TAILLE_TRAME_MAX` et écrit dans le
/// même souffle le contrôle « la charge maximale de `TAILLE_TRAME_MAX` passe ».
/// C'est la seconde lecture qui est retenue, parce que c'est celle qui rend le
/// module cohérent avec `pont::decoupe`, dont le `max` est bien une taille de
/// charge. Une trame pleine pèse donc `TAILLE_TRAME_MAX + TAILLE_ENTETE_FIXE +
/// la longueur de l'en-tête JSON` : le nom est trompeur, le dire coûte trois
/// lignes, le taire coûterait un dépassement de MTU applicatif un jour.
pub const TAILLE_TRAME_MAX: usize = 64 * 1024;

/// Longueur de la partie fixe d'une trame : version, type, corrélation,
/// longueur d'en-tête.
pub const TAILLE_ENTETE_FIXE: usize = 1 + 1 + 4 + 4;

// Requêtes pont → navigateur (v1).
pub const TYPE_LISTER: u8 = 1;
pub const TYPE_ATTRIBUTS: u8 = 2;
pub const TYPE_LIRE: u8 = 3;
// Réponses navigateur → pont (v1).
pub const TYPE_ENTREES: u8 = 64;
pub const TYPE_META: u8 = 65;
pub const TYPE_DONNEES: u8 = 66;
pub const TYPE_ECHEC: u8 = 127;

/// Cause d'un échec renvoyé par le navigateur.
///
/// ⚠️ La représentation sur le fil est du **kebab-case**, et les variantes à
/// deux mots ou plus sont celles qui se cassent en silence : ce dépôt a laissé
/// passer une variante `battement-recu` verte sur cinquante tests parce que
/// rien n'épinglait ses octets. `un_code_d_echec_a_une_forme_epinglee_sur_le_fil`
/// épingle les sept, littéralement.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CodeEchec {
    Introuvable,
    CheminIntrouvable,
    AccesRefuse,
    ProtegeEnEcriture,
    NonSupporte,
    TropGrand,
    Interne,
}

/// Une trame décodée. Emprunte les octets d'entrée : ni l'en-tête ni la charge
/// ne sont recopiés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trame<'a> {
    pub version: u8,
    pub type_message: u8,
    pub correlation: u32,
    /// JSON UTF-8. **Non validé ici** : le décodeur rend les octets, l'appelant
    /// les analyse. Un en-tête mal formé est une erreur de l'appelant, pas de
    /// la trame.
    pub entete: &'a [u8],
    /// Octets bruts, **jamais encodés**.
    pub charge: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErreurTrame {
    #[error("trame tronquée : {recu} octets reçus, {attendu} attendus")]
    TropCourte { recu: usize, attendu: usize },
    #[error("version de fichiers non supportée : {0}")]
    VersionNonSupportee(u8),
    #[error("en-tête de {longueur} octets annoncé, {disponible} disponibles")]
    EnteteDeborde { longueur: u64, disponible: usize },
    #[error("charge de {recu} octets, maximum {max}")]
    ChargeTropGrande { recu: usize, max: usize },
}

/// Encode une trame. L'en-tête est du JSON UTF-8, la charge des octets bruts.
pub fn encoder(type_message: u8, correlation: u32, entete: &str, charge: &[u8]) -> Vec<u8> {
    let entete = entete.as_bytes();
    let mut out = Vec::with_capacity(TAILLE_ENTETE_FIXE + entete.len() + charge.len());
    out.push(FICHIERS_VERSION);
    out.push(type_message);
    out.extend_from_slice(&correlation.to_le_bytes());
    out.extend_from_slice(&(entete.len() as u32).to_le_bytes());
    out.extend_from_slice(entete);
    out.extend_from_slice(charge);
    out
}

/// Décode une trame, ou dit précisément pourquoi elle est refusée.
pub fn decoder(octets: &[u8]) -> Result<Trame<'_>, ErreurTrame> {
    if octets.len() < TAILLE_ENTETE_FIXE {
        return Err(ErreurTrame::TropCourte {
            recu: octets.len(),
            attendu: TAILLE_ENTETE_FIXE,
        });
    }
    let version = octets[0];
    if version != FICHIERS_VERSION {
        return Err(ErreurTrame::VersionNonSupportee(version));
    }
    let correlation = u32::from_le_bytes([octets[2], octets[3], octets[4], octets[5]]);
    let longueur_entete = u32::from_le_bytes([octets[6], octets[7], octets[8], octets[9]]);
    let reste = &octets[TAILLE_ENTETE_FIXE..];
    // Comparaison en `u64` : `longueur_entete as usize` déborderait sur une
    // cible 32 bits, et rendrait cette borne inopérante là où elle est le plus
    // nécessaire.
    if u64::from(longueur_entete) > reste.len() as u64 {
        return Err(ErreurTrame::EnteteDeborde {
            longueur: u64::from(longueur_entete),
            disponible: reste.len(),
        });
    }
    let (entete, charge) = reste.split_at(longueur_entete as usize);
    if charge.len() > TAILLE_TRAME_MAX {
        return Err(ErreurTrame::ChargeTropGrande {
            recu: charge.len(),
            max: TAILLE_TRAME_MAX,
        });
    }
    Ok(Trame { version, type_message: octets[1], correlation, entete, charge })
}

#[cfg(test)]
mod tests;
