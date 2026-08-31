//! Les tests d'hôte de `superviseur::placement`.
//!
//! **Extrait de `placement.rs` VERBATIM.** Le lot 33 y a corrigé le
//! commentaire de `win::poser`, devenu faux (il justifiait l'absence de
//! `SW_MAXIMIZE` par un argument qui ne vaut que si le recadrage reste à la
//! taille de la sortie — ce qui n'est plus le cas), et le fichier s'est
//! retrouvé à **500 lignes EXACTEMENT**, c'est-à-dire à sa porte. Le dépôt a
//! payé six fois « la marge regagnée qu'on traite comme acquise » et neuf fois
//! le naufrage du 487 : on extrait plutôt que de laisser un fichier au
//! millimètre du plafond.
//!
//! ⚠️ **Cette extraction SUIT son addition, là où celle de
//! `windows_source/sortie.rs` la PRÉCÉDAIT** — et c'est dit plutôt que
//! maquillé. La correction de commentaire qui l'a rendue nécessaire est
//! arrivée en cours de lot, avec la mesure de la barre des tâches ; elle
//! n'était pas prévisible au moment où le plafond a été calculé.
//!
//! Aucune assertion, aucun commentaire n'a été réécrit au déplacement. Les
//! DEUX modules (`tests` et `tests_taille`) sont conservés distincts : ils
//! l'étaient, et les fondre aurait été une réécriture.

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

mod tests_taille {
    // ⚠️ `super::super::*` et non `super::*` : SEULE modification du
    // déplacement, et elle est mécanique. Ce module reste imbriqué (il l'était),
    // mais son parent n'est plus `placement` — c'est le module `tests` que ce
    // fichier EST désormais. Sans ce cran de plus, `taille_retenue` et
    // `sortie_assez_grande` ne se résoudraient pas.
    use super::super::*;

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
