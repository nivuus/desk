//! Tests d'hôte du module PUR `accent`.
//!
//! Ils vivent à part pour que `accent.rs` garde sa marge : le sous-bloc A1 est
//! neuf de bout en bout, et son module pur est le seul poste de travail du
//! dépôt qui n'ait aucun point de départ (aucune occurrence de `HICON`,
//! `GetIconInfo`, `WM_GETICON` ni `GCLP_HICON` n'existait avant lui).
//!
//! 🔴 **Chaque test nomme ce qui le rend ROUGE**, et les rouges de mutation ont
//! été jouées au harnais du §6.4 du plan — copie nommée, `diff` non vide,
//! restauration depuis la copie, `sha256sum` égal.

use super::*;

/// Un pixel RGBA.
fn px(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
}

/// Construit une tranche RGBA de `n` pixels à partir d'une liste
/// `(couleur, répétitions)`, et rend `(octets, largeur, hauteur)` avec une
/// géométrie 1×n cohérente — `dominante` refuse une longueur incohérente.
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
    // ROUGE : l'arbre intact avant que `dominante` n'existe.
    let (o, l, h) = image(&[
        (px(60, 110, 240, 255), 40), // bleu, majoritaire
        (px(250, 160, 40, 255), 10), // orange, minoritaire
    ]);
    let d = dominante(&o, l, h).expect("des pixels chromatiques survivent");
    assert_eq!(d, [60, 110, 240], "le seau le plus peuplé est le bleu");
}

#[test]
fn les_pixels_transparents_ne_comptent_pas() {
    // ROUGE : retirer le filtre `ALPHA_MIN` — le rouge transparent, trois fois
    // plus nombreux, l'emporterait sur le bleu opaque.
    let (o, l, h) = image(&[
        (px(240, 40, 40, 0), 30),    // rouge, ENTIÈREMENT TRANSPARENT
        (px(60, 110, 240, 255), 10), // bleu, opaque
    ]);
    let d = dominante(&o, l, h).expect("les pixels opaques survivent");
    assert_eq!(d, [60, 110, 240], "seuls les pixels opaques comptent");
}

#[test]
fn un_contour_noir_majoritaire_ne_gagne_pas() {
    // ROUGE : retirer le filtre de luminance — le contour sombre, deux fois
    // plus nombreux, l'emporterait sur la teinte.
    //
    // ⚠️ Le contour est un bleu TRÈS SOMBRE (5, 5, 60) et non un noir pur :
    // un noir pur serait déjà rejeté par la SATURATION, et le test ne dirait
    // alors rien du filtre de luminance qu'il prétend éprouver.
    assert!(
        (60u8 - 5u8) >= SATURATION_MIN,
        "le contour doit PASSER le filtre de saturation, sinon ce test \
         n'éprouve pas la luminance"
    );
    let (o, l, h) = image(&[
        (px(5, 5, 60, 255), 40),      // contour sombre, majoritaire
        (px(250, 160, 40, 255), 20),  // orange, minoritaire
    ]);
    let d = dominante(&o, l, h).expect("l'orange survit");
    assert_eq!(d, [250, 160, 40], "le contour sombre ne décide pas de la teinte");
}

#[test]
fn une_icone_entierement_grise_rend_None() {
    // 🔴 C'EST LA PREUVE QUE `None` EST ATTEIGNABLE, donc que la clause 2
    // n'est pas un ornement.
    // ROUGE : retirer le filtre de saturation ⟹ `Some([128,128,128])`.
    let (o, l, h) = image(&[(px(128, 128, 128, 255), 64)]);
    assert_eq!(dominante(&o, l, h), None, "aucun pixel chromatique ne survit");
}

#[test]
fn la_moyenne_du_seau_n_est_pas_le_centre_du_seau() {
    // ROUGE : rendre le centre du seau quantifié au lieu de la moyenne.
    let (o, l, h) = image(&[(px(200, 40, 40, 255), 16)]);
    let d = dominante(&o, l, h).expect("le rouge survit");
    assert_eq!(d, [200, 40, 40], "la moyenne rend une teinte RÉELLE de l'image");
    // Le centre du seau vaudrait (6·32+16, 1·32+16, 1·32+16) = (208, 48, 48).
    assert_ne!(d, [208, 48, 48], "et surtout PAS une teinte de la grille");
}

#[test]
fn une_geometrie_incoherente_rend_None() {
    // ROUGE : retirer le garde de longueur ⟹ `Some`, sur une tranche dont la
    // géométrie annoncée ne décrit pas le contenu.
    let o = vec![60u8, 110, 240, 255];
    assert_eq!(dominante(&o, 4, 4), None, "1 pixel pour 4×4 annoncés");
    assert_eq!(dominante(&[], 0, 0), None, "une image vide n'a pas de dominante");
}

#[test]
fn en_hexa_rend_six_chiffres_minuscules() {
    // ROUGE : `{:X}` au lieu de `{:x}`, ou trois chiffres au lieu de six.
    assert_eq!(en_hexa([0x0a, 0xbc, 0xde]), "#0abcde", "zéro de tête conservé");
    assert_eq!(en_hexa([0xAB, 0xCD, 0xEF]), "#abcdef", "MINUSCULES");
    assert_eq!(en_hexa([0, 0, 0]), "#000000");
    assert_eq!(en_hexa([255, 255, 255]), "#ffffff");
}

#[test]
fn un_suivi_neuf_annonce_sa_premiere_lecture() {
    // ROUGE : construire `SuiviAccent` depuis la première lecture, à la
    // `SuiviBordure` ⟹ le navigateur ne reçoit jamais la couleur initiale.
    let mut s = SuiviAccent::neuf();
    assert_eq!(s.observer("#7aa2f7"), Some("#7aa2f7".to_string()));
}

#[test]
fn un_suivi_n_annonce_pas_deux_fois_la_meme_couleur() {
    // 🔴 C'EST LE TEST D'HÔTE DU CRITÈRE ④.
    // ROUGE : retirer la comparaison ⟹ une annonce par tour, soit douze par
    // minute et par fenêtre à `PERIODE_ACCENT = 5 s`.
    let mut s = SuiviAccent::neuf();
    assert_eq!(s.observer("#7aa2f7"), Some("#7aa2f7".to_string()), "la première");
    assert_eq!(s.observer("#7aa2f7"), None, "la même : rien");
    assert_eq!(s.observer("#7aa2f7"), None, "encore la même : toujours rien");
    assert_eq!(s.observer("#fa8c16"), Some("#fa8c16".to_string()), "un changement");
    assert_eq!(s.observer("#fa8c16"), None, "puis plus rien");
}
