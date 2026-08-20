//! Tests d'hôte du lecteur de répertoire d'icônes.
//!
//! 🔴 ILS LISENT DEUX FICHIERS RÉELS, VERSÉS, ET STRUCTURELLEMENT DIFFÉRENTS
//! SUR LE SEUL OCTET QUI COMPTE — `48` contre `0`. Ils sont fabriqués par
//! `agent/testdata/fabriquer-temoins-ico.py`, versé avec eux : personne n'a à
//! croire à leur contenu, le script se relit et se rejoue sur l'hôte, sans
//! Windows.

use super::*;

/// Le témoin qui ne contient QUE du 48×48 — et que le Shell rend pourtant en
/// 256×256 32bpp.
const TEMOIN_48: &[u8] = include_bytes!("../../../../testdata/g2-temoin-48.ico");
/// Le témoin qui contient un VRAI 256×256, donc `bWidth == 0`.
const TEMOIN_256: &[u8] = include_bytes!("../../../../testdata/g2-temoin-256.ico");

#[test]
fn le_temoin_48_annonce_48_et_rien_d_autre() {
    assert_eq!(tailles_icondir(TEMOIN_48), Some(vec![48]));
    assert_eq!(maximum(&[48]), SourceMax::Pixels(48));
}

/// 🔴 LA ROUGE PRINCIPALE, ET LA SEULE QUI SOIT INVISIBLE SANS LE TÉMOIN.
///
/// `bWidth == 0` vaut 256. Un lecteur qui rendrait `0` ferait passer ce test
/// à `Some(vec![0])`, et `maximum` classerait la plus grande icône du corpus
/// SOUS un 16×16 — silencieusement, sur une image qui, elle, serait juste.
#[test]
fn le_temoin_256_porte_bwidth_zero_et_vaut_256() {
    // L'octet lui-même, relu depuis le fichier : c'est ce qui rend le test
    // décidable plutôt que confiant.
    assert_eq!(TEMOIN_256[6], 0, "bWidth du témoin 256 doit être l'octet 0");
    assert_eq!(TEMOIN_48[6], 48, "bWidth du témoin 48 doit être 48");
    assert_eq!(tailles_icondir(TEMOIN_256), Some(vec![256]));
    assert_eq!(maximum(&[256]), SourceMax::Pixels(256));
}

/// 🔴 CE QUE LE SOUS-BLOC EXISTE POUR TENIR : les deux témoins se DISTINGUENT
/// par la ressource, là où toute mesure sur l'image rendue les confondrait.
#[test]
fn les_deux_temoins_se_distinguent_par_la_ressource() {
    let a = maximum(&tailles_icondir(TEMOIN_48).expect("48 lisible"));
    let b = maximum(&tailles_icondir(TEMOIN_256).expect("256 lisible"));
    assert_ne!(a, b);
    assert_eq!(a, SourceMax::Pixels(48));
    assert_eq!(b, SourceMax::Pixels(256));
}

