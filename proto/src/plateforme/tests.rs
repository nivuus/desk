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
//!
//! ⚠️ **CE FICHIER A ÉTÉ DÉCOUPÉ UNE SECONDE FOIS (sous-bloc G2, tâche 1)** :
//! il était monté à 561 lignes et figurait au tableau de dette de `CLAUDE.md`.
//! Ce qui reste ici est le **cycle de vie** — version, refus, enrôlement,
//! battement, et la conformité aux vecteurs partagés, qui porte l'assertion
//! sur `doc["version"]` et se lit donc comme un test de version. La **gestion
//! d'apps** vit désormais dans `plateforme/tests_apps.rs`. Aucun test n'a été
//! ajouté, retiré ni réécrit par ce second déplacement.

use super::*;

#[test]
fn serialise_l_enrolement_en_kebab_case() {
    let json = serde_json::to_string(&VersLaPlateforme::enroler("w1", "chut")).expect("sér.");
    assert_eq!(json, r#"{"type":"enroler","v":3,"vm":"w1","secret":"chut"}"#);
}

#[test]
fn serialise_le_battement() {
    let json = serde_json::to_string(&VersLaPlateforme::battement()).expect("sér.");
    assert_eq!(json, r#"{"type":"battement","v":3}"#);
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
        r#"{"type":"battement-recu","v":3,"jeton":"kkk","expire_a":1787136774000}"#
    );
}

#[test]
fn serialise_l_enrole_et_le_refus() {
    let json = serde_json::to_string(&DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"enrole","v":3,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}"#
    );
    let json = serde_json::to_string(&DepuisLaPlateforme::refus(MotifCanal::Enrolement))
        .expect("sér.");
    assert_eq!(json, r#"{"type":"refus","v":3,"motif":"enrolement"}"#);
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
        r#"{"type":"enroler","v":4,"vm":"w","secret":"s"}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement() {
    assert!(
        serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement","v":4}"#).is_err()
    );
}

#[test]
fn rejette_la_version_suivante_sur_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"enrole","v":4,"prefixe":"P","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"battement-recu","v":4,"jeton":"j","expire_a":1}"#
    )
    .is_err());
}

/// ❌ **CE TEST ÉPINGLAIT LE DÉFAUT 2, ET IL EST RETOURNÉ LE 20 AOÛT 2026.**
/// Il exigeait qu'un refus d'une version voisine soit REJETÉ — c'est-à-dire
/// exactement ce qui empêchait un agent périmé de lire pourquoi il l'était.
/// La propriété qu'il gardait (« chaque variante entrante contrôle sa
/// version ») reste gardée par ses quatre jumeaux ci-dessus et par
/// `les_messages_autres_que_le_refus_restent_refuses_sur_une_version_divergente` ;
/// **le refus, lui, en est retiré à dessein**, et c'est ce que ce test dit
/// désormais. Le champ `v` reste OBLIGATOIRE : la tolérance porte sur sa
/// VALEUR, jamais sur sa présence.
#[test]
fn le_refus_tolere_toute_version_mais_exige_le_champ() {
    let lu: DepuisLaPlateforme =
        serde_json::from_str(r#"{"type":"refus","v":4,"motif":"version"}"#).expect("lisible");
    assert_eq!(lu, DepuisLaPlateforme::Refus { version: 4, motif: "version".into() });
    // Sans `v`, en revanche, c'est toujours une forme invalide : un message
    // sans version n'est pas un message d'une version que nous ignorons. Et
    // `v: null` non plus — c'est le trou exact que `verifie_version` ferme
    // pour les autres variantes, et que `version_toleree` ne rouvre pas.
    assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"refus","motif":"version"}"#)
        .is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":null,"motif":"version"}"#
    )
    .is_err());
    // Et la forme reste GELÉE : un champ de plus est refusé
    // (`deny_unknown_fields`), ce qui est la clause 3 de l'en-tête du module —
    // écrite comme une contrainte sur les versions FUTURES, éprouvée ici.
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":4,"motif":"version","detail":"x"}"#
    )
    .is_err());
}

#[test]
fn rejette_un_type_inconnu() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"vol","v":3}"#).is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"vol","v":3}"#).is_err());
}

#[test]
fn rejette_un_champ_inconnu() {
    // `deny_unknown_fields` : un champ de trop est une divergence de
    // format, pas une extension tolérable — le canal n'a qu'une version.
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"battement","v":3,"bonus":1}"#
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

/// 🔴 LA ROUGE DU BUMP LUI-MÊME. Si `PLATEFORME_VERSION` restait à 1, ce
/// test resterait vert sur les seules variantes neuves et la rupture ne
/// serait pas jouée : c'est ici qu'on assène qu'un agent déployé au format
/// v1 N'EST PLUS COMPRIS, et que le refus `version` NE SE RÉESSAIE PAS
/// (en-tête du module). Agent et plateforme se déploient au même commit.
#[test]
fn les_variantes_de_p3_rejettent_desormais_la_version_1() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"enroler","v":1,"vm":"w","secret":"s"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement","v":1}"#).is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"enrole","v":1,"prefixe":"P","jeton":"j","expire_a":1}"#
    )
    .is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"battement-recu","v":1,"jeton":"j","expire_a":1}"#
    )
    .is_err());
    // ⚠️ LE REFUS EST DÉLIBÉRÉMENT ABSENT DE CETTE LISTE depuis la correction
    // du 20 août 2026 : il est la SEULE variante hors versionnement, et un
    // agent v1 doit précisément pouvoir lire le refus qui lui apprend qu'il
    // est périmé. Voir `le_refus_tolere_toute_version_mais_exige_le_champ`.
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":1,"motif":"version"}"#
    )
    .is_ok());
}

