//! Tests d'hôte du magasin. Purs : aucun fichier, aucun COM, aucune VM.

use super::*;

/// Trois PNG factices, dont deux IDENTIQUES — le cas ×3 tiré de la mesure du
/// 20 août 2026 (l'empreinte `77852FCF…`, partagée par trois applications).
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
    // 🔴 QUATRE AJOUTS, DEUX ENTRÉES. Un magasin qui ACCUMULERAIT en
    // compterait quatre — et sur le corpus réel, 153 au lieu de 99.
    assert_eq!(m.len(), 2, "l'accumulation est ce que ce module existe pour éviter");
}

#[test]
fn l_empreinte_est_celle_du_contenu_et_rien_d_autre() {
    // Le vecteur de réponse connue de FIPS 180-4 pour la chaîne vide : c'est
    // `apps::sha256` qui le tient, et ce test dit seulement que le magasin ne
    // s'interpose pas.
    assert_eq!(
        empreinte(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(empreinte(b"abc").len(), 64);
    assert!(empreinte(b"abc").chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
}

/// 🔴 `remplacer` JETTE, IL NE FUSIONNE PAS.
#[test]
fn remplacer_fait_disparaitre_le_catalogue_precedent() {
    let mut m = Magasin::new();
    let ancienne = m.ajouter(b"tour-1".to_vec());
    let mut neuf = Magasin::new();
    let neuve = neuf.ajouter(b"tour-2".to_vec());
    m.remplacer(neuf);
    assert!(!m.contient(&ancienne), "le tour précédent doit avoir DISPARU");
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

/// 🔴 LA RÈGLE DU CRITÈRE ⑤ : ce qui est déjà connu n'est PAS redemandé.
#[test]
fn manquantes_ne_rend_que_ce_qui_manque() {
    let connues: BTreeSet<String> = ["a".into(), "b".into()].into_iter().collect();
    let annoncees = vec!["a".to_string(), "c".into(), "b".into(), "d".into()];
    assert_eq!(manquantes(&annoncees, &connues), vec!["c".to_string(), "d".into()]);
    // Tout est connu : rien n'est redemandé, et c'est ce que la plateforme
    // traduit par « aucun message ».
    let tout: BTreeSet<String> = annoncees.iter().cloned().collect();
    assert!(manquantes(&annoncees, &tout).is_empty());
}

/// 🔴 UN MAGASIN VIDE FAIT TOUT REDEMANDER — l'état d'un premier démarrage, et
/// celui d'un magasin PERDU. `manquantes` qui rendrait l'ensemble vide ici
/// ferait que plus rien ne serait jamais téléversé, en silence.
#[test]
fn un_ensemble_connu_vide_fait_tout_redemander() {
    let annoncees = vec!["a".to_string(), "b".into()];
    assert_eq!(manquantes(&annoncees, &BTreeSet::new()), annoncees);
}

/// L'ordre d'annonce est préservé, et les doublons ne sont demandés qu'une
/// fois — un même PNG partagé par vingt-sept applications ne se téléverse pas
/// vingt-sept fois.
#[test]
fn l_ordre_est_preserve_et_les_doublons_fondus() {
    let annoncees = vec!["z".to_string(), "a".into(), "z".into(), "m".into(), "a".into()];
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
