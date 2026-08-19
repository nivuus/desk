//! Conformité des en-têtes aux vecteurs partagés. **Purs, exécutés sur l'hôte.**

use super::*;

/// 🔴 **CE TEST EST LE CONTRÔLE QUI RATTRAPE UN RENOMMAGE**, et c'est la
/// raison pour laquelle ces structures ont quitté `agent/src/pont/entetes.rs`
/// pour `proto/`.
///
/// Tant qu'elles vivaient dans l'agent seul, le jumeau TypeScript devait les
/// reproduire à la main : un champ renommé d'un côté cassait le pont **sans
/// casser un seul test**. C'est le patron exact que ce dépôt a déjà payé —
/// `TYPES_AGENT` écrit à la main sans être confronté à son union, et la
/// variante `battement-recu` restée verte sur cinquante tests parce que rien
/// n'épinglait ses octets.
///
/// `proto/fichiers-vectors.json` est lu **ici ET dans
/// `proto/ts/fichiers-entetes.test.ts`** : un renommage n'a plus qu'un côté à
/// casser pour être vu.
#[test]
fn conformite_aux_vecteurs_partages() {
    let brut = include_str!("../../../fichiers-vectors.json");
    let doc: serde_json::Value = serde_json::from_str(brut).expect("vecteurs valides");

    // 🔴 La version du fichier EST celle du protocole. Sans cette assertion, un
    // bump d'un seul côté ne se verrait nulle part — c'est la lacune que
    // `input.rs::conformite_aux_vecteurs_partages` traîne et que
    // `plateforme.rs` a corrigée pour son fichier.
    assert_eq!(
        doc["version"].as_u64().expect("clé version"),
        u64::from(super::super::FICHIERS_VERSION),
        "la version des vecteurs a dérivé de FICHIERS_VERSION"
    );

    let cas = doc["cases"].as_array().expect("tableau de cas");
    // 🔴 ANTI-TAUTOLOGIE : un fichier de vecteurs VIDE ferait passer toute la
    // boucle sans rien éprouver.
    assert!(!cas.is_empty(), "au moins un vecteur attendu");

    let mut vus = 0;
    for c in cas {
        let nom = c["name"].as_str().expect("nom");
        let attendu = c["json"].as_str().expect("json attendu");

        // Chaque forme est sérialisée depuis ses champs, puis relue depuis le
        // JSON attendu : les DEUX sens, sur le MÊME vecteur.
        match c["forme"].as_str().expect("forme") {
            "chemin" => {
                let v = Chemin { chemin: c["chemin"].as_str().unwrap().to_string() };
                verifier(nom, attendu, &v);
            }
            "lire" => {
                let v = Lire {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    position: c["position"].as_u64().unwrap(),
                    longueur: c["longueur"].as_u64().unwrap() as u32,
                };
                verifier(nom, attendu, &v);
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
                            taille: e["taille"].as_u64().unwrap(),
                            modifie: e["modifie"].as_i64().unwrap(),
                        })
                        .collect(),
                };
                verifier(nom, attendu, &v);
            }
            "meta" => {
                let v = Meta {
                    repertoire: c["repertoire"].as_bool().unwrap(),
                    taille: c["taille"].as_u64().unwrap(),
                    modifie: c["modifie"].as_i64().unwrap(),
                };
                verifier(nom, attendu, &v);
            }
            "donnees" => {
                let v = Donnees {
                    position: c["position"].as_u64().unwrap(),
                    longueur: c["longueur"].as_u64().unwrap() as u32,
                };
                verifier(nom, attendu, &v);
            }
            "echec" => {
                let code: crate::fichiers::CodeEchec =
                    serde_json::from_value(c["code"].clone()).expect("code d'échec connu");
                verifier(nom, attendu, &Echec { code });
            }
            autre => panic!("forme inconnue dans les vecteurs : {autre}"),
        }
        vus += 1;
    }
    // 🔴 Le compte est comparé à celui du fichier : sans lui, une `forme` mal
    // orthographiée ferait sauter des cas en silence. Le `panic!` ci-dessus
    // n'est atteint que par une valeur PRÉSENTE et inconnue, jamais par un cas
    // qu'une future refonte de la boucle sauterait.
    assert_eq!(vus, cas.len(), "tous les cas doivent être exercés");
}

/// Sérialise, compare au vecteur, relit le vecteur, compare à la valeur.
fn verifier<T>(nom: &str, attendu: &str, valeur: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(
        serde_json::to_string(valeur).expect("sérialisation"),
        attendu,
        "sérialisation du vecteur « {nom} »"
    );
    let relu: T = serde_json::from_str(attendu).expect("désérialisation");
    assert_eq!(&relu, valeur, "désérialisation du vecteur « {nom} »");
}

/// Un en-tête auquel il manque un champ est **rejeté**, jamais silencieusement
/// complété — la doctrine de version de [`crate::control`], appliquée aux
/// en-têtes : aucun `#[serde(default)]` nulle part.
#[test]
fn un_entete_incomplet_est_rejete_plutot_que_complete() {
    assert!(serde_json::from_str::<Meta>(r#"{"repertoire":false,"taille":1}"#).is_err());
    assert!(serde_json::from_str::<Donnees>(r#"{"position":0}"#).is_err());
    assert!(serde_json::from_str::<Lire>(r#"{"chemin":"a","position":0}"#).is_err());
}
