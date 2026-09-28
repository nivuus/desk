//! Control session messages, between the supervisor and the shell page.
//!
//! Signaling only relays: it is here that the shape of messages is
//! decided, and it must match exactly what `client/src/shell.ts`
//! expects — and what `plateforme/src/signaling/relais.ts` accepts to
//! relay (`TYPES_RELAYES`).
//!
//! ❌ **This path said `signaling/src/server.ts`, and that file no longer
//! exists**: the `signaling/` package was absorbed by `plateforme/` in
//! sub-block P1 of sub-project ⑤. **The stated property, for its part, stays
//! TRUE** — `TYPES_RELAYES` still carries the same six types (`offer`,
//! `answer`, `fenetre-ouverte`, `fenetre-fermee`, `refus`, `viewport`),
//! reread on August 19th, 2026. A one-line debt, left by P1 because
//! `agent/` was then the scope of concurrent work, and settled
//! here.
//!
//! **No `#[cfg(windows)]`**: these messages are pure serialisation,
//! and it is precisely the kind of contract that must be exercised on the host —
//! a field name drifting on the agent side is otherwise only seen in a real
//! session, on the VM.

use serde::{Deserialize, Serialize};

/// Reserved name of the control session. The supervisor declares itself there as
/// `agent`, the shell page as `client`.
///
/// ⚠️ **IT IS NO LONGER A SESSION IDENTIFIER ON ITS OWN** (sub-block P3):
/// it is the name that follows the VM's prefix. Go through
/// [`session_de_controle`], never through this bare constant — two VMs that
/// both opened `bureau` would contend for the same session on the
/// platform, and the second would be refused with "an agent is already connected".
pub const NOM_SESSION_DE_CONTROLE: &str = "bureau";

/// The prefix separator, as spec §3.4 writes it.
///
/// ⚠️ It also appears in the TURN identifier the platform derives
/// (`<expiry>:<session>`), which therefore becomes three segments. The prefix
/// being `base64url` it cannot contain one: the first bound stays
/// unambiguous. **Property not exercised against a live coturn.**
pub const SEPARATEUR_PREFIXE: char = ':';

/// Composes a session identifier: `<prefix>:<name>`, or `<name>` alone
/// when no prefix is known.
///
/// 🔴 **The empty prefix must give back EXACTLY today's name.** A
/// silent `":bureau"` is the name of no existing session, and nothing would
/// signal it.
pub fn composer(prefixe: &str, nom: &str) -> String {
    if prefixe.is_empty() {
        return nom.to_string();
    }
    format!("{prefixe}{SEPARATEUR_PREFIXE}{nom}")
}

/// The full identifier of the control session for a given prefix.
pub fn session_de_controle(prefixe: &str) -> String {
    composer(prefixe, NOM_SESSION_DE_CONTROLE)
}

/// Reserved name of the **files bridge** signaling session.
///
/// DISTINCT from [`NOM_SESSION_DE_CONTROLE`]: the relay only accepts one `agent`
/// and one `client` per identifier (`plateforme/src/signaling/appariement.ts`),
/// and the shell page already holds the `client` role of `bureau`. Two
/// `PeerConnection`s to the same VM therefore require two identifiers.
///
/// ⚠️ **IT IS NOT A SESSION IDENTIFIER ON ITS OWN**, exactly like
/// its neighbour since sub-block P3: go through [`session_du_pont`], never
/// through this bare constant. Two VMs that both opened `files` would
/// contend for the same session on the platform, and the second would be
/// refused with "an agent is already connected".
///
/// *(F1's plan wrote `pub const SESSION_DU_PONT: &str = "files"`,
/// used as is, and noted that the identifier "is not namespaced
/// per user". It was written before P3 set the prefix: the
/// remark is therefore OBSOLETE — the prefix is that namespace — and the bare form
/// would have reintroduced the defect P3 had just fixed, on the only
/// session that would have escaped it.)*
pub const NOM_SESSION_DU_PONT: &str = "fichiers";

