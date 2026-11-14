//! What the input log may say about an input message.
//!
//! `proto::input::InputMessage` derives `Debug`, and a key event's `Debug`
//! carries its scancode: logging it as is writes every key typed into the
//! remote session, passwords included, to `agent.log` as soon as the log
//! level allows `debug`. This view keeps the variant and the pointer
//! fields, and leaves out what identifies a key — the same intent as the
//! hand-written `Debug` of `ClientControl` and `AgentControl`, which keep
//! clipboard content out of the logs.

use proto::input::InputMessage;

/// `Debug` view of an input message that never shows which key was typed.
pub(super) struct LoggableInput<'a>(pub(super) &'a InputMessage);

impl std::fmt::Debug for LoggableInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            InputMessage::Key { .. } => f.write_str("Key { .. }"),
            other => other.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_event_is_logged_without_its_scancode() {
        let key = InputMessage::Key {
            scancode: 0x1E,
            pressed: true,
            extended: false,
        };
        let text = format!("{:?}", LoggableInput(&key));
        assert_eq!(text, "Key { .. }");
        assert!(!text.contains("30") && !text.contains("scancode"));
    }

    #[test]
    fn a_pointer_event_is_logged_as_before() {
        let motion = InputMessage::MouseMove { x: 12, y: 34 };
        assert_eq!(
            format!("{:?}", LoggableInput(&motion)),
            format!("{motion:?}")
        );
    }
}
