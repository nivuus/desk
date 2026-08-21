//! Les deux tests pilotés par `plateforme-vectors.json`.
//!
//! 🔴 EXTRAITS PARCE QUE `tests.rs` A FRANCHI 500 LIGNES — 523 —, et la
//! doctrine de `CLAUDE.md` est de rattraper par une EXTRACTION, jamais par une
//! compression. Le sous-bloc G2 avait déjà découpé ce fichier (561 → 449) ; les
//! branches et les gardes de G3 l'ont ramené au-dessus du plafond.
//!
//! 🔴 LA FRONTIÈRE EST CELLE DE LA SOURCE DE VÉRITÉ, pas un découpage de
//! commodité : **ces deux tests-là sont les seuls que le FICHIER DE VECTEURS
//! pilote**, et les seuls qui échouent quand un vecteur dérive. Les autres
//! éprouvent la forme depuis des littéraux écrits sur place.
//!
//! ⚠️ TRANSPOSITION VERBATIM. Le contrôle est le COMPTE, annoncé avant d'être
//! mesuré : `cargo test -p proto` rendait 106 avant, il doit rendre 106 après.

use crate::plateforme::*;
use crate::plateforme::tests::etrangere;

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
                    "catalogue" => VersLaPlateforme::catalogue(
                        case["complet"].as_bool().unwrap(),
                        serde_json::from_value(case["applications"].clone())
                            .expect("applications"),
                        serde_json::from_value(case["disparues"].clone())
                            .expect("disparues"),
                    ),
                    "lancee" => VersLaPlateforme::lancee(
                        case["demande"].as_str().unwrap(),
                        serde_json::from_value(case["issue"].clone()).expect("issue"),
                    ),
                    "progression" => VersLaPlateforme::progression(
                        case["installation"].as_str().unwrap(),
                        serde_json::from_value(case["phase"].clone()).expect("phase"),
                        case["octets_faits"].as_u64().unwrap(),
                        case["octets_total"].as_u64().unwrap(),
                        case["ecoule_ms"].as_u64().unwrap(),
                    ),
                    // ⚠️ `motif` et `code_sortie` SE LISENT PAR `from_value`, ce
                    // qui distingue le champ ABSENT du champ à `null` — et
                    // c'est exactement la distinction que `option_obligatoire`
                    // rétablit sur le fil. Un `as_str().map(...)` les
                    // confondrait, et le vecteur cesserait d'éprouver la garde.
                    "termine" => VersLaPlateforme::termine(
                        case["installation"].as_str().unwrap(),
                        serde_json::from_value(case["issue"].clone()).expect("issue"),
                        serde_json::from_value(case["motif"].clone()).expect("motif"),
                        serde_json::from_value(case["code_sortie"].clone())
                            .expect("code_sortie"),
                        case["journal"].as_str().unwrap(),
                        case["journal_tronque"].as_bool().unwrap(),
                    ),
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
                    // ⚠️ `depuis_mot` ET NON `serde_json::from_value` : le
                    // motif n'est plus une forme serde depuis la correction du
                    // 20 août 2026, c'est un mot. Un vecteur portant un mot
                    // inconnu échoue donc ICI, ce qui est le comportement
                    // voulu — un vecteur de round-trip ne peut porter qu'un
                    // motif que la plateforme sait ÉMETTRE.
                    "refus" => DepuisLaPlateforme::refus(
                        MotifCanal::depuis_mot(case["motif"].as_str().expect("motif"))
                            .expect("motif connu"),
                    ),
                    "lancer" => DepuisLaPlateforme::lancer(
                        case["demande"].as_str().unwrap(),
                        case["cle"].as_str().unwrap(),
                    ),
                    "icones-manquantes" => DepuisLaPlateforme::icones_manquantes(
                        serde_json::from_value(case["empreintes"].clone())
                            .expect("empreintes"),
                    ),
                    "installer" => DepuisLaPlateforme::installer(
                        case["installation"].as_str().unwrap(),
                        case["url"].as_str().unwrap(),
                        case["nom"].as_str().unwrap(),
                        case["taille"].as_u64().unwrap(),
                        case["sha256"].as_str().unwrap(),
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

/// Les refus que les DEUX bouts doivent savoir lire, figés dans le fichier de
/// vecteurs partagés — `ts/plateforme.test.ts` lit exactement les mêmes.
///
/// 🔴 SANS CE VECTEUR PARTAGÉ, LE REMÈDE POURRAIT NE VIVRE QUE D'UN CÔTÉ, et
/// c'est précisément le mode de divergence que `plateforme-vectors.json`
/// existe pour fermer.
#[test]
fn conformite_aux_refus_lisibles_partages() {
    let raw = include_str!("../../plateforme-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(raw).expect("vecteurs valides");
    let refus = doc["refus_lisibles"].as_array().expect("tableau refus_lisibles");
    // 🔴 ANTI-TAUTOLOGIE, et le compte est ÉCRIT EN DUR : un tableau vide, ou
    // amputé d'un cas, ferait passer la boucle sans rien éprouver.
    assert_eq!(refus.len(), 4, "quatre refus lisibles attendus");

    for cas in refus {
        let name = cas["name"].as_str().expect("nom");
        let brut = cas["json"].as_str().expect("json");
        let lu: DepuisLaPlateforme = serde_json::from_str(brut)
            .unwrap_or_else(|erreur| panic!("refus « {name} » illisible : {erreur}"));
        let DepuisLaPlateforme::Refus { version, motif } = lu else {
            panic!("le vecteur « {name} » n'a pas été lu comme un refus");
        };
        assert_eq!(u64::from(version), cas["v"].as_u64().expect("v"), "version de « {name} »");
        // 🔴 CE CAS EST LE SEUL DONT LE NOM AFFIRME QUELQUE CHOSE SUR LA
        // VERSION COURANTE, ET IL AVAIT DÉJÀ VIEILLI : le sous-bloc G2 a monté
        // `PLATEFORME_VERSION` de 2 à 3 sans reprendre ce vecteur, qui portait
        // donc `"v":2` sous un nom disant « notre version ». Trouvé par G3, qui
        // montait à son tour. **La rattacher à la constante est ce qui empêche
        // le nom de re-vieillir en silence au bump suivant** — un test, plutôt
        // qu'une vigilance.
        if name == "refus_de_notre_version" {
            assert_eq!(
                version, PLATEFORME_VERSION,
                "« refus_de_notre_version » ne porte PLUS la version courante : son nom est devenu faux"
            );
        }
        assert_eq!(motif, cas["motif"].as_str().expect("motif"), "motif de « {name} »");
    }
}
