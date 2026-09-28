//! Reading an icon directory — `GRPICONDIR` of a PE module, `ICONDIR` of an
//! `.ico` — from a `&[u8]`, and deriving the image's PROVENANCE from it.
//!
//! 🔴 IT IS THE PROOF OF SUB-BLOCK G2, AND THAT IS WHY THIS MODULE IS PURE.
//!
//! The question G2 exists to settle is: is this 256×256 icon
//! a real 256, or the upscaling of a small one? **It cannot be
//! answered from the rendered image.** Measured on 20 August 2026 on the two witnesses
//! that `agent/testdata/fabriquer-temoins-ico.py` produces, and that this module
//! reads in its tests:
//!
//! ```text
//! === g2plan-temoin-48.ico
//!    ICONDIR (the RESOURCE)          -> entrees=1 tailles=48
//!    ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
//!    ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
//!    PrivateExtractIcons idx0 256    -> 256x256 32bpp
//! === g2plan-temoin-256.ico
//!    ICONDIR (the RESOURCE)          -> entrees=1 tailles=256
//!    ShellImageFactory 256 ICONONLY  -> 256x256 32bpp
//!    ShellImageFactory 256 +BIGGEROK -> 256x256 32bpp
//!    PrivateExtractIcons idx0 256    -> 256x256 32bpp
//! ```
//!
//! **The four rendering lines are IDENTICAL; only the `ICONDIR` line
//! differs.** A file that contains ONLY 48×48 renders 256×256 32bpp, through
//! both APIs, without `SIIGBF_SCALEUP` and **even with
//! `SIIGBF_BIGGERSIZEOK`** — that is, explicitly telling the Shell
//! that a larger size would do.
//!
//! This module is **PURE, with no `cfg`**, and its tests run on the Linux
//! host. Without this split, acceptance criterion ② would have no host
//! test and its only proof would be a control-flow argument — the
//! exact situation that defect F1 of sub-block D7 paid for.
//!
//! What stays `#[cfg(windows)]` is OBTAINING these bytes, and only
//! that: `apps::icone::lecture_pe`.

use proto::plateforme::SourceMax;

/// Les six premiers octets, communs aux DEUX formats.
const ENTETE: usize = 6;
/// The size of a `GRPICONDIR` entry — it ends with a `WORD nID`.
const ENTREE_GRP: usize = 14;
/// The size of an `ICONDIR` entry — it ends with a `DWORD dwImageOffset`.
const ENTREE_ICO: usize = 16;

/// 🔴 `bWidth == 0` MEANS 256, AND IT IS THE ONLY TRAP OF THE PARSE.
///
/// The field is ONE BYTE, and 256 does not fit in it. A reader that returned `0`
/// would make a 256 icon say it has a ZERO size — and the `max()` of
/// [`maximum`] would rank it **below any other entry**, including
/// below a 16×16. The largest icon of the corpus would become the smallest,
/// silently.
fn largeur(octet: u8) -> u16 {
    if octet == 0 {
        256
    } else {
        u16::from(octet)
    }
}

/// The body shared by both readers — the header, then an entry stride.
///
/// ⚠️ IT IS PRIVATE, AND THE TWO PUBLIC READERS ARE TWO DISTINCT
/// FUNCTIONS RATHER THAN ONE WITH A FLAG. The first six bytes of both
/// formats are IDENTICAL: nothing in the bytes themselves says which one
/// you hold. A flag would therefore make the caller carry a decision it
/// could get wrong without any check seeing it — it is the
/// caller that KNOWS where its bytes come from, and the type of the function
/// it chooses is the only trace of that knowledge.
fn sizes(octets: &[u8], pas: usize) -> Option<Vec<u16>> {
    if octets.len() < ENTETE {
        return None;
    }
    let reserved = u16::from_le_bytes([octets[0], octets[1]]);
    let type_ = u16::from_le_bytes([octets[2], octets[3]]);
    let compte = usize::from(u16::from_le_bytes([octets[4], octets[5]]));
    // 🔴 THE HEADER IS CHECKED, otherwise four arbitrary bytes would pass
    // for an icon directory and return noise — indistinguishable from a
    // measurement.
    if reserved != 0 || type_ != 1 {
        return None;
    }
    // A TRUNCATED buffer returns `None`, it does not overflow and does not return a
    // partial list: a partial list would be a false measurement, and a
    // `max()` over it would lie without saying so.
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

/// The sizes of an `ICONDIR` — the directory of a STANDALONE `.ico`.
///
/// Entries of **16** bytes: they end with a `DWORD dwImageOffset`.
pub fn icondir_sizes(octets: &[u8]) -> Option<Vec<u16>> {
    sizes(octets, ENTREE_ICO)
}

/// The sizes of a `GRPICONDIR` — the `RT_GROUP_ICON` resource of a PE module.
///
/// Entries of **14** bytes: they end with a `WORD nID`, the identifier
/// of the matching `RT_ICON` resource, where an `.ico` carries a four-byte
/// offset.
pub fn grpicondir_sizes(octets: &[u8]) -> Option<Vec<u16>> {
    sizes(octets, ENTREE_GRP)
}

/// The largest PRESENT entry, or [`SourceMax::NonMesuree`].
///
/// 🔴 AN EMPTY LIST RETURNS `NonMesuree`, NEVER `Pixels(0)`. The two values
/// do not say the same thing: `Pixels(0)` would claim to have measured an
/// icon of zero pixels, when `NonMesuree` says nothing could be measured. The
/// whole G2 chain — the wire, the column, the route — exists to keep these
/// two distinct.
pub fn maximum(sizes: &[u16]) -> SourceMax {
    match sizes.iter().copied().max() {
        Some(px) => SourceMax::Pixels(px),
        None => SourceMax::NonMesuree,
    }
}

#[cfg(test)]
#[path = "ressource/tests.rs"]
mod tests;
