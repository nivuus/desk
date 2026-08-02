//! Apparier une sortie virtuelle fraîchement créée à une sortie DXGI, puis y
//! poser la fenêtre.
//!
//! **Le pilote et DXGI ne parlent pas le même langage.** Le premier rend un
//! identifiant de cible qui lui appartient, le second énumère par
//! `(index_adaptateur, index_sortie)`. Aucune correspondance n'est exposée :
//! l'appariement se fait donc par dimensions et par élimination.
//!
//! **`GetDesc`/`DesktopCoordinates` est la source de vérité, jamais WMI** —
//! le champ WMI a été vu périmé de 68 s sur ce terrain, et la sortie virtuelle
//! y était annoncée 5120×1440 quand DXGI la mesurait 3413×960 (facteur DPI de
//! 1,5). Un placement calculé sur la valeur WMI serait décalé d'autant.

use crate::geometry::Rect;
// `crate::sortie_dxgi`, pas `crate::capture` : `capture` est `#![cfg(windows)]`
// dans son ensemble et n'existe pas du tout à la compilation sur l'hôte Linux
// — voir le commentaire de tête de `sortie_dxgi.rs`. `capture.rs` réexporte ce
// même type sous `crate::capture::SortieDxgi` pour le code Windows.
use crate::sortie_dxgi::SortieDxgi;

/// Tolérance de position et de taille, en pixels, avant de replacer.
///
/// Les bordures invisibles de DWM décalent couramment `GetWindowRect` de
/// quelques pixels par rapport à ce que `SetWindowPos` a demandé. Sans
/// tolérance, le superviseur replacerait la fenêtre à chaque tour de boucle.
///
/// **Quatre, et non deux** : la valeur retenue prend une marge délibérée
/// au-delà du décalage habituel — un écart de quatre pixels sur une fenêtre
/// plein cadre est invisible, là où un replacement en boucle ne l'est pas. (Le
/// commentaire disait « un ou deux pixels » face à une constante à 4 ; c'est le
/// texte qui était en retard, la constante est celle qu'on veut.)
///
/// Cette même tolérance sert désormais aussi à l'appariement d'une sortie
/// fraîchement créée (`sortie_par_dimensions`) : elle doit être déclarée avant
/// cette fonction dans le fichier.
const TOLERANCE_PX: i64 = 4;

/// Vrai si deux tailles se correspondent à `TOLERANCE_PX` près.
///
/// **Le même prédicat que `sortie_par_dimensions`, et c'est le point.** Une
/// sortie appariée à la création doit être jugée réutilisable à la relance
/// (`table::viewport_recu`) : deux tolérances distinctes feraient détruire
/// puis recréer une sortie parfaitement bonne — exactement la recréation que
/// le sous-bloc D3 existe pour supprimer.
pub fn taille_compatible(a: (u32, u32), b: (u32, u32)) -> bool {
    let proche = |x: u32, y: u32| (x as i64 - y as i64).abs() <= TOLERANCE_PX;
    proche(a.0, b.0) && proche(a.1, b.1)
}

/// Sortie DXGI correspondant à des dimensions demandées, parmi celles qui ne
/// sont pas déjà attribuées.
///
/// **Tolérante de `TOLERANCE_PX`, et pas davantage.** L'égalité stricte était
/// le choix initial, pour ne pas masquer le facteur d'échelle décrit en tête de
/// module ; la recette D1 a montré qu'elle rendait l'ouverture impossible sur
/// une course de rattachement de quelques pixels (1280×713 rendue 1280×720).
/// La tolérance retenue est celle du replacement — quatre pixels — très loin
/// du facteur 1,5 qui reste, lui, refusé.
///
/// `deja_prises` désigne par NOM DXGI (`\\.\DISPLAYn`), stable, et non plus
/// par un couple d'index d'énumération — positionnel, il change dès qu'une
/// sortie apparaît ou disparaît.
pub fn sortie_par_dimensions(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && taille_compatible((s.rect.width, s.rect.height), (largeur, hauteur))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}

/// Vrai si la fenêtre a quitté sa sortie ou changé de taille au point qu'il
/// faille la remettre en place.
///
/// C'est le cas que la spec §6 prévoit : une application peut se déplacer ou
/// se retailler d'elle-même, et une fenêtre qui déborde de sa sortie donne une
/// capture tronquée sans que rien ne le signale.
pub fn doit_etre_replacee(actuel: &Rect, cible: &Rect) -> bool {
    let ecart = |a: i64, b: i64| (a - b).abs() > TOLERANCE_PX;
    ecart(actuel.x as i64, cible.x as i64)
        || ecart(actuel.y as i64, cible.y as i64)
        || ecart(actuel.width as i64, cible.width as i64)
        || ecart(actuel.height as i64, cible.height as i64)
}

