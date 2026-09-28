//! Tests du module [`crate::plateforme`].
//!
//! Extracted from `proto/src/plateforme.rs` VERBATIM (sub-block G1, task 1): the
//! parent file was at 410 lines, 244 of them tests, and the 500-line
//! rule requires the extraction to precede the addition. No test was
//! added, removed or rewritten by this move.
//!
//! ⚠️ `include_str!` resolves RELATIVE TO THE FILE THAT CONTAINS IT: the
//! path of the shared vectors thus gained a `../` by going down one
//! level. It is the only line whose TEXT differs from the original; all the
//! rest only lost its four spaces of wrapper indentation.
//!
//! ⚠️ **THIS FILE WAS SPLIT A SECOND TIME (sub-block G2, task 1)**:
//! it had grown to 561 lines and appeared in the debt table of `CLAUDE.md`.
//! What remains here is the **lifecycle** — version, refusal, enrolment,
//! heartbeat, and conformance to the shared vectors, which carries the assertion
//! on `doc["version"]` and thus reads as a version test. **App
//! management** now lives in `plateforme/tests_apps.rs`. No test was
//! added, removed or rewritten by this second move.

use super::*;

/// Renders the template with a version that **is NOT ours**.
///
/// 🔴 WHY A TEMPLATE RATHER THAN A LITERAL, AND IT IS A LESSON PAID FOR AT THE
/// G3 BUMP. The six tests below carried a hard-coded `"v":5` — "the next
/// version" as it read in the time of G2. Sub-block G3 raised
/// `PLATEFORME_VERSION` to 4, and **the six then asserted that OUR OWN
/// version is rejected**. They failed loudly, which is the right
/// behaviour — but they had to be reopened one by one, and the next bump
/// would have started again. **Derived from the constant, the foreign version can
/// no longer age.**
///
/// The template carries the marker `"v":0`: zero is nobody's version, so
/// a template one forgot to pass through here would fail, instead of
/// passing while testing something other than what it announces.
pub(super) fn etrangere(gabarit: &str) -> String {
    assert!(
        gabarit.contains("\"v\":0"),
        "the template must carry the marker \"v\":0"
    );
    gabarit.replace(
        "\"v\":0",
        &format!("\"v\":{}", PLATEFORME_VERSION.wrapping_add(1)),
    )
}

#[test]
fn serialises_the_enrolment_in_kebab_case() {
    let json = serde_json::to_string(&VersLaPlateforme::enroler("w1", "chut")).expect("ser.");
    assert_eq!(
        json,
        r#"{"type":"enroler","v":5,"vm":"w1","secret":"chut"}"#
    );
}

#[test]
fn serialises_the_heartbeat() {
    let json = serde_json::to_string(&VersLaPlateforme::battement()).expect("ser.");
    assert_eq!(json, r#"{"type":"battement","v":5}"#);
}

#[test]
fn serialises_the_received_heartbeat_in_kebab_case() {
    // 🔴 `battement-recu` IS THE ONLY TWO-WORD VARIANT OF THE MODULE, hence
    // the only one where `kebab-case` and `snake_case` differ. Without this test,
    // switching `rename_all` to `snake_case` turned NOTHING red — MEASURED: the
    // mutation stayed green on the 50 tests. Rust would then emit
    // `battement_recu` where the TypeScript mirror reads `battement-recu`, and
    // the two ends would diverge SILENTLY on the message the agent
    // receives most often.
    let json = serde_json::to_string(&DepuisLaPlateforme::battement_recu(
        "kkk",
        1_787_136_774_000,
    ))
    .expect("ser.");
    assert_eq!(
        json,
        r#"{"type":"battement-recu","v":5,"jeton":"kkk","expire_a":1787136774000}"#
    );
}

#[test]
fn serialises_the_enrolled_and_the_refusal() {
    let json = serde_json::to_string(&DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742))
        .expect("ser.");
    assert_eq!(
        json,
        r#"{"type":"enrole","v":5,"prefixe":"PPP","jeton":"jjj","expire_a":1787136773742}"#
    );
    let json =
        serde_json::to_string(&DepuisLaPlateforme::refus(MotifCanal::Enrolement)).expect("ser.");
    assert_eq!(json, r#"{"type":"refus","v":5,"motif":"enrolement"}"#);
}

// 🔴 ONE VERSION TEST PER INCOMING VARIANT, never a single one for all.
// `check_version` is wired variant by variant: omitting it on ONE
// alone would leave that hole open, and a single test would not see it.

#[test]
fn rejects_an_absent_version_on_enroler() {
    assert!(serde_json::from_str::<VersLaPlateforme>(
        r#"{"type":"enroler","vm":"w","secret":"s"}"#
    )
    .is_err());
}

#[test]
fn rejects_an_absent_version_on_battement() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement"}"#).is_err());
}

