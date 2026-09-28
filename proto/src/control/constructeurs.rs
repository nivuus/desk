//! Constructors for [`ClientControl`] and [`AgentControl`]: each one sets the
//! current protocol version, so no call site has to.

use super::*;

impl ClientControl {
    /// Builds a resize message at the current protocol version.
    pub fn resize(width: u32, height: u32) -> Self {
        ClientControl::Resize {
            version: CONTROL_VERSION,
            width,
            height,
        }
    }

    /// Builds a paste message at the current protocol version.
    pub fn clipboard(text: impl Into<String>) -> Self {
        ClientControl::Clipboard {
            version: CONTROL_VERSION,
            text: text.into(),
        }
    }
}

impl AgentControl {
    /// Builds an "agent ready" message at the current protocol version.
    pub fn ready(width: u32, height: u32, mic: bool) -> Self {
        AgentControl::Ready {
            version: CONTROL_VERSION,
            width,
            height,
            mic,
        }
    }

    /// Builds an end-of-session message at the current protocol version.
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

    /// Is this window's microphone heard by the VM?
    ///
    /// ⚠️ **Does NOT raise `CONTROL_VERSION`** — see the doc of the variant.
    pub fn mic_state(granted: bool) -> AgentControl {
        AgentControl::MicState {
            version: CONTROL_VERSION,
            granted,
        }
    }

    /// The VM clipboard has changed.
    ///
    /// ⚠️ **`CONTROL_VERSION` DOES NOT GO UP for this variant, and it is not
    /// an oversight.** The two checks of `v` — `verifie_version`
    /// above and `parseAgentControl` on the TypeScript side — are **strict
    /// equalities**: raising the version would reject **all** messages,
    /// `Ready` and `SessionEnd` included. A TOTAL incompatibility
    /// would replace a PER-MESSAGE degradation. The precedent is the
    /// `ready` builder of this same `impl` (the `mic` field was added
    /// to `Ready` without raising the version, for the same reason).
    ///
    /// ⚠️ This sentence said "THREE lines from here": `pub fn ready` is
    /// thirty-two lines higher, and already was at the time of writing. **A
    /// distance deictic ages at the first insertion**; name the
    /// thing, never count the lines that separate you from it.
    pub fn clipboard(text: Option<String>, bytes: u32) -> AgentControl {
        AgentControl::Clipboard {
            version: CONTROL_VERSION,
            text,
            bytes,
        }
    }

    /// ⚠️ **Does NOT raise `CONTROL_VERSION`** — see the doc of the variant.
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
