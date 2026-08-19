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
/// fraîchement créée (`sortie_assez_grande`) : elle doit être déclarée avant
/// cette fonction dans le fichier.
const TOLERANCE_PX: i64 = 4;

/// Vrai si une sortie peut servir un viewport donné.
///
/// **Une inégalité, plus une égalité, et c'est tout le sous-bloc D10.** Une
/// sortie virtuelle ne naît PAS à la taille demandée : elle naît à la dernière
/// taille laissée au registre par un `CDS_UPDATEREGISTRY` antérieur (D8,
/// tâche 3bis — confirmé, reproduit, jamais expliqué). Sur cette VM le registre
/// est resté à 3840×2160, et l'égalité à quatre pixels près refusait donc
/// TOUTE sortie : le produit plafonnait à trois fenêtres, aux six exécutions
/// de la recette ③ de D9, sans exception.
///
/// Le produit n'écrit plus au registre depuis D9, mais **rien ne nettoie ce qui
/// y est déjà écrit** — et la portée du blocage (par GUID ou globale) reste
/// inconnue. D'où le choix de tolérer plutôt que de nettoyer : ainsi la
/// question devient **sans objet**, et non résolue.
///
/// La tolérance de `TOLERANCE_PX` est conservée dans le sens du MANQUE, pour la
/// course de rattachement relevée par la recette D1 (sortie créée à 1280×713,
/// rendue à 1280×720 un essai sur deux).
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool {
    let assez = |s: u32, d: u32| s as i64 + TOLERANCE_PX >= d as i64;
    assez(sortie.0, demandee.0) && assez(sortie.1, demandee.1)
}

/// La taille à laquelle la fenêtre est posée, et que la capture recadre.
///
/// `min` axe par axe, **sans préserver le rapport d'aspect** : on recadre une
/// texture, on ne la met pas à l'échelle. C'est l'inverse de
/// `windows_source_sortie::borner_a_la_taille_max`, qui redimensionne et doit
/// donc, lui, préserver ce rapport.
///
/// Dimensions paires (l'encodeur NV12 les exige) et jamais nulles (une boîte
/// vidéo repliée émet `(0, 0)`, cas réel relevé en D8).
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32) {
    let retenir = |d: u32, s: u32| (d.min(s).max(2)) & !1;
    (retenir(demandee.0, sortie.0), retenir(demandee.1, sortie.1))
}

