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
/// `(colour, repetitions)`, and returns `(bytes, width, height)` with a
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
    let d = dominante(&o, l, h).expect("chromatic pixels survive");
    assert_eq!(
        d,
        [60, 110, 240],
        "the most populated bucket is the blue one"
    );
}

#[test]
fn les_pixels_transparents_ne_comptent_pas() {
    // RED: remove the `ALPHA_MIN` filter — the transparent red, three times
    // more numerous, would beat the opaque blue.
    let (o, l, h) = image(&[
        (px(240, 40, 40, 0), 30),    // red, ENTIRELY TRANSPARENT
        (px(60, 110, 240, 255), 10), // bleu, opaque
    ]);
    let d = dominante(&o, l, h).expect("the opaque pixels survive");
    assert_eq!(d, [60, 110, 240], "only opaque pixels count");
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
            "the outline must PASS the saturation filter, otherwise this test \
             does not exercise the luminance"
        )
    };
    let (o, l, h) = image(&[
        (px(5, 5, 60, 255), 40),     // contour sombre, majoritaire
        (px(250, 160, 40, 255), 20), // orange, minoritaire
    ]);
    let d = dominante(&o, l, h).expect("the orange survives");
    assert_eq!(
        d,
        [250, 160, 40],
        "the dark outline does not decide the hue"
    );
}

#[test]
fn une_icone_entierement_grise_rend_none() {
    // 🔴 THIS IS THE PROOF THAT `None` IS REACHABLE, hence that clause 2
    // is not an ornament.
    // RED: remove the saturation filter ⟹ `Some([128,128,128])`.
    let (o, l, h) = image(&[(px(128, 128, 128, 255), 64)]);
    assert_eq!(dominante(&o, l, h), None, "no chromatic pixel survives");
}

#[test]
fn la_moyenne_du_seau_n_est_pas_le_centre_du_seau() {
    // RED: return the centre of the quantised bucket instead of the average.
    let (o, l, h) = image(&[(px(200, 40, 40, 255), 16)]);
    let d = dominante(&o, l, h).expect("the red survives");
    assert_eq!(
        d,
        [200, 40, 40],
        "the average yields a REAL hue of the image"
    );
    // The centre of the bucket would be (6·32+16, 1·32+16, 1·32+16) = (208, 48, 48).
    assert_ne!(d, [208, 48, 48], "and above all NOT a hue of the grid");
}

#[test]
fn une_geometrie_incoherente_rend_none() {
    // RED: remove the length guard ⟹ `Some`, on a slice whose
    // announced geometry does not describe its content.
    let o = vec![60u8, 110, 240, 255];
    assert_eq!(dominante(&o, 4, 4), None, "1 pixel for 4×4 announced");
    assert_eq!(dominante(&[], 0, 0), None, "an empty image has no dominant");
}

#[test]
fn en_hexa_rend_six_chiffres_minuscules() {
    // RED: `{:X}` instead of `{:x}`, or three digits instead of six.
    assert_eq!(en_hexa([0x0a, 0xbc, 0xde]), "#0abcde", "leading zero kept");
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
        "the first one"
    );
    assert_eq!(s.observer("#7aa2f7"), None, "the same: nothing");
    assert_eq!(s.observer("#7aa2f7"), None, "still the same: still nothing");
    assert_eq!(
        s.observer("#fa8c16"),
        Some("#fa8c16".to_string()),
        "a change"
    );
    assert_eq!(s.observer("#fa8c16"), None, "then nothing more");
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
    assert_eq!(t[3], 0x07, "the alpha of the first pixel");
    assert_eq!(t[7], 0xf0, "the alpha of the second");
}

#[test]
fn bgra_en_rgba_est_son_propre_inverse() {
    // A property, not an example: applied twice, it returns the
    // original. This is what forbids it from doing anything else on the way.
    let original: Vec<u8> = (0u8..=63).collect();
    let mut t = original.clone();
    super::bgra_en_rgba(&mut t);
    assert_ne!(t, original, "a single pass MUST change something");
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
    let lu_sans =
        super::dominante(&sans, 8, 8).expect("a saturated flat area must yield a dominant");

    super::bgra_en_rgba(&mut sans);
    let read_with = super::dominante(&sans, 8, 8).expect("same after conversion");

    assert_eq!(
        read_with,
        [0xd0, 0x80, 0x40],
        "converted: the WARM hue, the image's own"
    );
    assert_eq!(
        lu_sans,
        [0x40, 0x80, 0xd0],
        "without conversion: the COLD hue, red and blue swapped"
    );
    assert_ne!(
        lu_sans, read_with,
        "the two readings DIFFER: the order matters"
    );
}
