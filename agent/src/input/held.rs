//! The keys and mouse buttons a session currently HOLDS pressed, and the
//! releases that undo them.
//!
//! The input channel is unordered and unreliable (`ordered: false,
//! maxRetransmits: 0`, `client/src/webrtc.ts`): a key-up or button-up can be
//! lost, and a tab can close while a key is held. Windows then keeps Shift,
//! Ctrl or the left button down for every later input in the VM until
//! something injects the matching release. This type is the bookkeeping that
//! lets the agent do it itself — on loss of focus and when the injector goes.
//!
//! **Pure, and outside the `#[cfg(windows)]` of `input.rs`, on purpose**: it is
//! the logic both owners share (the transport, which sees what the peer
//! pressed, and the Windows injector, which knows what it injected), and it
//! must be testable on the host.

use proto::input::{InputMessage, MouseButton};

/// One thing held down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    Key { scancode: u16, extended: bool },
    Button(MouseButton),
}

/// What is held, in the order it was pressed.
#[derive(Debug, Default)]
pub struct HeldInputs {
    order: Vec<Held>,
    /// The last absolute position the peer gave: a button release carries
    /// one, and the cursor's own position is the only one that moves nothing.
    position: (u16, u16),
}

impl HeldInputs {
    /// Updates the held set from one input message.
    pub fn observer(&mut self, message: &InputMessage) {
        let (item, is_press) = match *message {
            InputMessage::Key {
                scancode,
                pressed,
                extended,
            } => (Held::Key { scancode, extended }, pressed),
            InputMessage::MouseButton {
                button,
                pressed,
                x,
                y,
            } => {
                self.position = (x, y);
                (Held::Button(button), pressed)
            }
            InputMessage::MouseMove { x, y } => {
                self.position = (x, y);
                return;
            }
            InputMessage::Wheel { .. }
            | InputMessage::MouseMoveRelative { .. }
            | InputMessage::Gamepad(_) => return,
        };
        self.order.retain(|m| *m != item);
        if is_press {
            self.order.push(item);
        }
    }

    /// The releases of everything held, LAST PRESSED FIRST, and forgets them.
    pub fn release_all(&mut self) -> Vec<InputMessage> {
        let (x, y) = self.position;
        self.order
            .drain(..)
            .rev()
            .map(|item| match item {
                Held::Key { scancode, extended } => InputMessage::Key {
                    scancode,
                    pressed: false,
                    extended,
                },
                Held::Button(button) => InputMessage::MouseButton {
                    button,
                    pressed: false,
                    x,
                    y,
                },
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "held/tests.rs"]
mod tests;
