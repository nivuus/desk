//! Tests du module [`crate::plateforme`] — la GESTION D'APPLICATIONS.
//!
//! Extrait de `proto/src/plateforme/tests.rs` VERBATIM (sous-bloc G2, tâche 1) :
//! ce fichier était à 561 lignes et figurait au tableau de dette de
//! `CLAUDE.md` SANS POINT DE CHUTE, avec cette phrase — « c'est au chantier
//! qui les rouvrira de le choisir ». G2 les rouvre, et la règle du dépôt est
//! que le découpage rétroactif se fait au moment où l'on travaille dedans.
//! **Aucun test n'a été ajouté, retiré ni réécrit par ce déplacement** ; le
//! compte de `cargo test -p proto` a été annoncé AVANT d'être mesuré, et il est
//! resté à 83.
//!
//! **La frontière est celle que le protocole porte déjà** : le cycle de vie
//! (version, refus, enrôlement, battement) reste chez `tests.rs`, la gestion
//! d'apps vient ici. ⚠️ `les_variantes_de_p3_rejettent_desormais_la_version_1`
//! est RESTÉ chez `tests.rs` bien qu'il vécût dans la section G1 : il n'éprouve
//! que des variantes du cycle de vie, et le ranger ici l'aurait classé sur son
//! adresse plutôt que sur son objet.

use super::*;

// ---------------------------------------------------------------------------
// Sous-bloc G1 — catalogue d'applications et lancement (v2).
// ---------------------------------------------------------------------------

fn app_temoin() -> Application {
    Application {
        cle: "a1b2".into(),
        nom: "Bloc-notes".into(),
        chemin: r"C:\Users\u\Desktop\Bloc-notes.lnk".into(),
        cible: r"c:\windows\system32\notepad.exe".into(),
        arguments: String::new(),
        repertoire: r"c:\windows\system32".into(),
        icone: Some("a1b2".repeat(16)),
        source_max: SourceMax::Pixels(256),
    }
}

/// Le cas que le sous-bloc G2 doit rendre INDOLORE : une application dont
/// l'extraction d'icône a échoué. **Une application sans icône vaut mieux
/// qu'une application absente** (spec §7).
fn app_sans_icone() -> Application {
    Application {
        icone: None,
        source_max: SourceMax::NonMesuree,
        ..app_temoin()
    }
}

#[test]
fn serialise_le_catalogue() {
    let json = serde_json::to_string(&VersLaPlateforme::catalogue(
        true,
        vec![app_temoin()],
        vec!["disparue-1".into()],
    ))
    .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"catalogue","v":4,"complet":true,"applications":[{"cle":"a1b2","nom":"Bloc-notes","chemin":"C:\\Users\\u\\Desktop\\Bloc-notes.lnk","cible":"c:\\windows\\system32\\notepad.exe","arguments":"","repertoire":"c:\\windows\\system32","icone":"a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2","source_max":{"pixels":256}}],"disparues":["disparue-1"]}"#
    );
}

#[test]
fn serialise_le_lancer() {
    let json = serde_json::to_string(&DepuisLaPlateforme::lancer("d-7", "a1b2")).expect("sér.");
    assert_eq!(json, r#"{"type":"lancer","v":4,"demande":"d-7","cle":"a1b2"}"#);
}

#[test]
fn serialise_la_lancee() {
    let json = serde_json::to_string(&VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"lancee","v":4,"demande":"d-7","issue":"raccourci"}"#
    );
}

/// ⚠️ LACUNE ATTENDUE, ÉCRITE PLUTÔT QUE DÉCOUVERTE : aucune des quatre
/// issues n'a DEUX mots, donc `kebab-case` et `snake_case` ne diffèrent sur
/// aucune d'elles. Ce test fige les quatre chaînes, mais il ne peut pas
/// rougir sur un changement de `rename_all` — contrairement à
/// `battement-recu`, qui est le seul témoin de ce genre dans ce module. Le
/// jour où une issue à deux mots apparaîtra, elle devra porter son propre
/// test de casse, sans quoi elle divergera du miroir TypeScript en silence.
#[test]
fn serialise_les_quatre_issues() {
    let attendus = [
        (IssueLancement::Raccourci, "raccourci"),
        (IssueLancement::Cible, "cible"),
        (IssueLancement::Inconnue, "inconnue"),
        (IssueLancement::Echec, "echec"),
    ];
    for (issue, attendu) in attendus {
        assert_eq!(
            serde_json::to_string(&issue).expect("sér."),
            format!("\"{attendu}\"")
        );
    }
}

#[test]
fn rejette_une_version_absente_sur_catalogue() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"catalogue","complet":true,"applications":[],"disparues":[]}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_catalogue() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        &super::tests::etrangere(r#"{"type":"catalogue","v":0,"complet":true,"applications":[],"disparues":[]}"#)
    )
    .is_err());
}

#[test]
fn rejette_une_version_absente_sur_lancee() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"lancee","demande":"d","issue":"echec"}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_lancee() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        &super::tests::etrangere(r#"{"type":"lancee","v":0,"demande":"d","issue":"echec"}"#)
    )
    .is_err());
}

#[test]
fn rejette_une_version_absente_sur_lancer() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"lancer","demande":"d","cle":"c"}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_lancer() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        &super::tests::etrangere(r#"{"type":"lancer","v":0,"demande":"d","cle":"c"}"#)
    )
    .is_err());
}

#[test]
fn rejette_un_champ_inconnu_sur_lancer() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"lancer","v":4,"demande":"d","cle":"c","bonus":1}"#
    )
    .is_err());
}

