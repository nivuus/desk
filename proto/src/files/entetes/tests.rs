//! Conformance of the headers to the shared vectors. **Pure, run on the host.**

use super::*;

/// 🔴 **THIS TEST IS THE CHECK THAT CATCHES A RENAME**, and it is the
/// reason why these structures left `agent/src/pont/entetes.rs`
/// for `proto/`.
///
/// As long as they lived in the agent alone, the TypeScript twin had to
/// reproduce them by hand: a field renamed on one side broke the bridge **without
/// breaking a single test**. It is the exact pattern this repository has already paid for —
/// `TYPES_AGENT` written by hand without being confronted with its union, and the
/// `battement-recu` variant that stayed green on fifty tests because nothing
/// pinned its bytes.
///
/// `proto/fichiers-vectors.json` is read **here AND in
/// `proto/ts/fichiers-entetes.test.ts`**: a rename now only has one side to
/// break to be seen.
#[test]
fn conformite_aux_vecteurs_partages() {
    let brut = include_str!("../../../fichiers-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(brut).expect("vecteurs valides");

    // 🔴 The version of the file IS that of the protocol. Without this assertion, a
    // bump on one side only would show nowhere — it is the gap that
    // `input.rs::conformite_aux_vecteurs_partages` drags along and that
    // `plateforme.rs` fixed for its file.
    assert_eq!(
        doc["version"].as_u64().expect("clé version"),
        u64::from(super::super::FILES_VERSION),
        "la version des vecteurs a dérivé de FICHIERS_VERSION"
    );

    let cas = doc["cases"].as_array().expect("tableau de cas");
    // 🔴 ANTI-TAUTOLOGY: an EMPTY vector file would let the whole
    // loop pass without testing anything.
    assert!(!cas.is_empty(), "au moins un vecteur attendu");

    let mut vus = 0;
    for c in cas {
        let nom = c["name"].as_str().expect("nom");
        let attendu = c["json"].as_str().expect("json attendu");

        // Each shape is serialized from its fields, then read back from the
        // expected JSON: BOTH directions, on the SAME vector.
        match c["forme"].as_str().expect("forme") {
            "chemin" => {
                let v = Chemin {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                };
                verify(nom, attendu, &v);
            }
            "lire" => {
                let v = Lire {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    position: c["position"].as_u64().unwrap(),
                    length: c["longueur"].as_u64().unwrap() as u32,
                };
                verify(nom, attendu, &v);
            }
            "entrees" => {
                let v = Entrees {
                    entrees: c["entrees"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|e| EntreeJson {
                            nom: e["nom"].as_str().unwrap().to_string(),
                            repertoire: e["repertoire"].as_bool().unwrap(),
                            size: e["taille"].as_u64().unwrap(),
                            modified: e["modifie"].as_i64().unwrap(),
                        })
                        .collect(),
                };
                verify(nom, attendu, &v);
            }
            "meta" => {
                let v = Meta {
                    nom: c["nom"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                    size: c["taille"].as_u64().unwrap(),
                    modified: c["modifie"].as_i64().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "donnees" => {
                let v = Data {
                    position: c["position"].as_u64().unwrap(),
                    length: c["longueur"].as_u64().unwrap() as u32,
                };
                verify(nom, attendu, &v);
            }
            "ecrire" => {
                let v = Write {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    position: c["position"].as_u64().unwrap(),
                    length: c["longueur"].as_u64().unwrap() as u32,
                    premier: c["premier"].as_bool().unwrap(),
                    last: c["dernier"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "creer" => {
                let v = Create {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "renommer" => {
                let v = Renommer {
                    de: c["de"].as_str().unwrap().to_string(),
                    vers: c["vers"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "supprimer" => {
                let v = Delete {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "dues" => {
                let v = Dues {
                    dues: c["dues"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| Due {
                            chemin: d["chemin"].as_str().unwrap().to_string(),
                            octets: d["octets"].as_u64().unwrap(),
                        })
                        .collect(),
                    retenues: c["retenues"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "bonjour" => {
                let v = Bonjour {
                    racine: c["racine"].as_str().unwrap().to_string(),
                    forcer: c["forcer"].as_bool().unwrap(),
                };
                verify(nom, attendu, &v);
            }
            "echec" => {
                let code: crate::files::CodeEchec =
                    serde_json::from_value(c["code"].clone()).expect("code d'échec connu");
                verify(nom, attendu, &Echec { code });
            }
            autre => panic!("forme inconnue dans les vecteurs : {autre}"),
        }
        vus += 1;
    }
    // 🔴 The count is compared to that of the file: without it, a misspelt `forme`
    // would silently skip cases. The `panic!` above
    // is only reached by a PRESENT and unknown value, never by a case
    // that a future rework of the loop would skip.
    assert_eq!(vus, cas.len(), "tous les cas doivent être exercés");
}

/// Serializes, compares to the vector, reads the vector back, compares to the value.
fn verify<T>(nom: &str, attendu: &str, value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(
        serde_json::to_string(value).expect("sérialisation"),
        attendu,
        "sérialisation du vecteur « {nom} »"
    );
    let relu: T = serde_json::from_str(attendu).expect("désérialisation");
    assert_eq!(&relu, value, "désérialisation du vecteur « {nom} »");
}

/// A header missing a field is **rejected**, never silently
/// completed — the version doctrine of [`crate::control`], applied to
/// headers: no `#[serde(default)]` anywhere.
#[test]
fn un_entete_incomplet_est_rejete_plutot_que_complete() {
    assert!(serde_json::from_str::<Meta>(r#"{"nom":"a","repertoire":false,"taille":1}"#).is_err());
    // 🔴 **Without `nom`, the placeholder would be created under the name the application
    // TYPED**, and not under the one that exists on the local machine — two names for
    // one file, one of which exists nowhere.
    assert!(
        serde_json::from_str::<Meta>(r#"{"repertoire":false,"taille":1,"modifie":0}"#).is_err()
    );
    assert!(serde_json::from_str::<Data>(r#"{"position":0}"#).is_err());
    assert!(serde_json::from_str::<Lire>(r#"{"chemin":"a","position":0}"#).is_err());
    // 🔴 The two flags of `Write` are the ones whose absence is the most
    // costly: without `premier`, the stream would open with `keepExistingData` and
    // a file rewritten shorter would keep its tail of bytes — the EXACT
    // defect of the old bridge (spec §12). Without `last`, the `close()` would
    // never come and the write would **never** be committed.
    assert!(serde_json::from_str::<Write>(
        r#"{"chemin":"a","position":0,"longueur":1,"dernier":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Write>(
        r#"{"chemin":"a","position":0,"longueur":1,"premier":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Create>(r#"{"chemin":"a"}"#).is_err());
    assert!(serde_json::from_str::<Due>(r#"{"chemin":"a"}"#).is_err());
    // 🔴 **A `Renommer` without `vers` is the case that DESTROYS**: filled in
    // silently with an empty string, it would rename to the root — or, if
    // the caller skipped its guard, would overwrite the source with itself. It is the
    // only header of this protocol where a missing field has a destructive
    // consequence, and that is why it is named here rather than counted.
    assert!(serde_json::from_str::<Renommer>(r#"{"de":"a","repertoire":false}"#).is_err());
    assert!(serde_json::from_str::<Renommer>(r#"{"de":"a","vers":"b"}"#).is_err());
    assert!(serde_json::from_str::<Delete>(r#"{"chemin":"a"}"#).is_err());
    // 🔴 **F5 — THE SECOND CASE WHERE A MISSING FIELD HAS A CONSEQUENCE, and it
    // goes in the DANGEROUS direction.** A `Dues` without `retenues` filled in
    // silently would be `false` = "the bridge pushes", that is the reverse of
    // what `Bonjour` exists to prevent: writing the files of one session
    // into the folder of another. **These two lines pin the ABSENCE of a
    // default**, which no vector could do — a vector pins a
    // shape that passes, never a shape that must be refused.
    assert!(serde_json::from_str::<Dues>(r#"{"dues":[]}"#).is_err());
    assert!(serde_json::from_str::<Bonjour>(r#"{"racine":"Documents"}"#).is_err());
    assert!(serde_json::from_str::<Bonjour>(r#"{"forcer":false}"#).is_err());
}

/// 🔴 **THE TWELVE SHAPES HAVE THEIR VECTOR** — and that is what keeps a
/// new shape from being added without being pinned.
///
/// The loop of [`conformite_aux_vecteurs_partages`] only tests the shapes
/// PRESENT in the file: adding `Renommer` to the code without giving it a
/// vector would go unnoticed there. This test counts the distinct shapes of the
/// file and requires them to be the twelve the protocol carries.
///
/// ⚠️ **`TYPE_FAIT` has no shape**: its header is `{}`. Counting it
/// would expect a vector for a structure that does not exist. **`F5`
/// adds a SECOND type in this case — `TYPE_RAFRAICHIR`**: twelve shapes for
/// fourteen types, and the gap is exactly those two.
///
/// *(This test was called `les_neuf_formes_ont_leur_vecteur` until F3, which
/// adds two, then `les_onze_…` until F5, which adds one. **Renaming it
/// rather than silently lengthening its set** is what `pont::notifications`
/// did with its own mask guard, for the same reason: a name that lies
/// about its count is a name one stops reading.)*
#[test]
fn les_douze_formes_ont_leur_vecteur() {
    let brut = include_str!("../../../fichiers-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(brut).expect("vecteurs valides");
    let mut formes: Vec<&str> = doc["cases"]
        .as_array()
        .expect("tableau de cas")
        .iter()
        .map(|c| c["forme"].as_str().expect("forme"))
        .collect();
    formes.sort_unstable();
    formes.dedup();
    assert_eq!(
        formes,
        [
            "bonjour",
            "chemin",
            "creer",
            "donnees",
            "dues",
            "echec",
            "ecrire",
            "entrees",
            "lire",
            "meta",
            "renommer",
            "supprimer",
        ],
        "une forme du protocole n'a pas de vecteur, ou un vecteur n'a pas de forme"
    );
}
