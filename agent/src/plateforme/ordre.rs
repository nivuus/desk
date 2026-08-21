//! Ce que la plateforme demande à la boucle d'applications.
//!
//! 🔴 EXTRAIT DE `agent/src/plateforme.rs` VERBATIM (sous-bloc G2), PARCE QUE
//! LE PLAFOND DE 500 LIGNES A ÉTÉ FRANCHI — 504 — ET QUE LA DOCTRINE DU DÉPÔT
//! EST DE RATTRAPER PAR UNE EXTRACTION, JAMAIS PAR UNE COMPRESSION.
//!
//! ⚠️ **L'EXTRACTION AURAIT DÛ PRÉCÉDER L'ADDITION.** Le plan de G2 annonçait
//! ce fichier à 453 lignes pour « +1 branche descendante » ; la branche et sa
//! documentation l'ont porté à 504. **Le franchissement est DÉCLARÉ**, comme
//! ce dépôt l'exige de ses trois franchissements de D10 et de ses deux de D9.
//!
//! ⚠️ Ce n'est PAS la « Convention de module enfant » de `CLAUDE.md`, qui vise
//! les modules extraits d'un parent `#[cfg(windows)]` : ce parent-ci n'a aucun
//! `cfg`, et c'est le même mécanisme employé pour l'autre raison — la règle des
//! 500 lignes.

/// Ce que la plateforme demande à la boucle d'applications.
///
/// ❌ **« LES DEUX MESSAGES DESCENDANTS » EST DEVENU FAUX AU SOUS-BLOC G3 : IL
/// Y EN A TROIS.** `DepuisLaPlateforme::Installer` s'y ajoute, et il ne va PAS
/// au même consommateur — d'où une SECONDE file, `Canal::installations()`.
///
/// ⚠️ **L'ARGUMENT CI-DESSOUS N'EST PAS RÉFUTÉ POUR AUTANT, et c'est ce qui
/// mérite d'être lu.** Le risque qu'il nomme — « un ajout futur en oublie une »
/// — est réel, et il vaut d'un consommateur unique. Il n'y en a plus un : la
/// découverte draine `Ordre` sur son fil COM, l'installation draine sa file sur
/// `tokio`. Poser `Installer` ici ferait TÉLÉCHARGER PLUSIEURS CENTAINES DE
/// MÉGAOCTETS AU FIL COM, qui cesserait de réconcilier — c'est-à-dire que le
/// catalogue se figerait pendant exactement l'installation dont on attend qu'il
/// rende compte. Le risque de l'oubli est traité autrement : la file d'en face
/// n'a **qu'un seul type de message**, donc rien à oublier, et `Ordre` reste
/// exhaustif pour SON consommateur — exactement comme ces lignes le demandent.
/// Voir `agent/src/plateforme/installation.rs`.
///
/// 🔴 UN ENUM PLUTÔT QU'UNE SECONDE FILE, ET C'EST UNE DÉCISION. Les deux
/// messages descendants vont au MÊME consommateur — la boucle d'apps, sur son
/// fil COM dédié —, et deux files l'obligeraient à interroger les deux à
/// chaque tour d'attente, avec le risque qu'un ajout futur en oublie une. Un
/// enum rend l'exhaustivité vérifiable par le compilateur là où deux files la
/// laisseraient à la vigilance.
///
/// ⚠️ IL NE TRANSPORTE AUCUN OCTET D'IMAGE. `IconesManquantes` ne porte qu'un
/// inventaire d'empreintes ; les images montent par `PUT /icone/:sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// Lancer une application, par sa clé. `demande` apparie la réponse.
    Lancer { demande: String, cle: String },
    /// Téléverser les icônes que la plateforme n'a pas.
    IconesManquantes { empreintes: Vec<String> },
}
