//! Tests of the v1 binary frame. Pure, run on the host.

use super::*;

/// The pinned vector, **hard-coded here AND in `proto/ts/fichiers.test.ts`**.
///
/// ⚠️ It is the only way to see an endianness divergence between the
/// two implementations turn RED: a Rust→Rust round trip stays green whatever the
/// byte order, as long as it is the same on both sides of the same file. The
/// bytes below are therefore the source of truth of the format, not a
/// consequence of the code.
///
/// Breakdown: version 1 | type 66 (`TYPE_DATA`) | correlation
/// 0x0A0B0C0D in little-endian | header length 2 in little-endian |
/// header `{}` | payload `00 FF 7F 80`.
pub(super) const VECTEUR_EPINGLE: &[u8] = &[
    1, 66, 0x0D, 0x0C, 0x0B, 0x0A, 2, 0, 0, 0, b'{', b'}', 0x00, 0xFF, 0x7F, 0x80,
];

#[test]
fn une_trame_sans_version_est_refusee() {
    // A zero-byte frame does not carry its version: it is rejected, never
    // completed with the current version. It is the doctrine of `control.rs:37-40`.
    assert!(matches!(
        decoder(&[]),
        Err(FrameError::TropCourte { recu: 0, .. })
    ));
    // And anything shorter than the fixed header is too, even by
    // a single byte.
    let presque = vec![0u8; FIXED_HEADER_SIZE - 1];
    assert!(matches!(
        decoder(&presque),
        Err(FrameError::TropCourte { .. })
    ));
}

#[test]
fn une_trame_de_version_2_est_refusee() {
    let mut octets = encoder(TYPE_LISTER, 7, "{}", b"");
    octets[0] = FILES_VERSION + 1;
    assert_eq!(
        decoder(&octets),
        Err(FrameError::VersionNonSupportee(FILES_VERSION + 1))
    );
}

#[test]
fn a_header_whose_length_overflows_the_frame_is_refused() {
    let mut octets = encoder(TYPE_ENTREES, 1, "{}", b"charge");
    // Header length announced as `u32::MAX`: without a bound, the `split_at`
    // slice would panic out of bounds.
    octets[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decoder(&octets),
        Err(FrameError::EnteteDeborde { .. })
    ));

    // And an overflow of ONE SINGLE byte is refused too: that is where
    // the strict-inequality error lives.
    let mut juste_un_de_trop = encoder(TYPE_ENTREES, 1, "ab", b"");
    juste_un_de_trop[6..10].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(
        decoder(&juste_un_de_trop),
        Err(FrameError::EnteteDeborde {
            length: 3,
            disponible: 2
        })
    ));
}

#[test]
fn un_aller_retour_conserve_les_octets_bruts() {
    // 0x00 and 0xFF are the two bytes a textual encoding damages first.
    let charge: Vec<u8> = (0u16..=255).map(|o| o as u8).collect();
    let octets = encoder(TYPE_DATA, 0xDEAD_BEEF, r#"{"position":0}"#, &charge);
    let trame = decoder(&octets).expect("trame licite");
    assert_eq!(trame.version, FILES_VERSION);
    assert_eq!(trame.type_message, TYPE_DATA);
    assert_eq!(trame.correlation, 0xDEAD_BEEF);
    assert_eq!(trame.entete, br#"{"position":0}"#);
    assert_eq!(
        trame.charge,
        &charge[..],
        "la charge doit sortir octet pour octet"
    );
}

#[test]
fn une_charge_vide_et_un_entete_vide_sont_licites() {
    // The minimal frame: it is that of an acknowledgement without a body, and it is lawful.
    let octets = encoder(TYPE_META, 0, "", b"");
    assert_eq!(octets.len(), FIXED_HEADER_SIZE);
    let trame = decoder(&octets).expect("la trame minimale est licite");
    assert_eq!(trame.entete, b"");
    assert_eq!(trame.charge, b"");
    assert_eq!(trame.correlation, 0);
}

#[test]
fn the_maximum_payload_of_max_frame_size_passes() {
    // At the EXACT threshold: `MAX_FRAME_SIZE` bytes pass, `+ 1` does not.
    // It is the strict inequality that is tested, not the bound in general.
    let pleine = vec![0xABu8; MAX_FRAME_SIZE];
    let octets = encoder(TYPE_DATA, 1, "{}", &pleine);
    let trame = decoder(&octets).expect("le seuil exact doit passer");
    assert_eq!(trame.charge.len(), MAX_FRAME_SIZE);

    let trop = vec![0xABu8; MAX_FRAME_SIZE + 1];
    let octets = encoder(TYPE_DATA, 1, "{}", &trop);
    assert!(matches!(
        decoder(&octets),
        Err(FrameError::ChargeTropGrande {
            max: MAX_FRAME_SIZE,
            ..
        })
    ));
}

#[test]
fn the_pinned_vector_decodes_as_announced() {
    // Pins the wire format, independently of the code that encodes it: if
    // `encoder` switched to big-endian, this assert would fall and the round trip
    // above would stay green.
    let trame = decoder(VECTEUR_EPINGLE).expect("vecteur épinglé licite");
    assert_eq!(trame.version, 1);
    assert_eq!(trame.type_message, TYPE_DATA);
    assert_eq!(trame.correlation, 0x0A0B_0C0D);
    assert_eq!(trame.entete, b"{}");
    assert_eq!(trame.charge, &[0x00, 0xFF, 0x7F, 0x80]);
    // …and the encoder REPRODUCES it byte for byte.
    assert_eq!(
        encoder(TYPE_DATA, 0x0A0B_0C0D, "{}", &[0x00, 0xFF, 0x7F, 0x80]),
        VECTEUR_EPINGLE
    );
}

#[test]
fn un_code_d_echec_a_une_forme_epinglee_sur_le_fil() {
    // ⚠️ The TWO-WORD variants are the ones that break silently: this
    // repository let `battement-recu` through green on fifty tests because
    // nothing pinned its bytes. All ELEVEN are pinned literally, and
    // in BOTH directions — serializing then deserializing would only prove the
    // consistency of serde with itself.
    //
    // ⚠️ The THREE new ones of F2 all have two words or more, and the only one
    // of F3 — `repertoire-non-vide` — has THREE.
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
        (CodeEchec::RepertoireNonVide, "\"repertoire-non-vide\""),
    ];
    for (code, texte) in attendu {
        assert_eq!(
            serde_json::to_string(&code).unwrap(),
            texte,
            "sérialisation de {code:?}"
        );
        assert_eq!(
            serde_json::from_str::<CodeEchec>(texte).unwrap(),
            code,
            "désérialisation de {texte}"
        );
    }
}

#[test]
fn les_types_de_message_ne_se_chevauchent_pas() {
    // A type duplicated between a request and an answer would mean an answer
    // would be handled as a request. The sweep forbids it, and it covers
    // any future addition — a hand enumeration would not have.
    let all = [
        TYPE_LISTER,
        TYPE_ATTRIBUTS,
        TYPE_LIRE,
        TYPE_WRITE,
        TYPE_CREATE,
        TYPE_RENOMMER,
        TYPE_DELETE,
        TYPE_DUES,
        TYPE_ENTREES,
        TYPE_META,
        TYPE_DATA,
        TYPE_FAIT,
        TYPE_ECHEC,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b, "deux types de message partagent la valeur {a}");
        }
    }
}

