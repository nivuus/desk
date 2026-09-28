//! The host tests of `placement` bearing on the TWO invisible frames
//! of a Windows window: DWM's fringe, and the border Windows paints.
//!
//! **Extracted from `placement/tests.rs` VERBATIM**, on August 31st, 2026, by a
//! DEDICATED task and BEFORE the addition it prepared — the original file
//! was at **485 lines**, hence fifteen from the ceiling, and the fix "a
//! DESIGNATED output is served whatever its size" had to put its
//! own cases there.
//!
//! ⚠️ **Here the extraction is strictly verbatim, and it is not
//! luck**: the two modules stay NESTED one notch (`mod lisere` in this
//! file-module), exactly as they were in `mod tests`. Their
//! `use super::super::*;` therefore still designates `placement`, without a single
//! import line having to move. Flattening them into top-level modules would have
//! required rewriting those two lines — the repository writes that an extraction is
//! never verbatim; this one is, because the form was chosen for it.
//!
//! No assertion, no comment was rewritten in the move.

mod lisere {
    use super::super::*;
    use crate::geometry::Rect;

    /// 🔴 **THE NUMBERS COME FROM THE PROBE, NOT FROM A CALCULATION ON WHAT THEY
    /// JUDGE.** Recorded in SESSION 1 on August 31st, 2026, through an `/it`
    /// scheduled task, on the owner's LIVE session while they were testing:
    ///
    /// ```text
    /// GetWindowRect = 1732x1032+1280+0   <- exactement la `retenue` du journal
    /// DWM frame     = 1718x1025+1287+0
    /// lisere : gauche=7 haut=0 droite=7 bas=7
    /// ```
    ///
    /// Both served windows returned **the same fringe**, on two
    /// different outputs.
    const MESURE: Lisere = Lisere {
        gauche: 7,
        haut: 0,
        droite: 7,
        bas: 7,
    };

    /// The target as the supervisor computes it: origin of the output
    /// `\\.\DISPLAY6` (+1280+0), retained size `1732x1032` — both read in
    /// the product's log.
    fn cible_mesuree() -> Rect {
        Rect {
            x: 1280,
            y: 0,
            width: 1732,
            height: 1032,
        }
    }

    /// What `SetWindowPos` must receive for the eye to see exactly the
    /// target: the target inflated by the fringe, shifted by its top-left corner.
    #[test]
    fn le_rectangle_pose_est_la_cible_gonflee_du_lisere() {
        let pose = rect_a_poser(&cible_mesuree(), MESURE);
        assert_eq!(
            pose,
            Rect {
                x: 1273,
                y: 0,
                width: 1746,
                height: 1039
            }
        );
    }

    /// 🔴 **THE PROPERTY THAT MATTERS, AND IT IS A ROUND TRIP**: what
    /// DWM will return of the placed rectangle must be **exactly** the target. It is
    /// what guarantees that no desktop pixel remains in the image.
    ///
    /// The "DWM simulation" is not begging the question: it
    /// applies the DEFINITION of the fringe (visible frame = raw shrunk on
    /// each side), as the probe measured it, and not an inversion of
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

    /// 🔴 **THE GUARD AGAINST 1 Hz OSCILLATION.** `rectangle_de` now returns
    /// the VISIBLE frame, and the periodic check compares it to the
    /// target: if the two halves of the fix did not go together,
    /// the gap would be permanent and the window would be placed again **every
    /// second**. This test is the pure version of that loop.
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
        assert!(
            !doit_etre_replacee(&visible, &cible),
            "replacement en boucle"
        );
        // …and the counter-example: if we compared the RAW rectangle to the
        // target — what `rectangle_de` did before this fix —, the
        // check would replace indefinitely.
        assert!(
            doit_etre_replacee(&pose, &cible),
            "le brut DOIT differer de la cible, sinon ce test ne prouve rien"
        );
    }

    /// ⚠️ **THE FRINGE IS NOT SYMMETRIC, AND ASSUMING IT WOULD SHIFT
    /// THE IMAGE.** `top = 0` because the title bar is painted. This test
    /// uses four DIFFERENT values so that any confusion between two
    /// sides turns it red — a `left` used in place of the `top`
    /// would go unnoticed with the measured fringe, where three sides out of four
    /// are 7.
    #[test]
    fn chaque_cote_du_lisere_est_honore_separement() {
        let l = Lisere {
            gauche: 3,
            haut: 5,
            droite: 11,
            bas: 17,
        };
        let pose = rect_a_poser(
            &Rect {
                x: 100,
                y: 200,
                width: 1000,
                height: 500,
            },
            l,
        );
        assert_eq!(
            pose,
            Rect {
                x: 97,
                y: 195,
                width: 1014,
                height: 522
            }
        );
    }

    /// The fallback: DWM refuses, the fringe is zero, and we get **exactly
    /// the behaviour from before this fix**. A correction that could
    /// not disarm itself would be worse than the defect.
    #[test]
    fn un_lisere_nul_rend_la_cible_telle_quelle() {
        let cible = cible_mesuree();
        assert_eq!(rect_a_poser(&cible, Lisere::NUL), cible);
        assert_eq!(taille_a_poser((1732, 1032), Lisere::NUL), (1732, 1032));
        assert!(Lisere::NUL.est_nul());
        assert!(!MESURE.est_nul());
    }

    /// The counterpart for the CAPTURER path, which resizes without moving.
    #[test]
    fn la_taille_posee_est_la_taille_visible_gonflee_du_lisere() {
        assert_eq!(taille_a_poser((1732, 1032), MESURE), (1746, 1039));
    }

    /// An aberrant fringe — DWM returning anything — must not
    /// overflow the arithmetic and produce a tiny window.
    #[test]
    fn un_lisere_aberrant_ne_fait_pas_deborder() {
        let fou = Lisere {
            gauche: -100_000,
            haut: 0,
            droite: -100_000,
            bas: 0,
        };
        let pose = rect_a_poser(
            &Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            fou,
        );
        assert_eq!(
            pose.width, 0,
            "saturation vers le bas, jamais un repli par le haut"
        );
    }
}

