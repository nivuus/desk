//! Manipulation de flux H.264 en format Annex-B.
//!
//! The Media Foundation encoder produces NALs prefixed by start codes
//! (`00 00 01` or `00 00 00 01`). str0m expects complete access units,
//! one per displayed image.

/// Video RTP clock, in hertz. Value imposed by RFC 3551.
pub const CLOCK_RATE_HZ: u64 = 90_000;

/// An access unit: all the NALs making up a displayable image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    /// Complete Annex-B stream of the unit, start codes included.
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    /// Presentation timestamp, in units of 1/90000 s.
    pub pts_90k: u64,
}

/// Splits an Annex-B stream into NALs, start codes removed.
pub fn split_annex_b(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut nals = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;

    while i < stream.len() {
        let code_len = start_code_len(stream, i);
        if code_len > 0 {
            if let Some(begin) = start {
                push_nal(&mut nals, &stream[begin..i]);
            }
            i += code_len;
            start = Some(i);
        } else {
            i += 1;
        }
    }

    if let Some(begin) = start {
        push_nal(&mut nals, &stream[begin..]);
    }
    nals
}

fn push_nal(nals: &mut Vec<Vec<u8>>, nal: &[u8]) {
    if !nal.is_empty() {
        nals.push(nal.to_vec());
    }
}

/// Length of the start code at the given position, or 0 if there is none.
fn start_code_len(stream: &[u8], i: usize) -> usize {
    if stream[i..].starts_with(&[0, 0, 0, 1]) {
        4
    } else if stream[i..].starts_with(&[0, 0, 1]) {
        3
    } else {
        0
    }
}

const NAL_TYPE_IDR: u8 = 5;
const NAL_TYPE_NON_IDR: u8 = 1;
const NAL_TYPE_SPS: u8 = 7;

fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |b| b & 0x1F)
}

/// True if the slice is the first of its image (`first_mb_in_slice == 0`).
///
/// `first_mb_in_slice` is the very first field of the slice header,
/// coded in unsigned Exp-Golomb (ue(v)) and starting at the first byte following
/// the NAL header byte. In this coding, the value zero fits in a single
/// bit set to 1: it is therefore enough to test the high-order bit of that byte.
/// A slice without a payload byte (NAL truncated to its header alone)
/// is treated as a continuation rather than as the start of an image,
/// so as not to fragment an already corrupted stream further.
fn is_first_slice(nal: &[u8]) -> bool {
    nal.get(1).is_some_and(|b| b & 0x80 != 0)
}

/// True if the set of NALs contains an instantaneous decoding refresh image.
pub fn is_keyframe(nals: &[Vec<u8>]) -> bool {
    nals.iter().any(|nal| nal_type(nal) == NAL_TYPE_IDR)
}

/// Groups the NALs of a stream into access units, one per displayable image.
///
/// An image can be spread over several slice NALs (one per group
/// of macroblocks, for example when the encoder sub-splits to respect
/// an H.264 level constraint). Only the *first* slice of an image
/// (`first_mb_in_slice == 0`) opens a new access unit; the following
/// slices of the same image join it. The parameter NALs (SPS/PPS)
/// preceding the first slice are attached to the unit that follows them.
pub fn group_access_units(stream: &[u8], fps: u32) -> Vec<AccessUnit> {
    let nals = split_annex_b(stream);
    let tick = if fps == 0 {
        0
    } else {
        CLOCK_RATE_HZ / fps as u64
    };

    let mut units: Vec<AccessUnit> = Vec::new();
    let mut current: Vec<Vec<u8>> = Vec::new();
    let mut slice_seen = false;

    for nal in nals {
        let kind = nal_type(&nal);
        let is_slice = kind == NAL_TYPE_IDR || kind == NAL_TYPE_NON_IDR;
        // A new image only begins at the first slice that makes it up;
        // the following slices of the same image do not trigger a flush.
        let starts_new_picture = is_slice && is_first_slice(&nal);

        // A parameter NAL, or the first slice of a new image,
        // opens the next unit — but only if the current unit already
        // contains an image (otherwise we are still building it).
        if slice_seen && (starts_new_picture || kind == NAL_TYPE_SPS) {
            flush(&mut units, &mut current, tick);
            slice_seen = false;
        }

        if is_slice {
            slice_seen = true;
        }
        current.push(nal);
    }
    flush(&mut units, &mut current, tick);
    units
}

