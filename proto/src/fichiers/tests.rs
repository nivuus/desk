//! Tests de la trame binaire v1. Purs, exécutés sur l'hôte.

use super::*;

/// Le vecteur épinglé, **écrit en dur ici ET dans `proto/ts/fichiers.test.ts`**.
///
/// ⚠️ C'est la seule façon de voir ROUGE une divergence d'endianness entre les
/// deux implémentations : un aller-retour Rust→Rust reste vert quel que soit le
/// boutisme, du moment qu'il est le même des deux côtés du même fichier. Les
/// octets ci-dessous sont donc la source de vérité du format, pas une
/// conséquence du code.
///
/// Décomposition : version 1 | type 66 (`TYPE_DONNEES`) | corrélation
/// 0x0A0B0C0D en petit-boutiste | longueur d'en-tête 2 en petit-boutiste |
/// en-tête `{}` | charge `00 FF 7F 80`.
pub(super) const VECTEUR_EPINGLE: &[u8] = &[
    1, 66, 0x0D, 0x0C, 0x0B, 0x0A, 2, 0, 0, 0, b'{', b'}', 0x00, 0xFF, 0x7F, 0x80,
];

#[test]
fn une_trame_sans_version_est_refusee() {
    // Une trame de zéro octet ne porte pas sa version : elle est rejetée, jamais
    // complétée par la version courante. C'est la doctrine de `control.rs:37-40`.
    assert!(matches!(decoder(&[]), Err(ErreurTrame::TropCourte { recu: 0, .. })));
    // Et tout ce qui est plus court que l'en-tête fixe l'est aussi, y compris à
    // un octet près.
    let presque = vec![0u8; TAILLE_ENTETE_FIXE - 1];
    assert!(matches!(decoder(&presque), Err(ErreurTrame::TropCourte { .. })));
}

#[test]
fn une_trame_de_version_2_est_refusee() {
    let mut octets = encoder(TYPE_LISTER, 7, "{}", b"");
    octets[0] = FICHIERS_VERSION + 1;
    assert_eq!(
        decoder(&octets),
        Err(ErreurTrame::VersionNonSupportee(FICHIERS_VERSION + 1))
    );
}

#[test]
fn un_entete_dont_la_longueur_deborde_la_trame_est_refuse() {
    let mut octets = encoder(TYPE_ENTREES, 1, "{}", b"charge");
    // Longueur d'en-tête annoncée à `u32::MAX` : sans borne, la tranche
    // `split_at` paniquerait hors limites.
    octets[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(decoder(&octets), Err(ErreurTrame::EnteteDeborde { .. })));

    // Et le débordement d'UN SEUL octet est refusé aussi : c'est là que vit
    // l'erreur d'inégalité stricte.
    let mut juste_un_de_trop = encoder(TYPE_ENTREES, 1, "ab", b"");
    juste_un_de_trop[6..10].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(
        decoder(&juste_un_de_trop),
        Err(ErreurTrame::EnteteDeborde { longueur: 3, disponible: 2 })
    ));
}