mod bordure_peinte {
    use super::super::*;
    use crate::geometry::Rect;

    /// 🔴 **THE NUMBERS COME FROM THE IMAGE'S PIXELS, NOT FROM A CALCULATION.**
    /// Capture of the virtual output in session 1, August 31st, 2026, crop
    /// 1548×1032 — colours read on the edges and on their neighbours:
    ///
    /// ```text
    /// row 0       (TOP)    #494949 | row 1        #F3F3F3
    /// row 1031    (BOTTOM) #2F2F2F | row 1030     #F0F0F0
    /// column 0    (LEFT)   #2F2F2F | column 1     #FFFFFF
    /// column 1547 (RIGHT)  #2F2F2F | column 1546  #F0F0F0
    /// ```
    ///
    /// One dark pixel on all four edges, light just inside. And
    /// `GetSystemMetrics(SM_CXBORDER/SM_CYBORDER)` returns `(1, 1)` at 96 DPI:
    /// **the image measurement and the system metric agree**, which
    /// is what allows trusting the latter rather than writing `1`.
    const BORDURE: (i32, i32) = (1, 1);
    const DWM: Lisere = Lisere {
        gauche: 7,
        haut: 0,
        droite: 7,
        bas: 7,
    };

    #[test]
    fn l_enveloppe_ajoute_la_bordure_peinte_au_lisere_invisible() {
        assert_eq!(
            enveloppe(DWM, BORDURE),
            Lisere {
                gauche: 8,
                haut: 1,
                droite: 8,
                bas: 8
            }
        );
    }

    /// 🔴 **THE ROUND TRIP THAT HOLDS BOTH HALVES TOGETHER.** `poser` places
    /// at `crop + enveloppe`; `rectangle_de` returns `visible frame − border`.
    /// The result must be **exactly** the crop, otherwise the periodic check
    /// sees a permanent gap.
    #[test]
    fn poser_puis_relire_redonne_exactement_le_recadrage() {
        let crop = Rect {
            x: 1280,
            y: 0,
            width: 1548,
            height: 1032,
        };
        let pose = rect_a_poser(&crop, enveloppe(DWM, BORDURE));
        // What DWM will return of the placed rectangle: the placed one, shrunk by the
        // INVISIBLE fringe alone — the painted border, for its part, is part of the seen frame.
        let cadre_vu = Rect {
            x: pose.x + DWM.gauche,
            y: pose.y + DWM.haut,
            width: (pose.width as i32 - DWM.gauche - DWM.droite) as u32,
            height: (pose.height as i32 - DWM.haut - DWM.bas) as u32,
        };
        assert_eq!(sans_la_bordure(&cadre_vu, BORDURE), crop);
        assert!(!doit_etre_replacee(
            &sans_la_bordure(&cadre_vu, BORDURE),
            &crop
        ));
    }

    /// The painted border indeed falls **OUTSIDE** the crop: the visible frame
    /// overflows by exactly one pixel on each side, and that is where Windows
    /// paints its dark line.
    #[test]
    fn la_ligne_sombre_tombe_hors_du_recadrage() {
        let crop = Rect {
            x: 1280,
            y: 0,
            width: 1548,
            height: 1032,
        };
        let pose = rect_a_poser(&crop, enveloppe(DWM, BORDURE));
        let cadre_vu_gauche = pose.x + DWM.gauche;
        assert_eq!(
            crop.x - cadre_vu_gauche,
            BORDURE.0,
            "le bord peint doit etre EN DEHORS"
        );
        let cadre_vu_droite = pose.x + pose.width as i32 - DWM.droite;
        assert_eq!(cadre_vu_droite - (crop.x + crop.width as i32), BORDURE.0);
    }

    /// The fallback: no painted border → the envelope is the fringe alone, and
    /// `sans_la_bordure` is the identity. The previous behaviour, exactly.
    #[test]
    fn sans_bordure_peinte_on_retrouve_le_comportement_precedent() {
        assert_eq!(enveloppe(DWM, (0, 0)), DWM);
        let r = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        assert_eq!(sans_la_bordure(&r, (0, 0)), r);
    }

    /// An aberrant border must not overflow the arithmetic and
    /// return a giant crop through integer wraparound.
    #[test]
    fn une_bordure_aberrante_ne_fait_pas_deborder() {
        let r = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        assert_eq!(sans_la_bordure(&r, (100, 100)).width, 0);
    }
}
