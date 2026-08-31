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

mod lisere {
    use super::super::*;
    use crate::geometry::Rect;

    /// 🔴 **LES NOMBRES VIENNENT DE LA SONDE, PAS D'UN CALCUL SUR CE QU'ILS
    /// JUGENT.** Relevés en SESSION 1 le 31 août 2026, par tâche planifiée
    /// `/it`, sur la session VIVANTE du propriétaire pendant qu'il testait :
    ///
    /// ```text
    /// GetWindowRect = 1732x1032+1280+0   <- exactement la `retenue` du journal
    /// DWM frame     = 1718x1025+1287+0
    /// lisere : gauche=7 haut=0 droite=7 bas=7
    /// ```
    ///
    /// Les deux fenêtres servies ont rendu **le même lisère**, sur deux
    /// sorties différentes.
    const MESURE: Lisere = Lisere { gauche: 7, haut: 0, droite: 7, bas: 7 };

    /// La cible telle que le superviseur la calcule : origine de la sortie
    /// `\\.\DISPLAY6` (+1280+0), taille retenue `1732x1032` — les deux lues au
    /// journal du produit.
    fn cible_mesuree() -> Rect {
        Rect { x: 1280, y: 0, width: 1732, height: 1032 }
    }

    /// Ce que `SetWindowPos` doit recevoir pour que l'œil voie exactement la
    /// cible : la cible gonflée du lisère, décalée de son coin haut-gauche.
    #[test]
    fn le_rectangle_pose_est_la_cible_gonflee_du_lisere() {
        let pose = rect_a_poser(&cible_mesuree(), MESURE);
        assert_eq!(pose, Rect { x: 1273, y: 0, width: 1746, height: 1039 });
    }

    /// 🔴 **LA PROPRIÉTÉ QUI COMPTE, ET ELLE EST UN ALLER-RETOUR** : ce que
    /// DWM rendra du rectangle posé doit être **exactement** la cible. C'est
    /// elle qui garantit qu'il ne reste aucun pixel de bureau dans l'image.
    ///
    /// La « simulation de DWM » n'est pas une pétition de principe : elle
    /// applique la DÉFINITION du lisère (cadre visible = brut rétréci de
    /// chaque côté), telle que la sonde l'a mesurée, et non une inversion de
    /// `rect_a_poser`.
    #[test]
    fn le_cadre_visible_du_rectangle_pose_redonne_exactement_la_cible() {
        let cible = cible_mesuree();
        let pose = rect_a_poser(&cible, MESURE);
        let visible = Rect {
            x: pose.x + MESURE.gauche,
            y: pose.y + MESURE.haut,
            width: (pose.width as i32 - MESURE.gauche - MESURE.droite) as u32,
            height: (pose.height as i32 - MESURE.haut - MESURE.bas) as u32,
        };
        assert_eq!(visible, cible);
    }

    /// 🔴 **LE GARDE CONTRE L'OSCILLATION À 1 Hz.** `rectangle_de` rend
    /// désormais le cadre VISIBLE, et le contrôle périodique le compare à la
    /// cible : si les deux moitiés du correctif n'allaient pas ensemble,
    /// l'écart serait permanent et la fenêtre serait reposée **chaque
    /// seconde**. Ce test est la version pure de cette boucle.
    #[test]
    fn apres_compensation_le_controle_periodique_ne_replace_plus() {
        let cible = cible_mesuree();
        let pose = rect_a_poser(&cible, MESURE);
        let visible = Rect {
            x: pose.x + MESURE.gauche,
            y: pose.y + MESURE.haut,
            width: (pose.width as i32 - MESURE.gauche - MESURE.droite) as u32,
            height: (pose.height as i32 - MESURE.haut - MESURE.bas) as u32,
        };
        assert!(!doit_etre_replacee(&visible, &cible), "replacement en boucle");
        // …et le contre-exemple : si l'on comparait le rectangle BRUT à la
        // cible — ce que faisait `rectangle_de` avant ce correctif —, le
        // contrôle replacerait indéfiniment.
        assert!(
            doit_etre_replacee(&pose, &cible),
            "le brut DOIT differer de la cible, sinon ce test ne prouve rien"
        );
    }

    /// ⚠️ **LE LISÈRE N'EST PAS SYMÉTRIQUE, ET LE SUPPOSER DÉCALERAIT
    /// L'IMAGE.** `haut = 0` parce que la barre de titre est peinte. Ce test
    /// emploie quatre valeurs DIFFÉRENTES pour que toute confusion entre deux
    /// côtés le fasse rougir — un `gauche` employé à la place du `haut`
    /// passerait inaperçu avec le lisère mesuré, où trois côtés sur quatre
    /// valent 7.
    #[test]
    fn chaque_cote_du_lisere_est_honore_separement() {
        let l = Lisere { gauche: 3, haut: 5, droite: 11, bas: 17 };
        let pose = rect_a_poser(&Rect { x: 100, y: 200, width: 1000, height: 500 }, l);
        assert_eq!(pose, Rect { x: 97, y: 195, width: 1014, height: 522 });
    }

    /// Le repli : DWM refuse, le lisère est nul, et l'on retrouve **exactement
    /// le comportement d'avant ce correctif**. Une correction qui ne saurait
    /// pas se désarmer serait pire que le défaut.
    #[test]
    fn un_lisere_nul_rend_la_cible_telle_quelle() {
        let cible = cible_mesuree();
        assert_eq!(rect_a_poser(&cible, Lisere::NUL), cible);
        assert_eq!(taille_a_poser((1732, 1032), Lisere::NUL), (1732, 1032));
        assert!(Lisere::NUL.est_nul());
        assert!(!MESURE.est_nul());
    }

