use super::*;
use crate::h264::AccessUnit;
use std::io::Cursor;

#[test]
fn une_attache_fait_l_aller_retour() {
    let message = VersCapteur::Attache {
        session: "w-1".into(),
        hwnd: 0x1a2b,
        sortie: r"\\.\DISPLAY8".into(),
        fps: 90,
        debit: 8_000_000,
        taille: (1280, 720),
        origine_qpc: 123_456_789,
    };
    let mut tampon = Vec::new();
    ecrire_json(&mut tampon, &message).unwrap();
    let mut lecteur = Cursor::new(tampon);
    match lire_trame(&mut lecteur).unwrap() {
        Trame::Json(octets) => {
            assert_eq!(serde_json::from_slice::<VersCapteur>(&octets).unwrap(), message)
        }
        autre => panic!("attendu du JSON, reçu {autre:?}"),
    }
}

/// L'identité est la trame qui apparie la connexion média à la session
/// déjà attachée sur la connexion de commandes : un nom de champ qui
/// dériverait ferait échouer l'appariement en session réelle seulement.
#[test]
fn une_identite_fait_l_aller_retour() {
    let message = VersCapteur::Identite { session: "w-1".into() };
    let mut tampon = Vec::new();
    ecrire_json(&mut tampon, &message).unwrap();
    let mut lecteur = Cursor::new(tampon);
    match lire_trame(&mut lecteur).unwrap() {
        Trame::Json(octets) => {
            assert_eq!(serde_json::from_slice::<VersCapteur>(&octets).unwrap(), message)
        }
        autre => panic!("attendu du JSON, reçu {autre:?}"),
    }
}

#[test]
fn chaque_reponse_fait_l_aller_retour() {
    for message in [
        DepuisCapteur::Attachee { largeur: 1280, hauteur: 720 },
        DepuisCapteur::Refus { motif: "sortie inconnue".into() },
        DepuisCapteur::Taille { largeur: 1280, hauteur: 720 },
        DepuisCapteur::Fait,
        DepuisCapteur::Erreur { motif: "encodeur perdu".into() },
        DepuisCapteur::Etat { vivante: true, epuisee: false, largeur: 1280, hauteur: 720 },
    ] {
        let mut tampon = Vec::new();
        ecrire_json(&mut tampon, &message).unwrap();
        let mut lecteur = Cursor::new(tampon);
        let Trame::Json(octets) = lire_trame(&mut lecteur).unwrap() else {
            panic!("attendu du JSON")
        };
        assert_eq!(serde_json::from_slice::<DepuisCapteur>(&octets).unwrap(), message);
    }
}

/// L'unité d'accès voyage en BINAIRE BRUT, jamais en base64 : c'est le
/// seul message dont le volume compte (8 Mb/s par fenêtre).
#[test]
fn une_unite_d_acces_fait_l_aller_retour_sans_reencodage() {
    let unite = AccessUnit {
        data: vec![0, 0, 0, 1, 0x67, 0xff, 0x00, 0x01],
        is_keyframe: true,
        pts_90k: 90_000,
    };
    let mut tampon = Vec::new();
    ecrire_image(&mut tampon, &unite).unwrap();
    // 4 (longueur) + 1 (étiquette) + 8 (pts) + 1 (clé) + 8 (données)
    assert_eq!(tampon.len(), 22, "cadrage inattendu : {tampon:?}");
    let mut lecteur = Cursor::new(tampon);
    match lire_trame(&mut lecteur).unwrap() {
        Trame::Image(rendue) => assert_eq!(rendue, unite),
        autre => panic!("attendu une image, reçu {autre:?}"),
    }
}

#[test]
fn deux_trames_a_la_suite_se_lisent_dans_l_ordre() {
    let mut tampon = Vec::new();
    ecrire_json(&mut tampon, &DepuisCapteur::Fait).unwrap();
    ecrire_image(
        &mut tampon,
        &AccessUnit { data: vec![9, 9], is_keyframe: false, pts_90k: 7 },
    )
    .unwrap();
    let mut lecteur = Cursor::new(tampon);
    assert!(matches!(lire_trame(&mut lecteur).unwrap(), Trame::Json(_)));
    assert!(matches!(lire_trame(&mut lecteur).unwrap(), Trame::Image(_)));
}

/// Une étiquette inconnue est REFUSÉE, jamais ignorée : un flux mal
/// aligné doit tuer le canal plutôt que de faire dériver la lecture.
#[test]
fn une_etiquette_inconnue_est_refusee() {
    let mut tampon = Vec::new();
    tampon.extend_from_slice(&2u32.to_le_bytes());
    tampon.push(99);
    tampon.push(0);
    assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
}

#[test]
fn une_trame_tronquee_est_refusee() {
    let mut tampon = Vec::new();
    ecrire_json(&mut tampon, &DepuisCapteur::Fait).unwrap();
    tampon.truncate(tampon.len() - 1);
    assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
}

/// Sans cette borne, une longueur corrompue ferait réserver des gigaoctets
/// avant même de lire un octet de corps.
#[test]
fn une_longueur_aberrante_est_refusee_avant_toute_allocation() {
    let mut tampon = Vec::new();
    tampon.extend_from_slice(&(TAILLE_MAX as u32 + 1).to_le_bytes());
    tampon.push(1);
    assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
}

#[test]
fn une_visibilite_traverse_le_canal_du_capteur() {
    let message = VersCapteur::Visibilite { visible: true, focalisee: false };
    let json = serde_json::to_string(&message).expect("sérialisation");
    let relu: VersCapteur = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(relu, message);
}

#[test]
fn un_sommeil_traverse_le_canal_du_capteur() {
    let message = DepuisCapteur::Sommeil { endormie: true, raison: "masquee".into() };
    let json = serde_json::to_string(&message).expect("sérialisation");
    let relu: DepuisCapteur = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(relu, message);
}

#[test]
fn une_part_traverse_l_encodage_json() {
    let mut tampon = Vec::new();
    ecrire_json(&mut tampon, &DepuisCapteur::Part { bps: 4_000_000 }).unwrap();
    let mut lecture = &tampon[..];
    let Trame::Json(corps) = lire_trame(&mut lecture).unwrap() else {
        panic!("une trame JSON était attendue");
    };
    let message: DepuisCapteur = serde_json::from_slice(&corps).unwrap();
    assert_eq!(message, DepuisCapteur::Part { bps: 4_000_000 });
}

/// Une image de zéro octet n'existe pas : elle signalerait un cadrage
/// perdu, pas une image vide.
#[test]
fn une_image_sans_en_tete_complet_est_refusee() {
    let mut tampon = Vec::new();
    tampon.extend_from_slice(&3u32.to_le_bytes());
    tampon.push(ETIQUETTE_IMAGE);
    tampon.extend_from_slice(&[0, 0]);
    assert!(lire_trame(&mut Cursor::new(tampon)).is_err());
}
