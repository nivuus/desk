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

/// Rend le gabarit avec une version qui **n'est PAS la nôtre**.
///
/// 🔴 POURQUOI UN GABARIT PLUTÔT QU'UN LITTÉRAL, ET C'EST UNE LEÇON PAYÉE AU
/// BUMP DE G3. Les six tests ci-dessous portaient `"v":5` en dur — « la version
/// suivante » telle qu'elle se lisait au temps de G2. Le sous-bloc G3 a monté
/// `PLATEFORME_VERSION` à 4, et **les six ont alors affirmé que NOTRE PROPRE
/// version est rejetée**. Ils ont échoué bruyamment, ce qui est le bon
/// comportement — mais il a fallu les rouvrir un par un, et le prochain bump
/// aurait recommencé. **Dérivée de la constante, la version étrangère ne peut
/// plus vieillir.**
///
/// Le gabarit porte le repère `"v":0` : zéro n'est la version de personne, donc
/// un gabarit qu'on aurait oublié de faire passer ici échouerait, au lieu de
/// passer en éprouvant autre chose que ce qu'il annonce.
pub(super) fn etrangere(gabarit: &str) -> String {
    assert!(gabarit.contains("\"v\":0"), "le gabarit doit porter le repère \"v\":0");
    gabarit.replace("\"v\":0", &format!("\"v\":{}", PLATEFORME_VERSION.wrapping_add(1)))
}


#[test]
fn serialise_l_enrolement_en_kebab_case() {
    let json = serde_json::to_string(&VersLaPlateforme::enroler("w1", "chut")).expect("sér.");
    assert_eq!(json, r#"{"type":"enroler","v":5,"vm":"w1","secret":"chut"}"#);
}

#[test]
fn serialise_le_battement() {
    let json = serde_json::to_string(&VersLaPlateforme::battement()).expect("sér.");
    assert_eq!(json, r#"{"type":"battement","v":5}"#);
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
        r#"{"type":"battement-recu","v":5,"jeton":"kkk","expire_a":1787136774000}"#
    );
}

#[test]
fn serialise_l_enrole_et_le_refus() {
    let json = serde_json::to_string(&DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"enrole","v":5,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}"#
    );
    let json = serde_json::to_string(&DepuisLaPlateforme::refus(MotifCanal::Enrolement))
        .expect("sér.");
    assert_eq!(json, r#"{"type":"refus","v":5,"motif":"enrolement"}"#);
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
    assert!(serde_json::from_str::<VersLaPlateforme>(&etrangere(
        r#"{"type":"enroler","v":0,"vm":"w","secret":"s"}"#
    ))
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement() {
    assert!(
        serde_json::from_str::<VersLaPlateforme>(&etrangere(r#"{"type":"battement","v":0}"#))
            .is_err()
    );
}

#[test]
fn rejette_la_version_suivante_sur_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(&etrangere(
        r#"{"type":"enrole","v":0,"prefixe":"P","jeton":"j","expire_a":1}"#
    ))
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(&etrangere(
        r#"{"type":"battement-recu","v":0,"jeton":"j","expire_a":1}"#
    ))
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
        serde_json::from_str(&etrangere(r#"{"type":"refus","v":0,"motif":"version"}"#))
            .expect("lisible");
    // ⚠️ LA VERSION ATTENDUE SE DÉRIVE ELLE AUSSI. Elle valait `4` en dur, ce
    // qui était juste tant que 4 n'était la version de personne ; le bump de G3
    // l'a rendue nôtre, et l'assertion a échoué. C'est le même piège que
    // `etrangere` referme au-dessus, et il vaut aussi pour ce qu'on ATTEND, pas
    // seulement pour ce qu'on ENVOIE.
    assert_eq!(
        lu,
        DepuisLaPlateforme::Refus {
            version: PLATEFORME_VERSION.wrapping_add(1),
            motif: "version".into()
        }
    );
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
        &etrangere(r#"{"type":"refus","v":0,"motif":"version","detail":"x"}"#)
    )
    .is_err());
}

#[test]
fn rejette_un_type_inconnu() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"vol","v":5}"#).is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"vol","v":5}"#).is_err());
}

#[test]
fn rejette_un_champ_inconnu() {
    // `deny_unknown_fields` : un champ de trop est une divergence de
    // format, pas une extension tolérable — le canal n'a qu'une version.
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"battement","v":5,"bonus":1}"#
    )
    .is_err());
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
