//! Transcription de l'ABI du pilote d'affichage virtuel SudoVDA.
//!
//! Separated from `moniteurs.rs` — which is the CLIENT — because it is another
//! responsibility: here we only translate into Rust what the upstream header
//! says, with the provenance of each constant. The day the driver changes
//! version, it is this file alone that gets reopened.
//!
//! Provenance and degree of confidence of each element:
//! `docs/superpowers/plans/journaux-mesures-prealables/canal-de-controle.md`.
//! In short: the interface GUID and 2 of the 6 IOCTL codes are confirmed
//! byte for byte in the DLL installed on this VM; the other 4 codes and
//! ALL the structure layouts are an upstream reading not confirmed
//! locally, from a header eleven months older than the installed driver.
//!
//! What the probe of `contrat.rs` tested on the VM, and which is therefore no longer
//! a mere reading: the sizes of `VersionProtocole` (4 bytes) and of
//! `Veille` (8 bytes), as well as the four version bytes themselves. The ORDER
//! of the fields, for its part, is tested nowhere.

use windows::core::GUID;

/// SudoVDA device interface — **confirmed by presence of bytes**
/// in the `SudoVDA.dll` installed on this VM (canal-de-controle.md §5.3).
/// Not to be confused with the class GUID `{4D36E968-…}`, which is Windows'
/// standard `Display` class and only serves installation.
pub(super) const INTERFACE_PILOTE: GUID =
    GUID::from_u128(0xe5bc_c234_1e0c_418a_a0d4_ef8b_7501_414d);

// `CTL_CODE(FILE_DEVICE_UNKNOWN = 0x22, function, METHOD_BUFFERED = 0,
// FILE_ANY_ACCESS = 0)` = `(0x22 << 16) | (function << 2)`. The two codes
// marked "confirmed" were found as bytes in the installed DLL; the
// others come from the same macro applied to the same upstream header.
//
// The only code this module does not use (`IOCTL_SET_RENDER_ADAPTER`
// `0x0022_2008`) is deliberately not declared: an unused constant
// is a compilation warning, and an unused constant is
// tested by nothing anyway.

/// Confirmed by bytes (offset 16316 of the local DLL).
pub(super) const IOCTL_ADD_OUTPUT: u32 = 0x0022_2000;
/// Not confirmed by bytes — same macro, same upstream header.
pub(super) const IOCTL_RETIRER_SORTIE: u32 = 0x0022_2004;
/// Confirmed by bytes (offset 16284 of the local DLL).
pub(super) const IOCTL_LIRE_VEILLE: u32 = 0x0022_200C;
/// Not confirmed by bytes — it is the simplest buffer of the six, hence the
/// first that `valider_contrat()` tests.
pub(super) const IOCTL_LIRE_VERSION_PROTOCOLE: u32 = 0x0022_23FC;
/// Not confirmed by bytes. Neither input nor output: the only one of the six whose two
/// buffers are empty, hence the only one whose layout cannot be
/// wrong. It rearms the driver's watchdog, which removes the outputs of a
/// client that went silent — see `Veille` below and `montee.rs`.
pub(super) const IOCTL_PINGUER: u32 = 0x0022_2220;

/// Input buffer of `IOCTL_ADD_OUTPUT` (`VIRTUAL_DISPLAY_ADD_PARAMS`).
///
/// No `#pragma pack` upstream: MSVC/x64 natural alignment, that is 4 here.
/// Expected offsets: 0, 4, 8, 12, 28, 42 — total 56 bytes, checked by
/// the compile-time assertion below.
///
/// None of these fields is ever read back from Rust —
/// the only reader is the driver, at the other end of the `DeviceIoControl`. Removing
/// them to silence the lint would amount to changing the layout of the
/// buffer, that is breaking exactly what this structure describes.
/// Hence the `allow` below, which is PERMANENT and not a dated temporary:
/// no future consumer will ever read these fields back.
#[allow(dead_code)]
#[repr(C)]
pub(super) struct DemandeAjout {
    pub(super) largeur: u32,
    pub(super) hauteur: u32,
    pub(super) hertz: u32,
    /// Chosen by US, not returned by the driver: it is the removal key.
    pub(super) guid_moniteur: GUID,
    pub(super) nom_peripherique: [u8; 14],
    pub(super) numero_serie: [u8; 14],
}

