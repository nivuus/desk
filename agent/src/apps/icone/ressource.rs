//! Lire un répertoire d'icônes — `GRPICONDIR` d'un module PE, `ICONDIR` d'un
//! `.ico` — depuis un `&[u8]`, et en tirer la PROVENANCE de l'image.
//!
//! 🔴 C'EST LA PREUVE DU SOUS-BLOC G2, ET C'EST POURQUOI CE MODULE EST PUR.
//!
//! La question que G2 existe pour trancher est : cette icône 256×256 est-elle
//! un vrai 256, ou l'agrandissement d'une petite ? **Elle ne peut pas se
//! répondre sur l'image rendue.** Mesuré le 20 août 2026 sur les deux témoins
//! que `agent/testdata/fabriquer-temoins-ico.py` fabrique, et que ce module
//! lit dans ses tests :
//!
//! ```text
//! === g2plan-temoin-48.ico
//!    ICONDIR (la RESSOURCE)          -> entrees=1 tailles=48
//!    ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
//!    ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
//!    PrivateExtractIcons idx0 256    -> 256x256 32bpp
//! === g2plan-temoin-256.ico
//!    ICONDIR (la RESSOURCE)          -> entrees=1 tailles=256
//!    ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
//!    ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
//!    PrivateExtractIcons idx0 256    -> 256x256 32bpp
//! ```
//!
//! **Les quatre lignes de rendu sont IDENTIQUES ; seule la ligne `ICONDIR`
//! diffère.** Un fichier qui ne contient QUE du 48×48 rend 256×256 32bpp, par
//! les deux API, sans `SIIGBF_SCALEUP` et **y compris avec
//! `SIIGBF_BIGGERSIZEOK`** — c'est-à-dire en disant explicitement au Shell
//! qu'une taille plus grande conviendrait.
//!
//! Ce module est **PUR, sans aucun `cfg`**, et ses tests courent sur l'hôte
//! Linux. Sans cette coupure, le critère ② de recette n'aurait aucun test
//! d'hôte et sa seule preuve serait un argument de flot de contrôle — la
//! situation exacte que le défaut F1 du sous-bloc D7 a payée.
//!
//! Ce qui reste `#[cfg(windows)]` est l'OBTENTION de ces octets, et elle
//! seule : `apps::icone::lecture_pe`.

use proto::plateforme::SourceMax;

/// Les six premiers octets, communs aux DEUX formats.
const ENTETE: usize = 6;
/// La taille d'une entrée de `GRPICONDIR` — elle finit par un `WORD nID`.
const ENTREE_GRP: usize = 14;
/// La taille d'une entrée d'`ICONDIR` — elle finit par un `DWORD dwImageOffset`.
const ENTREE_ICO: usize = 16;

/// 🔴 `bWidth == 0` VAUT 256, ET C'EST LE SEUL PIÈGE DE L'ANALYSE.
///
/// Le champ fait UN OCTET, et 256 n'y tient pas. Un lecteur qui rendrait `0`
/// ferait dire à une icône 256 qu'elle est de taille NULLE — et le `max()` de
/// [`maximum`] la classerait **sous n'importe quelle autre entrée**, y compris
/// sous un 16×16. La plus grande icône du corpus deviendrait la plus petite,
/// silencieusement.
fn largeur(octet: u8) -> u16 {
    if octet == 0 { 256 } else { u16::from(octet) }
}

/// Le corps commun aux deux lecteurs — l'en-tête, puis un pas d'entrée.
///
/// ⚠️ IL EST PRIVÉ, ET LES DEUX LECTEURS PUBLICS SONT DEUX FONCTIONS
/// DISTINCTES PLUTÔT QU'UNE AVEC UN DRAPEAU. Les six premiers octets des deux
/// formats sont IDENTIQUES : rien, dans les octets eux-mêmes, ne dit lequel on
/// tient. Un drapeau ferait donc porter à l'appelant une décision qu'il
/// pourrait se tromper à prendre sans qu'aucun contrôle ne le voie — c'est
/// l'appelant qui SAIT d'où viennent ses octets, et le type de la fonction
/// qu'il choisit est la seule trace de ce savoir.
fn tailles(octets: &[u8], pas: usize) -> Option<Vec<u16>> {
    if octets.len() < ENTETE {
        return None;
    }
    let reserved = u16::from_le_bytes([octets[0], octets[1]]);
    let type_ = u16::from_le_bytes([octets[2], octets[3]]);
    let compte = usize::from(u16::from_le_bytes([octets[4], octets[5]]));
    // 🔴 L'EN-TÊTE EST VÉRIFIÉ, sans quoi quatre octets arbitraires passeraient
    // pour un répertoire d'icônes et rendraient du bruit — indiscernable d'une
    // mesure.
    if reserved != 0 || type_ != 1 {
        return None;
    }
    // Un tampon TRONQUÉ rend `None`, il ne déborde pas et ne rend pas une
    // liste partielle : une liste partielle serait une mesure fausse, et un
    // `max()` sur elle mentirait sans le dire.
    let fin = ENTETE.checked_add(compte.checked_mul(pas)?)?;
    if octets.len() < fin {
        return None;
    }
    Some(
        (0..compte)
            .map(|i| largeur(octets[ENTETE + i * pas]))
            .collect(),
    )
}

/// Les tailles d'un `ICONDIR` — le répertoire d'un `.ico` AUTONOME.
///
/// Entrées de **16** octets : elles finissent par un `DWORD dwImageOffset`.
pub fn tailles_icondir(octets: &[u8]) -> Option<Vec<u16>> {
    tailles(octets, ENTREE_ICO)
}

/// Les tailles d'un `GRPICONDIR` — la ressource `RT_GROUP_ICON` d'un module PE.
///
/// Entrées de **14** octets : elles finissent par un `WORD nID`, l'identifiant
/// de la ressource `RT_ICON` correspondante, là où un `.ico` porte un offset
/// de quatre octets.
pub fn tailles_grpicondir(octets: &[u8]) -> Option<Vec<u16>> {
    tailles(octets, ENTREE_GRP)
}

/// La plus grande entrée PRÉSENTE, ou [`SourceMax::NonMesuree`].
///
/// 🔴 UNE LISTE VIDE REND `NonMesuree`, JAMAIS `Pixels(0)`. Les deux valeurs
/// ne disent pas la même chose : `Pixels(0)` affirmerait avoir mesuré une
/// icône de zéro pixel, quand `NonMesuree` dit qu'on n'a rien pu mesurer. La
/// chaîne entière de G2 — le fil, la colonne, la route — existe pour tenir ces
/// deux-là distinctes.
pub fn maximum(tailles: &[u16]) -> SourceMax {
    match tailles.iter().copied().max() {
        Some(px) => SourceMax::Pixels(px),
        None => SourceMax::NonMesuree,
    }
}

#[cfg(test)]
#[path = "ressource/tests.rs"]
mod tests;
