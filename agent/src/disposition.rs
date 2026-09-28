//! Splitting a desktop into disjoint slots, one per test pattern.
//!
//! Portable like `geometry.rs`, and tested for the same reason the
//! crops are tested there: a layout where two slots overlap
//! would make the bench's elimination gate fail without any capture
//! path being at fault. The bench would blame capture for a defect of the bench.

use crate::geometry::Rect;

/// Dimensions below which a slot is not worth measuring.
pub const TUILE_MIN_LARGEUR: u32 = 320;
pub const TUILE_MIN_HAUTEUR: u32 = 240;

/// Splits `bureau` into `n` disjoint slots, in the squarest grid
/// possible.
///
/// Returns `None` if the slots would go below `TUILE_MIN_*`: better
/// a clean refusal than a measurement on windows too small to represent
/// anything of the product.
pub fn tuiles(bureau: Rect, n: u32) -> Option<Vec<Rect>> {
    if n == 0 {
        return None;
    }
    // Squarest grid possible: `columns` is the smallest integer
    // whose square reaches `n`. Computed by a loop rather than by
    // `(n as f64).sqrt().ceil()`, whose floating-point rounding is wrong for
    // some perfect squares depending on the platform.
    let mut columns = 1u32;
    while columns * columns < n {
        columns += 1;
    }
    let lignes = n.div_ceil(columns);

    let largeur = (bureau.width / columns) & !1;
    let hauteur = (bureau.height / lignes) & !1;
    if largeur < TUILE_MIN_LARGEUR || hauteur < TUILE_MIN_HAUTEUR {
        return None;
    }

    let mut places = Vec::with_capacity(n as usize);
    for index in 0..n {
        let column = index % columns;
        let ligne = index / columns;
        places.push(Rect {
            x: bureau.x + (column * largeur) as i32,
            y: bureau.y + (ligne * hauteur) as i32,
            width: largeur,
            height: hauteur,
        });
    }
    Some(places)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::rects_overlap;

    /// The desktop surveyed on the VM on 30/07/2026.
    const BUREAU: Rect = Rect {
        x: 0,
        y: 0,
        width: 2400,
        height: 1080,
    };

    #[test]
    fn eight_slots_do_not_overlap_and_fit_in_the_desktop() {
        let places = tuiles(BUREAU, 8).expect("huit places sur 2400x1080");
        assert_eq!(places.len(), 8);
        for (i, a) in places.iter().enumerate() {
            assert!(a.x >= 0 && a.y >= 0);
            assert!(a.x as u32 + a.width <= BUREAU.width);
            assert!(a.y as u32 + a.height <= BUREAU.height);
            for b in places.iter().skip(i + 1) {
                assert!(!rects_overlap(*a, *b), "{a:?} recouvre {b:?}");
            }
        }
    }

    #[test]
    fn les_places_ont_des_dimensions_paires() {
        // The H.264 encoder refuses odd dimensions in 4:2:0, as
        // `geometry::crop_region` recalls.
        for place in tuiles(BUREAU, 8).unwrap() {
            assert_eq!(place.width % 2, 0);
            assert_eq!(place.height % 2, 0);
        }
    }

    #[test]
    fn une_place_unique_couvre_presque_tout_le_bureau() {
        let places = tuiles(BUREAU, 1).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].width, 2400);
        assert_eq!(places[0].height, 1080);
    }

    #[test]
    fn un_bureau_trop_petit_fait_refuser_la_disposition() {
        // Refuse outright rather than return tiny slots: a
        // measurement on 80x60 windows would say nothing about the product.
        let etroit = Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        };
        assert_eq!(tuiles(etroit, 8), None);
    }

    #[test]
    fn the_requested_slot_count_is_honoured_even_if_the_grid_is_wider() {
        // Seven slots fit in a 3x3 grid: two cells stay empty,
        // and we must not return nine slots for all that.
        assert_eq!(tuiles(BUREAU, 7).unwrap().len(), 7);
    }
}
