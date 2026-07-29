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

/// Convertit une coordonnée normalisée sur la fenêtre en coordonnée normalisée
/// sur le bureau virtuel, seule forme acceptée par `SendInput` en mode absolu
/// (tâche 12).
///
/// `x`/`y` sont dans `0..=65535` relativement à la zone client de `window`.
/// Le résultat est dans `0..=65535` relativement à `desktop`. Le calcul
/// compose deux passages : coordonnée normalisée → pixel écran (via
/// `window`), puis pixel écran → coordonnée normalisée sur le bureau (via
/// `desktop`).
///
/// Arithmétique en `f64` plutôt qu'en entier, à dessein : `window.x`/
/// `desktop.x` (`i32`) et `window.width`/`desktop.width` (`u32`) tiennent
/// tous exactement dans la mantisse 52 bits d'un `f64` (le plus grand, tout
/// `u32`, tient sur 32 bits), donc aucune perte de précision — et
/// contrairement à une multiplication en `i32`/`i64`, une valeur `f64` ne
/// panique jamais par débordement : au pire elle sature vers l'infini, et la
/// conversion finale `as i32` sur un flottant hors bornes sature elle aussi
/// (comportement garanti par Rust depuis la 1.45) plutôt que de produire un
/// résultat indéfini. Le `.clamp(0.0, 65535.0)` avant conversion couvre donc
/// à la fois les débordements représentables et les cas déjà dans les bornes.
pub fn to_virtual_desktop(x: u16, y: u16, window: Rect, desktop: Rect) -> (i32, i32) {
    // Position en pixels écran, au centre du pixel visé.
    let screen_x = window.x as f64 + (x as f64 / 65535.0) * window.width as f64;
    let screen_y = window.y as f64 + (y as f64 / 65535.0) * window.height as f64;

    // `.max(1.0)` évite toute division par zéro pour un bureau dégénéré
    // (largeur ou hauteur nulle) sans avoir à traiter ce cas séparément.
    let width = (desktop.width as f64).max(1.0);
    let height = (desktop.height as f64).max(1.0);
    let normalized_x = ((screen_x - desktop.x as f64) / width * 65535.0).round();
    let normalized_y = ((screen_y - desktop.y as f64) / height * 65535.0).round();

    (clamp_normalized(normalized_x), clamp_normalized(normalized_y))
}

/// Borne une coordonnée normalisée dans `0..=65535`, y compris pour un
/// flottant déjà hors de portée d'un `i32` (voir la note de
/// `to_virtual_desktop` sur la saturation des conversions `as`).
fn clamp_normalized(value: f64) -> i32 {
    value.clamp(0.0, 65535.0) as i32
}

/// Comme [`to_virtual_desktop`], mais en mappant sur la région **réellement
/// montrée au client** plutôt que sur la zone client complète.
///
/// La distinction n'est pas théorique. La capture encode
/// `crop_region(window, …)`, c'est-à-dire l'intersection de la fenêtre avec
/// l'écran ; le navigateur normalise donc ses coordonnées sur cette
/// intersection. Mapper l'injection sur la zone client entière fait dériver le
/// pointeur de tout ce qui dépasse : constaté le 29/07/2026 avec une zone
/// client de 1178 px pour un bureau de 1080, soit 98 px d'erreur au bas de
/// l'image et zéro en haut.
///
/// Les deux fonctions doivent donc rester appelées avec la même région. Rend
/// `None` quand la fenêtre est entièrement hors de l'écran — il n'y a alors
/// aucune image, donc aucune coordonnée à convertir.
pub fn to_virtual_desktop_visible(
    x: u16,
    y: u16,
    window: Rect,
    desktop: Rect,
) -> Option<(i32, i32)> {
    let visible = crop_region(window, desktop.width, desktop.height)?;
    Some(to_virtual_desktop(x, y, visible, desktop))
}