#[test]
fn rejects_an_absent_version_on_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"enrole","prefixe":"P","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejects_an_absent_version_on_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"battement-recu","jeton":"j","expire_a":1}"#
    )
    .is_err());
}

#[test]
fn rejects_an_absent_version_on_refus() {
    assert!(
        serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"refus","motif":"version"}"#)
            .is_err()
    );
}

#[test]
fn rejects_the_next_version_on_enroler() {
    assert!(serde_json::from_str::<VersLaPlateforme>(&etrangere(
        r#"{"type":"enroler","v":0,"vm":"w","secret":"s"}"#
    ))
    .is_err());
}

#[test]
fn rejects_the_next_version_on_battement() {
    assert!(
        serde_json::from_str::<VersLaPlateforme>(&etrangere(r#"{"type":"battement","v":0}"#))
            .is_err()
    );
}

#[test]
fn rejects_the_next_version_on_enrole() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(&etrangere(
        r#"{"type":"enrole","v":0,"prefixe":"P","jeton":"j","expire_a":1}"#
    ))
    .is_err());
}

#[test]
fn rejects_the_next_version_on_battement_recu() {
    assert!(serde_json::from_str::<DepuisLaPlateforme>(&etrangere(
        r#"{"type":"battement-recu","v":0,"jeton":"j","expire_a":1}"#
    ))
    .is_err());
}

/// ❌ **THIS TEST PINNED DEFECT 2, AND IT WAS REVERSED ON 20 AUGUST 2026.**
/// It required a refusal of a neighbouring version to be REJECTED — that is
/// exactly what kept an obsolete agent from reading why it was.
/// The property it guarded ("each incoming variant checks its
/// version") stays guarded by its four twins above and by
/// `messages_other_than_the_refusal_stay_refused_on_a_diverging_version`;
/// **the refusal, however, is removed from it on purpose**, and that is what this test
/// now says. The `v` field stays MANDATORY: the tolerance bears on its
/// VALUE, never on its presence.
#[test]
fn the_refusal_tolerates_any_version_but_requires_the_field() {
    let lu: DepuisLaPlateforme =
        serde_json::from_str(&etrangere(r#"{"type":"refus","v":0,"motif":"version"}"#))
            .expect("lisible");
    // ⚠️ THE EXPECTED VERSION IS DERIVED TOO. It was a hard-coded `4`, which
    // was right as long as 4 was nobody's version; the G3 bump
    // made it ours, and the assertion failed. It is the same trap that
    // `etrangere` closes above, and it holds also for what we EXPECT, not
    // only for what we SEND.
    assert_eq!(
        lu,
        DepuisLaPlateforme::Refus {
            version: PLATEFORME_VERSION.wrapping_add(1),
            motif: "version".into()
        }
    );
    // Without `v`, on the other hand, it is still an invalid shape: a message
    // without a version is not a message of a version we do not know. And
    // `v: null` neither — it is the exact hole that `check_version` closes
    // for the other variants, and that `version_toleree` does not reopen.
    assert!(
        serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"refus","motif":"version"}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":null,"motif":"version"}"#
    )
    .is_err());
    // And the shape stays FROZEN: one more field is refused
    // (`deny_unknown_fields`), which is clause 3 of the module header —
    // written as a constraint on FUTURE versions, tested here.
    assert!(serde_json::from_str::<DepuisLaPlateforme>(&etrangere(
        r#"{"type":"refus","v":0,"motif":"version","detail":"x"}"#
    ))
    .is_err());
}

