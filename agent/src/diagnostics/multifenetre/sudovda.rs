//! Transcription de l'ABI du pilote d'affichage virtuel SudoVDA.
//!
//! Séparé de `moniteurs.rs` — qui est le CLIENT — parce que c'est une autre
//! responsabilité : ici on ne fait que traduire en Rust ce que dit l'en-tête
//! amont, avec la provenance de chaque constante. Le jour où le pilote change
//! de version, c'est ce fichier seul qu'on rouvre.
//!
//! Provenance et degré de confiance de chaque élément :
//! `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.
//! En deux mots : le GUID d'interface et 2 des 6 codes IOCTL sont confirmés
//! octet pour octet dans la DLL installée sur cette VM ; les 4 autres codes et
//! TOUTES les dispositions de structures sont une lecture amont non confirmée
//! localement, d'un en-tête antérieur de onze mois au pilote installé.
//!
//! Ce que la sonde de `contrat.rs` a éprouvé sur la VM, et qui n'est donc plus
//! une simple lecture : les tailles de `VersionProtocole` (4 o.) et de
//! `Veille` (8 o.), ainsi que les quatre octets de version eux-mêmes. L'ORDRE
//! des champs, lui, n'est éprouvé nulle part.

use windows::core::GUID;

/// Interface de périphérique de SudoVDA — **confirmée par présence d'octets**
/// dans le `SudoVDA.dll` installé sur cette VM (canal-de-controle.md §5.3).
/// À ne pas confondre avec le GUID de classe `{4D36E968-…}`, qui est la classe
/// `Display` standard de Windows et ne sert qu'à l'installation.
pub(super) const INTERFACE_PILOTE: GUID = GUID::from_u128(0xe5bc_c234_1e0c_418a_a0d4_ef8b_7501_414d);

// `CTL_CODE(FILE_DEVICE_UNKNOWN = 0x22, fonction, METHOD_BUFFERED = 0,
// FILE_ANY_ACCESS = 0)` = `(0x22 << 16) | (fonction << 2)`. Les deux codes
// marqués « confirmé » ont été retrouvés en octets dans la DLL installée ; les
// autres proviennent de la même macro appliquée au même en-tête amont.
//
// Le seul code que ce module n'emploie pas (`IOCTL_SET_RENDER_ADAPTER`
// `0x0022_2008`) n'est volontairement pas déclaré : une constante inutilisée
// est un avertissement de compilation, et une constante non employée n'est de
// toute façon éprouvée par rien.

/// Confirmé par octets (offset 16316 de la DLL locale).
pub(super) const IOCTL_AJOUTER_SORTIE: u32 = 0x0022_2000;
/// Non confirmé par octets — même macro, même en-tête amont.
pub(super) const IOCTL_RETIRER_SORTIE: u32 = 0x0022_2004;
/// Confirmé par octets (offset 16284 de la DLL locale).
pub(super) const IOCTL_LIRE_VEILLE: u32 = 0x0022_200C;
/// Non confirmé par octets — c'est le tampon le plus simple des six, donc le
/// premier que `valider_contrat()` éprouve.
pub(super) const IOCTL_LIRE_VERSION_PROTOCOLE: u32 = 0x0022_23FC;
/// Non confirmé par octets. Ni entrée ni sortie : le seul des six dont les deux
/// tampons soient vides, donc le seul dont la disposition ne puisse pas être
/// fausse. Il réarme le chien de garde du pilote, qui retire les sorties d'un
/// client devenu muet — voir `Veille` ci-dessous et `montee.rs`.
pub(super) const IOCTL_PINGUER: u32 = 0x0022_2220;

/// Tampon d'entrée de `IOCTL_AJOUTER_SORTIE` (`VIRTUAL_DISPLAY_ADD_PARAMS`).
///
/// Aucun `#pragma pack` en amont : alignement naturel MSVC/x64, soit 4 ici.
/// Offsets attendus : 0, 4, 8, 12, 28, 42 — total 56 octets, vérifié par
/// l'assertion de compilation plus bas.
///
/// Aucun de ces champs n'est jamais relu depuis Rust —
/// le seul lecteur est le pilote, à l'autre bout du `DeviceIoControl`. Les
/// retirer pour faire taire le lint reviendrait à changer la disposition du
/// tampon, c'est-à-dire à casser exactement ce que cette structure décrit.
/// D'où l'`allow` ci-dessous, qui est PERMANENT et non un provisoire daté :
/// aucun consommateur futur ne relira jamais ces champs.
#[allow(dead_code)]
#[repr(C)]
pub(super) struct DemandeAjout {
    pub(super) largeur: u32,
    pub(super) hauteur: u32,
    pub(super) hertz: u32,
    /// Choisi par NOUS, pas rendu par le pilote : c'est la clé de retrait.
    pub(super) guid_moniteur: GUID,
    pub(super) nom_peripherique: [u8; 14],
    pub(super) numero_serie: [u8; 14],
}

