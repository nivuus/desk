//! Is this frame the relay REFUSING the role this socket declared?
//!
//! PURE, and tested on the host: `signalisation.rs`, which acts on the
//! answer, is `#![cfg(windows)]` and is not.
//!
//! 🔴 **A REFUSED DECLARATION WAS LOGGED, THEN IGNORED.** The relay answers a
//! declaration it refuses with `{"type":"error","reason":…,"motif":…}`
//! (`plateforme/src/signaling/relais.ts`). The control session wrote "control
//! session REFUSED" and kept reading a socket on which it held NO role: when
//! the refusal was `role-occupe` — its own previous socket, dead but not yet
//! reaped on the platform side after a network cut — nothing ever declared
//! again, and no window could be announced until the agent was restarted.
//!
//! A refused declaration is the END of that connection. The caller treats it
//! as a lost connection, so the ordinary resumption applies: a new socket, a
//! fresh token, and the exponential backoff of [`super::reprise_controle`],
//! which a connection that short never re-arms.
//!
//! ⚠️ **ON THE TYPED `motif`, NEVER ON THE SENTENCE.** Every refusal of a
//! declaration carries one (`role-occupe`, the guard's reasons, the volume
//! brake's); the errors that leave the socket usable — a malformed message,
//! an unknown type — carry none. `reason` is prose for a human and gets
//! reworded.

/// The relay's refusal of a role declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationRefusal {
    /// The stable keyword, for the log and for code.
    pub motif: String,
    /// The sentence the relay wrote for a human, when it wrote one.
    pub reason: Option<String>,
}

/// Reads a text frame of the control session: `Some` when it is the relay
/// refusing the declaration, `None` for everything else.
pub fn declaration_refusal(text: &str) -> Option<DeclarationRefusal> {
    let frame = serde_json::from_str::<serde_json::Value>(text).ok()?;
    if frame.get("type")?.as_str()? != "error" {
        return None;
    }
    Some(DeclarationRefusal {
        motif: frame.get("motif")?.as_str()?.to_string(),
        reason: frame
            .get("reason")
            .and_then(|r| r.as_str())
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_role_is_a_refusal() {
        // The exact frame `relais.ts` emits.
        let frame = r#"{"type":"error","reason":"an agent is already connected to session P:bureau","motif":"role-occupe"}"#;
        assert_eq!(
            declaration_refusal(frame),
            Some(DeclarationRefusal {
                motif: "role-occupe".into(),
                reason: Some("an agent is already connected to session P:bureau".into()),
            })
        );
    }

    #[test]
    fn every_typed_refusal_counts_whatever_its_reason() {
        for motif in [
            "jeton-expire",
            "jeton-absent",
            "session-refusee",
            "trop-de-requetes",
            "a-reason-added-tomorrow",
        ] {
            let frame = format!(r#"{{"type":"error","motif":"{motif}","retryApresS":12}}"#);
            let refusal = declaration_refusal(&frame).expect("a typed refusal");
            assert_eq!(refusal.motif, motif);
            assert_eq!(refusal.reason, None);
        }
    }

    #[test]
    fn an_untyped_error_leaves_the_connection_in_place() {
        // What the relay answers to a message it cannot relay: the socket and
        // its role are intact.
        assert_eq!(
            declaration_refusal(r#"{"type":"error","reason":"unknown type: x"}"#),
            None
        );
        assert_eq!(
            declaration_refusal(r#"{"type":"error","reason":"invalid JSON"}"#),
            None
        );
    }

    #[test]
    fn ordinary_frames_are_not_refusals() {
        for frame in [
            r#"{"type":"pair-present"}"#,
            r#"{"type":"peer-gone"}"#,
            r#"{"type":"viewport","session":"w-1","largeur":800,"hauteur":600}"#,
            // A control message that happens to carry a `motif` field.
            r#"{"type":"refus","titre":"x","motif":"role-occupe"}"#,
            r#"{"motif":"role-occupe"}"#,
            r#"{"type":"error","motif":42}"#,
            "null",
            "[1,2]",
            "not json",
        ] {
            assert_eq!(declaration_refusal(frame), None, "{frame}");
        }
    }
}