#[cfg(windows)]
mod win {
    use anyhow::{Context, Result};

    use super::*;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE, SW_SHOWNORMAL,
    };

    /// Pose la fenêtre sur la sortie et lui donne exactement sa taille.
    ///
    /// **Pas de maximisation.** `SW_MAXIMIZE` ferait adopter à la fenêtre la
    /// zone de travail du moniteur, barre des tâches déduite : l'image
    /// capturée ne remplirait alors pas la sortie, et le bas du flux serait
    /// une bande de bureau vide. On pose la taille exacte de la sortie.
    ///
    /// La fenêtre est d'abord restaurée : une fenêtre minimisée ou déjà
    /// maximisée ignore silencieusement `SetWindowPos`.
    pub fn poser(hwnd: HWND, cible: &Rect) -> Result<()> {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                cible.x,
                cible.y,
                cible.width as i32,
                cible.height as i32,
                // `SWP_NOACTIVATE` : poser une fenêtre ne doit pas voler le
                // premier plan à celle que l'utilisateur manipule.
                SWP_NOACTIVATE,
            )
            .context("SetWindowPos vers la sortie virtuelle")?;
        }
        Ok(())
    }

    /// Rectangle actuel de la fenêtre, en coordonnées du bureau virtuel.
    ///
    /// `GetWindowRect` et non `GetClientRect` : c'est la position dans
    /// l'espace du bureau qu'on compare à celle de la sortie, et
    /// `GetClientRect` rend un rectangle dont l'origine est toujours (0,0).
    pub fn rectangle_de(hwnd: HWND) -> Result<Rect> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut r = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut r) }.context("GetWindowRect")?;
        Ok(Rect {
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(0) as u32,
            height: (r.bottom - r.top).max(0) as u32,
        })
    }
}

