//! Tests du module [`crate::plateforme`].
//!
//! Extrait de `proto/src/plateforme.rs` VERBATIM (sous-bloc G1, tâche 1) : le
//! fichier parent était à 410 lignes dont 244 de tests, et la règle des 500
//! lignes exige que l'extraction précède l'addition. Aucun test n'a été
//! ajouté, retiré ni réécrit par ce déplacement.
//!
//! ⚠️ `include_str!` résout RELATIVEMENT AU FICHIER QUI LE CONTIENT : le
//! chemin des vecteurs partagés a donc gagné un `../` en descendant d'un
//! niveau. C'est la seule ligne dont le TEXTE diffère de l'original ; tout le
//! reste n'a perdu que ses quatre espaces d'indentation d'enveloppe.

use super::*;

#[test]
fn serialise_l_enrolement_en_kebab_case() {
    let json = serde_json::to_string(&VersLaPlateforme::enroler("w1", "chut")).expect("sér.");
    assert_eq!(json, r#"{"type":"enroler","v":1,"vm":"w1","secret":"chut"}"#);
}

#[test]
fn serialise_le_battement() {
    let json = serde_json::to_string(&VersLaPlateforme::battement()).expect("sér.");
    assert_eq!(json, r#"{"type":"battement","v":1}"#);
}

#[test]
fn serialise_le_battement_recu_en_kebab_case() {
    // 🔴 `battement-recu` EST LA SEULE VARIANTE A DEUX MOTS DU MODULE, donc
    // la seule dont `kebab-case` et `snake_case` diffèrent. Sans ce test,
    // passer `rename_all` en `snake_case` ne rougissait RIEN — MESURE : la
    // mutation restait verte sur les 50 tests. Rust émettrait alors
    // `battement_recu` là où le miroir TypeScript lit `battement-recu`, et
    // les deux bouts divergeraient EN SILENCE sur le message que l'agent
    // reçoit le plus souvent.
    let json = serde_json::to_string(&DepuisLaPlateforme::battement_recu("kkk", 1_787_136_774_000))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"battement-recu","v":1,"jeton":"kkk","expire_a":1787136774000}"#
    );
}

#[test]
fn serialise_l_enrole_et_le_refus() {
    let json = serde_json::to_string(&DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"enrole","v":1,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}"#
    );
    let json = serde_json::to_string(&DepuisLaPlateforme::refus(MotifCanal::Enrolement))
        .expect("sér.");
    assert_eq!(json, r#"{"type":"refus","v":1,"motif":"enrolement"}"#);
}

// 🔴 UN TEST DE VERSION PAR VARIANTE ENTRANTE, jamais un seul pour toutes.
// `verifie_version` est branchée variante par variante : l'omettre sur UNE
// seule laisserait ce trou-là ouvert, et un test unique ne le verrait pas.

#[test]
fn rejette_une_version_absente_sur_enroler() {
    assert!(
        serde_json::from_str::<VersLaPlateforme>(r#"{"type":"enroler","vm":"w","secret":"s"}"#)
            .is_err()
    );
}

#[test]
fn rejette_une_version_absente_sur_battement() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement"}"#).is_err());
}

#[test]
fn rejette_une_version_absente_sur_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"enrole","prefixe":"P","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejette_une_version_absente_sur_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"battement-recu","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejette_une_version_absente_sur_refus() {
    assert!(
        serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"refus","motif":"version"}"#)
            .is_err()
    );
}

#[test]
fn rejette_la_version_suivante_sur_enroler() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"enroler","v":2,"vm":"w","secret":"s"}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement() {
    assert!(
        serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement","v":2}"#).is_err()
    );
}

#[test]
fn rejette_la_version_suivante_sur_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"enrole","v":2,"prefixe":"P","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"battement-recu","v":2,"jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_refus() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":2,"motif":"version"}"#
    )
    .is_err());
}

#[test]
fn rejette_un_type_inconnu() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"vol","v":1}"#).is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"vol","v":1}"#).is_err());
}

#[test]
fn rejette_un_champ_inconnu() {
    // `deny_unknown_fields` : un champ de trop est une divergence de
    // format, pas une extension tolérable — le canal n'a qu'une version.
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"battement","v":1,"bonus":1}"#
    )
    .is_err());
}

