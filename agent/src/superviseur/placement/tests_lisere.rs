//! Les tests d'hôte de `placement` qui portent sur les DEUX cadres invisibles
//! d'une fenêtre Windows : le lisère de DWM, et la bordure que Windows peint.
//!
//! **Extrait de `placement/tests.rs` VERBATIM**, le 31 août 2026, par une
//! tâche DÉDIÉE et AVANT l'addition qu'elle préparait — le fichier d'origine
//! était à **485 lignes**, donc à quinze du plafond, et le correctif « une
//! sortie DÉSIGNÉE est servie quelle que soit sa taille » devait y poser ses
//! propres cas.
//!
//! ⚠️ **Ici l'extraction est rigoureusement verbatim, et ce n'est pas une
//! chance** : les deux modules restent NESTED d'un cran (`mod lisere` dans ce
//! fichier-module), exactement comme ils l'étaient dans `mod tests`. Leur
//! `use super::super::*;` désigne donc toujours `placement`, sans qu'une seule
//! ligne d'import ait à bouger. Les aplatir en modules de premier niveau aurait
//! obligé à réécrire ces deux lignes — le dépôt écrit qu'une extraction n'est
//! jamais verbatim ; celle-ci l'est, parce que la forme a été choisie pour.
//!
//! Aucune assertion, aucun commentaire n'a été réécrit au déplacement.

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
