//! Host tests of the PURE `accent` module.
//!
//! They live apart so that `accent.rs` keeps its margin: sub-block A1 is
//! new from end to end, and its pure module is the only workstation of the
//! repository that has no starting point (no occurrence of `HICON`,
//! `GetIconInfo`, `WM_GETICON` or `GCLP_HICON` existed before it).
//!
//! 🔴 **Each test names what makes it RED**, and the mutation reds have
//! been played on the harness of §6.4 of the plan — named copy, non-empty `diff`,
//! restoration from the copy, equal `sha256sum`.

use super::*;

/// Un pixel RGBA.
fn px(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
}

/// Builds an RGBA slice of `n` pixels from a list
/// `(couleur, répétitions)`, and returns `(octets, largeur, hauteur)` with a
/// consistent 1×n geometry — `dominante` refuses an inconsistent length.
fn image(motif: &[([u8; 4], usize)]) -> (Vec<u8>, u32, u32) {
    let mut octets = Vec::new();
    for (couleur, n) in motif {
        for _ in 0..*n {
            octets.extend_from_slice(couleur);
        }
    }
    let n = octets.len() / 4;
    (octets, n as u32, 1)
}

#[test]
fn une_icone_a_dominante_bleue_rend_du_bleu() {
    // RED: the intact tree before `dominante` exists.
    let (o, l, h) = image(&[
        (px(60, 110, 240, 255), 40), // bleu, majoritaire
        (px(250, 160, 40, 255), 10), // orange, minoritaire
    ]);
    let d = dominante(&o, l, h).expect("des pixels chromatiques survivent");
    assert_eq!(d, [60, 110, 240], "le seau le plus peuplé est le bleu");
}

#[test]
fn les_pixels_transparents_ne_comptent_pas() {
    // RED: remove the `ALPHA_MIN` filter — the transparent red, three times
    // more numerous, would beat the opaque blue.
    let (o, l, h) = image(&[
        (px(240, 40, 40, 0), 30),    // red, ENTIRELY TRANSPARENT
        (px(60, 110, 240, 255), 10), // bleu, opaque
    ]);
    let d = dominante(&o, l, h).expect("les pixels opaques survivent");
    assert_eq!(d, [60, 110, 240], "seuls les pixels opaques comptent");
}

#[test]
fn un_contour_noir_majoritaire_ne_gagne_pas() {
    // RED: remove the luminance filter — the dark outline, twice
    // as numerous, would beat the hue.
    //
    // ⚠️ The outline is a VERY DARK blue (5, 5, 60) and not a pure black:
    // a pure black would already be rejected by SATURATION, and the test would
    // then say nothing about the luminance filter it claims to test.
    const {
        assert!(
            (60u8 - 5u8) >= SATURATION_MIN,
            "le contour doit PASSER le filtre de saturation, sinon ce test \
             n'éprouve pas la luminance"
        )
    };
    let (o, l, h) = image(&[
        (px(5, 5, 60, 255), 40),     // contour sombre, majoritaire
        (px(250, 160, 40, 255), 20), // orange, minoritaire
    ]);
    let d = dominante(&o, l, h).expect("l'orange survit");
    assert_eq!(
        d,
        [250, 160, 40],
        "le contour sombre ne décide pas de la teinte"
    );
}

#[test]
fn une_icone_entierement_grise_rend_none() {
    // 🔴 THIS IS THE PROOF THAT `None` IS REACHABLE, hence that clause 2
    // is not an ornament.
    // RED: remove the saturation filter ⟹ `Some([128,128,128])`.
    let (o, l, h) = image(&[(px(128, 128, 128, 255), 64)]);
    assert_eq!(
        dominante(&o, l, h),
        None,
        "aucun pixel chromatique ne survit"
    );
}

#[test]
fn la_moyenne_du_seau_n_est_pas_le_centre_du_seau() {
    // RED: return the centre of the quantised bucket instead of the average.
    let (o, l, h) = image(&[(px(200, 40, 40, 255), 16)]);
    let d = dominante(&o, l, h).expect("le rouge survit");
    assert_eq!(
        d,
        [200, 40, 40],
        "la moyenne rend une teinte RÉELLE de l'image"
    );
    // The centre of the bucket would be (6·32+16, 1·32+16, 1·32+16) = (208, 48, 48).
    assert_ne!(d, [208, 48, 48], "et surtout PAS une teinte de la grille");
}

#[test]
fn une_geometrie_incoherente_rend_none() {
    // RED: remove the length guard ⟹ `Some`, on a slice whose
    // announced geometry does not describe its content.
    let o = vec![60u8, 110, 240, 255];
    assert_eq!(dominante(&o, 4, 4), None, "1 pixel pour 4×4 annoncés");
    assert_eq!(
        dominante(&[], 0, 0),
        None,
        "une image vide n'a pas de dominante"
    );
}

#[test]
fn en_hexa_rend_six_chiffres_minuscules() {
    // RED: `{:X}` instead of `{:x}`, or three digits instead of six.
    assert_eq!(
        en_hexa([0x0a, 0xbc, 0xde]),
        "#0abcde",
        "zéro de tête conservé"
    );
    assert_eq!(en_hexa([0xAB, 0xCD, 0xEF]), "#abcdef", "MINUSCULES");
    assert_eq!(en_hexa([0, 0, 0]), "#000000");
    assert_eq!(en_hexa([255, 255, 255]), "#ffffff");
}

#[test]
fn un_suivi_neuf_annonce_sa_premiere_lecture() {
    // RED: build `SuiviAccent` from the first reading, the
    // `SuiviBordure` way ⟹ the browser never receives the initial colour.
    let mut s = SuiviAccent::neuf();
    assert_eq!(s.observer("#7aa2f7"), Some("#7aa2f7".to_string()));
}

