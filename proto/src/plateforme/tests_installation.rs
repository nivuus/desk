//! The INSTALLATION messages, on the Rust side.
//!
//! 🔴 THIS FILE EXISTS FOR WHAT THE VECTORS CANNOT TEST.
//! `plateforme-vectors.json` is a set of ROUND-TRIPS: it freezes the strings
//! that both languages must produce and read back, and says nothing of what
//! must be REFUSED. Yet the most fragile guard of v4 is a refusal — a
//! `termine` whose `motif` key is MISSING.

use super::*;

fn termine_complet() -> String {
    format!(
        r#"{{"type":"termine","v":{PLATEFORME_VERSION},"installation":"i-1","issue":"reussie","motif":null,"code_sortie":0,"journal":"","journal_tronque":false}}"#
    )
}

#[test]
fn reads_a_complete_termine() {
    let lu: VersLaPlateforme = serde_json::from_str(&termine_complet()).expect("lisible");
    assert_eq!(
        lu,
        VersLaPlateforme::termine("i-1", Issue::Reussie, None, Some(0), "", false)
    );
}

/// 🔴 THE GUARD THAT MATTERS, AND WITHOUT IT THE FIELD WOULD SILENTLY BE
/// OPTIONAL.
///
/// `serde_derive` treats any field of type `Option<T>` as carrying an
/// IMPLICIT `#[serde(default)]`: a missing field becomes `None` without any
/// `default` having been written, and `deny_unknown_fields` changes nothing — it
/// looks at EXTRA fields, never at missing ones. That is exactly
/// what sub-block G2 measured for its `icone` field, and `champs::
/// option_obligatoire` is the generic form of its remedy.
///
/// **What would be lost without it**: the `termine` of an agent from an
/// older version — which has neither `motif` nor `code_sortie` — would be accepted by a
/// v4 platform, with a reason and a code silently missing. That is the
/// exact disguise the version bump exists to prevent.
#[test]
fn refuses_a_termine_missing_an_optional_key() {
    for cle in ["motif", "code_sortie"] {
        let ampute: String = {
            let mut doc: serde_json::Value =
                serde_json::from_str(&termine_complet()).expect("valide");
            doc.as_object_mut().expect("objet").remove(cle);
            doc.to_string()
        };
        assert!(
            serde_json::from_str::<VersLaPlateforme>(&ampute).is_err(),
            "a `termine` without « {cle} » must be refused, it must not be completed"
        );
    }
}

#[test]
fn accepts_null_on_both_keys_and_tells_it_from_absence() {
    let with_null = termine_complet().replace(r#""code_sortie":0"#, r#""code_sortie":null"#);
    let lu: VersLaPlateforme = serde_json::from_str(&with_null).expect("lisible");
    let VersLaPlateforme::Termine {
        motif, code_sortie, ..
    } = lu
    else {
        panic!("not a termine");
    };
    assert_eq!(motif, None);
    assert_eq!(code_sortie, None);
}

/// 🔴 A NEGATIVE EXIT CODE IS LEGITIMATE on Windows: failure `HRESULT`s
/// have the high bit set and read as a signed `i32`. Refusing it
/// would confuse "abnormal code" with "ordinary failure".
#[test]
fn accepts_a_negative_exit_code() {
    let negatif = termine_complet().replace(r#""code_sortie":0"#, r#""code_sortie":-1073741510"#);
    let lu: VersLaPlateforme = serde_json::from_str(&negatif).expect("lisible");
    let VersLaPlateforme::Termine { code_sortie, .. } = lu else {
        panic!("not a termine");
    };
    assert_eq!(code_sortie, Some(-1_073_741_510));
}

/// 🔴 THIS IS THE RED OF G1 LEGACY ITEM No. 9, AND IT IS PLAYABLE AT LAST.
///
/// G1 MEASURED that a `rename_all` is **unobservable** on an enum whose variants
/// all fit in one word: switching `kebab-case` to `snake_case` on
/// `IssueLancement` — `raccourci`, `cible`, `inconnue`, `echec` — leaves
/// `cargo test -p proto` entirely green. [`Issue`] has **two** two-word
/// variants, and that is deliberate: `SansEffet` and `IssueInconnue` make the
/// mutation visible.
///
/// ⚠️ **THE G3 PLAN PRESCRIBED THIS RED ON `Phase`, QUOTING
/// `sans-effet`** — a contradiction in its own text: `sans-effet`
/// belongs to `Issue`, and `Phase` has no two-word variant. It is
/// played here, on the enum that can carry it, and **we did NOT invent a
/// fourth phase** to make a mutation observable.
#[test]
fn the_two_two_word_outcome_variants_travel_in_kebab_case() {
    let paires = [
        (Issue::Reussie, "reussie"),
        (Issue::SansEffet, "sans-effet"),
        (Issue::IssueInconnue, "issue-inconnue"),
        (Issue::Refusee, "refusee"),
    ];
    for (variante, mot) in paires {
        assert_eq!(
            serde_json::to_string(&variante).expect("ser."),
            format!("\"{mot}\""),
            "variant {variante:?} must travel in kebab-case"
        );
        let relu: Issue = serde_json::from_str(&format!("\"{mot}\"")).expect("deser.");
        assert_eq!(relu, variante);
    }
    // ⚠️ AND THE `snake_case` FORM IS REFUSED, which is the half that makes
    // the mutation go red: without this line, a changed `rename_all` would make
    // the test wrong in one direction only.
    assert!(serde_json::from_str::<Issue>("\"sans_effet\"").is_err());
    assert!(serde_json::from_str::<Issue>("\"issue_inconnue\"").is_err());
}

#[test]
fn the_three_phases_travel_by_their_word() {
    for (variante, mot) in [
        (Phase::Transfert, "transfert"),
        (Phase::Execution, "execution"),
        (Phase::Reconciliation, "reconciliation"),
    ] {
        assert_eq!(
            serde_json::to_string(&variante).expect("ser."),
            format!("\"{mot}\"")
        );
    }
    // ⚠️ `empreinte` IS NOT A PHASE OF THIS CHANNEL: it takes place in the
    // BROWSER, before the platform has a single row to write.
    assert!(serde_json::from_str::<Phase>("\"empreinte\"").is_err());
}

/// The shape of the downstream `Installer`, and the fact that the URL travels in it — never
/// the bytes.
#[test]
fn the_install_order_carries_a_url_and_not_bytes() {
    let ordre = DepuisLaPlateforme::installer("i-1", "http://h:8080/t/c", "setup.exe", 42, "ab");
    let chain = serde_json::to_string(&ordre).expect("ser.");
    assert!(chain.contains(r#""url":"http://h:8080/t/c""#));
    assert!(!chain.contains("base64"));
    let relu: DepuisLaPlateforme = serde_json::from_str(&chain).expect("deser.");
    assert_eq!(relu, ordre);
}
