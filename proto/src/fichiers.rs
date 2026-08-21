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
///
/// 🔴 **F2 AJOUTE QUATRE TYPES DE MESSAGE ET NE L'INCRÉMENTE PAS, et c'est une
/// décision, pas un oubli.** F1 l'a laissée à 1 « précisément pour que
/// l'arrivée de ces verbes soit une rupture visible » (son legs 13) — mais la
/// rupture est **ADDITIVE** : un pont v1 en lecture seule et un client v1 qui
/// sait écrire s'entendent sans réserve, le client ignorant simplement des
/// types qu'il ne recevra jamais. Incrémenter à 2 casserait la compatibilité
/// dans le seul sens où elle n'a aucune valeur — les deux bouts sont livrés
/// ensemble — et ferait échouer une session en cours pendant une migration.
///
/// ⚠️ **Ce qui l'incrémentera est un changement de FORME, pas d'inventaire** :
/// un champ renommé, un ordre d'octets différent, un en-tête dont le sens
/// change. Ceux-là, un pair d'une autre version ne peut pas les ignorer.
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

// Requêtes pont → navigateur — **elles ATTENDENT une réponse**.
pub const TYPE_LISTER: u8 = 1;
pub const TYPE_ATTRIBUTS: u8 = 2;
pub const TYPE_LIRE: u8 = 3;
pub const TYPE_ECRIRE: u8 = 4; // F2 — en-tête `Ecrire`, la charge porte les octets
pub const TYPE_CREER: u8 = 5; // F2 — en-tête `Creer`, charge vide
pub const TYPE_RENOMMER: u8 = 7; // F3 — en-tête `Renommer`, charge vide
pub const TYPE_SUPPRIMER: u8 = 8; // F3 — en-tête `Supprimer`, charge vide

// Annonces pont → navigateur — **elles n'attendent RIEN**.
//
// 🔴 **TROISIÈME FAMILLE, et elle casse l'invariant que le navigateur énonce
// en majuscules** (`client/src/fichiers/protocole.ts`) : « une requête reçoit
// toujours une réponse ». Une ANNONCE n'en reçoit aucune — aucune entrée de
// table ne lui correspond côté pont, et n'y pas répondre ne laisse donc rien
// en vol. **La liste des annonces est CLOSE**, et c'est ce qui empêche cette
// famille de devenir le bras fourre-tout silencieux que ce dépôt a payé quatre
// fois sur `capteur/pont_media.rs`.
pub const TYPE_DUES: u8 = 6; // F2 — en-tête `Dues`, charge vide

// Réponses navigateur → pont.
pub const TYPE_ENTREES: u8 = 64;
pub const TYPE_META: u8 = 65;
pub const TYPE_DONNEES: u8 = 66;
pub const TYPE_FAIT: u8 = 67; // F2 — en-tête VIDE `{}`, charge vide
pub const TYPE_ECHEC: u8 = 127;

// ✅ **7 ET 8 SONT PRIS, ET PAR CELUI POUR QUI ILS ÉTAIENT RÉSERVÉS.** *(Ces
// lignes disaient « RÉSERVÉS à F3 » ; F3 les a prises, et la réservation est
// devenue un constat plutôt que d'être laissée au futur.)* La réservation a
// tenu son office : F2 a pris 4, 5 et 6 **en sautant** 7 et 8, si bien
// qu'aucun des deux sous-blocs n'a eu à renuméroter — et une renumérotation
// tardive est exactement le geste par lequel une référence survit à ce qu'elle
// désigne.
//
// ⚠️ **La numérotation n'est donc PAS contiguë : 6 est une ANNONCE, 7 et 8 sont
// des REQUÊTES.** L'ordre des valeurs ne dit rien de la famille ; c'est
// l'aiguillage nommé de `client/src/fichiers/protocole.ts` qui la dit, et lui
// seul.

/// Cause d'un échec renvoyé par le navigateur.
///
/// ⚠️ La représentation sur le fil est du **kebab-case**, et les variantes à
/// deux mots ou plus sont celles qui se cassent en silence : ce dépôt a laissé
/// passer une variante `battement-recu` verte sur cinquante tests parce que
/// rien n'épinglait ses octets. `un_code_d_echec_a_une_forme_epinglee_sur_le_fil`
/// épingle les **onze**, littéralement — les dix de F2, plus
/// `RepertoireNonVide` que F3 ajoute, **à trois mots**, donc de la famille
/// exacte qui casse en silence.
///
/// ⚠️ **Le plan de F2 en annonçait NEUF et appelait `CasseAmbigue` « la
/// neuvième variante ».** Sa tâche 1 en ajoute déjà deux aux sept de F1
/// (`DisquePlein`, `DejaPresent`), ce qui fait neuf ; sa tâche 9 en ajoute une
/// troisième. **`CasseAmbigue` est donc la DIXIÈME**, et le compte du plan est
/// corrigé ici plutôt que recopié.
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
    /// Le disque du poste local est plein — `QuotaExceededError` côté
    /// navigateur (F2).
    ///
    /// 🔴 **CE CODE N'ATTEINT AUCUNE APPLICATION WINDOWS, et le dire ici est le
    /// seul moyen qu'un successeur ne croie pas le contraire.** Il naît d'une
    /// poussée d'écriture, c'est-à-dire APRÈS que l'application a refermé son
    /// handle et cru avoir enregistré : il n'y a plus aucune commande ProjFS à
    /// compléter. Ce code sert au JOURNAL et au compteur d'écritures dues de la
    /// page-shell, jamais à un `HRESULT` rendu à qui que ce soit.
    DisquePlein,
    /// Une entrée du même nom existe déjà (F2).
    DejaPresent,
    /// 🔴 **Le poste local porte un homonyme qui ne diffère QUE par la casse, et
    /// l'écrivain a REFUSÉ d'écrire.**
    ///
    /// Le cas qui l'atteint : un fichier créé dans la VM avec une casse
    /// différente d'une entrée locale existante. Le système de fichiers du
    /// poste local est insensible à la casse sur Windows et sur macOS ;
    /// `getFileHandle('CASSE.TXT', { create: true })` y ouvrirait donc
    /// `Casse.txt` et **l'écraserait**. Refuser bruyamment est le seul
    /// arbitrage disponible entre « refuser à tort » et « écraser le mauvais
    /// fichier » — voir `client/src/fichiers/ecriture.ts`.
    CasseAmbigue,
    /// 🔴 **Le poste local refuse de supprimer un répertoire NON VIDE, et cela
    /// veut dire que le MIROIR A DÉRIVÉ.**
    ///
    /// F3 appelle `removeEntry(nom)` **sans `recursive`** : un geste dans la VM
    /// ne doit pas déclencher une destruction récursive sur le disque du poste
    /// local, sur la foi d'un miroir qu'aucune preuve ne dit à jour. Windows ne
    /// supprime jamais un répertoire non vide en un geste non plus —
    /// l'Explorateur et `rd /s` effacent les enfants un à un, et chaque enfant
    /// produit sa propre notification.
    ///
    /// 🔵 **Ce code est donc DIAGNOSTIQUE, et c'est ce qui le distingue des
    /// trois de F2** : le recevoir signifie que le poste local porte des
    /// entrées que la VM ne connaît pas. Il atteint bien une application
    /// Windows — la suppression naît d'une notification POST, mais la voie
    /// `PRE_DELETE` la précède, et c'est `ERROR_DIR_NOT_EMPTY` qu'un
    /// successeur y lirait.
    RepertoireNonVide,
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

pub mod entetes;

#[cfg(test)]
mod tests;
