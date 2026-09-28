//! Host tests of the store. Pure: no file, no COM, no VM.

use super::*;

/// Three fake PNGs, two of them IDENTICAL — the ×3 case taken from the measurement of
/// 20 August 2026 (the fingerprint `77852FCF…`, shared by three applications).
#[test]
fn deux_ajouts_du_meme_contenu_ne_font_qu_une_entree() {
    let mut m = Magasin::new();
    let a = m.ajouter(b"\x89PNG-un".to_vec());
    let b = m.ajouter(b"\x89PNG-un".to_vec());
    let c = m.ajouter(b"\x89PNG-un".to_vec());
    let d = m.ajouter(b"\x89PNG-deux".to_vec());
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_ne!(a, d);
    // 🔴 FOUR ADDITIONS, TWO ENTRIES. A store that ACCUMULATED would
    // count four — and on the real corpus, 153 instead of 99.
    assert_eq!(
        m.len(),
        2,
        "l'accumulation est ce que ce module existe pour éviter"
    );
}

#[test]
fn l_empreinte_est_celle_du_contenu_et_rien_d_autre() {
    // The FIPS 180-4 known-answer vector for the empty string: it is
    // `apps::sha256` that holds it, and this test only says that the store does not
    // get in the way.
    assert_eq!(
        empreinte(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(empreinte(b"abc").len(), 64);
    assert!(empreinte(b"abc")
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
}

/// 🔴 `remplacer` DISCARDS, IT DOES NOT MERGE.
#[test]
fn remplacer_fait_disparaitre_le_catalogue_precedent() {
    let mut m = Magasin::new();
    let ancienne = m.ajouter(b"tour-1".to_vec());
    let mut neuf = Magasin::new();
    let neuve = neuf.ajouter(b"tour-2".to_vec());
    m.remplacer(neuf);
    assert!(
        !m.contient(&ancienne),
        "le tour précédent doit avoir DISPARU"
    );
    assert!(m.contient(&neuve));
    assert_eq!(m.len(), 1);
}

#[test]
fn les_octets_se_relisent_a_l_identique() {
    let mut m = Magasin::new();
    let e = m.ajouter(b"\x89PNG\r\n\x1a\n-corps".to_vec());
    assert_eq!(m.octets(&e), Some(&b"\x89PNG\r\n\x1a\n-corps"[..]));
    assert_eq!(m.octets("pas-une-empreinte"), None);
    assert!(m.contient(&e));
    assert!(!m.contient("pas-une-empreinte"));
}

/// 🔴 THE RULE OF CRITERION ⑤: what is already known is NOT requested again.
#[test]
fn manquantes_ne_rend_que_ce_qui_manque() {
    let connues: BTreeSet<String> = ["a".into(), "b".into()].into_iter().collect();
    let annoncees = vec!["a".to_string(), "c".into(), "b".into(), "d".into()];
    assert_eq!(
        manquantes(&annoncees, &connues),
        vec!["c".to_string(), "d".into()]
    );
    // Everything is known: nothing is requested again, and that is what the platform
    // translates into "no message".
    let tout: BTreeSet<String> = annoncees.iter().cloned().collect();
    assert!(manquantes(&annoncees, &tout).is_empty());
}

/// 🔴 AN EMPTY STORE MAKES EVERYTHING BE REQUESTED AGAIN — the state of a first start, and
/// that of a LOST store. A `manquantes` that returned the empty set here
/// would mean nothing would ever be uploaded again, silently.
#[test]
fn un_ensemble_connu_vide_fait_tout_redemander() {
    let annoncees = vec!["a".to_string(), "b".into()];
    assert_eq!(manquantes(&annoncees, &BTreeSet::new()), annoncees);
}

/// The order of announcement is preserved, and duplicates are only requested
/// once — the same PNG shared by twenty-seven applications is not uploaded
/// twenty-seven times.
#[test]
fn l_ordre_est_preserve_et_les_doublons_fondus() {
    let annoncees = vec![
        "z".to_string(),
        "a".into(),
        "z".into(),
        "m".into(),
        "a".into(),
    ];
    assert_eq!(
        manquantes(&annoncees, &BTreeSet::new()),
        vec!["z".to_string(), "a".into(), "m".into()]
    );
}

#[test]
fn les_empreintes_du_magasin_sont_celles_de_ce_qu_il_porte() {
    let mut m = Magasin::new();
    let a = m.ajouter(b"un".to_vec());
    let b = m.ajouter(b"deux".to_vec());
    assert_eq!(m.empreintes(), [a, b].into_iter().collect::<BTreeSet<_>>());
    assert!(Magasin::new().is_empty());
}
