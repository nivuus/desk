//! The REDACTION of control messages in the log: what `Debug` shows,
//! and above all what it does NOT show.
//!
//! **Extracted from `control.rs` at the closing survey of sub-block P2**: the two
//! `impl` below had taken it to **494 lines, margin 6**, on a ceiling
//! of 500. It is the THIRD gate of this sub-block, and the repository's rule is
//! to extract, never to compress a comment to get back under the line.
//!
//! ⚠️ **This use of `#[path]` is OUTSIDE the scope of the "Child
//! module convention" of `docs/claude/module-conventions.md`**, which itself declares its exclusion: same
//! Rust mechanism, another reason — the 500-line rule —, exactly like
//! `superviseur/table.rs`. This module is NOT hoisted to the root.

use super::{AgentControl, ClientControl};

// ---------------------------------------------------------------------------
// 🔴 THE CLIPBOARD MUST NEVER REACH A LOG, AND `#[derive(Debug)]`
//    PUT IT THERE.
// ---------------------------------------------------------------------------
//
// **Measured, not conjectured**: the acceptance run of sub-block P2 found, in
// `agent.log`, four lines of the shape
//
//     INFO agent::demarrage: control received session=… \
//          Clipboard { version: 3, text: "alpha-arme-1-crwor9" }
//
// that is, **the content of the user's clipboard, in clear, in
// a log that this repository POURS INTO GIT**. Decision D-P1-7 forbids it
// by name ("ONE SINGLE TRACE, AND NEVER THE TEXT"), and P1 had kept to it
// in the downstream direction — `capteur::sommeil::presse_papier::distribuer`
// logs only `octets` and `refus`.
//
// ⚠️ **It is NOT a defect of the logging site.** The faulty trace
// (`demarrage.rs`, `on_control`) PREDATES the clipboard workstream:
// it prints the received message through `?message`, which was harmless as long
// as no variant carried private content. It is P2 that made this
// trace dangerous by adding `ClientControl::Clipboard`, and a remedy placed
// on the site would have let the NEXT site leak.
//
// **The remedy is therefore at the TYPE, and it is exhaustive by construction**: these
// two `impl` are written by hand, so that adding a variant forces
// deciding what it shows. The text is replaced by its SIZE — which
// keeps the log's full diagnostic power, the size being precisely
// what the bound and the refusal are about.

impl std::fmt::Debug for ClientControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientControl::Resize {
                version,
                width,
                height,
            } => f
                .debug_struct("Resize")
                .field("v", version)
                .field("width", width)
                .field("height", height)
                .finish(),
            ClientControl::Visibility {
                version,
                visible,
                focused,
            } => f
                .debug_struct("Visibility")
                .field("v", version)
                .field("visible", visible)
                .field("focused", focused)
                .finish(),
            // 🔴 THE SIZE, NEVER THE TEXT.
            ClientControl::Clipboard { version, text } => f
                .debug_struct("Clipboard")
                .field("v", version)
                .field("octets", &text.len())
                .finish(),
        }
    }
}

impl std::fmt::Debug for AgentControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // 🔴 THE SIZE AND THE REFUSAL, NEVER THE TEXT. `bytes` already carries the
            // size in the protocol: we do not recompute it, we show it.
            AgentControl::Clipboard {
                version,
                text,
                bytes,
            } => f
                .debug_struct("Clipboard")
                .field("v", version)
                .field("refus", &text.is_none())
                .field("octets", bytes)
                .finish(),
            // The other variants carry no private content: their derived
            // shape is reproduced as is, by the only means left
            // once `derive` is removed.
            AgentControl::Ready {
                version,
                width,
                height,
                mic,
            } => f
                .debug_struct("Ready")
                .field("v", version)
                .field("width", width)
                .field("height", height)
                .field("mic", mic)
                .finish(),
            AgentControl::SessionEnd { version, reason } => f
                .debug_struct("SessionEnd")
                .field("v", version)
                .field("reason", reason)
                .finish(),
            AgentControl::Pointer {
                version,
                visible,
                shape,
            } => f
                .debug_struct("Pointer")
                .field("v", version)
                .field("visible", visible)
                .field("shape", shape)
                .finish(),
            AgentControl::Asleep {
                version,
                asleep,
                reason,
            } => f
                .debug_struct("Asleep")
                .field("v", version)
                .field("asleep", asleep)
                .field("reason", reason)
                .finish(),
            AgentControl::Rumble {
                version,
                left,
                right,
            } => f
                .debug_struct("Rumble")
                .field("v", version)
                .field("left", left)
                .field("right", right)
                .finish(),
            AgentControl::Capabilities {
                version,
                gamepad,
                clipboard,
            } => f
                .debug_struct("Capabilities")
                .field("v", version)
                .field("gamepad", gamepad)
                .field("clipboard", clipboard)
                .finish(),
            AgentControl::Fullscreen { version, active } => f
                .debug_struct("Fullscreen")
                .field("v", version)
                .field("active", active)
                .finish(),
            // Sub-block A1. 🔴 **THIS `match` IS WHAT FORCED ME TO DECIDE**, and
            // it is exactly what its header promises: "adding a
            // variant forces deciding what it shows". The decision is
            // to SHOW the colour, and the reason is that this message carries
            // nothing private — no `hwnd`, no PID, no window title, no executable
            // path. A hue is a public visual property of the
            // application, and it is the only field useful for diagnosis.
            //
            // ⚠️ **If a future accent variant carried the title or the
            // path of the application, it would have to be redacted HERE**, and not
            // at the logging site: it is the lesson of P2, whose leak
            // came from an EARLIER, harmless trace made dangerous
            // by a new variant.
            AgentControl::Accent { version, couleur } => f
                .debug_struct("Accent")
                .field("v", version)
                .field("couleur", couleur)
                .finish(),
            AgentControl::Link {
                version,
                bitrate,
                width,
                height,
                quality,
                adaptation,
            } => f
                .debug_struct("Link")
                .field("v", version)
                .field("bitrate", bitrate)
                .field("width", width)
                .field("height", height)
                .field("quality", quality)
                .field("adaptation", adaptation)
                .finish(),
            // Block E3. This `match` did its job a second time: it
            // FORCED the decision, as its header promises. It is to
            // show `granted`, and the reason is that a boolean has nothing to
            // disclose — no window identity, no title, no PID. It is
            // also the only field the variant carries.
            AgentControl::MicState { version, granted } => f
                .debug_struct("MicState")
                .field("v", version)
                .field("granted", granted)
                .finish(),
        }
    }
}
