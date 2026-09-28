//! Description of the multi-window probe's test patterns: the colour a
//! window must paint, and the verdict returned on a pixel read in a captured
//! image.
//!
//! Portable on purpose, like `geometry.rs`: it is the judge of the bench's
//! elimination gate ("does the covered window still render its test pattern").
//! A broken judge and a bench that finds nothing produce the same silence —
//! hence the tests, run on the Linux host.

/// Maximum number of simultaneous test patterns — the bench's target.
pub const MIRES_MAX: u8 = 8;

/// Red of test pattern no. 0. Not zero: a red at 0 would be confused with black
/// on a noisy reading.
const BASE_IDENTITE: u8 = 16;
/// Red gap between two neighbouring test patterns. Far beyond the tolerance:
/// confusing two test patterns would make the elimination gate pass a path that
/// captures the wrong window, exactly the defect being looked for.
const PAS_IDENTITE: u8 = 24;
/// Blue common to all test patterns: a sign that we are indeed reading a test pattern.
const BLEU_MIRE: u8 = 96;
/// Green of even and odd frames. The alternation makes the animation
/// detectable, and Desktop Duplication only emits an image if the desktop changes.
const VERT_PAIR: u8 = 32;
const VERT_IMPAIR: u8 = 224;
/// Tolerated gap per channel on a pixel read.
const TOLERANCE: i16 = 4;
/// Below it, on all three channels, the image is deemed black.
const SEUIL_NOIR: u8 = 12;

/// What a pixel read says about the expected window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The expected test pattern, within tolerance.
    Juste,
    /// The test pattern of ANOTHER window: the path captures the wrong content.
    Voisine(u8),
    /// Black image: the path returns nothing of the content.
    Noire,
    /// Neither a test pattern, nor black.
    Inconnue,
}

/// Colour test pattern `id` must paint at frame `trame`, as `(r, g, b)`.
pub fn couleur_mire(id: u8, trame: u64) -> (u8, u8, u8) {
    debug_assert!(id < MIRES_MAX);
    let rouge = BASE_IDENTITE + id * PAS_IDENTITE;
    let vert = if trame.is_multiple_of(2) {
        VERT_PAIR
    } else {
        VERT_IMPAIR
    };
    (rouge, vert, BLEU_MIRE)
}

fn proche(value: u8, attendu: u8) -> bool {
    (value as i16 - attendu as i16).abs() <= TOLERANCE
}

/// Identifies the window whose test pattern this pixel carries, if it carries one.
pub fn identifier(pixel: (u8, u8, u8)) -> Option<u8> {
    let (rouge, vert, bleu) = pixel;
    if !proche(bleu, BLEU_MIRE) {
        return None;
    }
    if !proche(vert, VERT_PAIR) && !proche(vert, VERT_IMPAIR) {
        return None;
    }
    let ecart = rouge as i16 - BASE_IDENTITE as i16;
    if ecart < 0 {
        return None;
    }
    let id = ecart / PAS_IDENTITE as i16;
    // The remainder must fall on an exact multiple of the step, within tolerance:
    // without this check, any shade of red would be attributed to a
    // test pattern by mere division.
    if (ecart - id * PAS_IDENTITE as i16).abs() > TOLERANCE || id >= MIRES_MAX as i16 {
        return None;
    }
    Some(id as u8)
}

/// Juge un pixel lu contre la mire attendue.
pub fn verdict(attendu: u8, pixel: (u8, u8, u8)) -> Verdict {
    let (rouge, vert, bleu) = pixel;
    if rouge < SEUIL_NOIR && vert < SEUIL_NOIR && bleu < SEUIL_NOIR {
        return Verdict::Noire;
    }
    match identifier(pixel) {
        Some(id) if id == attendu => Verdict::Juste,
        Some(id) => Verdict::Voisine(id),
        None => Verdict::Inconnue,
    }
}

/// Which path is checked at round `tour`, among `count` paths.
///
/// The multi-output set-up removes covering — one window per
/// output, nothing can hide another — so the elimination gate of the
/// single-output bench no longer has a purpose. The risk becomes pairing: that
/// path *i* actually captures output *j*, or black.
///
/// Checking the N paths at each round would detect it, but would make the
/// CPU cost of the check grow with N: the frame rate surveyed at N=8 would include eight
/// times that cost and would be comparable to nothing — the mistake already paid for in the
/// previous work stream, where the scope of the pixel reading changed
/// midway. The rotation covers all paths for **one** reading per
/// round, whatever N.
pub fn voie_controlee(tour: u64, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some((tour % count as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_pattern_colour_identifies_its_window() {
        for id in 0..MIRES_MAX {
            assert_eq!(identifier(couleur_mire(id, 0)), Some(id));
            assert_eq!(identifier(couleur_mire(id, 1)), Some(id));
        }
    }

    #[test]
    fn frame_alternation_changes_the_green_without_touching_the_identity() {
        let paire = couleur_mire(3, 10);
        let impaire = couleur_mire(3, 11);
        assert_ne!(
            paire.1, impaire.1,
            "without visible alternation, Desktop Duplication emits nothing"
        );
        assert_eq!(paire.0, impaire.0);
        assert_eq!(identifier(impaire), Some(3));
    }

    #[test]
    fn a_neighbouring_window_test_pattern_is_rejected() {
        // The exact case the elimination gate must catch: the capture
        // of a covered window returns the content of the one on top.
        assert_eq!(verdict(3, couleur_mire(4, 0)), Verdict::Voisine(4));
    }

    #[test]
    fn a_black_image_is_rejected() {
        // PrintWindow on a D3D window typically returns black: it is a
        // path failure, not an unknown test pattern.
        assert_eq!(verdict(0, (0, 0, 0)), Verdict::Noire);
    }

    #[test]
    fn a_read_gap_within_tolerance_stays_correct() {
        let (r, g, b) = couleur_mire(5, 0);
        assert_eq!(verdict(5, (r + 2, g + 2, b + 2)), Verdict::Juste);
    }

    #[test]
    fn a_foreign_colour_is_unknown() {
        // The desktop background, a PowerShell console: neither a test pattern, nor black.
        assert_eq!(verdict(0, (255, 255, 255)), Verdict::Inconnue);
        assert_eq!(identifier((1, 36, 86)), None);
    }

    /// The property that matters: over k·N rounds, each path is checked
    /// exactly k times. A check that favoured one path would leave
    /// the others uncovered, and it is precisely cross-pairing
    /// between outputs that this set-up must detect.
    #[test]
    fn the_rotation_checks_each_lane_the_same_number_of_times() {
        for count in 1..=8usize {
            let mut comptes = vec![0usize; count];
            for tour in 0..(count as u64 * 7) {
                let voie = voie_controlee(tour, count).expect("non-zero count");
                comptes[voie] += 1;
            }
            assert!(
                comptes.iter().all(|compte| *compte == 7),
                "count = {count}, counts = {comptes:?}"
            );
        }
    }

    #[test]
    fn the_rotation_never_designates_a_non_existent_lane() {
        for count in 1..=8usize {
            for tour in 0..100u64 {
                let voie = voie_controlee(tour, count).expect("non-zero count");
                assert!(voie < count, "lane {voie} outside the {count} lanes");
            }
        }
    }

    /// Zero paths is not a caller error to report through a panic: the
    /// bench must be able to ask without knowing, and check nothing.
    #[test]
    fn without_a_lane_there_is_nothing_to_check() {
        assert_eq!(voie_controlee(0, 0), None);
        assert_eq!(voie_controlee(42, 0), None);
    }
}
