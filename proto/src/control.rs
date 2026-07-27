//! Messages du canal de contrôle (fiable, ordonné, faible débit).
//!
//! Format JSON versionné : `{"type":"...","v":1,"...":...}` (`type` sert de tag
//! interne à l'enum et est toujours émis en premier par serde). Le champ `v` est
//! obligatoire et vérifié à la désérialisation : un message sans `v`, ou avec un
//! `v` différent de [`CONTROL_VERSION`], est rejeté.

use serde::{Deserialize, Serialize};

/// Version du protocole de contrôle. Incrémenter à tout changement de format.
pub const CONTROL_VERSION: u8 = 1;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_le_redimensionnement() {
        let json = serde_json::to_string(&ClientControl::resize(1280, 720)).expect("sérialisation");
        // `type` est le tag interne de l'enum : serde l'émet avant les autres
        // champs, y compris `v`, quel que soit leur ordre de déclaration.
        assert_eq!(json, r#"{"type":"resize","v":1,"width":1280,"height":720}"#);
    }

    #[test]
    fn deserialise_le_redimensionnement() {
        let msg: ClientControl =
            serde_json::from_str(r#"{"v":1,"type":"resize","width":800,"height":600}"#)
                .expect("désérialisation");
        assert_eq!(msg, ClientControl::resize(800, 600));
    }

    #[test]
    fn serialise_ready_et_session_end() {
        assert_eq!(
            serde_json::to_string(&AgentControl::ready(1920, 1080)).unwrap(),
            r#"{"type":"ready","v":1,"width":1920,"height":1080}"#
        );
        assert_eq!(
            serde_json::to_string(&AgentControl::session_end("fenêtre fermée")).unwrap(),
            r#"{"type":"session-end","v":1,"reason":"fenêtre fermée"}"#
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
}
