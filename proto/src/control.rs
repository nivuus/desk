//! Control channel messages (reliable, ordered, low throughput).
//!
//! Versioned JSON format: `{"type":"...","v":3,"...":...}` (`type` serves as the
//! internal tag of the enum and is always emitted first by serde). The `v` field is
//! mandatory and checked on deserialization: a message without `v`, or with a
//! `v` other than [`CONTROL_VERSION`], is rejected.

use serde::{Deserialize, Serialize};

/// Version of the control protocol. Increment on any format change.
///
/// v2 (workstream B): added `Pointer`, `Rumble` and `Capabilities`.
/// v3 (workstream C): added `Link`.
pub const CONTROL_VERSION: u8 = 3;

/// Cursor shape — **extracted** to `control/curseur.rs` by sub-block
/// E3, so that the `MicState` variant fits under the 500-line gate
/// without compression. The type stays `pub` and re-exported here: no call
/// site of `proto::control::CursorShape` has moved.
#[path = "control/curseur.rs"]
mod curseur;
pub use curseur::CursorShape;

// Note: no `default` on the `v` field — a message without a `v` field must be
// rejected (mandatory field), not silently filled in with the current
// version. `default` would short-circuit `deserialize_with` when the field is
// absent, which would break the check.
fn check_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    if v != CONTROL_VERSION {
        return Err(serde::de::Error::custom(format!(
            "unsupported control version: {v}"
        )));
    }
    Ok(v)
}

/// The two vocabularies of the link state — **extracted** to
/// `control/lien.rs` by the cross-cutting review of block E3, so that its
/// wording fixes fit under the 500-line gate without
/// compression. Both types stay `pub` and re-exported here: no call
/// site has moved.
#[path = "control/lien.rs"]
mod lien;
pub use lien::{LinkAdaptation, LinkQuality};

/// Message from the web client to the agent.
///
/// 🔴 **`Debug` is IMPLEMENTED BY HAND, never derived, and it is the only
/// rampart against a clipboard leak into the log.** See the `impl` further
/// down, which carries the measurement that made it necessary.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientControl {
    Resize {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        width: u32,
        height: u32,
    },
    /// Visibility of the browser window, and whether it has focus.
    ///
    /// **Two signals in a single message, and the second is not
    /// decorative**: visibility alone would not be enough to order the pool
    /// of the capturer when several windows are visible at the same time — they
    /// then have exactly the same visibility.
    Visibility {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        visible: bool,
        focused: bool,
    },
    /// The user pasted into the session window: here is what the
    /// clipboard of THEIR machine carries (sub-block P2 of the clipboard workstream).
    ///
    /// Emitted on a TRUSTED `paste` event, never on polling:
    /// the client calls `navigator.clipboard.readText()` nowhere, therefore
    /// requests no permission, and reads the user's clipboard
    /// only at the exact moment the user expresses the intent to
    /// paste. Measured favourable on a focused `<video>`, two runs —
    /// `docs/superpowers/plans/journaux-presse-papier-p2/p2-paste-video-*.json`.
    ///
    /// ⚠️ **`text` is a `String`, NOT an `Option<String>`, and the asymmetry
    /// with `AgentControl::Clipboard` is intended** — it is not an oversight.
    /// Over there, the `None` CARRIES the size refusal, because the refusal comes from
    /// the agent and must climb up to the banner. Here the direction is reversed: it is the
    /// **client** that bounds before emitting (it has the banner at hand), and
    /// the agent that refuses by logging, without sending anything back. An `Option` on
    /// this side would therefore have nobody to write it and nobody to read it.
    ///
    /// **No `bytes` field either, for the same reason**: over there it serves
    /// to make the message self-describing in the log AND to feed the
    /// banner; here the size is read from `text.len()`, and there is no
    /// banner to feed.
    Clipboard {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        text: String,
    },
}

