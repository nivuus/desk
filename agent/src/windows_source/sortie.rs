//! Construction d'une `WindowsSource` sur une sortie DXGI entière.
//!
//! C'est le mode du sous-bloc D1 : une fenêtre par sortie virtuelle, donc
//! plus rien à recadrer — la sortie *est* la fenêtre.

use crate::geometry::Rect;

/// Région à capturer dans la texture d'une sortie dupliquée.
///
/// **Relative à la sortie, pas au bureau virtuel.** `DesktopCapture::sur_sortie`
/// rend une texture qui couvre cette sortie seule ; son origine dans l'espace
/// du bureau virtuel (par exemple x=2400 pour une sortie posée à droite du
/// bureau physique) n'y a aucun sens. Passer les coordonnées de bureau
/// donnerait une image décalée ou vide.
///
/// Les dimensions sont alignées sur des valeurs paires : l'encodeur NV12 les
/// exige, et une sortie virtuelle créée à une taille impaire par un viewport
/// impair est un cas réel.
pub fn region_de_sortie(largeur: u32, hauteur: u32) -> Option<Rect> {
    let largeur = largeur & !1;
    let hauteur = hauteur & !1;
    if largeur < 2 || hauteur < 2 {
        return None;
    }
    Some(Rect { x: 0, y: 0, width: largeur, height: hauteur })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_region_couvre_toute_la_sortie_a_partir_de_son_origine_propre() {
        assert_eq!(
            region_de_sortie(1600, 900),
            Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
        );
    }

    #[test]
    fn les_dimensions_impaires_sont_alignees_vers_le_bas() {
        assert_eq!(
            region_de_sortie(1601, 901),
            Some(Rect { x: 0, y: 0, width: 1600, height: 900 })
        );
    }

    #[test]
    fn une_sortie_degeneree_ne_donne_aucune_region() {
        assert_eq!(region_de_sortie(1, 900), None);
        assert_eq!(region_de_sortie(0, 0), None);
    }
}