// ---------------------------------------------------------------------------
// Sous-bloc G2 — les icônes, leur provenance, et l'inventaire des manquantes.
// ---------------------------------------------------------------------------

/// 🔴 LA FORME DE `SourceMax` SUR LE FIL, FIGÉE OCTET POUR OCTET.
///
/// C'est le critère ④ à sa source : `NonMesuree` ne peut structurellement pas
/// être un nombre. Représenter la valeur par `0` ou par `256` — ce qu'un
/// entier nullable inviterait à faire — ferait dire à une provenance INCONNUE
/// qu'elle vaut 256, c'est-à-dire exactement ce que tout le sous-bloc existe
/// pour distinguer.
#[test]
fn serialise_les_deux_formes_de_source_max() {
    assert_eq!(
        serde_json::to_string(&SourceMax::Pixels(256)).expect("sér."),
        r#"{"pixels":256}"#
    );
    assert_eq!(
        serde_json::to_string(&SourceMax::NonMesuree).expect("sér."),
        r#""non-mesuree""#
    );
    // Et l'aller-retour, dans les deux sens.
    for valeur in [SourceMax::Pixels(48), SourceMax::Pixels(256), SourceMax::NonMesuree] {
        let json = serde_json::to_string(&valeur).expect("sér.");
        let relu: SourceMax = serde_json::from_str(&json).expect("désér.");
        assert_eq!(valeur, relu);
    }
}

/// 🔴 LE TEST QUI NOMME LA LACUNE REFERMÉE, ET IL EST LE SEUL DE CE MODULE À
/// POUVOIR ROUGIR SUR `rename_all`.
///
/// `IssueLancement` porte quatre variantes d'UN SEUL MOT : `kebab-case` et
/// `snake_case` y produisent les mêmes chaînes, et son propre commentaire
/// inscrit cette lacune — mutation mesurée, `cargo test -p proto` restait
/// vert. `NonMesuree` a DEUX mots, donc `non-mesuree` contre `non_mesuree` :
/// muter le `rename_all` de [`SourceMax`] en `snake_case` fait ÉCHOUER ce
/// test, et c'est la démonstration que la convention y est OBSERVABLE.
///
/// ⚠️ La lacune reste OUVERTE pour `IssueLancement`, qu'aucune tâche de G2 ne
/// touche.
#[test]
fn la_convention_de_nommage_de_source_max_est_observable() {
    assert_eq!(
        serde_json::to_string(&SourceMax::NonMesuree).expect("sér."),
        r#""non-mesuree""#,
        "un tiret, pas un tiret bas : c'est ce que le miroir TypeScript lit"
    );
    assert!(serde_json::from_str::<SourceMax>(r#""non_mesuree""#).is_err());
}

/// Une application SANS icône traverse le fil, et son absence est explicite.
#[test]
fn serialise_une_application_sans_icone() {
    let json = serde_json::to_string(&app_sans_icone()).expect("sér.");
    assert!(
        json.contains(r#""icone":null"#),
        "l'absence d'icône s'ÉCRIT, elle ne se tait pas : {json}"
    );
    assert!(json.contains(r#""source_max":"non-mesuree""#), "{json}");
    let relu: Application = serde_json::from_str(&json).expect("désér.");
    assert_eq!(relu, app_sans_icone());
}

/// 🔴 AUCUN `#[serde(default)]` SUR LES DEUX CHAMPS NEUFS.
///
/// Un `default` ferait accepter en silence le catalogue d'un agent v2 — et
/// c'est précisément le déguisement que le bump de version existe pour
/// empêcher : la rupture doit se dire `version`, jamais `forme`, et surtout
/// pas rien du tout.
#[test]
fn rejette_une_application_a_qui_il_manque_un_champ_neuf() {
    let sans_icone = r#"{"cle":"a","nom":"n","chemin":"c","cible":"t","arguments":"","repertoire":"r","source_max":"non-mesuree"}"#;
    assert!(serde_json::from_str::<Application>(sans_icone).is_err());
    let sans_source = r#"{"cle":"a","nom":"n","chemin":"c","cible":"t","arguments":"","repertoire":"r","icone":null}"#;
    assert!(serde_json::from_str::<Application>(sans_source).is_err());
}

#[test]
fn serialise_les_icones_manquantes() {
    let json = serde_json::to_string(&DepuisLaPlateforme::icones_manquantes(vec![
        "a1b2".into(),
        "c3d4".into(),
    ]))
    .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"icones-manquantes","v":4,"empreintes":["a1b2","c3d4"]}"#
    );
    let relu: DepuisLaPlateforme = serde_json::from_str(&json).expect("désér.");
    assert_eq!(
        relu,
        DepuisLaPlateforme::icones_manquantes(vec!["a1b2".into(), "c3d4".into()])
    );
}

#[test]
fn rejette_une_version_absente_sur_icones_manquantes() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"icones-manquantes","empreintes":[]}"#
    )
    .is_err());
}

#[test]
fn rejette_la_version_suivante_sur_icones_manquantes() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        &super::tests::etrangere(r#"{"type":"icones-manquantes","v":0,"empreintes":[]}"#)
    )
    .is_err());
}

/// 🔴 LA ROUGE DU BUMP DE G2. Un agent v2 émet un catalogue sans les deux
/// champs neufs, et une plateforme v3 doit le REFUSER — pas le compléter, pas
/// l'ignorer.
#[test]
fn le_catalogue_d_un_agent_v2_est_refuse() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"catalogue","v":2,"complet":true,"applications":[],"disparues":[]}"#
    )
    .is_err());
}