/// The full identifier of the files bridge session for a given prefix.
pub fn session_du_pont(prefixe: &str) -> String {
    composer(prefixe, NOM_SESSION_DU_PONT)
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum VersLaShell {
    #[serde(rename = "fenetre-ouverte")]
    FenetreOuverte { session: String, titre: String },
    #[serde(rename = "fenetre-fermee")]
    FenetreFermee { session: String },
    #[serde(rename = "refus")]
    Refus { titre: String, motif: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum DepuisLaShell {
    #[serde(rename = "viewport")]
    Viewport {
        session: String,
        largeur: u32,
        hauteur: u32,
    },
    /// **A `client` peer has just joined the control session** —
    /// emitted by the RELAY, never by the page
    /// (`plateforme/src/signaling/pair-present.ts`). It is therefore not a
    /// message "from the shell" in the strict sense, but it arrives through the same
    /// socket and is read by the same deserialiser: putting it elsewhere
    /// would force `signalisation.rs` to hold two read paths for
    /// a single connection.
    ///
    /// 🔴 WHAT IT TRIGGERS, AND WHY IT EXISTS: the supervisor
    /// announces its windows WHEN IT DISCOVERS THEM, in a session
    /// where no one is listening yet — the relay then drops
    /// the announcement without a trace, and thirty seconds later the agent
    /// refuses its own windows for lack of a `viewport` in return
    /// (`table/orphelines.rs`). This message is the signal saying "someone
    /// is listening NOW, tell them again what you know". Production measurement
    /// of August 30th, 2026: see `pair-present.ts`.
    ///
    /// **No field**, on purpose: the relay knows nothing more than
    /// the arrival, and the agent needs nothing more.
    #[serde(rename = "pair-present")]
    PairPresent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_serialises_as_the_shell_expects_it() {
        let json = serde_json::to_string(&VersLaShell::FenetreOuverte {
            session: "w-1".into(),
            titre: "Bloc-notes".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"type":"fenetre-ouverte","session":"w-1","titre":"Bloc-notes"}"#
        );
    }

    #[test]
    fn a_close_and_a_refusal_serialise_as_the_shell_expects_them() {
        // `shell-page.ts` reads `message.session` on a closing and
        // `message.titre`/`message.motif` on a refusal: those names are the
        // contract, not a naming convenience on the Rust side.
        assert_eq!(
            serde_json::to_string(&VersLaShell::FenetreFermee {
                session: "w-2".into()
            })
            .unwrap(),
            r#"{"type":"fenetre-fermee","session":"w-2"}"#
        );
        assert_eq!(
            serde_json::to_string(&VersLaShell::Refus {
                titre: "Bloc-notes".into(),
                motif: "no output left".into(),
            })
            .unwrap(),
            r#"{"type":"refus","titre":"Bloc-notes","motif":"no output left"}"#
        );
    }

    #[test]
    fn a_shell_viewport_is_read() {
        let message: DepuisLaShell = serde_json::from_str(
            r#"{"type":"viewport","session":"w-1","largeur":1600,"hauteur":900}"#,
        )
        .unwrap();
        let DepuisLaShell::Viewport {
            session,
            largeur,
            hauteur,
        } = message
        else {
            panic!("a viewport must read as a viewport")
        };
        assert_eq!((session.as_str(), largeur, hauteur), ("w-1", 1600, 900));
    }

    #[test]
    fn an_unknown_shell_message_is_refused_rather_than_ignored() {
        let result: Result<DepuisLaShell, _> = serde_json::from_str(r#"{"type":"autre-chose"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn an_empty_prefix_gives_back_exactly_the_current_name() {
        // 🔴 THE MOST IMPORTANT TEST OF THIS FILE. Without it, setting the
        // separator unconditionally would give `":bureau"` and `":w-1"` —
        // which are the name of NO existing session, and nothing would
        // signal it: the shell page would wait for a window that does not come.
        assert_eq!(composer("", NOM_SESSION_DE_CONTROLE), "bureau");
        assert_eq!(composer("", "w-1"), "w-1");
        assert_eq!(session_de_controle(""), "bureau");
    }

    #[test]
    fn the_bridge_session_follows_the_prefix_like_the_control_one() {
        // 🔴 The bridge has its OWN session because the relay only accepts one
        // `agent` and one `client` per identifier, and the shell page already holds
        // the `client` role of `bureau`.
        //
        // It must follow the prefix exactly like its neighbour: F1's
        // plan, written before sub-block P3, prescribed a BARE constant
        // used as is. Two VMs would then both have opened
        // `files`, and the second would have been refused with "an agent is already
        // connected" — the very defect P3 had just fixed, reintroduced
        // on the only session that would have escaped it.
        assert_eq!(session_du_pont(""), "fichiers");
        assert_eq!(session_du_pont("Zm9vYmFy"), "Zm9vYmFy:fichiers");
        // …and the two sessions of the same VM stay DISTINCT, which is
        // this constant's whole reason to exist.
        assert_ne!(session_du_pont("Zm9vYmFy"), session_de_controle("Zm9vYmFy"));
        assert_ne!(session_du_pont(""), session_de_controle(""));
    }

    #[test]
    fn a_set_prefix_precedes_the_name_separated_by_a_colon() {
        assert_eq!(composer("Zm9vYmFy", "w-1"), "Zm9vYmFy:w-1");
        assert_eq!(session_de_controle("Zm9vYmFy"), "Zm9vYmFy:bureau");
    }

    /// Signaling also relays `ice-config` and `peer-gone` on this
    /// connection: they must fall on the "refused" side of this boundary,
    /// so that `signalisation.rs` ignores them without taking them for a
    /// viewport.
    ///
    /// ⚠️ **`pair-present` IS NOW ON THE OTHER SIDE OF THIS
    /// BOUNDARY**, and it is the only service message to have crossed it:
    /// it is emitted by the relay like those two, but the agent must ACT
    /// on it. The next test holds it — without it, a typo in the
    /// `rename` would make the message mute, which is exactly the defect
    /// this batch fixes, replayed one notch lower.
    #[test]
    fn signaling_service_messages_are_not_viewports() {
        assert!(serde_json::from_str::<DepuisLaShell>(r#"{"type":"peer-gone"}"#).is_err());
        assert!(
            serde_json::from_str::<DepuisLaShell>(r#"{"type":"ice-config","urls":[]}"#).is_err()
        );
    }

    /// 🔴 THE NAME ON THE WIRE IS THE CONTRACT, AND IT IS WRITTEN IN TWO
    /// DIFFERENT WORD STORES: here in Rust, and in
    /// `plateforme/src/signaling/pair-present.ts::TYPE_PAIR_PRESENT`. This
    /// test freezes the Rust half; if it turned red, the `rename`
    /// has drifted — and a drift of this name makes the mechanism MUTE, without
    /// any error, on both sides.
    #[test]
    fn a_peer_arrival_is_read_on_the_control_session() {
        let message: DepuisLaShell = serde_json::from_str(r#"{"type":"pair-present"}"#).unwrap();
        assert!(matches!(message, DepuisLaShell::PairPresent));
        // …and a superfluous field does not break it: the relay can add
        // one tomorrow without making the agent deaf.
        let with_extra: DepuisLaShell =
            serde_json::from_str(r#"{"type":"pair-present","role":"client"}"#).unwrap();
        assert!(matches!(with_extra, DepuisLaShell::PairPresent));
    }
}
