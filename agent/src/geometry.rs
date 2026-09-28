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
    Some(Rect {
        x: left as i32,
        y: top as i32,
        width,
        height,
    })
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

    (
        clamp_normalized(normalized_x),
        clamp_normalized(normalized_y),
    )
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
mod tests;
#[cfg(test)]
mod tests_injection;
