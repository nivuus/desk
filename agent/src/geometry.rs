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
///
/// Toute l'arithmétique se fait en `i64` : `window.x` (`i32`) et
/// `window.width`/`window.height` (`u32`) tiennent tous les deux dans un
/// `i64` sans perte, et leur somme aussi (au pire `i32::MAX + u32::MAX`,
/// très loin de `i64::MAX`). Un calcul équivalent en `i32` déborderait dès
/// que `window.x` est proche de `i32::MAX` — atteignable en pratique côté
/// souris (tâche 12), où les coordonnées viennent d'événements navigateur et
/// ne sont pas garanties raisonnables comme le sont celles issues de
/// `client_rect_on_screen`.
pub fn crop_region(window: Rect, desktop_width: u32, desktop_height: u32) -> Option<Rect> {
    let desktop_width = desktop_width as i64;
    let desktop_height = desktop_height as i64;

    let window_left = window.x as i64;
    let window_top = window.y as i64;
    let window_right = window_left + window.width as i64;
    let window_bottom = window_top + window.height as i64;

    let left = window_left.max(0);
    let top = window_top.max(0);
    let right = window_right.min(desktop_width);
    let bottom = window_bottom.min(desktop_height);

    if right <= left || bottom <= top {
        return None;
    }

    let width = ((right - left) as u32) & !1;
    let height = ((bottom - top) as u32) & !1;
    if width < 2 || height < 2 {
        return None;
    }

    // `left`/`top` sont bornés par `desktop_width`/`desktop_height` (via le
    // `.min()` ci-dessus) : pour toute résolution d'écran réaliste (très en
    // deçà de `i32::MAX`), ils tiennent sans troncature dans `i32`.
    Some(Rect { x: left as i32, y: top as i32, width, height })
}

/// Vrai si les deux rectangles ont une intersection non vide (frontières qui
/// se touchent exclues).
///
/// Sert au mode diagnostic `CAPTURE_TEST` pour vérifier qu'une région de
/// contrôle placée à l'opposé du bureau ne chevauche pas, même
/// partiellement, la fenêtre réellement capturée : sans cette garantie, une
/// preuve par comparaison de pixel serait invalidée par construction (les
/// deux zones pourraient légitimement montrer la même chose).
pub fn rects_overlap(a: Rect, b: Rect) -> bool {
    let a_left = a.x as i64;
    let a_top = a.y as i64;
    let a_right = a_left + a.width as i64;
    let a_bottom = a_top + a.height as i64;

    let b_left = b.x as i64;
    let b_top = b.y as i64;
    let b_right = b_left + b.width as i64;
    let b_bottom = b_top + b.height as i64;

    a_left < b_right && b_left < a_right && a_top < b_bottom && b_top < a_bottom
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

    #[test]
    fn fenetre_plus_grande_que_l_ecran_dans_les_deux_dimensions() {
        let w = Rect { x: -500, y: -300, width: 5000, height: 4000 };
        let r = crop_region(w, 1920, 1080).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 1920, 1080));
    }

    #[test]
    fn fenetre_exactement_a_la_limite_de_l_ecran() {
        // Le coin inférieur droit de la fenêtre touche exactement le bord de
        // l'écran (1920, 1080) : intersection non vide, rien à borner.
        let w = Rect { x: 1520, y: 780, width: 400, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), Some(w));

        // Un pixel plus loin : la fenêtre commence exactement là où l'écran
        // s'arrête, intersection vide.
        let w = Rect { x: 1920, y: 0, width: 400, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
    }

    #[test]
    fn rejette_une_largeur_ou_une_hauteur_nulle() {
        let w = Rect { x: 100, y: 100, width: 0, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
        let w = Rect { x: 100, y: 100, width: 300, height: 0 };
        assert_eq!(crop_region(w, 1920, 1080), None);
    }

    #[test]
    fn ne_deborde_pas_sur_des_coordonnees_extremes() {
        // Reproduction exacte du panique signalé en revue : l'addition
        // `window.x + window.width` en i32 débordait pour x proche de
        // i32::MAX. Doit désormais être rejeté proprement, pas paniquer.
        let w = Rect { x: i32::MAX - 10, y: 0, width: 1000, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);

        // Symétrique côté bas/droite pour y.
        let w = Rect { x: 0, y: i32::MAX - 10, width: 300, height: 1000 };
        assert_eq!(crop_region(w, 1920, 1080), None);

        // Coordonnées minimales : x très négatif, ne doit pas non plus
        // déborder. La fenêtre reste entièrement hors écran (son bord droit,
        // x + width, est toujours très négatif), donc rejetée — mais
        // proprement, sans panique.
        let w = Rect { x: i32::MIN + 10, y: i32::MIN + 10, width: 1000, height: 300 };
        assert_eq!(crop_region(w, 1920, 1080), None);
    }

    #[test]
    fn gere_une_largeur_superieure_a_i32_max() {
        // Signalé en revue comme déjà correct — vérifié explicitement plutôt
        // que supposé : une largeur au-delà de i32::MAX ne doit ni paniquer,
        // ni se retrouver tronquée en une valeur négative par un `as i32`.
        let w = Rect { x: 0, y: 0, width: u32::MAX, height: 300 };
        let r = crop_region(w, 1920, 1080).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (0, 0, 1920, 300));
    }

    #[test]
    fn rects_overlap_detecte_une_intersection() {
        let a = Rect { x: 0, y: 0, width: 100, height: 100 };
        let b = Rect { x: 50, y: 50, width: 100, height: 100 };
        assert!(rects_overlap(a, b));
    }

    #[test]
    fn rects_overlap_rejette_des_rectangles_disjoints() {
        let a = Rect { x: 0, y: 0, width: 100, height: 100 };
        let b = Rect { x: 200, y: 200, width: 100, height: 100 };
        assert!(!rects_overlap(a, b));
    }

    #[test]
    fn rects_overlap_rejette_des_rectangles_qui_se_touchent_sans_se_recouvrir() {
        let a = Rect { x: 0, y: 0, width: 100, height: 100 };
        let b = Rect { x: 100, y: 0, width: 100, height: 100 };
        assert!(!rects_overlap(a, b));
    }
}