/// Sortie DXGI capable de servir un viewport, parmi celles qui ne sont pas
/// déjà attribuées.
///
/// **`deja_prises` est ce qui empêche l'inégalité de tout casser.** Avec
/// l'égalité d'avant D10, deux fenêtres au même viewport se disputaient déjà
/// une sortie ; avec « au moins aussi grande », une seule grande sortie
/// conviendrait à TOUTES les fenêtres, et toutes montreraient la même image.
/// Le filtre désigne par NOM DXGI (`\\.\DISPLAYn`), stable, et non par un
/// couple d'index d'énumération, positionnel.
///
/// ⚠️ **L'appelant ne doit chercher QUE parmi les sorties APPARUES** (voir le
/// commentaire de `creation_sortie::creer_sortie`) : le viewport annoncé par le
/// navigateur peut égaler la résolution d'un moniteur PHYSIQUE, et l'inégalité
/// rend ce risque plus grand, pas moins — un moniteur 4K conviendrait
/// désormais à n'importe quel viewport.
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            s.attachee_au_bureau
                && sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur))
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
        // La première sortie doit rester INADÉQUATE sous l'inégalité de D10 —
        // sans quoi `.find()` s'arrêterait sur elle et le test ne prouverait
        // plus rien. Sa hauteur (800) est donc en dessous du viewport demandé
        // (900), exactement comme `n_apparie_pas_une_sortie_aux_mauvaises_
        // dimensions` reste inadéquate en restant plus petite sur les deux
        // axes : voir le rapport de la tâche 5, cette donnée n'est pas dans
        // le brief tel quel, qui rendait ce test rouge en l'état.
        let toutes = vec![
            sortie(0, 0, 0, 2400, 800, true),
            sortie(0, 1, 2400, 1600, 900, true),
        ];
        let trouvee = sortie_pour_viewport(&toutes, 1600, 900, &[]).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 1));
    }

    #[test]
    fn ignore_une_sortie_non_attachee() {
        // Une sortie créée mais que Windows n'a pas encore rattachée ne peut
        // rien afficher : la prendre donnerait une capture noire.
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, false)];
        assert!(sortie_pour_viewport(&toutes, 1600, 900, &[]).is_none());
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
        let trouvee = sortie_pour_viewport(&toutes, 1600, 900, &deja_prises).unwrap();
        assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 2));
    }

    #[test]
    fn ne_trouve_rien_quand_toutes_sont_prises() {
        let toutes = vec![sortie(0, 1, 2400, 1600, 900, true)];
        let deja_prises = vec!["\\\\.\\DISPLAY1".to_string()];
        assert!(sortie_pour_viewport(&toutes, 1600, 900, &deja_prises).is_none());
    }

    #[test]
    fn n_apparie_pas_une_sortie_aux_mauvaises_dimensions() {
        // ❌ **Ce commentaire disait : « Le facteur d'échelle DPI a déjà
        // produit un écart de 1,5 sur ce terrain (5120x1440 annoncé,
        // 3413x960 mesuré) : un appariement approximatif rendrait ce piège
        // invisible. » Le sous-bloc D10 l'a réfuté** — et le test
        // `un_facteur_d_echelle_est_desormais_recadre_et_non_refuse`, vingt
        // lignes plus bas, dit désormais le contraire. L'appariement EST
        // devenu délibérément approximatif (inégalité, plus tolérance de
        // 4 px) : un écart DPI n'est plus un motif de refus, il est recadré.
        // La protection a migré vers `taille_retenue`, qui borne la fenêtre à
        // ce que la sortie peut réellement porter. Relevé par la revue
        // transverse : c'est le seul commentaire de ce fichier que le
        // renommage `sortie_par_dimensions` → `sortie_pour_viewport` a laissé
        // intact sans le relire.
        //
        // Ce que ce test-ci exerce encore, et qui reste juste : une sortie
        // TROP PETITE sur un axe n'est toujours pas appariée.
        let toutes = vec![sortie(0, 1, 2400, 1067, 600, true)];
        assert!(sortie_pour_viewport(&toutes, 1600, 900, &[]).is_none());
    }

    /// §3.1 de la recette D1 : une sortie créée à 1280×713 a été rendue par
    /// DXGI à 1280×720 une fois, puis à 1280×713 l'essai suivant. L'égalité
    /// stricte rendait alors l'ouverture de la fenêtre impossible.
    #[test]
    fn un_ecart_dans_la_tolerance_apparie_quand_meme() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1280, 717)];
        let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &[]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY7".into()));
    }

    /// Le facteur DPI de 1,5 (5120×1440 annoncé par WMI, 3413×960 mesuré par
    /// DXGI) n'est plus un motif de REFUS : une sortie plus grande est
    /// recadrée. Ce qui protégeait contre lui — poser la fenêtre sur une
    /// texture aux mauvaises dimensions — est désormais assuré par
    /// `taille_retenue`, pas par l'appariement.
    #[test]
    fn un_facteur_d_echelle_est_desormais_recadre_et_non_refuse() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1920, 1080)];
        assert!(sortie_pour_viewport(&sorties, 1280, 720, &[]).is_some());
        assert_eq!(taille_retenue((1280, 720), (1920, 1080)), (1280, 720));
    }

    #[test]
    fn une_sortie_deja_prise_est_ignoree() {
        let sorties = vec![
            sortie_nommee("\\\\.\\DISPLAY7", 1280, 720),
            sortie_nommee("\\\\.\\DISPLAY8", 1280, 720),
        ];
        let trouvee =
            sortie_pour_viewport(&sorties, 1280, 720, &["\\\\.\\DISPLAY7".to_string()]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY8".into()));
    }

    /// Le cas produit de D9 : la sortie naît à 3840×2160 pour un viewport de
    /// 1280×720, et doit désormais être appariée.
    #[test]
    fn apparie_une_sortie_nee_beaucoup_plus_grande() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160)];
        let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &[]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY8".into()));
    }

    #[test]
    fn n_apparie_pas_une_sortie_trop_petite() {
        let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 1024, 576)];
        assert!(sortie_pour_viewport(&sorties, 1280, 720, &[]).is_none());
    }

    /// Le filtre sur les sorties DÉJÀ PRISES devient plus important, pas
    /// moins : avec une inégalité, une même grande sortie conviendrait à
    /// toutes les fenêtres, et toutes montreraient la même image.
    #[test]
    fn une_grande_sortie_deja_prise_n_est_pas_reattribuee() {
        let sorties = vec![
            sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160),
            sortie_nommee("\\\\.\\DISPLAY9", 3840, 2160),
        ];
        let trouvee =
            sortie_pour_viewport(&sorties, 1280, 720, &["\\\\.\\DISPLAY8".to_string()]);
        assert_eq!(trouvee.map(|s| s.nom_sortie), Some("\\\\.\\DISPLAY9".into()));
    }

    #[test]
    fn ignore_toujours_une_sortie_non_attachee() {
        // Une sortie que Windows n'a pas rattachée ne peut rien afficher :
        // la prendre donnerait une capture noire, quelle que soit sa taille.
        let toutes = vec![sortie(0, 1, 2400, 3840, 2160, false)];
        assert!(sortie_pour_viewport(&toutes, 1280, 720, &[]).is_none());
    }

    // Distinct de `sortie` ci-dessus (qui fixe `nom_sortie` à partir de
    // `index_sortie`) : les tests qui l'emploient veulent un nom explicite
    // pour vérifier l'identité de la sortie appariée, pas seulement son
    // existence. *(Ils étaient trois quand cette phrase a été écrite ; le
    // sous-bloc D10 en a ajouté trois de plus. Le compte n'est plus donné
    // ici — un nombre inscrit dans un commentaire dérive à la première
    // addition, et ce dépôt l'a payé six fois.)*
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

    /// Le fait produit de D9 : sur cette VM, les sorties naissent à 3840×2160
    /// parce que le registre y est resté. `taille_compatible` refusait, et le
    /// produit plafonnait à trois fenêtres.
    #[test]
    fn une_sortie_nee_trop_grande_convient_desormais() {
        assert!(sortie_assez_grande((3840, 2160), (1280, 720)));
    }

    #[test]
    fn une_sortie_nee_trop_petite_ne_convient_pas() {
        assert!(!sortie_assez_grande((1024, 576), (1280, 720)));
    }

    /// La course de rattachement de D1 (1280×713 rendue 1280×720) reste
    /// couverte : quatre pixels de tolérance, comme le replacement.
    #[test]
    fn un_manque_de_quatre_pixels_reste_accepte() {
        assert!(sortie_assez_grande((1276, 716), (1280, 720)));
    }

    #[test]
    fn un_manque_de_sept_pixels_est_refuse() {
        assert!(!sortie_assez_grande((1280, 713), (1280, 720)));
    }

    #[test]
    fn la_taille_retenue_recadre_une_sortie_trop_grande() {
        assert_eq!(taille_retenue((1280, 720), (3840, 2160)), (1280, 720));
    }

    /// Née trop petite, la sortie est honorée à ce qu'elle offre : le client
    /// met à l'échelle. Aucun cas ne rend plus une sortie au pilote pour une
    /// question de taille.
    #[test]
    fn la_taille_retenue_se_borne_a_la_sortie_quand_celle_ci_est_plus_petite() {
        assert_eq!(taille_retenue((1280, 720), (1024, 576)), (1024, 576));
    }

    /// L'encodeur NV12 exige des dimensions paires, et une sortie née à une
    /// taille impaire est un cas réel (viewport impair, D1).
    #[test]
    fn la_taille_retenue_est_toujours_paire_et_jamais_nulle() {
        assert_eq!(taille_retenue((1281, 721), (3840, 2160)), (1280, 720));
        assert_eq!(taille_retenue((0, 0), (1280, 720)), (2, 2));
    }

    /// Les axes se bornent SÉPARÉMENT : on recadre, on ne met pas à
    /// l'échelle, donc il n'y a aucun rapport d'aspect à préserver ici —
    /// contrairement à `borner_a_la_taille_max`, qui, lui, redimensionne.
    #[test]
    fn les_deux_axes_se_bornent_separement() {
        assert_eq!(taille_retenue((1920, 720), (1280, 2160)), (1280, 720));
    }
}
