//! The GUID we assign to each virtual output, and its reverse
//! reading.
//!
//! Extracted from `pilote.rs` at the final branch review: that file was at
//! 493 lines for a ceiling of 500 (see `CLAUDE.md`), and fix I1
//! added number recycling to it. The addition therefore comes with its
//! extraction. No value changed along the way.
//!
//! Under `#[cfg(windows)]` not by choice but by necessity: `windows::core::GUID`
//! does not exist outside this target (`Cargo.toml` gates the `windows` crate on
//! `cfg(windows)`). The logic that is pure — the allocation and
//! recycling of numbers — lives in the sibling module `numeros`, outside any
//! `cfg`, and it is the one carrying the tests.

use windows::core::GUID;

/// Template of the GUID we assign to each created output: the 16
/// low-order bits carry a number, the rest is an arbitrary constant
/// chosen here. The GUID only needs to be unique and recognisable — if it
/// lingers one day in the driver's state, we will know where it comes from.
///
/// Numbers start again from zero at each run, and it is an assumed
/// choice: two runs therefore assign the same GUIDs. It is this
/// determinism that gives `purge::purger` a recognisable pattern to
/// find our orphaned outputs, which no in-memory table can designate
/// anymore.
const GABARIT_GUID_MONITEUR: u128 = 0x9c4a_1f6e_2b73_4d51_9e08_6775_4143_0000;

/// GUID assigned to the monitor carrying this `numero`.
///
/// `pub(crate)`: `purge.rs` computes EXACTLY the same sequence without live
/// state — it is this determinism that lets an inter-process purge
/// find the GUIDs of a run killed outright.
pub(crate) fn guid_pour(numero: u16) -> GUID {
    GUID::from_u128(GABARIT_GUID_MONITEUR | u128::from(numero))
}

/// Number carried by a GUID that `guid_pour` built, or `None` if this GUID
/// does not come from this template.
///
/// Serves recycling (fix I1): `pilote::oublier` returns to the allocator
/// the number of an output actually removed, and it only has the GUID at
/// hand.
///
/// **The reading is checked, not assumed.** The 16 low-order bits of a
/// `u128` land in the last two bytes of `data4`
/// (`GUID::from_u128` puts `(uuid as u64).to_be_bytes()` there), but we do not
/// rely on this reasoning: the number read back is fed back into `guid_pour`
/// and the rebuilt GUID must be identical. If the assumed layout were
/// wrong, the function returns `None` — hence no recycling, that is the
/// behaviour from before this fix — never a wrong number that would
/// reassign the GUID of a live output.
pub(crate) fn numero_de(guid: GUID) -> Option<u16> {
    let numero = u16::from_be_bytes([guid.data4[6], guid.data4[7]]);
    (guid_pour(numero) == guid).then_some(numero)
}
