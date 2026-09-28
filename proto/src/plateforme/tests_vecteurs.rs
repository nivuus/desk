//! The two tests driven by `plateforme-vectors.json`.
//!
//! 🔴 EXTRACTED BECAUSE `tests.rs` CROSSED 500 LINES — 523 —, and the
//! doctrine of `CLAUDE.md` is to catch up by an EXTRACTION, never by
//! compression. Sub-block G2 had already split this file (561 → 449); the
//! branches and guards of G3 brought it back above the ceiling.
//!
//! 🔴 THE BOUNDARY IS THAT OF THE SOURCE OF TRUTH, not a split of
//! convenience: **these two tests are the only ones the VECTOR FILE
//! drives**, and the only ones that fail when a vector drifts. The others
//! exercise the shape from literals written in place.
//!
//! ⚠️ VERBATIM TRANSPOSITION. The check is the COUNT, announced before being
//! measured: `cargo test -p proto` reported 106 before, it must report 106 after.

use crate::plateforme::*;

/// Conformance to the shared vectors.
///
/// 🔴 IT CHECKS `doc["version"]`, AND THIS IS THE `input.rs` GAP FIXED
/// FOR THIS FILE: `input.rs::conformance_to_the_shared_vectors` reads
/// `vectors.json` without ever checking its `version` key, and the ONLY
/// place in the repository that checks it is `ts/input.test.ts`. A vector whose
/// version had drifted would therefore pass the Rust side silently — MEASURED:
/// by removing the assertion below and setting the file to
/// `"version": 2`, the 52 tests stayed GREEN. Here, BOTH sides
/// check it.
#[test]
fn conformance_to_the_shared_vectors() {
    let raw = include_str!("../../plateforme-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(raw).expect("valid vectors");

    // 🔴 The version of the file IS that of the protocol. Without this
    // assertion, a bump on one side only would show up nowhere.
    assert_eq!(
        doc["version"].as_u64().expect("version key"),
        u64::from(PLATEFORME_VERSION),
        "the vectors version drifted from PLATEFORME_VERSION"
    );

    let cases = doc["cases"].as_array().expect("array of cases");
    // 🔴 ANTI-TAUTOLOGY: an EMPTY vector file would let the whole
    // loop pass without exercising anything. Same guard as `input.rs:326` and
    // `sous-ensemble.test.ts`. (policy: allow-fr - file name)
    assert!(!cases.is_empty(), "at least one vector expected");

    let mut vus = 0;
    for case in cases {
        let name = case["name"].as_str().expect("nom");
        let attendu = case["json"].as_str().expect("json expected");

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
                        serde_json::from_value(case["applications"].clone()).expect("applications"),
                        serde_json::from_value(case["disparues"].clone()).expect("disparues"),
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
                    // ⚠️ `motif` and `code_sortie` ARE READ THROUGH `from_value`, which
                    // tells the ABSENT field apart from the `null` field — and
                    // that is exactly the distinction `option_obligatoire`
                    // restores on the wire. An `as_str().map(...)` would
                    // confuse them, and the vector would stop exercising the guard.
                    "termine" => VersLaPlateforme::termine(
                        case["installation"].as_str().unwrap(),
                        serde_json::from_value(case["issue"].clone()).expect("issue"),
                        serde_json::from_value(case["motif"].clone()).expect("motif"),
                        serde_json::from_value(case["code_sortie"].clone()).expect("code_sortie"),
                        case["journal"].as_str().unwrap(),
                        case["journal_tronque"].as_bool().unwrap(),
                    ),
                    autre => panic!("unknown kind in the vers direction: {autre}"),
                };
                assert_eq!(
                    serde_json::to_string(&msg).expect("ser."),
                    attendu,
                    "serialisation of vector « {name} »"
                );
                let relu: VersLaPlateforme = serde_json::from_str(attendu).expect("deser.");
                assert_eq!(relu, msg, "deserialisation of vector « {name} »");
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
                    // ⚠️ `depuis_mot` AND NOT `serde_json::from_value`: the
                    // reason is no longer a serde shape since the fix of
                    // 20 August 2026, it is a word. A vector carrying an unknown
                    // word therefore fails HERE, which is the intended
                    // behaviour — a round-trip vector can only carry a
                    // reason the platform knows how to EMIT.
                    "refus" => DepuisLaPlateforme::refus(
                        MotifCanal::depuis_mot(case["motif"].as_str().expect("motif"))
                            .expect("known reason"),
                    ),
                    "lancer" => DepuisLaPlateforme::lancer(
                        case["demande"].as_str().unwrap(),
                        case["cle"].as_str().unwrap(),
                    ),
                    "icones-manquantes" => DepuisLaPlateforme::icones_manquantes(
                        serde_json::from_value(case["empreintes"].clone()).expect("empreintes"),
                    ),
                    "installer" => DepuisLaPlateforme::installer(
                        case["installation"].as_str().unwrap(),
                        case["url"].as_str().unwrap(),
                        case["nom"].as_str().unwrap(),
                        case["taille"].as_u64().unwrap(),
                        case["sha256"].as_str().unwrap(),
                    ),
                    autre => panic!("unknown kind in the depuis direction: {autre}"),
                };
                assert_eq!(
                    serde_json::to_string(&msg).expect("ser."),
                    attendu,
                    "serialisation of vector « {name} »"
                );
                let relu: DepuisLaPlateforme = serde_json::from_str(attendu).expect("deser.");
                assert_eq!(relu, msg, "deserialisation of vector « {name} »");
            }
            autre => panic!("unknown direction: {autre}"),
        }
        vus += 1;
    }
    // 🔴 The count is HARDCODED: without it, a misspelled `sens`
    // would silently skip cases — the `panic!` would not see them,
    // since it is only reached by a PRESENT and unknown value, not
    // by a case a future rework of the loop would skip.
    assert_eq!(vus, cases.len(), "every case must be exercised");
}