#[cfg(windows)]
pub use win::{poser, rectangle_de};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn sortie(a: u32, s: u32, x: i32, l: u32, h: u32, attachee: bool) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: a,
            index_sortie: s,
            adaptateur: "NVIDIA".into(),
            nom_sortie: format!("\\\\.\\DISPLAY{s}"),
            attachee_au_bureau: attachee,
            rect: Rect { x, y: 0, width: l, height: h },
        }
    }

    #[test]
    fn trouve_la_sortie_aux_dimensions_demandees() {
        let toutes = vec![
            sortie(0, 0, 0, 2400, 1080, true),
            sortie(0, 1, 2400, 1600, 900, true),
        ];
        let trouvee = sortie_par_dimensions(&toutes, 1600, 900, &[]).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 1));
    }

    #[test]
    fn ignore_une_sortie_non_attachee() {
        // Une sortie créée mais que Windows n'a pas encore rattachée ne peut
        // rien afficher : la prendre donnerait une capture noire.
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, false)];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &[]).is_none());
    }

    #[test]
    fn ignore_une_sortie_deja_attribuee() {
        // Deux fenêtres au même viewport : sans ce filtre, la seconde se
        // verrait attribuer la sortie de la première, et les deux flux
        // montreraient la même image.
        let toutes = vec![
            sortie(0, 1, 2400, 1600, 900, true),
            sortie(0, 2, 4000, 1600, 900, true),
        ];
        let deja_prises = vec!["\\\\.\\DISPLAY1".to_string()];
        let trouvee = sortie_par_dimensions(&toutes, 1600, 900, &deja_prises).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 2));
    }

    #[test]
    fn ne_trouve_rien_quand_toutes_sont_prises() {
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, true)];
        let deja_prises = vec!["\\\\.\\DISPLAY1".to_string()];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &deja_prises).is_none());
    }

    #[test]
    fn n_apparie_pas_une_sortie_aux_mauvaises_dimensions() {
        // Le facteur d'échelle DPI a déjà produit un écart de 1,5 sur ce
        // terrain (5120x1440 annoncé, 3413x960 mesuré) : un appariement
        // approximatif rendrait ce piège invisible.
        let toutes = vec![sortie(0, 1, 2400, 1067, 600, true)];
        assert!(sortie_par_dimensions(&toutes, 1600, 900, &[]).is_none());
    }

    /// §3.1 de la recette D1 : une sortie créée à 1280×713 a été rendue par
    /// DXGI à 1280×720 une fois, puis à 1280×713 l'essai suivant. L'égalité
    /// stricte rendait alors l'ouverture de la fenêtre impossible.
    #[test]
    fn un_ecart_dans_la_tolerance_apparie_quand_meme() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1280, 717)];
        let trouvee = sortie_par_dimensions(&sorties, 1280, 720, &[]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY7".into()));
    }

    /// La tolérance ne doit pas avaler le facteur DPI de 1,5 que le dépôt a
    /// relevé sur une sortie virtuelle : c'est le piège que l'égalité stricte
    /// protégeait, et qu'il faut continuer de voir échouer.
    #[test]
    fn un_facteur_d_echelle_n_apparie_pas() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1920, 1080)];
        assert!(sortie_par_dimensions(&sorties, 1280, 720, &[]).is_none());
    }

    #[test]
    fn une_sortie_deja_prise_est_ignoree() {
        let sorties = vec![
            sortie_nommee("\\\\.\\DISPLAY7", 1280, 720),
            sortie_nommee("\\\\.\\DISPLAY8", 1280, 720),
        ];
        let trouvee =
            sortie_par_dimensions(&sorties, 1280, 720, &["\\\\.\\DISPLAY7".to_string()]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY8".into()));
    }

    // Distinct de `sortie` ci-dessus (qui fixe `nom_sortie` à partir de
    // `index_sortie`) : ces trois tests veulent un nom explicite pour vérifier
    // l'identité de la sortie appariée, pas seulement son existence.
    fn sortie_nommee(nom: &str, largeur: u32, hauteur: u32) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: 0,
            index_sortie: 0,
            adaptateur: "essai".into(),
            nom_sortie: nom.into(),
            attachee_au_bureau: true,
            rect: Rect { x: 0, y: 0, width: largeur, height: hauteur },
        }
    }

    #[test]
    fn une_fenetre_a_sa_place_n_est_pas_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        assert!(!doit_etre_replacee(&cible, &cible));
    }

    #[test]
    fn une_fenetre_deplacee_hors_de_sa_sortie_est_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let ailleurs = Rect { x: 100, y: 50, width: 1600, height: 900 };
        assert!(doit_etre_replacee(&ailleurs, &cible));
    }

    #[test]
    fn une_fenetre_retaillee_par_l_application_est_replacee() {
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let retaillee = Rect { x: 2400, y: 0, width: 800, height: 600 };
        assert!(doit_etre_replacee(&retaillee, &cible));
    }

    #[test]
    fn un_ecart_d_un_pixel_ne_declenche_pas_de_replacement() {
        // Les bordures invisibles de DWM décalent couramment le rectangle
        // rendu par `GetWindowRect` de un ou deux pixels. Sans tolérance, le
        // superviseur replacerait la fenêtre à chaque tour de boucle, en
        // boucle, et volerait le focus indéfiniment.
        let cible = Rect { x: 2400, y: 0, width: 1600, height: 900 };
        let presque = Rect { x: 2401, y: 1, width: 1599, height: 899 };
        assert!(!doit_etre_replacee(&presque, &cible));
    }
}

#[cfg(test)]
mod tests_taille {
    use super::*;

    #[test]
    fn une_taille_identique_est_compatible() {
        assert!(taille_compatible((1280, 720), (1280, 720)));
    }

    /// La course de rattachement de la recette D1 : la sortie est créée à
    /// 1280×713 et DXGI la rend à 1280×720 un essai sur deux. Quatre pixels
    /// de tolérance ne couvrent PAS cet écart de sept — c'est
    /// `viewport_recu` qui doit alors détruire et recréer, pas apparier à
    /// tort.
    #[test]
    fn un_ecart_de_sept_pixels_n_est_pas_compatible() {
        assert!(!taille_compatible((1280, 713), (1280, 720)));
    }

    #[test]
    fn un_ecart_de_quatre_pixels_est_compatible() {
        assert!(taille_compatible((1276, 716), (1280, 720)));
    }

    /// Le facteur DPI de 1,5 que `CLAUDE.md` documente sur une sortie
    /// virtuelle doit rester refusé : l'accepter ferait poser la fenêtre sur
    /// une texture aux mauvaises dimensions.
    #[test]
    fn le_facteur_dpi_reste_refuse() {
        assert!(!taille_compatible((1280, 720), (1920, 1080)));
    }
}
