use super::*;

#[test]
fn serialise_le_redimensionnement() {
    let json = serde_json::to_string(&ClientControl::resize(1280, 720)).expect("sérialisation");
    // `type` est le tag interne de l'enum : serde l'émet avant les autres
    // champs, y compris `v`, quel que soit leur ordre de déclaration.
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
    let err = serde_json::from_str::<ClientControl>(
        r#"{"v":9,"type":"resize","width":1,"height":1}"#,
    );
    assert!(err.is_err());
}

#[test]
fn serialise_le_message_de_pointeur_en_kebab_case() {
    let json = serde_json::to_string(&AgentControl::pointer(false, CursorShape::NsResize))
        .expect("sérialisation");
    assert_eq!(json, r#"{"type":"pointer","v":3,"visible":false,"shape":"ns-resize"}"#);
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
    let message = AgentControl::capabilities(true);
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
    let err = serde_json::from_str::<AgentControl>(r#"{"type":"ready","v":1,"width":1,"height":1}"#);
    assert!(err.is_err());
}

#[test]
fn une_visibilite_se_relit_telle_qu_ecrite() {
    let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
    let message: ClientControl = serde_json::from_str(json).expect("visibilité valide");
    assert_eq!(
        message,
        ClientControl::Visibility { version: 3, visible: false, focused: false }
    );
}

#[test]
fn une_visibilite_de_mauvaise_version_est_rejetee() {
    let json = r#"{"type":"visibility","v":1,"visible":true,"focused":true}"#;
    assert!(serde_json::from_str::<ClientControl>(json).is_err());
}

#[test]
fn un_sommeil_s_ecrit_avec_son_type_en_tete_et_sa_raison() {
    let json = serde_json::to_string(&AgentControl::asleep(true, "evincee"))
        .expect("sérialisation");
    assert!(json.starts_with(r#"{"type":"asleep""#), "obtenu : {json}");
    assert!(json.contains(r#""asleep":true"#), "obtenu : {json}");
    assert!(json.contains(r#""reason":"evincee""#), "obtenu : {json}");
}

#[test]
fn le_plein_ecran_se_serialise_en_kebab_case_avec_sa_version() {
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
fn toutes_les_formes_de_curseur_sont_des_valeurs_css() {
    // Le client pose cette chaîne telle quelle dans `style.cursor` : une
    // valeur non reconnue serait silencieusement ignorée par le
    // navigateur, donc invisible en test.
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

/// Chantier E : `Ready` porte la disponibilité du micro, **sans bump de
/// `CONTROL_VERSION`**.
///
/// Le parseur TypeScript vérifie `v`, puis `type`, puis CASTE : un champ
/// supplémentaire est simplement ignoré par un client ancien. Et un client
/// RÉCENT parlant à un agent ANCIEN lit `mic === undefined`, donc falsy,
/// donc pas de bouton — exactement la règle de la spec §10, gratuitement.
#[test]
fn ready_porte_la_disponibilite_du_micro() {
    assert_eq!(
        serde_json::to_string(&AgentControl::ready(1920, 1080, true)).unwrap(),
        r#"{"type":"ready","v":3,"width":1920,"height":1080,"mic":true}"#
    );
}

/// « Le champ est optionnel à la lecture et SON ABSENCE VAUT `false` »
/// (spec §10) : un client récent face à un agent ancien n'affiche pas un
/// bouton qui ne mènerait nulle part.
///
/// ⚠️ `AgentControl` porte `deny_unknown_fields` : cela ne gêne pas
/// l'AJOUT d'un champ, mais un champ MANQUANT est une erreur de
/// désérialisation en Rust. `#[serde(default)]` est donc OBLIGATOIRE.
#[test]
fn un_ready_sans_micro_se_lit_avec_micro_faux() {
    let m: AgentControl =
        serde_json::from_str(r#"{"type":"ready","v":3,"width":1,"height":1}"#).unwrap();
    assert_eq!(m, AgentControl::ready(1, 1, false));
}

#[test]
fn serialise_le_presse_papier_avec_son_texte() {
    let json = serde_json::to_string(&AgentControl::clipboard(Some("bonjour".into()), 7))
        .expect("sérialisation");
    assert_eq!(json, r#"{"type":"clipboard","v":3,"text":"bonjour","bytes":7}"#);
}

/// 🔴 Un REFUS sérialise `"text":null`, champ PRÉSENT.
///
/// ROUGE si l'on pose `#[serde(skip_serializing_if = "Option::is_none")]` :
/// le champ disparaîtrait, et le client ne pourrait plus distinguer un refus
/// d'un message tronqué en route.
#[test]
fn un_refus_de_presse_papier_serialise_un_text_null_present() {
    let json =
        serde_json::to_string(&AgentControl::clipboard(None, 102_400)).expect("sérialisation");
    assert_eq!(json, r#"{"type":"clipboard","v":3,"text":null,"bytes":102400}"#);
    assert!(json.contains("\"text\":null"), "le champ text doit rester présent");
}

/// 🔴 Ce qui prouve que `verifie_version` est bien branché sur la variante
/// NEUVE — l'oublier est une erreur silencieuse, `version` n'étant vérifié
/// que par son attribut.
#[test]
fn un_presse_papier_en_version_2_est_rejete() {
    let brut = r#"{"type":"clipboard","v":2,"text":"bonjour","bytes":7}"#;
    let erreur = serde_json::from_str::<AgentControl>(brut).expect_err("v:2 doit être rejeté");
    assert!(
        erreur.to_string().contains("version de contrôle non supportée"),
        "message inattendu : {erreur}"
    );
}

/// ROUGE si l'on retire `deny_unknown_fields` de l'enum : ce test le fige
/// pour la variante neuve.
#[test]
fn un_presse_papier_portant_un_champ_inconnu_est_rejete() {
    let brut = r#"{"type":"clipboard","v":3,"text":"bonjour","bytes":7,"surprise":1}"#;
    assert!(serde_json::from_str::<AgentControl>(brut).is_err());
}