/// Message from the agent to the web client.
///
/// 🔴 **`Debug` is IMPLEMENTED BY HAND, never derived** — same reason as
/// for `ClientControl`: the `Clipboard` variant carries private content.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentControl {
    Ready {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        width: u32,
        height: u32,
        /// Is the microphone available for this session (workstream E)?
        ///
        /// **No bump of `CONTROL_VERSION` is needed**, in either
        /// direction: the TypeScript parser checks `v` then `type` then CASTS, so
        /// that an extra field is ignored by an old client; and
        /// a recent client facing an old agent reads `mic === undefined`,
        /// hence falsy, hence shows no button — the rule of spec §10,
        /// obtained for free.
        ///
        /// ⚠️ **`#[serde(default)]` is MANDATORY, not decorative**:
        /// `AgentControl` carries `deny_unknown_fields`, which does not prevent
        /// ADDING a field, but a MISSING field remains a
        /// deserialization error on the Rust side.
        ///
        /// ⚠️ **This flag is decided at ESTABLISHMENT and cannot
        /// express a later refusal**: exclusive use of the cable is acquired on the
        /// first upstream packet (block E2), hence after this message. **That
        /// half stays entirely true.**
        ///
        /// ✅ **Its consequence, however, no longer does.** This paragraph said:
        /// "A second user will see the button and will not get the sound. A NAMED
        /// gap, not a hidden one — the refusal is logged once on the agent
        /// side". **Block E3 closed the gap**: the refusal now climbs up
        /// as [`AgentControl::MicState`], emitted ON TRANSITION, and the
        /// button of the losing window says so. The log, for its part, stays single.
        ///
        /// ⚠️ **`mic` has NOT become redundant for all that, and the two do
        /// not say the same thing**: `mic` says "this session has an upstream
        /// track and a sink" — a WASAPI failure, an unnegotiated track —,
        /// `MicState` says "what this microphone picks up reaches the VM". A session
        /// can perfectly well have `mic: true` and `granted: false`, and that is
        /// even the nominal case of the losing window.
        #[serde(default)]
        mic: bool,
    },
    SessionEnd {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        reason: String,
    },
    /// Pointer state. `visible: false` means both "lock the
    /// pointer" and "show no cursor": it is a single
    /// observation on the agent side (the system cursor is hidden), hence a single
    /// message.
    Pointer {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        visible: bool,
        shape: CursorShape,
    },
    /// The window is asleep — its encoder and its duplication have been released.
    ///
    /// `reason` is `"masquee"` (the user wanted it) or `"evincee"` (the
    /// pool took its place while the user was looking at it). The two do not
    /// weigh the same for the user: the second deserves to be said.
    Asleep {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        asleep: bool,
        reason: String,
    },
    Rumble {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        left: u8,
        right: u8,
    },
    /// Emitted once per session, but NOT after `Ready` in practice:
    /// `agent/src/demarrage.rs` pushes this message into the control `mpsc` channel
    /// as soon as the transport starts, even before the data channel
    /// opens — the draining (`transport/tick.rs::act_on_timeout`) therefore queues
    /// it before `Event::ChannelOpen` adds `Ready` there. The real order
    /// is `Capabilities`, possibly a first `Pointer`, then `Ready`.
    /// No consequence today (the client handles the types
    /// independently, see `client/src/main.ts`), but a client that
    /// gated its initialisation on `Ready` would lose this message and the
    /// first `Pointer`: do not do that.
    Capabilities {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        gamepad: bool,
        /// Is browser → VM pasting available for this session
        /// (sub-block P2 of the clipboard workstream)?
        ///
        /// **The client GATES its keyboard exception on it.** Without this gate,
        /// `PRESSE_PAPIER=0` would produce the worst of both worlds: the client
        /// would hold back `Ctrl+V` (it would no longer send it on the input
        /// channel) while nobody would inject it on the VM side — the key
        /// would be lost, and the user would see a dead shortcut.
        ///
        /// ⚠️ **No bump of `CONTROL_VERSION`**, exactly like `mic`, and
        /// for the reason the comment on `mic` carries: an old client
        /// ignores an extra field, a recent client facing an old
        /// agent reads `undefined`, hence falsy, hence arms nothing.
        ///
        /// ⚠️ **`#[serde(default)]` is MANDATORY, and the reason is NOT
        /// `deny_unknown_fields`** — that one refuses an UNKNOWN field, whereas
        /// it is serde's default that refuses a MISSING field. The two
        /// mechanisms have nothing to do with each other; the spec confuses them, the comment
        /// on `mic` says the right thing.
        ///
        /// 🔴 **VALIDITY CONDITION OF THIS ANNOUNCEMENT, not to be lost.**
        /// It is emitted by the CHILD, whereas the clipboard belongs
        /// to the CAPTURER (D1). It is only true because both read the
        /// **same inherited environment variable**: `std::process::Command`
        /// inherits the parent's environment, and `superviseur/lanceur.rs` does not clear
        /// `PRESSE_PAPIER` when launching the capturer. **The day the capturer
        /// decides otherwise than by reading this variable — a per-session
        /// setting, a Windows capability probed live —, this announcement
        /// would become false SILENTLY.** It is not "the capturer announces
        /// its capability"; it is "both read the same variable".
        #[serde(default)]
        clipboard: bool,
    },
    /// The Windows application went fullscreen, or left it.
    ///
    /// **The client ARMS, it does not act**: `requestFullscreen()` requires a
    /// transient user activation that a data channel message does not
    /// provide. See `client/src/fullscreen.ts`.
    ///
    /// The direction is SINGLE — the browser never forces the state of the Windows
    /// window —, and that is what makes any oscillation impossible.
    Fullscreen {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        active: bool,
    },
    /// The VM clipboard has changed.
    ///
    /// Pushed **unsolicited**, and **on change only**: the
    /// owner compares the content with the last one emitted before emitting
    /// (guard no. 2 of D5), because the Windows sequence counter moves
    /// even on an identical rewrite — measured, probe P0 of 20 August 2026,
    /// `q2="bouge"` on two runs.
    ///
    /// `text` is `None` when the content exceeds the owner's bound
    /// (`agent::presse_papier::PRESSE_PAPIER_MAX`): it is **REFUSED, never
    /// truncated** — a silently amputated paste is the worst possible
    /// outcome, and it is worse than no paste at all, the user not
    /// being able to see that the end is missing.
    ///
    /// `bytes` then carries the refused size, in UTF-8 bytes **after
    /// line-ending normalisation**, so that the banner can state it;
    /// in the normal case it carries the size of the emitted text, which makes the
    /// message self-describing in the log. It is therefore not redundant with
    /// `text`.
    ///
    /// ⚠️ **`text` is not an `Option` for serialization convenience**: the
    /// field must stay PRESENT and be `null` on a refusal. Making it
    /// omittable (`skip_serializing_if`) would mean a client could no longer
    /// tell a refusal from a message truncated on the way.
    ///
    /// **One variant, not two**: the repository's precedent is `Asleep`
    /// (a state plus its reason) and `Link` (a decision plus its magnitudes);
    /// the repository has no precedent of two variants for a single state.
    Clipboard {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        text: Option<String>,
        bytes: u32,
    },
    /// The accent colour of the Windows window — the dominant hue of its
    /// icon (sub-block A1). Emitted **on change only**, and **its
    /// FIRST reading included**: without it, `--accent-fenetre` would
    /// never be set for the session.
    ///
    /// `couleur` is **`#rrggbb`, six lowercase hexadecimal digits, and nothing
    /// else**. ⚠️ **The format is a constraint of the DESIGN SYSTEM, not of the
    /// protocol**, and writing it here keeps a successor from believing it arbitrary
    /// and widening it: `client/src/design/contraste.ts::luminanceRelative`
    /// accepts only `#rgb`, `#rgba`, `#rrggbb` and `#rrggbbaa`, and **THROWS** on
    /// everything else. The client defends itself (`client/src/accent.ts` checks the
    /// shape BEFORE calling `rapportDeContraste`, and **without `try/catch`**),
    /// but the agent has no reason to send it a shape it will throw away.
    ///
    /// ⚠️ **This message carries NEITHER the `hwnd`, NOR the PID, NOR the title of the
    /// window**, and the second reason is a paid lesson: the session is
    /// already identified by the channel it arrives on, and **P2 found the
    /// clipboard IN CLEAR in `agent.log`** at an earlier logging site,
    /// harmless as long as no variant carried private
    /// content. The remedy applies **TO THE TYPE, not to the site**: a window
    /// title or an executable path here would replay that defect
    /// identically. `transport/controle.rs` logs only a TYPE NAME.
    ///
    /// 🔴 **`CONTROL_VERSION` DOES NOT GO UP**, and it is the written reasoning of
    /// D7 that `mic` and `Capabilities` already apply: a NEW variant
    /// of `AgentControl` is not a break. The client dispatches by `type`,
    /// and an old client falls into its final `else` and ignores the message.
    ///
    /// ⚠️ **`Capabilities` does NOT gain a field either**: the accent is
    /// not a capability the client must announce or discover — it
    /// receives it, or it does not.
    Accent {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        couleur: String,
    },
    /// Network link state, emitted at each change of adaptation
    /// decision — hence rarely, not every second.
    Link {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        bitrate: u32,
        width: u32,
        height: u32,
        quality: LinkQuality,
        adaptation: LinkAdaptation,
    },
    /// Is THIS window's microphone heard by the VM? (block E3)
    ///
    /// **Emitted ON TRANSITION, never on every deposit.** The microphone deposits a
    /// frame every 20 ms; emitting on every deposit would make fifty
    /// messages per second on the control channel, which is the
    /// *reliable, ordered, low throughput* channel described at the top of this file.
    ///
    /// 🔴 **This message exists because `Ready.mic` CANNOT express it**,
    /// and the doc of that field already says so: it is decided at ESTABLISHMENT,
    /// whereas exclusive use of the cable is acquired on the **first upstream
    /// packet** (block E2), hence after. The consequence measured by E2 and
    /// confirmed by reading the code in E3: with two windows, **the button of
    /// the loser lights up and nothing comes out** —
    /// `transport/piste_micro.rs::micro_disponible` never consults the
    /// mutex. It is legacy no. 2 of E2, and it is what this variant closes.
    ///
    /// ⚠️ **`granted` is a BOOLEAN, not a reason**, and it is a choice: the
    /// only refusal that exists is exclusive use of the cable. An open reason
    /// would invite filing the WASAPI failure there, and two ways of saying the same
    /// failure diverge.
    ///
    /// 🔴 **THIS FIELD IS AN EXCLUSIVITY VERDICT, NEVER A
    /// RECEIPT**, and the acceptance run of block E3 measured it: under
    /// `MICRO_FAUTE_ECRITURE`, the WASAPI render thread dies, the judge on CABLE
    /// Output reads an amplitude of **0.000000**, and the window nevertheless
    /// receives `granted: true`. The mutex lives in `PuitsCable::deposer`; the
    /// render thread is elsewhere, and nothing links them.
    ///
    /// **A `false` is therefore conclusive — another window holds the cable —
    /// whereas a `true` is not**: it rules out ONE cause of silence, not
    /// the others. Resting a "you are heard" on it would be a promise that
    /// this boolean cannot keep.
    ///
    /// ⚠️ **The name is in TWO words, and that is not decorative**: the
    /// sub-block G1 measured that a `rename_all` is **unobservable** on an
    /// enum whose variants all fit in a single word — the mutation
    /// `kebab-case` → `snake_case` left the suite entirely green there.
    /// `AgentControl` already carries `SessionEnd`, so the gap is closed there;
    /// `MicState` keeps it closed rather than reopening it.
    ///
    /// 🔴 **`CONTROL_VERSION` DOES NOT GO UP.** The two checks of `v` —
    /// `check_version` above and `parseAgentControl` on the TypeScript side —
    /// are **strict equalities**: raising it would reject **all**
    /// messages, `Ready` and `SessionEnd` included, and would replace a
    /// PER-MESSAGE degradation with a TOTAL incompatibility. An old client
    /// dispatches by `type` and falls into its final `else`. It is the
    /// written reasoning of D7, which `mic`, `Capabilities`, `Accent` and
    /// `Clipboard` all apply.
    ///
    /// ⚠️ **No private content** — the rule P2 paid for by finding the
    /// clipboard in clear in `agent.log`. A boolean has nothing to
    /// disclose, and the remedy applies **to the TYPE, not to the logging
    /// site**.
    MicState {
        #[serde(rename = "v", deserialize_with = "check_version")]
        version: u8,
        granted: bool,
    },
}

/// The REDACTION in the log — the two `impl Debug` written by hand, and the
/// reasoning that requires them. See its header comment.
#[path = "control/redaction.rs"]
mod redaction;

/// Constructors of both enums at the current protocol version, split out to
/// `control/constructeurs.rs` to stay under 500 lines.
#[path = "control/constructeurs.rs"]
mod constructeurs;

#[cfg(test)]
#[path = "control/tests.rs"]
mod tests;
