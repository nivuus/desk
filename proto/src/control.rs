//! Messages du canal de contrôle (fiable, ordonné, faible débit).
//!
//! Format JSON versionné : `{"type":"...","v":2,"...":...}` (`type` sert de tag
//! interne à l'enum et est toujours émis en premier par serde). Le champ `v` est
//! obligatoire et vérifié à la désérialisation : un message sans `v`, ou avec un
//! `v` différent de [`CONTROL_VERSION`], est rejeté.

use serde::{Deserialize, Serialize};

/// Version du protocole de contrôle. Incrémenter à tout changement de format.
///
/// v2 (chantier B) : ajout de `Pointer`, `Rumble` et `Capabilities`.
pub const CONTROL_VERSION: u8 = 2;

/// Forme du curseur, exprimée directement dans le vocabulaire de la
/// propriété CSS `cursor` : le client la pose telle quelle, sans table de
/// correspondance à maintenir de son côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    Default,
    Text,
    Wait,
    Progress,
    Crosshair,
    Pointer,
    Move,
    NotAllowed,
    Help,
    NsResize,
    EwResize,
    NwseResize,
    NeswResize,
}

// Note : pas de `default` sur le champ `v` — un message sans champ `v` doit être
// rejeté (champ obligatoire), pas silencieusement complété avec la version
// courante. `default` court-circuiterait `deserialize_with` quand le champ est
// absent, ce qui romprait la vérification.
fn verifie_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != CONTROL_VERSION {
        return Err(serde::de::Error::custom(format!(
            "version de contrôle non supportée : {v}"
        )));
    }
    Ok(v)
}

/// Message du client web vers l'agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientControl {
    Resize {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
    },
}

/// Message de l'agent vers le client web.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentControl {
    Ready {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        width: u32,
        height: u32,
    },
    SessionEnd {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        reason: String,
    },
    /// État du pointeur. `visible: false` signifie à la fois « verrouille le
    /// pointeur » et « n'affiche aucun curseur » : c'est une seule
    /// observation côté agent (le curseur système est masqué), donc un seul
    /// message.
    Pointer {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        visible: bool,
        shape: CursorShape,
    },
    Rumble {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        left: u8,
        right: u8,
    },
    /// Émis une seule fois par session, mais PAS après `Ready` en pratique :
    /// `agent/src/main.rs` pousse ce message dans le canal `mpsc` de contrôle
    /// dès le démarrage du transport, avant même l'ouverture du canal de
    /// données — le drainage (`transport.rs::act_on_timeout`) le met donc en
    /// file avant que `Event::ChannelOpen` n'y ajoute `Ready`. L'ordre réel
    /// est `Capabilities`, éventuellement un premier `Pointer`, puis `Ready`.
    /// Sans conséquence aujourd'hui (le client traite les types
    /// indépendamment, voir `client/src/main.ts`), mais un client qui
    /// gaterait son initialisation sur `Ready` perdrait ce message et le
    /// premier `Pointer` : ne pas le faire.
    Capabilities {
        #[serde(rename = "v", deserialize_with = "verifie_version")]
        version: u8,
        gamepad: bool,
    },
}

impl ClientControl {
    /// Construit un message de redimensionnement à la version courante du protocole.
    pub fn resize(width: u32, height: u32) -> Self {
        ClientControl::Resize { version: CONTROL_VERSION, width, height }
    }
}

impl AgentControl {
    /// Construit un message "agent prêt" à la version courante du protocole.
    pub fn ready(width: u32, height: u32) -> Self {
        AgentControl::Ready { version: CONTROL_VERSION, width, height }
    }

    /// Construit un message de fin de session à la version courante du protocole.
    pub fn session_end(reason: impl Into<String>) -> Self {
        AgentControl::SessionEnd { version: CONTROL_VERSION, reason: reason.into() }
    }

    pub fn pointer(visible: bool, shape: CursorShape) -> Self {
        AgentControl::Pointer { version: CONTROL_VERSION, visible, shape }
    }

    pub fn rumble(left: u8, right: u8) -> Self {
        AgentControl::Rumble { version: CONTROL_VERSION, left, right }
    }

    pub fn capabilities(gamepad: bool) -> Self {
        AgentControl::Capabilities { version: CONTROL_VERSION, gamepad }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_le_redimensionnement() {
        let json = serde_json::to_string(&ClientControl::resize(1280, 720)).expect("sérialisation");
        // `type` est le tag interne de l'enum : serde l'émet avant les autres
        // champs, y compris `v`, quel que soit leur ordre de déclaration.
        assert_eq!(json, r#"{"type":"resize","v":2,"width":1280,"height":720}"#);
    }

    #[test]
    fn deserialise_le_redimensionnement() {
        let msg: ClientControl =
            serde_json::from_str(r#"{"v":2,"type":"resize","width":800,"height":600}"#)
                .expect("désérialisation");
        assert_eq!(msg, ClientControl::resize(800, 600));
    }

    #[test]
    fn serialise_ready_et_session_end() {
        assert_eq!(
            serde_json::to_string(&AgentControl::ready(1920, 1080)).unwrap(),
            r#"{"type":"ready","v":2,"width":1920,"height":1080}"#
        );
        assert_eq!(
            serde_json::to_string(&AgentControl::session_end("fenêtre fermée")).unwrap(),
            r#"{"type":"session-end","v":2,"reason":"fenêtre fermée"}"#
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
        assert_eq!(json, r#"{"type":"pointer","v":2,"visible":false,"shape":"ns-resize"}"#);
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
    fn rejette_la_version_de_controle_1_devenue_obsolete() {
        let err = serde_json::from_str::<AgentControl>(r#"{"type":"ready","v":1,"width":1,"height":1}"#);
        assert!(err.is_err());
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
}
