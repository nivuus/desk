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

// Le bloc ci-dessous ne compile que sous Windows : il construit une
// `WindowsSource` réelle (types COM `HWND`/`DesktopCapture`/`H264Encoder`,
// tous eux-mêmes gated `#[cfg(windows)]`). `region_de_sortie` et ses tests
// restent AU-DESSUS de ce `cfg`, à portée du module, pour continuer de
// tourner sur l'hôte — voir la déclaration `#[path]` de ce fichier comme
// module `windows_source_sortie`, hors de tout `#[cfg(windows)]`, dans
// `main.rs`.
//
// Ce module (`windows_source_sortie`) est un FRÈRE de `windows_source`, pas
// un descendant (tous deux déclarés séparément à la racine du crate, voir
// `main.rs`) : la visibilité privée par défaut de Rust ne donnerait donc PAS
// accès aux champs de `WindowsSource` depuis ici. C'est pourquoi ces champs
// sont `pub(crate)` (voir leur commentaire dans `windows_source.rs`) plutôt
// que privés — le minimum qui permette au littéral `Self { … }` ci-dessous de
// compiler, sans les rendre publics hors du crate.
#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[cfg(windows)]
use crate::capture::DesktopCapture;
#[cfg(windows)]
use crate::encode::H264Encoder;
#[cfg(windows)]
use crate::windows_source::WindowsSource;

#[cfg(windows)]
impl WindowsSource {
    /// Construit une source capturant une sortie DXGI **entière**.
    ///
    /// Mode du sous-bloc D1 : la fenêtre a sa propre sortie virtuelle, il n'y
    /// a donc plus rien à recadrer ni aucune fenêtre à suivre. `hwnd` reste
    /// renseigné — l'injection d'entrée et le contrôle de vie en ont besoin —
    /// mais il ne sert plus au calcul de la région.
    pub fn sur_sortie(
        hwnd: HWND,
        index_adaptateur: u32,
        index_sortie: u32,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let capture = DesktopCapture::sur_sortie(index_adaptateur, index_sortie)?;
        let (dw, dh) = capture.desktop_size();
        let region = region_de_sortie(dw, dh).with_context(|| {
            format!("sortie {index_adaptateur}:{index_sortie} de dimensions inexploitables ({dw}x{dh})")
        })?;
        let (width, height) = (region.width, region.height);

        let mut encoder =
            H264Encoder::new(capture.device(), (width, height), (width, height), fps, bitrate)?;
        encoder.request_keyframe()?;

        Ok(Self::depuis_pieces(
            hwnd,
            capture,
            encoder,
            region,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
        ))
    }

    /// Assemblage final, partagé par les deux constructeurs (`WindowsSource::new`,
    /// dans `windows_source.rs`, et `sur_sortie` ci-dessus).
    ///
    /// Extrait pour que `new` (capture du bureau + recadrage de la fenêtre) et
    /// `sur_sortie` (capture d'une sortie entière) ne divergent pas sur
    /// l'initialisation des champs — ils ne diffèrent que par la façon
    /// d'obtenir la capture, l'encodeur et la région.
    ///
    /// `pub(crate)` plutôt que privée : appelée depuis `new`, dans le module
    /// frère `windows_source` (voir le commentaire de module ci-dessus) —
    /// le minimum de visibilité qui satisfait cet appel inter-module sans
    /// exposer la fonction hors du crate.
    pub(crate) fn depuis_pieces(
        hwnd: HWND,
        capture: DesktopCapture,
        encoder: H264Encoder,
        region: Rect,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Self {
        Self {
            hwnd,
            capture: Some(capture),
            region,
            encoder,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            last_pts_90k: None,
            fatal: false,
            encoder_warmed_up: false,
            ready: std::collections::VecDeque::new(),
        }
    }
}