/// 🔴 **A FULL `ECRIRE` FRAME PASSES, header included.**
///
/// ⚠️ **It is the check that tells apart the two possible readings of
/// `MAX_FRAME_SIZE`**, and the module itself announces it as a divergence:
/// the name says "frame", the value bounds the **payload**. If one day the bound
/// became `MAX_FRAME_SIZE - taille_entete`, a full write chunk —
/// that is, the NOMINAL case of a big file, the one `pont::decoupe`
/// produces on every round but the last — would be refused by the decoder. The
/// symptom would be a write that fails **only** on files of
/// more than 64 KiB.
#[test]
fn a_full_write_frame_passes_header_included() {
    let entete = serde_json::to_string(&entetes::Write {
        chemin: "dossier/un nom accentué très long pour gonfler l'en-tête.bin".to_string(),
        position: 65_536,
        length: MAX_FRAME_SIZE as u32,
        premier: false,
        last: false,
    })
    .expect("un en-tête Ecrire se sérialise toujours");
    let charge = vec![0xCDu8; MAX_FRAME_SIZE];
    let octets = encoder(TYPE_WRITE, 42, &entete, &charge);
    assert!(
        octets.len() > MAX_FRAME_SIZE,
        "la trame pèse PLUS que sa charge"
    );

    let trame = decoder(&octets).expect("une trame d'écriture pleine doit passer");
    assert_eq!(trame.type_message, TYPE_WRITE);
    assert_eq!(trame.charge.len(), MAX_FRAME_SIZE);
    let relu: entetes::Write = serde_json::from_slice(trame.entete).expect("en-tête relu");
    assert_eq!(relu.length as usize, trame.charge.len());
}

/// 🔴 **EACH MESSAGE TYPE HAS A PINNED VALUE, AND THE TEST NAMES IT.**
///
/// ⚠️ **`les_types_de_message_ne_se_chevauchent_pas` is NOT enough**, and that is
/// what justifies this test: it forbids two equal values, never a
/// SHIFT. Renumbering `TYPE_RENOMMER` from 7 to 9 would leave it green, and
/// yet an agent of the version before and a browser of the one after
/// would no longer understand each other — without `FILES_VERSION` having moved, since
/// adding a type is deemed additive.
///
/// It is the same gap this repository paid for on `battement-recu`: a test
/// that checks a PROPERTY of a set does not replace a test that pins
/// its ELEMENTS.
#[test]
fn a_message_type_has_a_pinned_value() {
    // ⚠️ 6 is an ANNOUNCEMENT, 7 and 8 are REQUESTS: the numbering is
    // not contiguous per family, and the gap of F2 — which skipped 7 and 8 for F3 —
    // is what avoided a late renumbering.
    assert_eq!(TYPE_LISTER, 1);
    assert_eq!(TYPE_ATTRIBUTS, 2);
    assert_eq!(TYPE_LIRE, 3);
    assert_eq!(TYPE_WRITE, 4);
    assert_eq!(TYPE_CREATE, 5);
    assert_eq!(TYPE_DUES, 6);
    assert_eq!(TYPE_RENOMMER, 7);
    assert_eq!(TYPE_DELETE, 8);
    assert_eq!(TYPE_ENTREES, 64);
    assert_eq!(TYPE_META, 65);
    assert_eq!(TYPE_DATA, 66);
    assert_eq!(TYPE_FAIT, 67);
    assert_eq!(TYPE_ECHEC, 127);
}
