//! Tests du module [`crate::plateforme`] — la GESTION D'APPLICATIONS.
//!
//! Extracted from `proto/src/plateforme/tests.rs` VERBATIM (sub-block G2, task 1):
//! this file was at 561 lines and appeared in the debt table of
//! `CLAUDE.md` WITH NO LANDING POINT, with this sentence — "it is up to the workstream
//! that reopens them to choose it". G2 reopens them, and the repository's rule is
//! that the retroactive split happens when one works in it.
//! **No test was added, removed or rewritten by this move**; the
//! count of `cargo test -p proto` was announced BEFORE being measured, and it
//! stayed at 83.
//!
//! **The boundary is the one the protocol already carries**: the lifecycle
//! (version, refusal, enrolment, heartbeat) stays in `tests.rs`, app
//! management comes here. ⚠️ `les_variantes_de_p3_rejettent_desormais_la_version_1`
//! STAYED in `tests.rs` although it lived in the G1 section: it only tests
//! lifecycle variants, and filing it here would have classified it by its
//! address rather than by its object.

use super::*;

// ---------------------------------------------------------------------------
// Sub-block G1 — application catalogue and launch (v2).
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
        // ⚠️ BOTH ARE FILLED IN IN THE WITNESS, AND NOT LEFT AT THEIR
        // NEUTRAL VALUE: an encoder that OMITTED either of them would render the
        // same JSON as a witness where they were `None` and `[]`, and the
        // round-trip test could not see it. The neutral case is tested
        // separately, by `app_sans_icone`.
        accent: Some("#3f2a7a".into()),
        associations: vec![".txt".into(), ".log".into()],
    }
}

/// The case sub-block G2 must make PAINLESS: an application whose
/// icon extraction failed. **An application without an icon is better
/// than an absent application** (spec §7).
fn app_sans_icone() -> Application {
    Application {
        icone: None,
        source_max: SourceMax::NonMesuree,
        // Without an icon, there is no dominant to compute: `accent` follows.
        accent: None,
        associations: Vec::new(),
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
        r##"{"type":"catalogue","v":5,"complet":true,"applications":[{"cle":"a1b2","nom":"Bloc-notes","chemin":"C:\\Users\\u\\Desktop\\Bloc-notes.lnk","cible":"c:\\windows\\system32\\notepad.exe","arguments":"","repertoire":"c:\\windows\\system32","icone":"a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2a1b2","source_max":{"pixels":256},"accent":"#3f2a7a","associations":[".txt",".log"]}],"disparues":["disparue-1"]}"##
    );
}

#[test]
fn serialise_le_lancer() {
    let json = serde_json::to_string(&DepuisLaPlateforme::lancer("d-7", "a1b2")).expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"lancer","v":5,"demande":"d-7","cle":"a1b2"}"#
    );
}

#[test]
fn serialise_la_lancee() {
    let json = serde_json::to_string(&VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci))
        .expect("sér.");
    assert_eq!(
        json,
        r#"{"type":"lancee","v":5,"demande":"d-7","issue":"raccourci"}"#
    );
}

/// ⚠️ EXPECTED GAP, WRITTEN RATHER THAN DISCOVERED: none of the four
/// outcomes has TWO words, so `kebab-case` and `snake_case` differ on
/// none of them. This test freezes the four strings, but it cannot
/// turn red on a change of `rename_all` — unlike
/// `battement-recu`, which is the only witness of that kind in this module. The
/// day a two-word outcome appears, it will have to carry its own
/// case test, otherwise it will diverge from the TypeScript mirror silently.
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
    assert!(
        serde_json::from_str::<VersLaPlateforme>(&super::tests::etrangere(
            r#"{"type":"catalogue","v":0,"complet":true,"applications":[],"disparues":[]}"#
        ))
        .is_err()
    );
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
    assert!(
        serde_json::from_str::<VersLaPlateforme>(&super::tests::etrangere(
            r#"{"type":"lancee","v":0,"demande":"d","issue":"echec"}"#
        ))
        .is_err()
    );
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
    assert!(
        serde_json::from_str::<DepuisLaPlateforme>(&super::tests::etrangere(
            r#"{"type":"lancer","v":0,"demande":"d","cle":"c"}"#
        ))
        .is_err()
    );
}

#[test]
fn rejette_un_champ_inconnu_sur_lancer() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"lancer","v":5,"demande":"d","cle":"c","bonus":1}"#
    )
    .is_err());
}

// ---------------------------------------------------------------------------
// Sub-block G2 — the icons, their provenance, and the inventory of the missing ones.
// ---------------------------------------------------------------------------

/// 🔴 THE WIRE SHAPE OF `SourceMax`, FROZEN BYTE FOR BYTE.
///
/// It is criterion ④ at its source: `NonMesuree` cannot structurally
/// be a number. Representing the value by `0` or by `256` — which a
/// nullable integer would invite — would make an UNKNOWN provenance say
/// it is 256, that is exactly what the whole sub-block exists
/// to distinguish.
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
    // And the round trip, in both directions.
    for value in [
        SourceMax::Pixels(48),
        SourceMax::Pixels(256),
        SourceMax::NonMesuree,
    ] {
        let json = serde_json::to_string(&value).expect("sér.");
        let relu: SourceMax = serde_json::from_str(&json).expect("désér.");
        assert_eq!(value, relu);
    }
}

/// 🔴 THE TEST THAT NAMES THE CLOSED GAP, AND IT IS THE ONLY ONE IN THIS MODULE
/// ABLE TO TURN RED ON `rename_all`.
///
/// `IssueLancement` carries four SINGLE-WORD variants: `kebab-case` and
/// `snake_case` produce the same strings there, and its own comment
/// records this gap — measured mutation, `cargo test -p proto` stayed
/// green. `NonMesuree` has TWO words, hence `non-mesuree` against `non_mesuree`:
/// mutating the `rename_all` of [`SourceMax`] to `snake_case` makes this
/// test FAIL, and it is the demonstration that the convention is OBSERVABLE there.
///
/// ⚠️ The gap stays OPEN for `IssueLancement`, which no G2 task
/// touches.
#[test]
fn la_convention_de_nommage_de_source_max_est_observable() {
    assert_eq!(
        serde_json::to_string(&SourceMax::NonMesuree).expect("sér."),
        r#""non-mesuree""#,
        "un tiret, pas un tiret bas : c'est ce que le miroir TypeScript lit"
    );
    assert!(serde_json::from_str::<SourceMax>(r#""non_mesuree""#).is_err());
}

/// An application WITHOUT an icon crosses the wire, and its absence is explicit.
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
/// A `default` would silently accept the catalogue of a v2 agent — and
/// that is precisely the disguise the version bump exists to
/// prevent: the break must say `version`, never `forme`, and above all
/// not nothing at all.
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
        r#"{"type":"icones-manquantes","v":5,"empreintes":["a1b2","c3d4"]}"#
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
    assert!(
        serde_json::from_str::<DepuisLaPlateforme>(&super::tests::etrangere(
            r#"{"type":"icones-manquantes","v":0,"empreintes":[]}"#
        ))
        .is_err()
    );
}

/// 🔴 THE RED OF THE G2 BUMP. A v2 agent emits a catalogue without the two
/// new fields, and a v3 platform must REFUSE it — not complete it, not
/// ignore it.
#[test]
fn le_catalogue_d_un_agent_v2_est_refuse() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"catalogue","v":2,"complet":true,"applications":[],"disparues":[]}"#
    )
    .is_err());
}