/// 🔴 LES DEUX PAS D'ENTRÉE NE SONT PAS INTERCHANGEABLES — MAIS PAS SUR UNE
/// SEULE ENTRÉE, ET C'EST MESURÉ PLUTÔT QUE SUPPOSÉ.
///
/// ❌ **UNE PREMIÈRE RÉDACTION DE CE TEST ÉTAIT VACUEUSE, et l'exécution l'a
/// dénoncée.** Elle affirmait que « le témoin à UNE SEULE entrée le montre
/// sans ambiguïté : lu avec le pas de 14, le témoin 48 ne rend plus 48 ».
/// **C'est faux.** Le `bWidth` de la PREMIÈRE entrée est à l'offset 6 dans les
/// deux formats — le pas ne sépare que les entrées SUIVANTES. Sur un
/// répertoire à une seule entrée, les deux lecteurs rendent donc `[48]` tous
/// les deux, et le test passait pour la mauvaise raison… jusqu'à ce qu'il
/// échoue, parce qu'il exigeait l'inverse.
///
/// Ce que le pas décide réellement est **le contrôle de longueur** et **les
/// entrées à partir de la seconde**. C'est donc là que ce test regarde.
#[test]
fn le_mauvais_pas_d_entree_se_voit_a_partir_de_la_SECONDE_entree() {
    // Le fait mesuré, écrit plutôt que tu : sur UNE entrée, les deux pas
    // s'accordent.
    assert_eq!(tailles_icondir(TEMOIN_48), Some(vec![48]));
    assert_eq!(tailles_grpicondir(TEMOIN_48), Some(vec![48]));

    // 🔴 À DEUX ENTRÉES, ILS DIVERGENT. Un `GRPICONDIR` de deux entrées porte
    // 6 + 2×14 = 34 octets ; lu au pas de 16, il en faudrait 38, et le
    // contrôle de longueur rend `None` — jamais une liste partielle.
    let mut grp = vec![0u8, 0, 1, 0, 2, 0];
    grp.extend_from_slice(&[32, 32, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 7, 0]);
    grp.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 8, 0]);
    assert_eq!(grp.len(), 6 + 2 * 14);
    assert_eq!(tailles_grpicondir(&grp), Some(vec![32, 256]));
    assert_eq!(tailles_icondir(&grp), None, "le pas de 16 ne tient pas dans 34 octets");

    // Et dans l'autre sens, sur un tampon assez grand pour les deux : les
    // SECONDES entrées sont lues à des offsets différents, donc les listes
    // diffèrent. C'est la vraie forme du bruit.
    let mut ico = vec![0u8, 0, 1, 0, 2, 0];
    ico.extend_from_slice(&[48, 48, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    ico.extend_from_slice(&[16, 16, 0, 0, 1, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(tailles_icondir(&ico), Some(vec![48, 16]));
    let au_mauvais_pas = tailles_grpicondir(&ico).expect("34 octets suffisent au pas de 14");
    assert_ne!(
        au_mauvais_pas,
        vec![48, 16],
        "au pas de 14, la seconde entrée est lue au mauvais offset"
    );
}

/// La forme `GRPICONDIR`, sur un tampon fabriqué à la main : deux entrées de
/// 14 octets, dont une à `bWidth == 0`.
#[test]
fn lit_un_grpicondir_a_deux_entrees() {
    let mut o = vec![0u8, 0, 1, 0, 2, 0]; // reserved=0, type=1, count=2
    let mut entree = |w: u8| {
        o.extend_from_slice(&[w, w, 0, 0]); // bWidth, bHeight, bColorCount, bReserved
        o.extend_from_slice(&[1, 0, 32, 0]); // wPlanes, wBitCount
        o.extend_from_slice(&[0, 0, 0, 0]); // dwBytesInRes
        o.extend_from_slice(&[7, 0]); // nID — DEUX octets, c'est ce qui fait 14
    };
    entree(32);
    entree(0);
    assert_eq!(tailles_grpicondir(&o), Some(vec![32, 256]));
    assert_eq!(maximum(&[32, 256]), SourceMax::Pixels(256));
}

/// 🔴 UNE LISTE VIDE REND `NonMesuree`, JAMAIS `Pixels(0)`.
#[test]
fn une_liste_vide_n_est_pas_une_taille_nulle() {
    assert_eq!(maximum(&[]), SourceMax::NonMesuree);
    assert_ne!(maximum(&[]), SourceMax::Pixels(0));
}

/// Un répertoire à ZÉRO entrée est bien lu — et il rend `NonMesuree`, pas
/// `None` : le format est valide, il n'y a simplement rien dedans.
#[test]
fn un_repertoire_a_zero_entree_se_lit_et_ne_mesure_rien() {
    let o = [0u8, 0, 1, 0, 0, 0];
    assert_eq!(tailles_icondir(&o), Some(vec![]));
    assert_eq!(maximum(&tailles_icondir(&o).expect("valide")), SourceMax::NonMesuree);
}

/// 🔴 UN EN-TÊTE NON VÉRIFIÉ LAISSERAIT QUATRE OCTETS ARBITRAIRES PASSER POUR
/// UN RÉPERTOIRE D'ICÔNES.
#[test]
fn refuse_un_en_tete_qui_n_en_est_pas_un() {
    // `reserved` non nul.
    assert_eq!(tailles_icondir(&[9, 0, 1, 0, 0, 0]), None);
    // `type` = 2, c'est un CURSEUR, pas une icône.
    assert_eq!(tailles_icondir(&[0, 0, 2, 0, 0, 0]), None);
    // Du texte quelconque.
    assert_eq!(tailles_icondir(b"MZ\x90\x00\x03\x00"), None);
    assert_eq!(tailles_grpicondir(b"MZ\x90\x00\x03\x00"), None);
}

/// 🔴 UN TAMPON TRONQUÉ REND `None`, IL NE DÉBORDE PAS ET NE REND PAS UNE
/// LISTE PARTIELLE. Une liste partielle serait une mesure FAUSSE.
#[test]
fn un_tampon_tronque_rend_none() {
    assert_eq!(tailles_icondir(&[]), None);
    assert_eq!(tailles_icondir(&[0, 0, 1, 0, 1]), None); // en-tête incomplet
    // Annonce trois entrées, n'en porte qu'une.
    let mut o = vec![0u8, 0, 1, 0, 3, 0];
    o.extend_from_slice(&[48; ENTREE_ICO]);
    assert_eq!(tailles_icondir(&o), None);
    // Le témoin réel, coupé en deux.
    assert_eq!(tailles_icondir(&TEMOIN_256[..10]), None);
}

/// Un compte d'entrées absurde ne doit pas faire déborder l'arithmétique.
#[test]
fn un_compte_absurde_ne_deborde_pas() {
    let o = [0u8, 0, 1, 0, 0xFF, 0xFF]; // 65 535 entrées annoncées, aucune portée
    assert_eq!(tailles_icondir(&o), None);
    assert_eq!(tailles_grpicondir(&o), None);
}
