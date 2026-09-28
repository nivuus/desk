//! Constructeurs de [`ClientControl`] et [`AgentControl`] : chacun pose la
//! version courante du protocole, pour qu'aucun site d'appel n'ait à le faire.

use super::*;

impl ClientControl {
    /// Construit un message de redimensionnement à la version courante du protocole.
    pub fn resize(width: u32, height: u32) -> Self {
        ClientControl::Resize {
            version: CONTROL_VERSION,
            width,
            height,
        }
    }

    /// Construit un message de collage à la version courante du protocole.
    pub fn clipboard(text: impl Into<String>) -> Self {
        ClientControl::Clipboard {
            version: CONTROL_VERSION,
            text: text.into(),
        }
    }
}

impl AgentControl {
    /// Construit un message "agent prêt" à la version courante du protocole.
    pub fn ready(width: u32, height: u32, mic: bool) -> Self {
        AgentControl::Ready {
            version: CONTROL_VERSION,
            width,
            height,
            mic,
        }
    }

    /// Construit un message de fin de session à la version courante du protocole.
    pub fn session_end(reason: impl Into<String>) -> Self {
        AgentControl::SessionEnd {
            version: CONTROL_VERSION,
            reason: reason.into(),
        }
    }

    pub fn pointer(visible: bool, shape: CursorShape) -> Self {
        AgentControl::Pointer {
            version: CONTROL_VERSION,
            visible,
            shape,
        }
    }

    pub fn asleep(asleep: bool, reason: &str) -> AgentControl {
        AgentControl::Asleep {
            version: CONTROL_VERSION,
            asleep,
            reason: reason.to_string(),
        }
    }

    pub fn fullscreen(active: bool) -> AgentControl {
        AgentControl::Fullscreen {
            version: CONTROL_VERSION,
            active,
        }
    }

    /// Le micro de cette fenêtre est-il entendu par la VM ?
    ///
    /// ⚠️ **Ne monte PAS `CONTROL_VERSION`** — voir la doc de la variante.
    pub fn mic_state(granted: bool) -> AgentControl {
        AgentControl::MicState {
            version: CONTROL_VERSION,
            granted,
        }
    }

    /// Le presse-papier de la VM a changé.
    ///
    /// ⚠️ **`CONTROL_VERSION` NE MONTE PAS pour cette variante, et ce n'est
    /// pas un oubli.** Les deux vérifications de `v` — `verifie_version`
    /// ci-dessus et `parseAgentControl` côté TypeScript — sont des **égalités
    /// strictes** : monter la version ferait rejeter **tous** les messages,
    /// `Ready` et `SessionEnd` compris. Une incompatibilité TOTALE
    /// remplacerait une dégradation PAR MESSAGE. Le précédent est le
    /// constructeur `ready` de cette même `impl` (le champ `mic` a été ajouté
    /// à `Ready` sans monter la version, pour la même raison).
    ///
    /// ⚠️ Cette phrase disait « à TROIS lignes d'ici » : `pub fn ready` est
    /// trente-deux lignes plus haut, et l'était déjà à l'écriture. **Un
    /// déictique de distance vieillit à la première insertion** ; nommer la
    /// chose, jamais compter les lignes qui l'en séparent.
    pub fn clipboard(text: Option<String>, bytes: u32) -> AgentControl {
        AgentControl::Clipboard {
            version: CONTROL_VERSION,
            text,
            bytes,
        }
    }

    /// ⚠️ **Ne monte PAS `CONTROL_VERSION`** — voir la doc de la variante.
    pub fn accent(couleur: impl Into<String>) -> AgentControl {
        AgentControl::Accent {
            version: CONTROL_VERSION,
            couleur: couleur.into(),
        }
    }

    pub fn rumble(left: u8, right: u8) -> Self {
        AgentControl::Rumble {
            version: CONTROL_VERSION,
            left,
            right,
        }
    }

    pub fn capabilities(gamepad: bool, clipboard: bool) -> Self {
        AgentControl::Capabilities {
            version: CONTROL_VERSION,
            gamepad,
            clipboard,
        }
    }

    pub fn link(
        bitrate: u32,
        taille: (u32, u32),
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    ) -> Self {
        AgentControl::Link {
            version: CONTROL_VERSION,
            bitrate,
            width: taille.0,
            height: taille.1,
            quality,
            adaptation,
        }
    }
}