    /// Le pendant pour le chemin du CAPTEUR, qui retaille sans déplacer.
    #[test]
    fn la_taille_posee_est_la_taille_visible_gonflee_du_lisere() {
        assert_eq!(taille_a_poser((1732, 1032), MESURE), (1746, 1039));
    }

    /// Un lisère aberrant — DWM qui rendrait n'importe quoi — ne doit pas
    /// faire déborder l'arithmétique et produire une fenêtre minuscule.
    #[test]
    fn un_lisere_aberrant_ne_fait_pas_deborder() {
        let fou = Lisere { gauche: -100_000, haut: 0, droite: -100_000, bas: 0 };
        let pose = rect_a_poser(&Rect { x: 0, y: 0, width: 100, height: 100 }, fou);
        assert_eq!(pose.width, 0, "saturation vers le bas, jamais un repli par le haut");
    }
}

mod bordure_peinte {
    use super::super::*;
    use crate::geometry::Rect;

    /// 🔴 **LES NOMBRES VIENNENT DES PIXELS DE L'IMAGE, PAS D'UN CALCUL.**
    /// Capture de la sortie virtuelle en session 1, 31 août 2026, recadrage
    /// 1548×1032 — couleurs relevées sur les bords et sur leurs voisines :
    ///
    /// ```text
    /// rangee 0    (HAUT)   #494949 | rangee 1     #F3F3F3
    /// rangee 1031 (BAS)    #2F2F2F | rangee 1030  #F0F0F0
    /// colonne 0   (GAUCHE) #2F2F2F | colonne 1    #FFFFFF
    /// colonne 1547(DROIT)  #2F2F2F | colonne 1546 #F0F0F0
    /// ```
    ///
    /// Un pixel sombre sur les quatre bords, clair juste en dedans. Et
    /// `GetSystemMetrics(SM_CXBORDER/SM_CYBORDER)` rend `(1, 1)` à 96 DPI :
    /// **la mesure de l'image et la métrique du système concordent**, ce qui
    /// est ce qui autorise à se fier à la seconde plutôt qu'à écrire `1`.
    const BORDURE: (i32, i32) = (1, 1);
    const DWM: Lisere = Lisere { gauche: 7, haut: 0, droite: 7, bas: 7 };

    #[test]
    fn l_enveloppe_ajoute_la_bordure_peinte_au_lisere_invisible() {
        assert_eq!(
            enveloppe(DWM, BORDURE),
            Lisere { gauche: 8, haut: 1, droite: 8, bas: 8 }
        );
    }

    /// 🔴 **L'ALLER-RETOUR QUI TIENT LES DEUX MOITIÉS ENSEMBLE.** `poser` pose
    /// à `crop + enveloppe` ; `rectangle_de` rend `cadre visible − bordure`.
    /// Le résultat doit être **exactement** le recadrage, sinon le contrôle
    /// périodique voit un écart permanent.
    #[test]
    fn poser_puis_relire_redonne_exactement_le_recadrage() {
        let crop = Rect { x: 1280, y: 0, width: 1548, height: 1032 };
        let pose = rect_a_poser(&crop, enveloppe(DWM, BORDURE));
        // Ce que DWM rendra du rectangle posé : le posé, rétréci du lisère
        // INVISIBLE seul — la bordure peinte, elle, fait partie du cadre vu.
        let cadre_vu = Rect {
            x: pose.x + DWM.gauche,
            y: pose.y + DWM.haut,
            width: (pose.width as i32 - DWM.gauche - DWM.droite) as u32,
            height: (pose.height as i32 - DWM.haut - DWM.bas) as u32,
        };
        assert_eq!(sans_la_bordure(&cadre_vu, BORDURE), crop);
        assert!(!doit_etre_replacee(&sans_la_bordure(&cadre_vu, BORDURE), &crop));
    }

    /// La bordure peinte tombe bien **HORS** du recadrage : le cadre visible
    /// déborde d'exactement un pixel de chaque côté, et c'est là que Windows
    /// peint sa ligne sombre.
    #[test]
    fn la_ligne_sombre_tombe_hors_du_recadrage() {
        let crop = Rect { x: 1280, y: 0, width: 1548, height: 1032 };
        let pose = rect_a_poser(&crop, enveloppe(DWM, BORDURE));
        let cadre_vu_gauche = pose.x + DWM.gauche;
        assert_eq!(crop.x - cadre_vu_gauche, BORDURE.0, "le bord peint doit etre EN DEHORS");
        let cadre_vu_droite = pose.x + pose.width as i32 - DWM.droite;
        assert_eq!(cadre_vu_droite - (crop.x + crop.width as i32), BORDURE.0);
    }

    /// Le repli : pas de bordure peinte → l'enveloppe est le lisère seul, et
    /// `sans_la_bordure` est l'identité. Le comportement d'avant, exactement.
    #[test]
    fn sans_bordure_peinte_on_retrouve_le_comportement_precedent() {
        assert_eq!(enveloppe(DWM, (0, 0)), DWM);
        let r = Rect { x: 10, y: 20, width: 100, height: 50 };
        assert_eq!(sans_la_bordure(&r, (0, 0)), r);
    }

    /// Une bordure aberrante ne doit pas faire déborder l'arithmétique et
    /// rendre un recadrage géant par repli entier.
    #[test]
    fn une_bordure_aberrante_ne_fait_pas_deborder() {
        let r = Rect { x: 0, y: 0, width: 10, height: 10 };
        assert_eq!(sans_la_bordure(&r, (100, 100)).width, 0);
    }
}