#[test]
fn un_aller_retour_conserve_les_octets_bruts() {
    // 0x00 et 0xFF sont les deux octets qu'un encodage textuel abîme en premier.
    let charge: Vec<u8> = (0u16..=255).map(|o| o as u8).collect();
    let octets = encoder(TYPE_DONNEES, 0xDEAD_BEEF, r#"{"position":0}"#, &charge);
    let trame = decoder(&octets).expect("trame licite");
    assert_eq!(trame.version, FICHIERS_VERSION);
    assert_eq!(trame.type_message, TYPE_DONNEES);
    assert_eq!(trame.correlation, 0xDEAD_BEEF);
    assert_eq!(trame.entete, br#"{"position":0}"#);
    assert_eq!(trame.charge, &charge[..], "la charge doit sortir octet pour octet");
}

#[test]
fn une_charge_vide_et_un_entete_vide_sont_licites() {
    // La trame minimale : c'est celle d'un accusé sans corps, et elle est licite.
    let octets = encoder(TYPE_META, 0, "", b"");
    assert_eq!(octets.len(), TAILLE_ENTETE_FIXE);
    let trame = decoder(&octets).expect("la trame minimale est licite");
    assert_eq!(trame.entete, b"");
    assert_eq!(trame.charge, b"");
    assert_eq!(trame.correlation, 0);
}

#[test]
fn la_charge_maximale_de_taille_trame_max_passe() {
    // Au seuil EXACT : `TAILLE_TRAME_MAX` octets passent, `+ 1` ne passe pas.
    // C'est l'inégalité stricte qui est éprouvée, pas la borne en général.
    let pleine = vec![0xABu8; TAILLE_TRAME_MAX];
    let octets = encoder(TYPE_DONNEES, 1, "{}", &pleine);
    let trame = decoder(&octets).expect("le seuil exact doit passer");
    assert_eq!(trame.charge.len(), TAILLE_TRAME_MAX);

    let trop = vec![0xABu8; TAILLE_TRAME_MAX + 1];
    let octets = encoder(TYPE_DONNEES, 1, "{}", &trop);
    assert!(matches!(
        decoder(&octets),
        Err(ErreurTrame::ChargeTropGrande { max: TAILLE_TRAME_MAX, .. })
    ));
}

#[test]
fn le_vecteur_epingle_se_decode_comme_annonce() {
    // Épingle le format sur le fil, indépendamment du code qui l'encode : si
    // `encoder` passait au gros-boutiste, cet assert tomberait et l'aller-retour
    // ci-dessus resterait vert.
    let trame = decoder(VECTEUR_EPINGLE).expect("vecteur épinglé licite");
    assert_eq!(trame.version, 1);
    assert_eq!(trame.type_message, TYPE_DONNEES);
    assert_eq!(trame.correlation, 0x0A0B_0C0D);
    assert_eq!(trame.entete, b"{}");
    assert_eq!(trame.charge, &[0x00, 0xFF, 0x7F, 0x80]);
    // …et l'encodeur le REPRODUIT à l'octet près.
    assert_eq!(
        encoder(TYPE_DONNEES, 0x0A0B_0C0D, "{}", &[0x00, 0xFF, 0x7F, 0x80]),
        VECTEUR_EPINGLE
    );
}

#[test]
fn un_code_d_echec_a_une_forme_epinglee_sur_le_fil() {
    // ⚠️ Les variantes à DEUX MOTS sont celles qui se cassent en silence : ce
    // dépôt a laissé passer `battement-recu` verte sur cinquante tests parce que
    // rien n'épinglait ses octets. Les DIX sont épinglées littéralement, et
    // dans les DEUX sens — sérialiser puis désérialiser ne prouverait que la
    // cohérence de serde avec lui-même.
    //
    // ⚠️ Les TROIS neuves de F2 sont toutes à deux mots ou plus.
    let attendu = [
        (CodeEchec::Introuvable, "\"introuvable\""),
        (CodeEchec::CheminIntrouvable, "\"chemin-introuvable\""),
        (CodeEchec::AccesRefuse, "\"acces-refuse\""),
        (CodeEchec::ProtegeEnEcriture, "\"protege-en-ecriture\""),
        (CodeEchec::NonSupporte, "\"non-supporte\""),
        (CodeEchec::TropGrand, "\"trop-grand\""),
        (CodeEchec::Interne, "\"interne\""),
        (CodeEchec::DisquePlein, "\"disque-plein\""),
        (CodeEchec::DejaPresent, "\"deja-present\""),
        (CodeEchec::CasseAmbigue, "\"casse-ambigue\""),
    ];
    for (code, texte) in attendu {
        assert_eq!(serde_json::to_string(&code).unwrap(), texte, "sérialisation de {code:?}");
        assert_eq!(
            serde_json::from_str::<CodeEchec>(texte).unwrap(),
            code,
            "désérialisation de {texte}"
        );
    }
}

#[test]
fn les_types_de_message_ne_se_chevauchent_pas() {
    // Un type dupliqué entre une requête et une réponse ferait qu'une réponse
    // serait traitée comme une requête. Le balayage l'interdit, et il couvre
    // toute addition future — une énumération à la main ne l'aurait pas fait.
    let tous = [
        TYPE_LISTER, TYPE_ATTRIBUTS, TYPE_LIRE, TYPE_ECRIRE, TYPE_CREER,
        TYPE_DUES,
        TYPE_ENTREES, TYPE_META, TYPE_DONNEES, TYPE_FAIT, TYPE_ECHEC,
    ];
    for (i, a) in tous.iter().enumerate() {
        for b in &tous[i + 1..] {
            assert_ne!(a, b, "deux types de message partagent la valeur {a}");
        }
    }
}

/// 🔴 **UNE TRAME `ECRIRE` PLEINE PASSE, en-tête compris.**
///
/// ⚠️ **C'est le contrôle qui distingue les deux lectures possibles de
/// `TAILLE_TRAME_MAX`**, et le module l'annonce lui-même comme une divergence :
/// le nom dit « trame », la valeur borne la **charge**. Si un jour la borne
/// devenait `TAILLE_TRAME_MAX - taille_entete`, un morceau plein d'écriture —
/// c'est-à-dire le cas NOMINAL d'un gros fichier, celui que `pont::decoupe`
/// produit à chaque tour sauf le dernier — serait refusé par le décodeur. Le
/// symptôme serait une écriture qui échoue **uniquement** sur les fichiers de
/// plus de 64 Kio.
#[test]
fn une_trame_ecrire_pleine_passe_entete_compris() {
    let entete = serde_json::to_string(&entetes::Ecrire {
        chemin: "dossier/un nom accentué très long pour gonfler l'en-tête.bin".to_string(),
        position: 65_536,
        longueur: TAILLE_TRAME_MAX as u32,
        premier: false,
        dernier: false,
    })
    .expect("un en-tête Ecrire se sérialise toujours");
    let charge = vec![0xCDu8; TAILLE_TRAME_MAX];
    let octets = encoder(TYPE_ECRIRE, 42, &entete, &charge);
    assert!(octets.len() > TAILLE_TRAME_MAX, "la trame pèse PLUS que sa charge");

    let trame = decoder(&octets).expect("une trame d'écriture pleine doit passer");
    assert_eq!(trame.type_message, TYPE_ECRIRE);
    assert_eq!(trame.charge.len(), TAILLE_TRAME_MAX);
    let relu: entetes::Ecrire = serde_json::from_slice(trame.entete).expect("en-tête relu");
    assert_eq!(relu.longueur as usize, trame.charge.len());
}
