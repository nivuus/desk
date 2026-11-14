//! Injection of the browser's inputs into the Windows session.
//!
//! The coordinate mapping (window → virtual desktop) is independent of
//! any Windows API and lives in `crate::geometry::to_virtual_desktop`,
//! tested on Linux (task 12: the brief expected it here, but `Rect` was
//! introduced at task 9 in `geometry.rs`, next to `crop_region` —
//! the mapping joins it there rather than duplicating `Rect`). This module therefore
//! only carries the `SendInput` system call, specific to Windows.

/// The four keyboard events of a paste: `Ctrl`↓, `V`↓, `V`↑, `Ctrl`↑.
///
/// **Outside the `#[cfg(windows)]`, on purpose**: it is a table, not a system
/// call, and the caller (`transport/boucle.rs`) is not gated.
///
/// 🔴 **SELF-SUFFICIENT IN MODIFIERS, and it is not over-caution.**
/// The client's input channel is `ordered: false, maxRetransmits: 0`
/// (`client/src/webrtc.ts`): the state of modifiers on the VM side at the moment of
/// injection **cannot be known** — the `Ctrl`↓ the client sent
/// may have arrived, been lost, or arrive afterwards. Setting the
/// four events ourselves makes the gesture independent of all that; one `Ctrl`↑ too
/// many is harmless, a missing `Ctrl`↓ would paste nothing.
///
/// 🔴 **ALL FOUR, AND IN THIS ORDER.** Omitting the final `Ctrl`↑ would leave
/// the application with a modifier PRESSED, and **any following keystroke
/// would become a shortcut** — the most insidious defect of this path, and
/// the one a test keeps red.
///
/// Scancodes taken from `client/src/scancodes.ts`, the table the client
/// already uses: `ControlLeft` = `0x1d`, `KeyV` = `0x2f`, `extended: false`
/// for both. Taking them from there rather than rediscovering them guarantees that
/// the VM receives exactly what it receives from a human keystroke.
///
/// ⚠️ **The repository's scope on `SetForegroundWindow` is NOT widened by
/// this table.** The `InputMessage::Key` arm of `InputInjector` already calls
/// `au_premier_plan()`, which this path inherits for free — but what
/// is MEASURED (D2) is "one keystroke per window, sequential probe, no
/// concurrent keystroke", and `SendInput` remains GLOBAL to the Windows session.
/// **Two simultaneous pastes from two windows remain outside what is
/// established**; P3 is the sub-block that will meet them.
pub const TOUCHES_COLLAGE: [proto::input::InputMessage; 4] = [
    proto::input::InputMessage::Key {
        scancode: 0x1d,
        pressed: true,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x2f,
        pressed: true,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x2f,
        pressed: false,
        extended: false,
    },
    proto::input::InputMessage::Key {
        scancode: 0x1d,
        pressed: false,
        extended: false,
    },
];

mod held;
pub use held::HeldInputs;

#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::InputInjector;
