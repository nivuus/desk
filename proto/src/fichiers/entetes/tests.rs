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
                    nom: c["nom"].as_str().unwrap().to_string(),
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
            "ecrire" => {
                let v = Ecrire {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    position: c["position"].as_u64().unwrap(),
                    longueur: c["longueur"].as_u64().unwrap() as u32,
                    premier: c["premier"].as_bool().unwrap(),
                    dernier: c["dernier"].as_bool().unwrap(),
                };
                verifier(nom, attendu, &v);
            }
            "creer" => {
                let v = Creer {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verifier(nom, attendu, &v);
            }
            "renommer" => {
                let v = Renommer {
                    de: c["de"].as_str().unwrap().to_string(),
                    vers: c["vers"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verifier(nom, attendu, &v);
            }
            "supprimer" => {
                let v = Supprimer {
                    chemin: c["chemin"].as_str().unwrap().to_string(),
                    repertoire: c["repertoire"].as_bool().unwrap(),
                };
                verifier(nom, attendu, &v);
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
    assert!(serde_json::from_str::<Meta>(r#"{"nom":"a","repertoire":false,"taille":1}"#).is_err());
    // 🔴 **Sans `nom`, le substitut serait créé sous le nom que l'application a
    // TAPÉ**, et non sous celui qui existe sur le poste local — deux noms pour
    // un fichier, dont un qui n'existe nulle part.
    assert!(
        serde_json::from_str::<Meta>(r#"{"repertoire":false,"taille":1,"modifie":0}"#).is_err()
    );
    assert!(serde_json::from_str::<Donnees>(r#"{"position":0}"#).is_err());
    assert!(serde_json::from_str::<Lire>(r#"{"chemin":"a","position":0}"#).is_err());
    // 🔴 Les deux drapeaux d'`Ecrire` sont ceux dont l'absence est la plus
    // coûteuse : sans `premier`, le flux s'ouvrirait avec `keepExistingData` et
    // un fichier réécrit plus court garderait sa queue d'octets — le défaut
    // EXACT de l'ancien pont (spec §12). Sans `dernier`, le `close()` ne
    // viendrait jamais et l'écriture ne serait **jamais** commise.
    assert!(serde_json::from_str::<Ecrire>(
        r#"{"chemin":"a","position":0,"longueur":1,"dernier":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Ecrire>(
        r#"{"chemin":"a","position":0,"longueur":1,"premier":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Creer>(r#"{"chemin":"a"}"#).is_err());
    assert!(serde_json::from_str::<Due>(r#"{"chemin":"a"}"#).is_err());
    // 🔴 **Un `Renommer` sans `vers` est le cas qui DÉTRUIT** : complété en
    // silence par une chaîne vide, il ferait renommer vers la racine — ou, si
    // l'appelant sautait sa garde, écraserait la source par elle-même. C'est le
    // seul en-tête de ce protocole dont un champ manquant a une conséquence
    // destructrice, et c'est pourquoi il est nommé ici plutôt que compté.
    assert!(serde_json::from_str::<Renommer>(r#"{"de":"a","repertoire":false}"#).is_err());
    assert!(serde_json::from_str::<Renommer>(r#"{"de":"a","vers":"b"}"#).is_err());
    assert!(serde_json::from_str::<Supprimer>(r#"{"chemin":"a"}"#).is_err());
}

/// 🔴 **LES ONZE FORMES ONT LEUR VECTEUR** — et c'est ce qui empêche qu'une
/// forme neuve soit ajoutée sans être épinglée.
///
/// La boucle de [`conformite_aux_vecteurs_partages`] n'éprouve que les formes
/// PRÉSENTES dans le fichier : ajouter `Renommer` au code sans lui donner de
/// vecteur y passerait inaperçu. Ce test compte les formes distinctes du
/// fichier et exige qu'elles soient les onze que le protocole porte.
///
/// ⚠️ **`TYPE_FAIT` n'a pas de forme** : son en-tête est `{}`. Le compter
/// ferait attendre un vecteur pour une structure qui n'existe pas.
///
/// *(Ce test s'appelait `les_neuf_formes_ont_leur_vecteur` jusqu'à F3, qui en
/// ajoute deux. **Le renommer plutôt que rallonger sa liste en silence** est ce
/// que `pont::notifications` a fait de son propre garde de masque, pour la même
/// raison : un nom qui ment sur son compte est un nom qu'on cesse de lire.)*
#[test]
fn les_onze_formes_ont_leur_vecteur() {
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
