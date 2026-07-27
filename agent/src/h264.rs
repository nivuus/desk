//! Manipulation de flux H.264 en format Annex-B.
//!
//! L'encodeur Media Foundation produit des NAL préfixées par des start codes
//! (`00 00 01` ou `00 00 00 01`). str0m attend des unités d'accès complètes,
//! une par image affichée.

/// Horloge RTP de la vidéo, en hertz. Valeur imposée par la RFC 3551.
pub const CLOCK_RATE_HZ: u64 = 90_000;

/// Une unité d'accès : toutes les NAL composant une image affichable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    /// Flux Annex-B complet de l'unité, start codes inclus.
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    /// Horodatage de présentation, en unités de 1/90000 s.
    pub pts_90k: u64,
}

/// Découpe un flux Annex-B en NAL, start codes retirés.
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

/// Longueur du start code à la position donnée, ou 0 s'il n'y en a pas.
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

/// Vrai si l'ensemble de NAL contient une image de référence instantanée.
pub fn is_keyframe(nals: &[Vec<u8>]) -> bool {
    nals.iter().any(|nal| nal_type(nal) == NAL_TYPE_IDR)
}

/// Regroupe les NAL d'un flux en unités d'accès, une par image affichable.
///
/// Une nouvelle unité commence à chaque NAL de tranche (IDR ou non-IDR) ; les
/// NAL de paramètres (SPS/PPS) qui la précèdent lui sont rattachées.
pub fn group_access_units(stream: &[u8], fps: u32) -> Vec<AccessUnit> {
    let nals = split_annex_b(stream);
    let tick = if fps == 0 { 0 } else { CLOCK_RATE_HZ / fps as u64 };

    let mut units: Vec<AccessUnit> = Vec::new();
    let mut current: Vec<Vec<u8>> = Vec::new();
    let mut slice_seen = false;

    for nal in nals {
        let kind = nal_type(&nal);
        let is_slice = kind == NAL_TYPE_IDR || kind == NAL_TYPE_NON_IDR;

        // Une NAL de paramètres après une tranche ouvre l'unité suivante.
        if slice_seen && (is_slice || kind == NAL_TYPE_SPS) {
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
        // Type de NAL = 5 bits de poids faible du premier octet.
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
        // La première unité contient les trois NAL avec leurs start codes.
        assert_eq!(units[0].data.len(), 3 * 4 + 2 + 2 + 2);
    }

    #[test]
    fn regroupe_un_flux_vide_sans_panique() {
        assert!(group_access_units(&[], 60).is_empty());
    }
}