fn flush(units: &mut Vec<AccessUnit>, current: &mut Vec<Vec<u8>>, tick: u64) {
    if current.is_empty() {
        return;
    }
    let mut data = Vec::new();
    for nal in current.iter() {
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.extend_from_slice(nal);
    }
    let index = units.len() as u64;
    units.push(AccessUnit {
        is_keyframe: is_keyframe(current),
        data,
        pts_90k: index * tick,
    });
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupe_avec_start_code_de_quatre_octets() {
        let stream = [0, 0, 0, 1, 0x67, 0xAA, 0, 0, 0, 1, 0x68, 0xBB];
        let nals = split_annex_b(&stream);
        assert_eq!(nals, vec![vec![0x67, 0xAA], vec![0x68, 0xBB]]);
    }

    #[test]
    fn decoupe_avec_start_code_de_trois_octets() {
        let stream = [0, 0, 1, 0x67, 0xAA, 0, 0, 1, 0x65, 0xBB];
        let nals = split_annex_b(&stream);
        assert_eq!(nals, vec![vec![0x67, 0xAA], vec![0x65, 0xBB]]);
    }

    #[test]
    fn ignore_les_octets_avant_le_premier_start_code() {
        let stream = [0xFF, 0xFF, 0, 0, 0, 1, 0x65, 0x01];
        assert_eq!(split_annex_b(&stream), vec![vec![0x65, 0x01]]);
    }

    #[test]
    fn renvoie_rien_sans_start_code() {
        assert!(split_annex_b(&[0xFF, 0xFE, 0xFD]).is_empty());
        assert!(split_annex_b(&[]).is_empty());
    }

    #[test]
    fn ignore_les_nal_vides() {
        let stream = [0, 0, 0, 1, 0, 0, 0, 1, 0x65, 0x01];
        assert_eq!(split_annex_b(&stream), vec![vec![0x65, 0x01]]);
    }

    #[test]
    fn detecte_une_image_cle_sur_nal_idr() {
        // NAL type = 5 low-order bits of the first byte.
        assert!(is_keyframe(&[vec![0x65, 0x00]])); // 0x65 & 0x1F == 5 → IDR
        assert!(!is_keyframe(&[vec![0x41, 0x00]])); // 0x41 & 0x1F == 1 → non IDR
        assert!(is_keyframe(&[vec![0x67, 0x00], vec![0x65, 0x00]])); // SPS puis IDR
        assert!(!is_keyframe(&[]));
    }

    #[test]
    fn regroupe_en_unites_d_acces_horodatees() {
        // Deux images : SPS+PPS+IDR, puis une tranche non-IDR.
        let mut stream = Vec::new();
        for nal in [vec![0x67u8, 0x42], vec![0x68, 0xCE], vec![0x65, 0x88]] {
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        stream.extend_from_slice(&[0, 0, 0, 1]);
        stream.extend_from_slice(&[0x41, 0x9A]);

        let units = group_access_units(&stream, 60);
        assert_eq!(units.len(), 2);
        assert!(units[0].is_keyframe);
        assert!(!units[1].is_keyframe);
        assert_eq!(units[0].pts_90k, 0);
        assert_eq!(units[1].pts_90k, 1500); // 90000 / 60
                                            // The first unit contains the three NALs with their start codes.
        assert_eq!(units[0].data.len(), 3 * 4 + 2 + 2 + 2);
    }

    #[test]
    fn regroupe_un_flux_vide_sans_panique() {
        assert!(group_access_units(&[], 60).is_empty());
    }

    #[test]
    fn regroupe_des_images_multi_tranches_en_une_seule_unite() {
        // Two images of three slices each. The high-order bit of the
        // first payload byte distinguishes the first slice
        // (0x88 / 0x9A, bit set) from the following ones (0x00, bit clear).
        let mut stream = Vec::new();
        // Image 1 (IDR) : trois tranches de type 5.
        for nal in [vec![0x65u8, 0x88], vec![0x65, 0x00], vec![0x65, 0x00]] {
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        // Image 2 (non-IDR) : trois tranches de type 1.
        for nal in [vec![0x41u8, 0x9A], vec![0x41, 0x00], vec![0x41, 0x00]] {
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }

        let units = group_access_units(&stream, 60);
        // Six slices, but only two images: a NAL counted wrongly per
        // slice would wrongly produce six units.
        assert_eq!(units.len(), 2);
        assert!(units[0].is_keyframe);
        assert!(!units[1].is_keyframe);
        assert_eq!(units[0].pts_90k, 0);
        assert_eq!(units[1].pts_90k, 1500);
    }

    #[test]
    fn compte_les_memes_unites_que_ffprobe_sur_le_flux_reel() {
        // Independent reference: `ffprobe -count_frames` reports 300 images
        // on this file, whereas each image is actually split into
        // eight slices by libx264 (level 3.1 constraint at 1280x720/60).
        // A naive splitting rule (one unit per slice) would produce 2400
        // units instead of 300.
        let path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let data = std::fs::read(path).expect("lecture du flux de test");
        let units = group_access_units(&data, 60);
        assert_eq!(units.len(), 300);
    }
}