/// Tampon de sortie de `IOCTL_AJOUTER_SORTIE` (`VIRTUAL_DISPLAY_ADD_OUT`).
///
/// `LUID` Win32 = `{ DWORD LowPart; LONG HighPart; }`, 8 octets alignés sur 4 —
/// écrit en deux champs plutôt qu'en `windows::Win32::Foundation::LUID` pour
/// que la disposition qu'on suppose soit lisible ici, là où elle est en jeu.
#[repr(C)]
#[derive(Default)]
pub(super) struct SortieAjoutee {
    pub(super) adaptateur_bas: u32,
    pub(super) adaptateur_haut: i32,
    /// C'est lui qui devient l'`IdSortie` du trait.
    pub(super) identifiant_cible: u32,
}

/// Tampon d'entrée de `IOCTL_RETIRER_SORTIE`
/// (`VIRTUAL_DISPLAY_REMOVE_PARAMS`) : le pilote retire par le GUID que le
/// client a choisi à l'ajout, pas par l'identifiant qu'il a rendu.
///
/// Ses champs ne sont pas davantage relus depuis Rust — même raison que
/// `DemandeAjout`, `allow` permanent compris.
#[allow(dead_code)]
#[repr(C)]
pub(super) struct DemandeRetrait {
    pub(super) guid_moniteur: GUID,
}

/// Tampon de sortie de `IOCTL_LIRE_VEILLE`
/// (`VIRTUAL_DISPLAY_GET_WATCHDOG_OUT`).
///
/// L'en-tête amont ne documente AUCUNE unité pour ces deux `UINT` — ni le nom
/// des champs (`Timeout`, `Countdown`) ni un commentaire ne la donnent. On ne
/// la suppose donc pas ici : la sonde relève les nombres bruts.
///
/// **Ce que l'épreuve de `montee.rs` a établi, et ce qu'elle n'a pas établi.**
/// Relevé une fois par seconde pendant 180 s, `decompte` ne décroît PAS d'une
/// unité par seconde : il oscille entre 2 et 3 par paliers de plusieurs
/// dizaines de secondes, et n'approche jamais de zéro. L'unité de `delai`
/// reste donc inconnue — la seule chose exclue est « secondes restantes avant
/// retrait ». L'épreuve n'isole pas non plus le compteur : Apollo tourne sur
/// cette VM et pingue le même pilote, donc ces paliers peuvent être ses pings
/// à lui. Voir `montee.rs`.
#[repr(C)]
#[derive(Default)]
pub(super) struct Veille {
    pub(super) delai: u32,
    pub(super) decompte: u32,
}

/// Tampon de sortie de `IOCTL_LIRE_VERSION_PROTOCOLE`
/// (`SUVDA_PROTOCAL_VERSION`, orthographe d'origine). Quatre octets : le
/// `bool` MSVC en occupe un seul.
#[repr(C)]
#[derive(Default, PartialEq, Eq)]
pub(super) struct VersionProtocole {
    pub(super) majeure: u8,
    pub(super) mineure: u8,
    pub(super) increment: u8,
    pub(super) version_de_test: u8,
}

// Les tailles sont la seule partie du contrat amont qu'on puisse vérifier sans
// la VM. Un champ oublié ou un type mal traduit ferait échouer la compilation
// ici plutôt que de partir en tampon mal formé vers un pilote noyau.
const _: () = {
    assert!(std::mem::size_of::<DemandeAjout>() == 56);
    assert!(std::mem::size_of::<SortieAjoutee>() == 12);
    assert!(std::mem::size_of::<DemandeRetrait>() == 16);
    assert!(std::mem::size_of::<Veille>() == 8);
    assert!(std::mem::size_of::<VersionProtocole>() == 4);
};

/// Copie une désignation ASCII dans un champ `CHAR[14]`, terminée par un NUL.
///
/// Tronque à 13 caractères utiles plutôt que de refuser : ces deux champs sont
/// cosmétiques (Apollo y met le nom et l'identifiant du client), et faire
/// échouer une création de moniteur pour un nom trop long serait absurde.
pub(super) fn en_champ_14(texte: &str) -> [u8; 14] {
    let mut champ = [0u8; 14];
    for (place, octet) in champ.iter_mut().zip(texte.bytes()).take(13) {
        *place = octet;
    }
    champ
}
