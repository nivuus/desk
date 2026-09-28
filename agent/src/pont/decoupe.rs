//! Splitting a range into bounded frames. **PURE**: no `cfg`, tested
//! in isolation on the host.
//!
//! ⚠️ *This module said "a READ range": since F2, the **write
//! thread** also uses it to split a local file to push — and
//! the three defects described below then cost the user's
//! file, on THEIR workstation, rather than a wrong hydration.*
//!
//! ⚠️ **It is the module where unit errors live, and that is why
//! it is pure and tested separately** (spec §7.3). Criterion (2) of F1's
//! acceptance run — the SHA-256 digest of the file read through the drive — is
//! exactly what this module can make fail: a file truncated by one
//! frame, ranges out of order, an overlap that duplicates
//! bytes. None of these three defects is visible to the eye on a text
//! file; all three break the digest.
//!
//! ⚠️ **The unit trap the type imposes**: `PRJ_GET_FILE_DATA_CB` receives
//! a `byteoffset: u64` and a `length: u32`. The position is therefore on 64 bits
//! — a file can exceed 4 GiB — but each chunk length fits
//! in 32 bits. The signature of [`decouper`] takes a length in `u64` and
//! returns lengths in `u32`: it is the conversion that would overflow if `max`
//! were not itself bounded, and the test covers it.

/// A contiguous range to request from the browser in a single frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Morceau {
    pub position: u64,
    pub longueur: u32,
}

/// Splits `[position, position + longueur)` into chunks of at most `max`
/// bytes, contiguous and increasing.
///
/// A zero length produces **no** chunk: an empty chunk
/// would cause an empty response frame that nothing would distinguish from an end
/// of file.
///
/// # Panique
///
/// If `max` is 0 — a split into zero-byte chunks does not terminate.
/// It is a programming error of the caller, not a runtime case:
/// `max` is a constant of the bridge, never a value received from the network.
pub fn decouper(position: u64, longueur: u64, max: usize) -> Vec<Morceau> {
    assert!(
        max > 0,
        "une découpe en morceaux de zéro octet ne se termine pas"
    );
    // `max` is bounded to `u32::MAX` before any conversion: it is here that the
    // overflow would occur on a 64-bit target, where `usize` is wider
    // than `u32`.
    let max = max.min(u32::MAX as usize) as u64;
    let mut morceaux = Vec::new();
    let mut reste = longueur;
    let mut curseur = position;
    while reste > 0 {
        let prise = reste.min(max);
        morceaux.push(Morceau {
            position: curseur,
            // `prise <= max <= u32::MAX`: the conversion cannot overflow,
            // and it is the `min` above that guarantees it, not a hope.
            longueur: prise as u32,
        });
        curseur += prise;
        reste -= prise;
    }
    morceaux
}

#[cfg(test)]
mod tests;
