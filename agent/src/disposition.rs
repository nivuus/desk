//! Découpe d'un bureau en places disjointes, une par mire.
//!
//! Portable comme `geometry.rs`, et testé pour la même raison qu'y sont
//! testés les recadrages : une disposition dont deux places se recouvrent
//! ferait échouer la porte éliminatoire du banc sans qu'aucune voie de
//! capture soit en cause. Le banc accuserait la capture d'un défaut du banc.

use crate::geometry::Rect;

/// Dimensions en deçà desquelles une place ne vaut pas la mesure.
pub const TUILE_MIN_LARGEUR: u32 = 320;
pub const TUILE_MIN_HAUTEUR: u32 = 240;

/// Découpe `bureau` en `n` places disjointes, en grille la plus carrée
/// possible.
///
/// Renvoie `None` si les places descendraient sous `TUILE_MIN_*` : mieux vaut
/// un refus net qu'une mesure sur des fenêtres trop petites pour représenter
/// quoi que ce soit du produit.
pub fn tuiles(bureau: Rect, n: u32) -> Option<Vec<Rect>> {
    if n == 0 {
        return None;
    }
    // Grille la plus carrée possible : `colonnes` est le plus petit entier
    // dont le carré atteint `n`. Calculé par boucle plutôt que par
    // `(n as f64).sqrt().ceil()`, dont l'arrondi flottant est faux pour
    // certains carrés parfaits selon la plateforme.
    let mut colonnes = 1u32;
    while colonnes * colonnes < n {
        colonnes += 1;
    }
    let lignes = n.div_ceil(colonnes);

    let largeur = (bureau.width / colonnes) & !1;
    let hauteur = (bureau.height / lignes) & !1;
    if largeur < TUILE_MIN_LARGEUR || hauteur < TUILE_MIN_HAUTEUR {
        return None;
    }

    let mut places = Vec::with_capacity(n as usize);
    for index in 0..n {
        let colonne = index % colonnes;
        let ligne = index / colonnes;
        places.push(Rect {
            x: bureau.x + (colonne * largeur) as i32,
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

    /// Le bureau relevé sur la VM le 30/07/2026.
    const BUREAU: Rect = Rect { x: 0, y: 0, width: 2400, height: 1080 };

    #[test]
    fn huit_places_ne_se_recouvrent_pas_et_tiennent_dans_le_bureau() {
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
        // L'encodeur H.264 refuse les dimensions impaires en 4:2:0, comme
        // le rappelle `geometry::crop_region`.
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
        // Refuser franchement plutôt que rendre des places minuscules : une
        // mesure sur des fenêtres de 80x60 ne dirait rien du produit.
        let etroit = Rect { x: 0, y: 0, width: 640, height: 480 };
        assert_eq!(tuiles(etroit, 8), None);
    }

    #[test]
    fn le_nombre_de_places_demande_est_respecte_meme_si_la_grille_est_plus_large() {
        // Sept places tiennent dans une grille 3x3 : deux cases restent vides,
        // et on ne doit pas rendre neuf places pour autant.
        assert_eq!(tuiles(BUREAU, 7).unwrap().len(), 7);
    }
}