/// The refusals BOTH ends must know how to read, frozen in the shared
/// vector file — `ts/plateforme.test.ts` reads exactly the same ones.
///
/// 🔴 WITHOUT THIS SHARED VECTOR, THE REMEDY COULD LIVE ON ONE SIDE ONLY, and
/// that is precisely the divergence mode `plateforme-vectors.json`
/// exists to close.
#[test]
fn conformance_to_the_shared_readable_refusals() {
    let raw = include_str!("../../plateforme-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(raw).expect("valid vectors");
    let refus = doc["refus_lisibles"]
        .as_array()
        .expect("tableau refus_lisibles");
    // 🔴 ANTI-TAUTOLOGY, and the count is HARDCODED: an empty array, or
    // one missing a case, would let the loop pass without exercising anything.
    assert_eq!(refus.len(), 4, "four readable refusals expected");

    for cas in refus {
        let name = cas["name"].as_str().expect("nom");
        let brut = cas["json"].as_str().expect("json");
        let lu: DepuisLaPlateforme = serde_json::from_str(brut)
            .unwrap_or_else(|error| panic!("refusal « {name} » unreadable: {error}"));
        let DepuisLaPlateforme::Refus { version, motif } = lu else {
            panic!("vector « {name} » was not read as a refusal");
        };
        assert_eq!(
            u64::from(version),
            cas["v"].as_u64().expect("v"),
            "version of « {name} »"
        );
        // 🔴 THIS CASE IS THE ONLY ONE WHOSE NAME CLAIMS SOMETHING ABOUT THE
        // CURRENT VERSION, AND IT HAD ALREADY AGED: sub-block G2 raised
        // `PLATEFORME_VERSION` from 2 to 3 without updating this vector, which
        // thus carried `"v":2` under a name saying "our version". Found by G3, which
        // was raising it in turn. **Tying it to the constant is what prevents
        // the name from aging again silently at the next bump** — a test, rather
        // than vigilance.
        if name == "refus_de_notre_version" {
            assert_eq!(
                version, PLATEFORME_VERSION,
                "« refus_de_notre_version » NO LONGER carries the current version: its name has become wrong"
            );
        }
        assert_eq!(
            motif,
            cas["motif"].as_str().expect("motif"),
            "reason of « {name} »"
        );
    }
}
