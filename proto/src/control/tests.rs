use super::*;

#[test]
fn serialise_le_redimensionnement() {
    let json = serde_json::to_string(&ClientControl::resize(1280, 720)).expect("sérialisation");
    // `type` is the internal tag of the enum: serde emits it before the other
    // fields, `v` included, whatever their declaration order.
    assert_eq!(json, r#"{"type":"resize","v":3,"width":1280,"height":720}"#);
}

#[test]
fn deserialise_le_redimensionnement() {
    let msg: ClientControl =
        serde_json::from_str(r#"{"v":3,"type":"resize","width":800,"height":600}"#)
            .expect("désérialisation");
    assert_eq!(msg, ClientControl::resize(800, 600));
}

#[test]
fn serialise_ready_et_session_end() {
    assert_eq!(
        serde_json::to_string(&AgentControl::ready(1920, 1080, false)).unwrap(),
        r#"{"type":"ready","v":3,"width":1920,"height":1080,"mic":false}"#
    );
    assert_eq!(
        serde_json::to_string(&AgentControl::session_end("fenêtre fermée")).unwrap(),
        r#"{"type":"session-end","v":3,"reason":"fenêtre fermée"}"#
    );
}

#[test]
fn rejette_un_type_inconnu() {
    let err = serde_json::from_str::<ClientControl>(r#"{"v":1,"type":"vol","x":1}"#);
    assert!(err.is_err());
}

#[test]
fn rejette_une_version_absente() {
    let err = serde_json::from_str::<ClientControl>(r#"{"type":"resize","width":1,"height":1}"#);
    assert!(err.is_err());
}

#[test]
fn rejette_une_version_inconnue() {
    let err =
        serde_json::from_str::<ClientControl>(r#"{"v":9,"type":"resize","width":1,"height":1}"#);
    assert!(err.is_err());
}

#[test]
fn serialise_le_message_de_pointeur_en_kebab_case() {
    let json = serde_json::to_string(&AgentControl::pointer(false, CursorShape::NsResize))
        .expect("sérialisation");
    assert_eq!(
        json,
        r#"{"type":"pointer","v":3,"visible":false,"shape":"ns-resize"}"#
    );
}

#[test]
fn round_trip_du_message_de_vibration() {
    let message = AgentControl::rumble(255, 0);
    let json = serde_json::to_string(&message).expect("sérialisation");
    let relu: AgentControl = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(message, relu);
}

#[test]
fn round_trip_des_capacites() {
    let message = AgentControl::capabilities(true, true);
    let json = serde_json::to_string(&message).expect("sérialisation");
    let relu: AgentControl = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(message, relu);
}

#[test]
fn serialise_l_etat_du_lien() {
    let json = serde_json::to_string(&AgentControl::link(
        4_000_000,
        (1280, 720),
        LinkQuality::Degradee,
        LinkAdaptation::Active,
    ))
    .expect("sérialisation");
    assert_eq!(
        json,
        r#"{"type":"link","v":3,"bitrate":4000000,"width":1280,"height":720,"quality":"degradee","adaptation":"active"}"#
    );
}

#[test]
fn deserialise_l_etat_du_lien() {
    let msg: AgentControl = serde_json::from_str(
        r#"{"type":"link","v":3,"bitrate":1500000,"width":960,"height":540,"quality":"insuffisante","adaptation":"indisponible"}"#,
    )
    .expect("désérialisation");
    assert_eq!(
        msg,
        AgentControl::link(
            1_500_000,
            (960, 540),
            LinkQuality::Insuffisante,
            LinkAdaptation::Indisponible
        )
    );
}

#[test]
fn rejette_la_version_de_controle_1_devenue_obsolete() {
    let err =
        serde_json::from_str::<AgentControl>(r#"{"type":"ready","v":1,"width":1,"height":1}"#);
    assert!(err.is_err());
}

#[test]
fn une_visibilite_se_relit_telle_qu_ecrite() {
    let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
    let message: ClientControl = serde_json::from_str(json).expect("visibilité valide");
    assert_eq!(
        message,
        ClientControl::Visibility {
            version: 3,
            visible: false,
            focused: false
        }
    );
}

#[test]
fn une_visibilite_de_mauvaise_version_est_rejetee() {
    let json = r#"{"type":"visibility","v":1,"visible":true,"focused":true}"#;
    assert!(serde_json::from_str::<ClientControl>(json).is_err());
}

#[test]
fn a_sleep_is_written_with_its_type_first_and_its_reason() {
    let json =
        serde_json::to_string(&AgentControl::asleep(true, "evincee")).expect("sérialisation");
    assert!(json.starts_with(r#"{"type":"asleep""#), "obtenu : {json}");
    assert!(json.contains(r#""asleep":true"#), "obtenu : {json}");
    assert!(json.contains(r#""reason":"evincee""#), "obtenu : {json}");
}

#[test]
fn fullscreen_serialises_in_kebab_case_with_its_version() {
    let json = serde_json::to_string(&AgentControl::fullscreen(true)).unwrap();
    assert!(json.contains(r#""type":"fullscreen""#), "{json}");
    assert!(json.contains(r#""active":true"#), "{json}");
    assert!(json.contains(r#""v":"#), "{json}");
}

#[test]
fn le_plein_ecran_fait_l_aller_retour() {
    let origine = AgentControl::fullscreen(false);
    let json = serde_json::to_string(&origine).unwrap();
    let relu: AgentControl = serde_json::from_str(&json).unwrap();
    assert_eq!(relu, origine);
}

#[test]
fn all_cursor_shapes_are_css_values() {
    // The client sets this string as is into `style.cursor`: an
    // unrecognised value would be silently ignored by the
    // browser, hence invisible in tests.
    for (forme, attendu) in [
        (CursorShape::Default, "default"),
        (CursorShape::Text, "text"),
        (CursorShape::NotAllowed, "not-allowed"),
        (CursorShape::NwseResize, "nwse-resize"),
    ] {
        let json = serde_json::to_string(&forme).expect("sérialisation");
        assert_eq!(json, format!("\"{attendu}\""));
    }
}

/// Workstream E: `Ready` carries the microphone availability, **without a bump of
/// `CONTROL_VERSION`**.
///
/// The TypeScript parser checks `v`, then `type`, then CASTS: an extra
/// field is simply ignored by an old client. And a RECENT client
/// talking to an OLD agent reads `mic === undefined`, hence falsy,
/// hence no button — exactly the rule of spec §10, for free.
#[test]
fn ready_porte_la_disponibilite_du_micro() {
    assert_eq!(
        serde_json::to_string(&AgentControl::ready(1920, 1080, true)).unwrap(),
        r#"{"type":"ready","v":3,"width":1920,"height":1080,"mic":true}"#
    );
}

/// "The field is optional on read and ITS ABSENCE MEANS `false`"
/// (spec §10): a recent client facing an old agent does not show a
/// button that would lead nowhere.
///
/// ⚠️ `AgentControl` carries `deny_unknown_fields`: that does not hinder
/// ADDING a field, but a MISSING field is a
/// deserialization error in Rust. `#[serde(default)]` is therefore MANDATORY.
#[test]
fn a_ready_without_mic_reads_with_mic_false() {
    let m: AgentControl =
        serde_json::from_str(r#"{"type":"ready","v":3,"width":1,"height":1}"#).unwrap();
    assert_eq!(m, AgentControl::ready(1, 1, false));
}

#[test]
fn serialises_the_clipboard_with_its_text() {
    let json = serde_json::to_string(&AgentControl::clipboard(Some("bonjour".into()), 7))
        .expect("sérialisation");
    assert_eq!(
        json,
        r#"{"type":"clipboard","v":3,"text":"bonjour","bytes":7}"#
    );
}

/// 🔴 A REFUSAL serializes `"text":null`, field PRESENT.
///
/// RED if one sets `#[serde(skip_serializing_if = "Option::is_none")]`:
/// the field would vanish, and the client could no longer tell a refusal
/// from a message truncated on the way.
#[test]
fn un_refus_de_presse_papier_serialise_un_text_null_present() {
    let json =
        serde_json::to_string(&AgentControl::clipboard(None, 102_400)).expect("sérialisation");
    assert_eq!(
        json,
        r#"{"type":"clipboard","v":3,"text":null,"bytes":102400}"#
    );
    assert!(
        json.contains("\"text\":null"),
        "le champ text doit rester présent"
    );
}

/// 🔴 What proves that `check_version` is indeed wired onto the NEW
/// variant — forgetting it is a silent error, `version` being checked
/// only through its attribute.
#[test]
fn un_presse_papier_en_version_2_est_rejete() {
    let brut = r#"{"type":"clipboard","v":2,"text":"bonjour","bytes":7}"#;
    let error = serde_json::from_str::<AgentControl>(brut).expect_err("v:2 doit être rejeté");
    assert!(
        error
            .to_string()
            .contains("version de contrôle non supportée"),
        "message inattendu : {error}"
    );
}

/// RED if one removes `deny_unknown_fields` from the enum: this test pins it
/// for the new variant.
#[test]
fn un_presse_papier_portant_un_champ_inconnu_est_rejete() {
    let brut = r#"{"type":"clipboard","v":3,"text":"bonjour","bytes":7,"surprise":1}"#;
    assert!(serde_json::from_str::<AgentControl>(brut).is_err());
}

// ---------------------------------------------------------------------------
// Sub-block P2 of the clipboard workstream — the browser → VM direction.
// ---------------------------------------------------------------------------

/// RED if the `ClientControl::Clipboard` variant is absent.
#[test]
fn deserialise_le_collage_venu_du_client() {
    let msg: ClientControl = serde_json::from_str(r#"{"v":3,"type":"clipboard","text":"bonjour"}"#)
        .expect("désérialisation");
    assert_eq!(msg, ClientControl::clipboard("bonjour"));
}

#[test]
fn serialise_le_collage_venu_du_client() {
    assert_eq!(
        serde_json::to_string(&ClientControl::clipboard("bonjour")).unwrap(),
        r#"{"type":"clipboard","v":3,"text":"bonjour"}"#
    );
}

/// 🔴 RED if one forgets `deserialize_with = "check_version"` on the
/// `version` field — **it is the line one omits when copying a neighbouring
/// variant**, and nothing else in this repository would see it: the variant
/// would work, it would simply accept any version at all.
#[test]
fn un_collage_client_a_la_mauvaise_version_est_rejete() {
    let error = serde_json::from_str::<ClientControl>(r#"{"v":2,"type":"clipboard","text":"x"}"#);
    assert!(error.is_err(), "une version 2 doit être refusée");
}

/// 🔴 RED if the variant were placed on an enum without `deny_unknown_fields`:
/// a badly behaved client could then push anything through.
#[test]
fn a_client_paste_with_an_extra_field_is_rejected() {
    let error =
        serde_json::from_str::<ClientControl>(r#"{"v":3,"type":"clipboard","text":"x","bytes":1}"#);
    assert!(error.is_err(), "un champ inconnu doit être refusé");
}

/// 🔴 ROUGE si l'on oublie `#[serde(default)]` sur `Capabilities::clipboard`.
///
/// ⚠️ **The reason is NOT `deny_unknown_fields`**, contrary to what the
/// spec claims: `deny_unknown_fields` refuses an UNKNOWN field; it is
/// serde's default that refuses a MISSING field. The two mechanisms have
/// nothing to do with each other, and it is the comment on `mic` that says the right thing.
///
/// An agent from before P2 does not emit this field; a recent deserializer must therefore
/// tolerate it and read `false`.
#[test]
fn capabilities_sans_clipboard_se_deserialise_a_false() {
    let msg: AgentControl = serde_json::from_str(r#"{"v":3,"type":"capabilities","gamepad":true}"#)
        .expect("désérialisation");
    assert_eq!(msg, AgentControl::capabilities(true, false));
}

/// RED if one set a `skip_serializing_if`: the field would vanish
/// when it is `false`, and the client could no longer tell "the agent
/// says no" from "the agent is too old to say". Both are handled
/// the same way today, but the distinction is what will one
/// day allow logging it.
#[test]
fn capabilities_serialise_les_deux_champs() {
    assert_eq!(
        serde_json::to_string(&AgentControl::capabilities(false, true)).unwrap(),
        r#"{"type":"capabilities","v":3,"gamepad":false,"clipboard":true}"#
    );
}

/// 🔴 **THE CLIPBOARD MUST NEVER REACH A LOG, AND THIS TEST IS
/// THE ONLY RAMPART.**
///
/// It was born from a MEASUREMENT, not a precaution: the P2 acceptance run found
/// in `agent.log` four lines carrying the clipboard content **in
/// clear** — `Clipboard { version: 3, text: "alpha-arme-1-crwor9" }` —, because
/// `demarrage.rs` prints the received message through `?message` and
/// `ClientControl` DERIVED `Debug`. Decision D-P1-7 forbids it by name.
///
/// RED if one puts `#[derive(Debug)]` back on either enum. The remedy
/// is at the TYPE and not at the logging site, precisely so that the
/// NEXT site does not have to think about it.
#[test]
fn the_clipboard_debug_shows_the_size_and_never_the_text() {
    let rendu = format!("{:?}", ClientControl::clipboard("mot-de-passe-tres-secret"));
    assert!(
        !rendu.contains("secret"),
        "le texte a fui au Debug : {rendu}"
    );
    assert!(
        rendu.contains("octets: 24"),
        "la taille doit rester lisible : {rendu}"
    );

    let descendant = format!(
        "{:?}",
        AgentControl::clipboard(Some("mot-de-passe".into()), 12)
    );
    assert!(
        !descendant.contains("mot-de-passe"),
        "le texte a fui au Debug : {descendant}"
    );
    assert!(
        descendant.contains("octets: 12"),
        "la taille doit rester lisible : {descendant}"
    );
    assert!(
        descendant.contains("refus: false"),
        "le refus doit rester lisible : {descendant}"
    );

    let refus = format!("{:?}", AgentControl::clipboard(None, 100_000));
    assert!(
        refus.contains("refus: true"),
        "un refus doit se lire : {refus}"
    );
}

/// The hand-written `Debug` must not SWALLOW the other variants on
/// the way: without this test, a variant rendered empty would go unnoticed, and the
/// log would lose all diagnostic power without anything saying so.
///
/// RED if a variant renders an empty shape or omits its fields.
#[test]
fn le_debug_manuel_conserve_les_champs_des_autres_variantes() {
    let r = format!("{:?}", ClientControl::resize(1280, 720));
    assert!(
        r.contains("Resize") && r.contains("1280") && r.contains("720"),
        "{r}"
    );

    let l = format!(
        "{:?}",
        AgentControl::link(
            4_000_000,
            (800, 600),
            LinkQuality::Degradee,
            LinkAdaptation::Active
        )
    );
    assert!(
        l.contains("Link") && l.contains("4000000") && l.contains("Degradee"),
        "{l}"
    );

    let c = format!("{:?}", AgentControl::capabilities(true, false));
    assert!(
        c.contains("gamepad: true") && c.contains("clipboard: false"),
        "{c}"
    );
}

// ── Bloc E3 : la variante `MicState` ────────────────────────────────────────
//
// ⚠️ **Divergence V1, noted on 21 August 2026 and HANDED DOWN, not closed:**
// there is NO shared vector file for `AgentControl`. The three
// `*-vectors.json` of the repository serve `input`, `plateforme` and `files`,
// never `control`. The tests below pin the wire shape **on the
// Rust side**; `proto/ts/control.test.ts` pins **its own**. The two
// agree because two hands wrote the same string, and **nothing
// checks it**: a key rename applied on one side only would stay green on
// both sides. The gap is PRE-EXISTING and GENERAL to `AgentControl` — E3
// is the first to name it, it does not create it.

#[test]
fn the_mic_state_serialises_in_kebab_case_with_its_version() {
    let json = serde_json::to_string(&AgentControl::mic_state(false)).unwrap();
    // The TWO-word name is what makes `rename_all` observable: on an enum
    // whose variants all fit in one word, the mutation
    // `kebab-case` → `snake_case` is invisible (measured by sub-block G1).
    assert!(json.contains(r#""type":"mic-state""#), "{json}");
    assert!(json.contains(r#""granted":false"#), "{json}");
    assert!(json.contains(r#""v":3"#), "{json}");
}

#[test]
fn the_mic_state_round_trips_both_ways() {
    for accorde in [true, false] {
        let origine = AgentControl::mic_state(accorde);
        let json = serde_json::to_string(&origine).unwrap();
        let relu: AgentControl = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, origine, "aller-retour de granted={accorde}");
    }
}

#[test]
fn un_etat_de_micro_a_la_mauvaise_version_est_rejete() {
    // It is the test that the red R1 must bring down, AND IT ALONE: removing
    // `deserialize_with` from the `MicState` variant alone establishes that the
    // check is wired variant by variant, and not once and for
    // all (pattern measured in P3: 1 failure out of 18).
    let error =
        serde_json::from_str::<AgentControl>(r#"{"type":"mic-state","v":2,"granted":true}"#)
            .expect_err("une version 2 doit être rejetée");
    assert!(
        error
            .to_string()
            .contains("version de contrôle non supportée"),
        "obtenu : {error}"
    );
}

#[test]
fn un_etat_de_micro_sans_version_est_rejete() {
    assert!(
        serde_json::from_str::<AgentControl>(r#"{"type":"mic-state","granted":true}"#).is_err(),
        "un message sans `v` doit être rejeté, jamais complété en silence"
    );
}

#[test]
fn l_etat_du_micro_ne_divulgue_rien_au_journal() {
    // The rule of `control/redaction.rs`, applied to the TYPE and not to the site:
    // P2 found the clipboard in clear in `agent.log` on an earlier,
    // harmless trace, made dangerous by a new variant.
    let rendu = format!("{:?}", AgentControl::mic_state(true));
    assert_eq!(
        rendu, "MicState { v: 3, granted: true }",
        "obtenu : {rendu}"
    );
}