#[test]
fn rejects_an_unknown_type() {
    assert!(serde_json::from_str::<VersLaPlateforme>(r#"{"type":"vol","v":5}"#).is_err());
    assert!(serde_json::from_str::<DepuisLaPlateforme>(r#"{"type":"vol","v":5}"#).is_err());
}

#[test]
fn rejects_an_unknown_field() {
    // `deny_unknown_fields`: an extra field is a format divergence,
    // not a tolerable extension — the channel has only one version.
    assert!(
        serde_json::from_str::<VersLaPlateforme>(r#"{"type":"battement","v":5,"bonus":1}"#)
            .is_err()
    );
}

#[test]
fn round_trip_of_the_three_answers() {
    for message in [
        DepuisLaPlateforme::enrole("PPP", "jjj", 1_787_136_773_742),
        DepuisLaPlateforme::battement_recu("kkk", 1_787_136_774_000),
        DepuisLaPlateforme::refus(MotifCanal::Sequence),
    ] {
        let json = serde_json::to_string(&message).expect("ser.");
        let relu: DepuisLaPlateforme = serde_json::from_str(&json).expect("deser.");
        assert_eq!(message, relu);
    }
}

/// 🔴 THE RED OF THE BUMP ITSELF. If `PLATEFORME_VERSION` stayed at 1, this
/// test would stay green on the new variants alone and the break would
/// not be played: it is here that we assert that an agent deployed in the
/// v1 format IS NO LONGER UNDERSTOOD, and that the `version` refusal IS NOT RETRIED
/// (module header). Agent and platform are deployed at the same commit.
#[test]
fn the_p3_variants_now_reject_version_1() {
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
    // ⚠️ THE REFUSAL IS DELIBERATELY ABSENT FROM THIS SET since the fix
    // of 20 August 2026: it is the ONLY variant outside versioning, and a
    // v1 agent must precisely be able to read the refusal that tells it it
    // is obsolete. See `the_refusal_tolerates_any_version_but_requires_the_field`.
    assert!(serde_json::from_str::<DepuisLaPlateforme>(
        r#"{"type":"refus","v":1,"motif":"version"}"#
    )
    .is_ok());
}

// ---------------------------------------------------------------------------
// Fix of 20 August 2026 — A REFUSAL MUST BE READABLE BY ITS RECIPIENT.
// ---------------------------------------------------------------------------

/// 🔴 THE RED OF DEFECT 2, AND IT IS EXACTLY THE CASE MEASURED IN THE G1 ACCEPTANCE RUN:
/// a v1 agent facing a v2 platform receives `{"type":"refus","v":2,
/// "motif":"version"}` and cannot read it, because `check_version`
/// ALSO applies to the refusal. It falls into the "unreadable" branch, which is
/// resumable, and loops with no end — 0 refusal line, 10 resumptions recorded.
///
/// The case is written in the SYMMETRIC direction (us v2, the sender v97) because
/// it is the one this repository can play without freezing a dead version: the
/// required property is "whatever the sender's version", and it
/// knows no direction.
#[test]
fn a_refusal_stays_readable_whatever_the_version_of_its_sender() {
    for brut in [
        r#"{"type":"refus","v":97,"motif":"version"}"#,
        r#"{"type":"refus","v":1,"motif":"enrolement"}"#,
    ] {
        let lu = serde_json::from_str::<DepuisLaPlateforme>(brut);
        assert!(
            lu.is_ok(),
            "refusal unreadable although it MUST be readable: {brut} -> {:?}",
            lu.err()
        );
    }
}

/// The other half, without which the tolerance above could be obtained by
/// no longer checking ANYTHING: every message that is not a refusal stays refused
/// on a diverging version. An `enrole` of an unknown version may carry
/// a meaning we do not know, and accepting it would be worse than rejecting it.
#[test]
fn messages_other_than_the_refusal_stay_refused_on_a_diverging_version() {
    for brut in [
        r#"{"type":"enrole","v":97,"prefixe":"P","jeton":"j","expire_a":1}"#,
        r#"{"type":"battement-recu","v":97,"jeton":"j","expire_a":1}"#,
        r#"{"type":"lancer","v":97,"demande":"d","cle":"c"}"#,
    ] {
        assert!(
            serde_json::from_str::<DepuisLaPlateforme>(brut).is_err(),
            "message of an unknown version accepted: {brut}"
        );
    }
}

/// The table of reasons, walked in BOTH DIRECTIONS over the four variants.
///
/// 🔴 IT IS WHAT REPLACES THE REMOVED `rename_all`, AND IT IS STRICTLY STRONGER
/// THAN IT. The gap this file documents for `IssueLancement` —
/// "no variant has two words, so `kebab-case` and `snake_case`
/// produce the same strings, and no test can turn red on a
/// change of convention" — holds identically for `MotifCanal`, whose
/// four variants are single-word. An explicit table, on the other hand, turns red on
/// any change of word, with one word as with two.
#[test]
fn the_reason_table_makes_the_round_trip_on_all_four() {
    let attendus = [
        (MotifCanal::Version, "version"),
        (MotifCanal::Forme, "forme"),
        (MotifCanal::Enrolement, "enrolement"),
        (MotifCanal::Sequence, "sequence"),
    ];
    // 🔴 ANTI-OMISSION: `ALL` must cover exactly the enumeration above.
    // A variant added without its line here would make this count wrong.
    assert_eq!(MotifCanal::ALL.len(), attendus.len());
    for (motif, mot) in attendus {
        assert!(MotifCanal::ALL.contains(&motif), "{mot} absent from TOUS");
        assert_eq!(motif.mot(), mot);
        assert_eq!(MotifCanal::depuis_mot(mot), Some(motif));
    }
    // A word we do not know NEVER becomes a default reason: it
    // returns `None`, and the caller logs it as is.
    assert_eq!(MotifCanal::depuis_mot("quota-depasse"), None);
    assert_eq!(MotifCanal::depuis_mot(""), None);
    assert_eq!(MotifCanal::depuis_mot("Version"), None);
}
