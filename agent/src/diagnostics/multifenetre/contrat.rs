//! `MULTIFENETRE_CONTRAT` probe: does the contract read upstream match the
//! virtual display driver actually installed on this VM?
//!
//! **Why this probe exists, although the plan did not provide for it.** The
//! plan stopped at a compilation check: nothing, within its
//! scope, ever called the driver. Yet only the interface GUID and two
//! of the six IOCTL codes are confirmed byte for byte in the installed DLL;
//! the layout of all structures comes from an upstream header eleven months
//! older than the driver (canal-de-controle.md §5.2, caveat). Stopping there
//! would make the next task discover a wrong contract **at the same time**
//! as it takes its measurement — and the two failures would be indistinguishable.
//!
//! It creates NO monitor, and only calls the two side-effect-free IOCTLs
//! with the simplest buffers: 4 then 8 bytes of output, no
//! input. It is the lead recommended by the end of §5.3 of the same document.

use anyhow::Result;

use crate::moniteurs_virtuels::pilote::ouvrir_pilote;
use crate::moniteurs_virtuels::sudovda::{Veille, VersionProtocole};

/// `VDAProtocolVersion = { 0, 2, 1, true }`, the constant of the upstream header of
/// September 2024.
///
/// It is compared with the survey, and not merely logged alongside: without
/// this comparison, `conforme` would only bear on SIZES, and a size
/// says nothing about the content. It is the only element of the survey that corroborates anything
/// other than a sizing.
const VERSION_AMONT: VersionProtocole = VersionProtocole {
    majeure: 0,
    mineure: 2,
    increment: 1,
    version_de_test: 1,
};

/// What a success establishes: that the interface GUID does open a live
/// device, that the `CTL_CODE` formula used for the four codes NOT
/// confirmed by bytes is the right one (`IOCTL_LIRE_VERSION_PROTOCOLE` is one
/// of them), that the driver returns exactly the number of bytes assumed by the
/// `#[repr(C)]` translation of these two structures, and that the four
/// version bytes coincide with the upstream constant.
///
/// What a success does NOT establish. First, nothing about
/// `VIRTUAL_DISPLAY_ADD_PARAMS`, whose 56 input bytes remain an
/// unconfirmed upstream reading. Then, the ORDER of the fields is only tested
/// PARTIALLY, and unevenly depending on the structure:
///
/// - a returned size, for its part, never says anything about offsets;
/// - `VersionProtocole` on the other hand is well constrained by the comparison of the
///   four bytes: out of the 24 permutations of `{0, 2, 1, 1}`, 22 give a
///   different quadruplet and would therefore be detected. Only the one that swaps
///   the last two fields — `increment` and `version_de_test`, both at
///   `1` — would go unnoticed;
/// - `Veille` is the only one out of reach of such a test: it returns
///   `delai` = `decompte`, two identical values, so the order of its two
///   fields is structurally indistinguishable.
pub(super) fn valider_contrat() -> Result<()> {
    let pilote = ouvrir_pilote()?;
    tracing::info!("SudoVDA device opened — the interface GUID is the right one");

    let (version, rendus_version) = pilote.version_protocole()?;
    tracing::info!(
        majeure = version.majeure,
        mineure = version.mineure,
        increment = version.increment,
        version_de_test = version.version_de_test,
        octets_rendus = rendus_version,
        attendus = std::mem::size_of::<VersionProtocole>(),
        "protocol version announced by the driver"
    );

    let (veille, rendus_veille) = pilote.veille()?;
    tracing::info!(
        delai = veille.delai,
        decompte = veille.decompte,
        octets_rendus = rendus_veille,
        attendus = std::mem::size_of::<Veille>(),
        "driver watchdog (unit not documented upstream — raw numbers)"
    );

    // The verdict is stated here rather than left to reading the log: what
    // matters is not that the calls succeeded, but that what they
    // return matches what we assume. A driver that gained a field
    // since the upstream header would succeed the call while returning a different
    // count.
    //
    // The two criteria are stated SEPARATELY, and the message says exactly
    // what each one tests — no more. Having a single boolean carry the word
    // "layout" while it only compares sizes would be asserting
    // beyond the survey.
    let sizes_match = rendus_version as usize == std::mem::size_of::<VersionProtocole>()
        && rendus_veille as usize == std::mem::size_of::<Veille>();
    let version_conforme = version == VERSION_AMONT;
    tracing::info!(
        sizes_match,
        version_conforme,
        conforme = sizes_match && version_conforme,
        // The sizes given in figures here are the ASSUMED ones, not the returned ones: this
        // message is constant, it will be emitted identically when
        // `sizes_match` is `false`. Announcing "the returned sizes
        // (4 and 8)" there would then assert exactly what the verdict denies.
        "verdict: the returned sizes are compared with the assumed sizes \
         (4 and 8), and the four version bytes with the upstream constant \
         {{0, 2, 1, true}} — the field order, for its part, is only \
         partially tested"
    );
    Ok(())
}
