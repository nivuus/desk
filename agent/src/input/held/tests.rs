use proto::input::{InputMessage, MouseButton};

use super::HeldInputs;

fn key(scancode: u16, pressed: bool) -> InputMessage {
    InputMessage::Key {
        scancode,
        pressed,
        extended: false,
    }
}

fn button(button: MouseButton, pressed: bool, x: u16, y: u16) -> InputMessage {
    InputMessage::MouseButton {
        button,
        pressed,
        x,
        y,
    }
}

#[test]
fn press_a_then_b_releases_b_then_a() {
    let mut held = HeldInputs::default();
    held.observer(&key(0x2a, true));
    held.observer(&key(0x1d, true));
    assert_eq!(held.release_all(), vec![key(0x1d, false), key(0x2a, false)]);
    assert!(
        held.release_all().is_empty(),
        "releasing must forget what it released"
    );
}

#[test]
fn a_released_key_is_not_released_again() {
    let mut held = HeldInputs::default();
    held.observer(&key(0x2a, true));
    held.observer(&key(0x1e, true));
    held.observer(&key(0x2a, false));
    assert_eq!(held.release_all(), vec![key(0x1e, false)]);
}

#[test]
fn an_extended_key_is_distinct_from_its_plain_twin() {
    // Right Ctrl is 0x1d EXTENDED, left Ctrl is 0x1d plain: releasing one
    // must not count as releasing the other.
    let mut held = HeldInputs::default();
    let right_ctrl = InputMessage::Key {
        scancode: 0x1d,
        pressed: true,
        extended: true,
    };
    held.observer(&right_ctrl);
    held.observer(&key(0x1d, false));
    assert_eq!(
        held.release_all(),
        vec![InputMessage::Key {
            scancode: 0x1d,
            pressed: false,
            extended: true,
        }]
    );
}

#[test]
fn a_repeated_press_is_held_once() {
    // Auto-repeat sends key-down again and again: one release is enough.
    let mut held = HeldInputs::default();
    for _ in 0..3 {
        held.observer(&key(0x1e, true));
    }
    assert_eq!(held.release_all(), vec![key(0x1e, false)]);
}

#[test]
fn a_held_button_is_released_at_the_last_known_position() {
    let mut held = HeldInputs::default();
    held.observer(&button(MouseButton::Left, true, 10, 20));
    held.observer(&InputMessage::MouseMove { x: 300, y: 400 });
    held.observer(&key(0x2a, true));
    assert_eq!(
        held.release_all(),
        vec![key(0x2a, false), button(MouseButton::Left, false, 300, 400)]
    );
}

#[test]
fn messages_that_hold_nothing_leave_the_set_empty() {
    let mut held = HeldInputs::default();
    held.observer(&InputMessage::Wheel {
        delta_x: 0,
        delta_y: 120,
    });
    held.observer(&InputMessage::MouseMoveRelative { dx: 3, dy: 4 });
    assert!(held.release_all().is_empty());
}