#[test]
fn un_suivi_n_annonce_pas_deux_fois_la_meme_couleur() {
    // 🔴 THIS IS THE HOST TEST OF CRITERION ④.
    // RED: remove the comparison ⟹ one announcement per tick, i.e. twelve per
    // minute and per window at `PERIODE_ACCENT = 5 s`.
    let mut s = SuiviAccent::neuf();
    assert_eq!(
        s.observer("#7aa2f7"),
        Some("#7aa2f7".to_string()),
        "la première"
    );
    assert_eq!(s.observer("#7aa2f7"), None, "la même : rien");
    assert_eq!(
        s.observer("#7aa2f7"),
        None,
        "encore la même : toujours rien"
    );
    assert_eq!(
        s.observer("#fa8c16"),
        Some("#fa8c16".to_string()),
        "un changement"
    );
    assert_eq!(s.observer("#fa8c16"), None, "puis plus rien");
}

// ═══════════════════════════════════════════════════════════════════════════
// `bgra_en_rgba` — THE CONVERSION SUB-PROJECT ① HAD LEFT UNTESTED
// (legacy RA1-6), moved down here by sub-block G5 because it needed it
// a SECOND time. Writing a second copy would have duplicated a rule that
// nobody checked.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bgra_en_rgba_echange_le_rouge_et_le_bleu() {
    // RED: `pixel.swap(0, 1)` or `pixel.swap(1, 2)` ⟹ the channels are
    // permuted differently, and the accent of a red icon would be announced blue.
    // A PLAUSIBLE and SILENT defect, which lived behind `#[cfg(windows)]`.
    let mut t = [0x11, 0x22, 0x33, 0x44];
    super::bgra_en_rgba(&mut t);
    assert_eq!(t, [0x33, 0x22, 0x11, 0x44]);
}

#[test]
fn bgra_en_rgba_ne_touche_jamais_l_alpha() {
    // 🔴 THIS IS THE CLAUSE THAT MATTERS: alpha is what the `ALPHA_MIN` filter
    // of `dominante` consumes. Swapping it with a colour channel would make that
    // filter absurd without any other test saying so.
    // RED: `pixel.swap(0, 3)` ⟹ this assertion fails, and so does the previous one.
    let mut t = [0x00, 0x00, 0xff, 0x07, 0xff, 0x00, 0x00, 0xf0];
    super::bgra_en_rgba(&mut t);
    assert_eq!(t[3], 0x07, "l'alpha du premier pixel");
    assert_eq!(t[7], 0xf0, "l'alpha du second");
}

#[test]
fn bgra_en_rgba_est_son_propre_inverse() {
    // A property, not an example: applied twice, it returns the
    // original. This is what forbids it from doing anything else on the way.
    let original: Vec<u8> = (0u8..=63).collect();
    let mut t = original.clone();
    super::bgra_en_rgba(&mut t);
    assert_ne!(t, original, "une seule passe DOIT changer quelque chose");
    super::bgra_en_rgba(&mut t);
    assert_eq!(t, original);
}

#[test]
fn bgra_en_rgba_laisse_un_reste_incomplet_tel_quel() {
    // ⚠️ This is not a convenient silence: a badly sized buffer is
    // refused further on by `dominante`, which compares the length with the product
    // `largeur × hauteur × 4`. It is written down so that nobody believes this
    // module validates a size.
    let mut t = [0x11, 0x22, 0x33, 0x44, 0xaa, 0xbb];
    super::bgra_en_rgba(&mut t);
    assert_eq!(t, [0x33, 0x22, 0x11, 0x44, 0xaa, 0xbb]);
}

#[test]
fn bgra_en_rgba_puis_dominante_rendent_la_couleur_reelle_du_bgra() {
    // 🔴 THE TEST THAT LINKS THE TWO, and it is the one that would have caught the
    // direction defect. The flat colour is `40 80 D0` IN BGRA, hence a warm hue
    // (0xD0, 0x80, 0x40) once converted, and a cold hue
    // (0x40, 0x80, 0xD0) if the conversion is forgotten.
    //
    // ⚠️ THE CHOICE OF COLOUR IS CONSTRAINED, AND THE FIRST DRAFT WAS WRONG:
    // a PURE RED flat colour (`00 00 D0` in BGRA) made `dominante` return `None`
    // IN BOTH DIRECTIONS — its luma is 23, below `LUMA_MIN = 32`.
    // The test failed on its FIXTURE, not on the code. This one survives both
    // readings: luma 144 and 117, saturation 144, both within the
    // bounds — so the gap measured below really is the one of the DIRECTION, and not
    // that of a pixel rejected on one side and not the other.
    let chaud_en_bgra: Vec<u8> = std::iter::repeat_n([0x40, 0x80, 0xd0, 0xff], 64)
        .flatten()
        .collect();

    let mut sans = chaud_en_bgra.clone();
    let lu_sans = super::dominante(&sans, 8, 8).expect("un aplat sature doit rendre une dominante");

    super::bgra_en_rgba(&mut sans);
    let lu_avec = super::dominante(&sans, 8, 8).expect("idem apres conversion");

    assert_eq!(
        lu_avec,
        [0xd0, 0x80, 0x40],
        "converti : la teinte CHAUDE, celle de l'image"
    );
    assert_eq!(
        lu_sans,
        [0x40, 0x80, 0xd0],
        "sans conversion : la teinte FROIDE, le rouge et le bleu echanges"
    );
    assert_ne!(
        lu_sans, lu_avec,
        "les deux lectures DIFFERENT : le sens compte"
    );
}
