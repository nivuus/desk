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

// ---------------------------------------------------------------------------
// Sous-bloc P2 du chantier presse-papier — le sens navigateur → VM.
// ---------------------------------------------------------------------------

/// ROUGE si la variante `ClientControl::Clipboard` est absente.
#[test]
fn deserialise_le_collage_venu_du_client() {
    let msg: ClientControl =
        serde_json::from_str(r#"{"v":3,"type":"clipboard","text":"bonjour"}"#)
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

/// 🔴 ROUGE si l'on oublie `deserialize_with = "verifie_version"` sur le champ
/// `version` — **c'est la ligne qu'on omet en recopiant une variante
/// voisine**, et rien d'autre dans ce dépôt ne le verrait : la variante
/// fonctionnerait, simplement elle accepterait n'importe quelle version.
#[test]
fn un_collage_client_a_la_mauvaise_version_est_rejete() {
    let erreur = serde_json::from_str::<ClientControl>(r#"{"v":2,"type":"clipboard","text":"x"}"#);
    assert!(erreur.is_err(), "une version 2 doit être refusée");
}

/// 🔴 ROUGE si l'on posait la variante sur un enum sans `deny_unknown_fields` :
/// un client mal conduit pourrait alors faire passer n'importe quoi.
#[test]
fn un_collage_client_avec_un_champ_en_trop_est_rejete() {
    let erreur = serde_json::from_str::<ClientControl>(
        r#"{"v":3,"type":"clipboard","text":"x","bytes":1}"#,
    );
    assert!(erreur.is_err(), "un champ inconnu doit être refusé");
}

/// 🔴 ROUGE si l'on oublie `#[serde(default)]` sur `Capabilities::clipboard`.
///
/// ⚠️ **La raison n'est PAS `deny_unknown_fields`**, contrairement à ce que la
/// spec avance : `deny_unknown_fields` refuse un champ INCONNU ; c'est le
/// défaut de serde qui refuse un champ MANQUANT. Les deux mécanismes n'ont
/// rien à voir, et c'est le commentaire de `mic` qui dit la chose juste.
///
/// Un agent d'avant P2 n'émet pas ce champ ; un désérialiseur récent doit donc
/// le tolérer et lire `false`.
#[test]
fn capabilities_sans_clipboard_se_deserialise_a_false() {
    let msg: AgentControl =
        serde_json::from_str(r#"{"v":3,"type":"capabilities","gamepad":true}"#)
            .expect("désérialisation");
    assert_eq!(msg, AgentControl::capabilities(true, false));
}

/// ROUGE si l'on posait un `skip_serializing_if` : le champ disparaîtrait
/// quand il vaut `false`, et le client ne pourrait plus distinguer « l'agent
/// dit non » de « l'agent est trop ancien pour le dire ». Les deux se traitent
/// de la même façon aujourd'hui, mais la distinction est ce qui permettra un
/// jour de le journaliser.
#[test]
fn capabilities_serialise_les_deux_champs() {
    assert_eq!(
        serde_json::to_string(&AgentControl::capabilities(false, true)).unwrap(),
        r#"{"type":"capabilities","v":3,"gamepad":false,"clipboard":true}"#
    );
}

/// 🔴 **LE PRESSE-PAPIER NE DOIT JAMAIS ATTEINDRE UN JOURNAL, ET CE TEST EST
/// LE SEUL REMPART.**
///
/// Il est né d'une MESURE, pas d'une précaution : la recette de P2 a relevé
/// dans `agent.log` quatre lignes portant le contenu du presse-papier **en
/// clair** — `Clipboard { version: 3, text: "alpha-arme-1-crwor9" }` —, parce
/// que `demarrage.rs` imprime le message reçu par `?message` et que
/// `ClientControl` DÉRIVAIT `Debug`. La décision D-P1-7 l'interdit nommément.
///
/// ROUGE si l'on remet `#[derive(Debug)]` sur l'un des deux enums. Le remède
/// est au TYPE et non au site de journalisation, précisément pour que le
/// PROCHAIN site n'ait pas à y penser.
#[test]
fn le_debug_du_presse_papier_montre_la_taille_et_jamais_le_texte() {
    let rendu = format!("{:?}", ClientControl::clipboard("mot-de-passe-tres-secret"));
    assert!(!rendu.contains("secret"), "le texte a fui au Debug : {rendu}");
    assert!(rendu.contains("octets: 24"), "la taille doit rester lisible : {rendu}");

    let descendant = format!("{:?}", AgentControl::clipboard(Some("mot-de-passe".into()), 12));
    assert!(!descendant.contains("mot-de-passe"), "le texte a fui au Debug : {descendant}");
    assert!(descendant.contains("octets: 12"), "la taille doit rester lisible : {descendant}");
    assert!(descendant.contains("refus: false"), "le refus doit rester lisible : {descendant}");

    let refus = format!("{:?}", AgentControl::clipboard(None, 100_000));
    assert!(refus.contains("refus: true"), "un refus doit se lire : {refus}");
}

/// Le `Debug` écrit à la main ne doit pas AVALER les autres variantes en
/// chemin : sans ce test, une variante rendue vide passerait inaperçue, et le
/// journal perdrait tout pouvoir de diagnostic sans que rien ne le dise.
///
/// ROUGE si une variante rend une forme vide ou omet ses champs.
#[test]
fn le_debug_manuel_conserve_les_champs_des_autres_variantes() {
    let r = format!("{:?}", ClientControl::resize(1280, 720));
    assert!(r.contains("Resize") && r.contains("1280") && r.contains("720"), "{r}");

    let l = format!(
        "{:?}",
        AgentControl::link(4_000_000, (800, 600), LinkQuality::Degradee, LinkAdaptation::Active)
    );
    assert!(l.contains("Link") && l.contains("4000000") && l.contains("Degradee"), "{l}");

    let c = format!("{:?}", AgentControl::capabilities(true, false));
    assert!(c.contains("gamepad: true") && c.contains("clipboard: false"), "{c}");
}

// ── Bloc E3 : la variante `MicState` ────────────────────────────────────────
//
// ⚠️ **Divergence V1, relevée le 21 août 2026 et LÉGUÉE, pas fermée :** il
// n'existe AUCUN fichier de vecteurs partagé pour `AgentControl`. Les trois
// `*-vectors.json` du dépôt servent `input`, `plateforme` et `fichiers`,
// jamais `control`. Les tests ci-dessous épinglent la forme de fil **côté
// Rust** ; `proto/ts/control.test.ts` épingle **la sienne**. Les deux
// s'accordent parce que deux mains ont écrit la même chaîne, et **rien ne le
// vérifie** : un renommage de clé appliqué d'un seul côté resterait vert des
// deux côtés. La lacune est PRÉEXISTANTE et GÉNÉRALE à `AgentControl` — E3
// est le premier à la nommer, il ne la crée pas.

#[test]
fn l_etat_du_micro_se_serialise_en_kebab_case_avec_sa_version() {
    let json = serde_json::to_string(&AgentControl::mic_state(false)).unwrap();
    // Le nom en DEUX mots est ce qui rend `rename_all` observable : sur un enum
    // dont toutes les variantes tiennent en un mot, la mutation
    // `kebab-case` → `snake_case` est invisible (mesuré par le sous-bloc G1).
    assert!(json.contains(r#""type":"mic-state""#), "{json}");
    assert!(json.contains(r#""granted":false"#), "{json}");
    assert!(json.contains(r#""v":3"#), "{json}");
}

#[test]
fn l_etat_du_micro_fait_l_aller_retour_dans_les_deux_sens() {
    for accorde in [true, false] {
        let origine = AgentControl::mic_state(accorde);
        let json = serde_json::to_string(&origine).unwrap();
        let relu: AgentControl = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, origine, "aller-retour de granted={accorde}");
    }
}

#[test]
fn un_etat_de_micro_a_la_mauvaise_version_est_rejete() {
    // C'est le test que la rouge R1 doit faire tomber, ET LUI SEUL : retirer
    // `deserialize_with` de la seule variante `MicState` établit que la
    // vérification est branchée variante par variante, et non une fois pour
    // toutes (patron mesuré en P3 : 1 échec sur 18).
    let erreur = serde_json::from_str::<AgentControl>(r#"{"type":"mic-state","v":2,"granted":true}"#)
        .expect_err("une version 2 doit être rejetée");
    assert!(
        erreur.to_string().contains("version de contrôle non supportée"),
        "obtenu : {erreur}"
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
    // La règle de `control/redaction.rs`, appliquée au TYPE et non au site :
    // P2 a trouvé le presse-papier en clair dans `agent.log` sur une trace
    // antérieure et inoffensive, rendue dangereuse par une variante neuve.
    let rendu = format!("{:?}", AgentControl::mic_state(true));
    assert_eq!(rendu, "MicState { v: 3, granted: true }", "obtenu : {rendu}");
}
