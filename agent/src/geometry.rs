//! Calculs géométriques partagés, indépendants de toute API système.

/// Rectangle en coordonnées écran, dimensions non signées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Intersecte le rectangle d'une fenêtre avec l'écran et aligne les dimensions
/// sur des valeurs paires.
///
/// L'alignement pair n'est pas cosmétique : l'encodeur H.264 travaille en
/// macroblocs et refuse les dimensions impaires en 4:2:0. Renvoie `None` si la
/// fenêtre est entièrement hors de l'écran ou si l'intersection est trop petite
/// pour être encodée.
pub fn crop_region(window: Rect, desktop_width: u32, desktop_height: u32) -> Option<Rect> {
    let left = window.x.max(0);
    let top = window.y.max(0);
    let right = (window.x + window.width as i32).min(desktop_width as i32);
    let bottom = (window.y + window.height as i32).min(desktop_height as i32);

    if right <= left || bottom <= top {
        return None;
    }

    let width = ((right - left) as u32) & !1;
    let height = ((bottom - top) as u32) & !1;
    if width < 2 || height < 2 {
        return None;
    }

    Some(Rect { x: left, y: top, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenetre_entierement_visible() {
        let w = Rect { x: 100, y: 50, width: 800, height: 600 };
        assert_eq!(crop_region(w, 1920, 1080), Some(w));
    }

    #[test]
    fn aligne_les_dimensions_impaires() {
        let w = Rect { x: 0, y: 0, width: 801, height: 601 };
        let r = crop_region(w, 1920, 1080).unwrap();
        assert_eq!((r.width, r.height), (800, 600));
    }

    #[test]
    fn borne_une_fenetre_qui_deborde_a_droite() {
        let w = Rect { x: 1800, y: 0, width: 400, height: 400 };
        let r = crop_region(w, 1920, 1080).unwrap();
        assert_eq!((r.x, r.width), (1800, 120));
    }

    #[test]
    fn borne_une_fenetre_a_coordonnees_negatives() {
        let w = Rect { x: -100, y: -50, width: 400, height: 300 };
        let r = crop_region(w, 1920, 1080).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 300, 250));
    }

    #[test]
    fn rejette_une_fenetre_hors_ecran() {
        let w = Rect { x: 5000, y: 0, width: 400, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
        let w = Rect { x: 0, y: -5000, width: 400, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
    }

    #[test]
    fn rejette_une_intersection_trop_petite() {
        let w = Rect { x: 1919, y: 0, width: 400, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
    }
}