/// Conformité aux vecteurs partagés.
///
/// 🔴 IL VÉRIFIE `doc["version"]`, ET C'EST LA LACUNE D'`input.rs` CORRIGÉE
/// POUR CE FICHIER-CI : `input.rs::conformite_aux_vecteurs_partages` lit
/// `vectors.json` sans jamais contrôler sa clé `version`, et le SEUL
/// endroit du dépôt qui la contrôle est `ts/input.test.ts`. Un vecteur dont
/// la version aurait dérivé passerait donc le Rust en silence — MESURÉ :
/// en retirant l'assertion ci-dessous et en portant le fichier à
/// `"version": 2`, les 52 tests restaient VERTS. Ici, les DEUX côtés la
/// vérifient.
#[test]
fn conformite_aux_vecteurs_partages() {
    let raw = include_str!("../../plateforme-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(raw).expect("vecteurs valides");

    // 🔴 La version du fichier EST celle du protocole. Sans cette
    // assertion, un bump d'un seul côté ne se verrait nulle part.
    assert_eq!(
        doc["version"].as_u64().expect("clé version"),
        u64::from(PLATEFORME_VERSION),
        "la version des vecteurs a dérivé de PLATEFORME_VERSION"
    );

    let cases = doc["cases"].as_array().expect("tableau de cas");
    // 🔴 ANTI-TAUTOLOGIE : un fichier de vecteurs VIDE ferait passer toute
    // la boucle sans rien éprouver. Même garde qu'`input.rs:326` et que
    // `sous-ensemble.test.ts`.
    assert!(!cases.is_empty(), "au moins un vecteur attendu");

    let mut vus = 0;
    for case in cases {
        let name = case["name"].as_str().expect("nom");
        let attendu = case["json"].as_str().expect("json attendu");

        match case["sens"].as_str().expect("sens") {
            "vers" => {
                let msg = match case["kind"].as_str().expect("kind") {
                    "enroler" => VersLaPlateforme::enroler(
                        case["vm"].as_str().unwrap(),
                        case["secret"].as_str().unwrap(),
                    ),
                    "battement" => VersLaPlateforme::battement(),
                    autre => panic!("kind inconnu dans le sens vers : {autre}"),
                };
                assert_eq!(
                    serde_json::to_string(&msg).expect("sér."),
                    attendu,
                    "sérialisation du vecteur « {name} »"
                );
                let relu: VersLaPlateforme =
                    serde_json::from_str(attendu).expect("désér.");
                assert_eq!(relu, msg, "désérialisation du vecteur « {name} »");
            }
            "depuis" => {
                let msg = match case["kind"].as_str().expect("kind") {
                    "enrole" => DepuisLaPlateforme::enrole(
                        case["prefixe"].as_str().unwrap(),
                        case["jeton"].as_str().unwrap(),
                        case["expire_a"].as_i64().unwrap(),
                    ),
                    "battement-recu" => DepuisLaPlateforme::battement_recu(
                        case["jeton"].as_str().unwrap(),
                        case["expire_a"].as_i64().unwrap(),
                    ),
                    "refus" => DepuisLaPlateforme::refus(
                        serde_json::from_value(case["motif"].clone()).expect("motif"),
                    ),
                    autre => panic!("kind inconnu dans le sens depuis : {autre}"),
                };
                assert_eq!(
                    serde_json::to_string(&msg).expect("sér."),
                    attendu,
                    "sérialisation du vecteur « {name} »"
                );
                let relu: DepuisLaPlateforme =
                    serde_json::from_str(attendu).expect("désér.");
                assert_eq!(relu, msg, "désérialisation du vecteur « {name} »");
            }
            autre => panic!("sens inconnu : {autre}"),
        }
        vus += 1;
    }
    // 🔴 Le compte est ÉCRIT EN DUR : sans lui, un `sens` mal orthographié
    // ferait sauter des cas en silence — le `panic!` ne les verrait pas,
    // puisqu'il n'est atteint que par une valeur PRÉSENTE et inconnue, pas
    // par un cas qu'une future refonte de la boucle sauterait.
    assert_eq!(vus, cases.len(), "tous les cas doivent être exercés");
}

#[test]
fn round_trip_des_trois_reponses() {
    for message in [
        DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742),
        DepuisLaPlateforme::battement_recu("kkk", 1_787_136_774_000),
        DepuisLaPlateforme::refus(MotifCanal::Sequence),
    ] {
        let json = serde_json::to_string(&message).expect("sér.");
        let relu: DepuisLaPlateforme = serde_json::from_str(&json).expect("désér.");
        assert_eq!(message, relu);
    }
}
