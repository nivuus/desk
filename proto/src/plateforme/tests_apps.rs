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
        r#"{"type":"catalogue","v":2,"complet":true,"applications":[{"cle":"a1b2","nom":"Bloc-notes","chemin":"C:\\Users\\u\\Desktop\\Bloc-notes.lnk","cible":"c:\\windows\\system32\\notepad.exe","arguments":"","repertoire":"c:\\windows\\system32"}],"disparues":["disparue-1"]}"#
    );
}

#[test]
fn serialise_le_lancer() {
    let json = serde_json::to_string(&DepuisLaPlateforme::lancer("d-7", "a1b2")).expect("sér.");
    assert_eq!(json, r#"{"type":"lancer","v":2,"demande":"d-7","cle":"a1b2"}"#);
}

#[test]
fn serialise_la_lancee() {
    let json = serde_json::to_string(&VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"lancee","v":2,"demande":"d-7","issue":"raccourci"}"#
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
        r#"{"type":"catalogue","v":3,"complet":true,"applications":[],"disparues":[]}"#
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
        r#"{"type":"lancee","v":3,"demande":"d","issue":"echec"}"#
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
        r#"{"type":"lancer","v":3,"demande":"d","cle":"c"}"#
    )
    .is_err());
}

#[test]
fn rejette_un_champ_inconnu_sur_lancer() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"lancer","v":2,"demande":"d","cle":"c","bonus":1}"#
    )
    .is_err());
}