// ---------------------------------------------------------------------------
// Correction du 20 août 2026 — UN REFUS DOIT ÊTRE LISIBLE PAR SON DESTINATAIRE.
// ---------------------------------------------------------------------------

/// 🔴 LA ROUGE DU DÉFAUT 2, ET C'EST EXACTEMENT LE CAS MESURÉ EN RECETTE G1 :
/// un agent v1 face à une plateforme v2 reçoit `{"type":"refus","v":2,
/// "motif":"version"}` et ne peut pas le lire, parce que `verifie_version`
/// s'applique AUSSI au refus. Il tombe dans la branche « illisible », qui est
/// reprenable, et boucle sans terme — 0 ligne de refus, 10 reprises relevées.
///
/// Le cas est écrit dans le sens SYMÉTRIQUE (nous v2, l'émetteur v97) parce
/// que c'est celui que ce dépôt peut jouer sans figer une version morte : la
/// propriété exigée est « quelle que soit la version de l'émetteur », et elle
/// ne connaît pas de sens.
#[test]
fn un_refus_reste_lisible_quelle_que_soit_la_version_de_son_emetteur() {
    for brut in [
        r#"{"type":"refus","v":97,"motif":"version"}"#,
        r#"{"type":"refus","v":1,"motif":"enrolement"}"#,
    ] {
        let lu = serde_json::from_str::<DepuisLaPlateforme>(brut);
        assert!(
            lu.is_ok(),
            "refus illisible alors qu'il DOIT l'être : {brut} -> {:?}",
            lu.err()
        );
    }
}

/// L'autre moitié, sans laquelle la tolérance ci-dessus pourrait s'obtenir en
/// ne vérifiant plus RIEN : tout message qui n'est pas un refus reste refusé
/// sur une version divergente. Un `enrole` d'une version inconnue peut porter
/// un sens que nous ignorons, et l'accepter serait pire que de le rejeter.
#[test]
fn les_messages_autres_que_le_refus_restent_refuses_sur_une_version_divergente() {
    for brut in [
        r#"{"type":"enrole","v":97,"prefixe":"P","jeton":"j","expire_a":1}"#,
        r#"{"type":"battement-recu","v":97,"jeton":"j","expire_a":1}"#,
        r#"{"type":"lancer","v":97,"demande":"d","cle":"c"}"#,
    ] {
        assert!(
            serde_json::from_str::<DepuisLaPlateforme>(brut).is_err(),
            "message d'une version inconnue accepté : {brut}"
        );
    }
}

/// La table des motifs, parcourue dans les DEUX SENS sur les quatre variantes.
///
/// 🔴 C'EST CE QUI REMPLACE LE `rename_all` RETIRÉ, ET C'EST STRICTEMENT PLUS
/// FORT QUE LUI. La lacune que ce fichier documente pour `IssueLancement` —
/// « aucune variante n'a deux mots, donc `kebab-case` et `snake_case`
/// produisent les mêmes chaînes, et aucun test ne peut rougir sur un
/// changement de convention » — vaut à l'identique pour `MotifCanal`, dont les
/// quatre variantes sont d'un seul mot. Une table explicite, elle, rougit sur
/// n'importe quel changement de mot, à un mot comme à deux.
#[test]
fn la_table_des_motifs_fait_l_aller_retour_sur_les_quatre() {
    let attendus = [
        (MotifCanal::Version, "version"),
        (MotifCanal::Forme, "forme"),
        (MotifCanal::Enrolement, "enrolement"),
        (MotifCanal::Sequence, "sequence"),
    ];
    // 🔴 ANTI-OUBLI : `TOUS` doit couvrir exactement l'énumération ci-dessus.
    // Une variante ajoutée sans sa ligne ici rendrait ce compte faux.
    assert_eq!(MotifCanal::TOUS.len(), attendus.len());
    for (motif, mot) in attendus {
        assert!(MotifCanal::TOUS.contains(&motif), "{mot} absent de TOUS");
        assert_eq!(motif.mot(), mot);
        assert_eq!(MotifCanal::depuis_mot(mot), Some(motif));
    }
    // Un mot que nous ne connaissons pas ne devient JAMAIS un motif par
    // défaut : il se rend `None`, et l'appelant le journalise tel quel.
    assert_eq!(MotifCanal::depuis_mot("quota-depasse"), None);
    assert_eq!(MotifCanal::depuis_mot(""), None);
    assert_eq!(MotifCanal::depuis_mot("Version"), None);
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
        assert_eq!(motif, cas["motif"].as_str().expect("motif"), "motif de « {name} »");
    }
}