/// Output buffer of `IOCTL_ADD_OUTPUT` (`VIRTUAL_DISPLAY_ADD_OUT`).
///
/// Win32 `LUID` = `{ DWORD LowPart; LONG HighPart; }`, 8 bytes aligned on 4 —
/// written as two fields rather than as `windows::Win32::Foundation::LUID` so
/// that the assumed layout is readable here, where it is at stake.
#[repr(C)]
#[derive(Default)]
pub(super) struct SortieAjoutee {
    pub(super) adaptateur_bas: u32,
    pub(super) adaptateur_haut: i32,
    /// It is the one that becomes the trait's `IdSortie`.
    pub(super) identifiant_cible: u32,
}

/// Input buffer of `IOCTL_RETIRER_SORTIE`
/// (`VIRTUAL_DISPLAY_REMOVE_PARAMS`): the driver removes through the GUID the
/// client chose at addition, not through the identifier it returned.
///
/// Its fields are not read back from Rust either — same reason as
/// `DemandeAjout`, permanent `allow` included.
#[allow(dead_code)]
#[repr(C)]
pub(super) struct DemandeRetrait {
    pub(super) guid_moniteur: GUID,
}

/// Tampon de sortie de `IOCTL_LIRE_VEILLE`
/// (`VIRTUAL_DISPLAY_GET_WATCHDOG_OUT`).
///
/// The upstream header documents NO unit for these two `UINT`s — neither the names
/// of the fields (`Timeout`, `Countdown`) nor a comment give it. We therefore do not
/// assume it here: the probe surveys the raw numbers.
///
/// **What the test of `montee.rs` noted — and why it concludes
/// nothing about the unit.** Read once per second for 180 s during which
/// we never pinged, `decompte` oscillates between 2 and 3 in steps of
/// several tens of seconds, and no output was removed.
///
/// **That excludes no unit, not even the second.** Apollo runs on this
/// VM and pings the same driver at a cadence of the order of a second: a
/// `Timeout` of three SECONDS rearmed by someone else would read exactly like this,
/// 2 or 3 without ever approaching zero. The unit of these two `UINT`s therefore remains
/// entirely unknown, and nothing is established about the fate of a lone and
/// silent client.
#[repr(C)]
#[derive(Default)]
pub(crate) struct Veille {
    pub(crate) delai: u32,
    pub(crate) decompte: u32,
}

/// Output buffer of `IOCTL_LIRE_VERSION_PROTOCOLE`
/// (`SUVDA_PROTOCAL_VERSION`, original spelling). Four bytes: the
/// MSVC `bool` occupies only one.
#[repr(C)]
#[derive(Default, PartialEq, Eq)]
pub(crate) struct VersionProtocole {
    pub(crate) majeure: u8,
    pub(crate) mineure: u8,
    pub(crate) increment: u8,
    pub(crate) version_de_test: u8,
}

// Sizes are the only part of the upstream contract that can be checked without
// the VM. A forgotten field or a mistranslated type would make compilation fail
// here rather than going off as a malformed buffer to a kernel driver.
const _: () = {
    assert!(std::mem::size_of::<DemandeAjout>() == 56);
    assert!(std::mem::size_of::<SortieAjoutee>() == 12);
    assert!(std::mem::size_of::<DemandeRetrait>() == 16);
    assert!(std::mem::size_of::<Veille>() == 8);
    assert!(std::mem::size_of::<VersionProtocole>() == 4);
};

/// Copies an ASCII designation into a `CHAR[14]` field, NUL-terminated.
///
/// Truncates to 13 useful characters rather than refusing: these two fields are
/// cosmetic (Apollo puts the client's name and identifier there), and making
/// a monitor creation fail for a name too long would be absurd.
pub(super) fn en_champ_14(texte: &str) -> [u8; 14] {
    let mut champ = [0u8; 14];
    for (place, octet) in champ.iter_mut().zip(texte.bytes()).take(13) {
        *place = octet;
    }
    champ
}