/// Borne une taille demandée pour que la fenêtre, dont le coin haut-gauche ne
/// bouge pas (`SWP_NOMOVE`), tienne entièrement dans le bureau.
///
/// Sans ce bornage, un viewport client plus haut que le bureau de la VM
/// produit une fenêtre qui dépasse : la capture la rogne, l'image prend un
/// rapport d'aspect que le conteneur du navigateur n'a pas — d'où des bandes
/// noires — et la partie basse de l'application devient inatteignable.
///
/// Le plancher de 2 px n'est pas cosmétique : `crop_region` refuse toute
/// région plus petite, et une taille nulle ferait échouer la capture.
pub fn borner_au_bureau(
    origin_x: i32,
    origin_y: i32,
    width: u32,
    height: u32,
    desktop_width: u32,
    desktop_height: u32,
) -> (u32, u32) {
    // Une origine négative laisse au contraire PLUS de place vers le bas et la
    // droite : `max(0)` évite d'en conclure une taille négative, `saturating_sub`
    // évite de déborder pour une origine au-delà du bureau.
    let disponible_x = (desktop_width as i64 - origin_x.max(0) as i64).max(2) as u32;
    let disponible_y = (desktop_height as i64 - origin_y.max(0) as i64).max(2) as u32;
    (width.min(disponible_x), height.min(disponible_y))
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

    const DESKTOP: Rect = Rect { x: 0, y: 0, width: 1920, height: 1080 };

    // --- Cohérence entre la région capturée et la région d'injection ---
    //
    // Cas réel relevé le 29/07/2026 : viewport client de 1187 px de haut,
    // bureau de 1080. La zone client de la fenêtre mesurait 1550×1178 à
    // l'origine (62, 0), donc 98 px sous l'écran. La capture encodait
    // 1550×1080 (l'intersection) pendant que l'injection mappait sur 1178 :
    // le clic dérivait de `t × 98` px, nul en haut, croissant vers le bas.

    #[test]
    fn le_milieu_de_l_image_vise_le_milieu_de_ce_qui_est_montre() {
        // Fenêtre débordant de 98 px sous un bureau de 1080.
        let window = Rect { x: 62, y: 0, width: 1550, height: 1178 };
        let desktop = Rect { x: 0, y: 0, width: 2400, height: 1080 };

        let (_, y) = to_virtual_desktop_visible(32768, 32768, window, desktop)
            .expect("la fenêtre est visible");

        // Le milieu de l'image montrée est le pixel écran 540, soit 32768 une
        // fois normalisé sur le bureau. Mapper sur la zone client complète
        // donnerait 589 px, soit 35742 — l'écart que voyait l'utilisateur.
        assert!((y - 32768).abs() <= 40, "y = {y}, attendu ~32768");
    }

    #[test]
    fn le_bas_de_l_image_vise_le_bas_de_ce_qui_est_montre() {
        let window = Rect { x: 62, y: 0, width: 1550, height: 1178 };
        let desktop = Rect { x: 0, y: 0, width: 2400, height: 1080 };

        let (_, y) = to_virtual_desktop_visible(0, 65535, window, desktop)
            .expect("la fenêtre est visible");

        assert_eq!(y, 65535, "le bas de l'image doit viser le bas du bureau");
    }

    #[test]
    fn l_axe_horizontal_reste_intact_quand_seul_le_bas_deborde() {
        // La largeur ne déborde pas : le mapping horizontal ne doit pas bouger.
        let window = Rect { x: 62, y: 0, width: 1550, height: 1178 };
        let desktop = Rect { x: 0, y: 0, width: 2400, height: 1080 };

        let (avec, _) = to_virtual_desktop_visible(32768, 0, window, desktop).unwrap();
        let (sans, _) = to_virtual_desktop(32768, 0, window, desktop);
        assert_eq!(avec, sans);
    }

    #[test]
    fn une_fenetre_entierement_visible_est_mappee_a_l_identique() {
        // Sans débordement, la correction ne doit rien changer : c'est ce qui
        // rendait le défaut invisible jusqu'ici.
        let window = Rect { x: 100, y: 50, width: 800, height: 600 };
        assert_eq!(
            to_virtual_desktop_visible(12345, 54321, window, DESKTOP),
            Some(to_virtual_desktop(12345, 54321, window, DESKTOP))
        );
    }

    #[test]
    fn une_fenetre_hors_ecran_ne_produit_aucune_coordonnee() {
        let window = Rect { x: 5000, y: 0, width: 400, height: 300 };
        assert_eq!(to_virtual_desktop_visible(0, 0, window, DESKTOP), None);
    }

    // --- Bornage du redimensionnement ---

    #[test]
    fn borne_une_hauteur_qui_depasserait_le_bas_du_bureau() {
        // Le cas réel : 1187 demandés depuis un viewport plus haut que le
        // bureau de la VM.
        assert_eq!(borner_au_bureau(62, 0, 1550, 1187, 2400, 1080), (1550, 1080));
    }

    #[test]
    fn tient_compte_de_l_origine_de_la_fenetre() {
        // Fenêtre déjà descendue de 100 px : il ne lui reste que 980.
        assert_eq!(borner_au_bureau(0, 100, 800, 1187, 2400, 1080), (800, 980));
    }

    #[test]
    fn ne_touche_pas_a_une_taille_qui_tient_deja() {
        assert_eq!(borner_au_bureau(62, 0, 1550, 900, 2400, 1080), (1550, 900));
    }

    #[test]
    fn borne_aussi_la_largeur() {
        assert_eq!(borner_au_bureau(2000, 0, 800, 500, 2400, 1080), (400, 500));
    }

    #[test]
    fn une_origine_negative_ne_produit_pas_une_taille_absurde() {
        // Fenêtre dont le coin haut-gauche est hors écran : le bornage ne doit
        // ni déborder, ni rendre une taille nulle qui ferait échouer la capture.
        let (w, h) = borner_au_bureau(-500, -300, 800, 600, 2400, 1080);
        assert!(w > 0 && h > 0, "taille = {w}x{h}");
        assert!(w <= 2400 && h <= 1080, "taille = {w}x{h}");
    }

    #[test]
    fn coin_superieur_gauche_d_une_fenetre_a_l_origine() {
        let window = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(0, 0, window, DESKTOP), (0, 0));
    }

    #[test]
    fn coin_inferieur_droit_d_une_fenetre_plein_ecran() {
        let window = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(65535, 65535, window, DESKTOP), (65535, 65535));
    }

    #[test]
    fn centre_d_une_fenetre_decalee() {
        // Fenêtre de 960×540 placée au centre : son centre est celui de l'écran.
        let window = Rect { x: 480, y: 270, width: 960, height: 540 };
        let (x, y) = to_virtual_desktop(32768, 32768, window, DESKTOP);
        assert!((x - 32768).abs() <= 40, "x = {x}");
        assert!((y - 32768).abs() <= 40, "y = {y}");
    }

    #[test]
    fn origine_d_une_fenetre_decalee() {
        let window = Rect { x: 960, y: 540, width: 960, height: 540 };
        let (x, y) = to_virtual_desktop(0, 0, window, DESKTOP);
        assert_eq!((x, y), (32768, 32768));
    }

    #[test]
    fn borne_les_debordements_sur_un_bureau_multi_ecrans() {
        // Bureau virtuel commençant en coordonnées négatives (écran à gauche).
        let desktop = Rect { x: -1920, y: 0, width: 3840, height: 1080 };
        let window = Rect { x: -1920, y: 0, width: 1920, height: 1080 };
        assert_eq!(to_virtual_desktop(0, 0, window, desktop), (0, 0));
        let (x, _) = to_virtual_desktop(65535, 0, window, desktop);
        assert!((x - 32768).abs() <= 40, "x = {x}");
    }

    #[test]
    fn ne_divise_jamais_par_zero() {
        let degenerate = Rect { x: 0, y: 0, width: 0, height: 0 };
        let (x, y) = to_virtual_desktop(32768, 32768, degenerate, degenerate);
        assert!((0..=65535).contains(&x) && (0..=65535).contains(&y));
    }

    #[test]
    fn ne_deborde_ni_ne_panique_sur_des_coordonnees_extremes() {
        // Même esprit que `gere_une_largeur_superieure_a_i32_max` pour
        // `crop_region` : une fenêtre aux coordonnées ou dimensions extrêmes
        // (jamais produites par `client_rect_on_screen` en pratique, mais pas
        // structurellement impossibles) ne doit ni paniquer, ni sortir de
        // `0..=65535`.
        let window = Rect { x: i32::MAX - 10, y: i32::MIN + 10, width: u32::MAX, height: u32::MAX };
        let desktop = Rect { x: 0, y: 0, width: 1, height: 1 };
        let (x, y) = to_virtual_desktop(65535, 0, window, desktop);
        assert!((0..=65535).contains(&x), "x = {x}");
        assert!((0..=65535).contains(&y), "y = {y}");

        // Bureau lui-même dégénéré à l'extrême, combiné à une fenêtre normale.
        let window = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let desktop = Rect { x: i32::MIN + 10, y: i32::MAX - 10, width: u32::MAX, height: 0 };
        let (x, y) = to_virtual_desktop(0, 65535, window, desktop);
        assert!((0..=65535).contains(&x), "x = {x}");
        assert!((0..=65535).contains(&y), "y = {y}");
    }
}
