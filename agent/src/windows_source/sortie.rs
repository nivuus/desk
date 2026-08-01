//! Construction d'une `WindowsSource` sur une sortie DXGI entière, et le
//! discriminant de mode qui distingue ce cas de l'autre.
//!
//! C'est le mode du sous-bloc D1 : une fenêtre par sortie virtuelle, donc
//! plus rien à recadrer — la sortie *est* la fenêtre.
//!
//! Deux moitiés, séparées par un `#[cfg(windows)]` en milieu de fichier :
//! au-dessus, le calcul pur de région et `ModeCapture`, tous deux testables sur
//! l'hôte Linux ; en dessous, le constructeur réel, qui manipule des types COM.

use crate::geometry::Rect;

/// Ce que la source capture — donc ce qu'un redimensionnement demandé par le
/// navigateur doit faire.
///
/// **Le discriminant manquait, et son absence était un défaut de sûreté.**
/// `WindowsSource::new` et `WindowsSource::sur_sortie` convergeaient tous deux
/// sur le même assemblage, sans rien retenir de leur différence : `resize`
/// appliquait donc le chemin « recadrage de fenêtre » y compris à une source
/// qui capture une sortie entière. Sur ce chemin-là, la fenêtre était
/// rétrécie (elle quittait sa sortie virtuelle, que le contrôle périodique du
/// superviseur tentait aussitôt de rattraper), la duplication de la sortie
/// était relâchée, et `DesktopCapture::new()` dupliquait **le bureau physique
/// primaire** — après quoi le repli de secours installait cette duplication-là
/// avec une région calculée pour la sortie virtuelle. La fenêtre du navigateur
/// affichait alors un coin du bureau réel de la VM à la place de son
/// application, pour un seul `warn!`. En mono-fenêtre ce repli était
/// acceptable ; en multi-fenêtres il fait fuir le contenu d'un moniteur vers
/// la session d'autrui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCapture {
    /// Le bureau entier est capturé, puis recadré sur la fenêtre. La fenêtre
    /// est redimensionnable et la région de recadrage suit sa taille : c'est
    /// le mode mono-fenêtre historique.
    FenetreRecadree,
    /// Une sortie DXGI entière est capturée : la sortie **est** la fenêtre.
    /// Mode du sous-bloc D1.
    SortieEntiere,
}

impl ModeCapture {
    /// Vrai si un redimensionnement doit réellement retailler la fenêtre et
    /// reconstruire la chaîne de capture.
    ///
    /// Faux en `SortieEntiere` : la spec §3.3 a tranché que le
    /// redimensionnement d'une fenêtre déjà ouverte est **hors périmètre de
    /// D1** — le pilote SudoVDA n'expose aucun changement de mode (aucun
    /// `SET_MODE` parmi ses six IOCTL), donc la sortie ne peut pas suivre. La
    /// seule issue correcte est de ne rien faire ; l'adaptation réseau, qui
    /// change la taille d'**encodage** et non celle de la source
    /// (`set_encode_size`), continue de fonctionner sans passer par ici.
    pub fn redimensionne_la_fenetre(self) -> bool {
        matches!(self, ModeCapture::FenetreRecadree)
    }
}

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

    /// Le défaut C1 en une ligne : c'est ce booléen qui empêche `resize` de
    /// retailler une fenêtre qui a sa propre sortie, donc de relâcher la
    /// duplication de cette sortie et de lui substituer celle du bureau
    /// physique.
    #[test]
    fn seul_le_mode_recadre_redimensionne_la_fenetre() {
        assert!(ModeCapture::FenetreRecadree.redimensionne_la_fenetre());
        assert!(!ModeCapture::SortieEntiere.redimensionne_la_fenetre());
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
// accès aux champs de `WindowsSource` depuis ici.
//
// **C'est pourquoi `sur_sortie` n'assemble PAS le littéral `Self { … }`
// lui-même** et passe par `WindowsSource::depuis_pieces`, restée dans
// `windows_source.rs` en `pub(crate) fn`. Une première version avait fait
// l'inverse — migrer aussi `depuis_pieces` — ce qui obligeait à ouvrir les
// TREIZE champs de `WindowsSource` en `pub(crate)`, dont `capture` et `fatal`
// qui portent un invariant inter-champs documenté comme fragile (voir leurs
// commentaires). Un appel `pub(crate)` à une fonction unique coûte une ligne
// et n'ouvre rien.
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
            // Le discriminant qui manquait : sans lui, `resize` retaillerait
            // cette fenêtre-ci et lui substituerait le bureau physique.
            ModeCapture::SortieEntiere,
        ))
    }
}
