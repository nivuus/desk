use std::time::Instant;

use proto::control::ClientControl;
use proto::input::{InputMessage, MouseButton};

use super::super::Session;
use crate::transport::fixtures;

fn session() -> Session {
    let source = Box::new(fixtures::video_test_source());
    Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session")
}

fn visibility(focused: bool) -> ClientControl {
    serde_json::from_str(&format!(
        r#"{{"type":"visibility","v":3,"visible":true,"focused":{focused}}}"#
    ))
    .expect("valid visibility")
}

const SHIFT: InputMessage = InputMessage::Key {
    scancode: 0x2a,
    pressed: true,
    extended: false,
};

#[test]
fn losing_focus_releases_what_the_peer_holds() {
    let mut session = session();
    let mut injected = Vec::new();
    let mut on_input = |m| injected.push(m);

    session.receive_input(SHIFT, &mut on_input);
    session.receive_input(
        InputMessage::MouseButton {
            button: MouseButton::Left,
            pressed: true,
            x: 7,
            y: 8,
        },
        &mut on_input,
    );
    session.release_on_focus_loss(&visibility(false), &mut on_input);

    assert_eq!(
        injected[2..],
        [
            InputMessage::MouseButton {
                button: MouseButton::Left,
                pressed: false,
                x: 7,
                y: 8,
            },
            InputMessage::Key {
                scancode: 0x2a,
                pressed: false,
                extended: false,
            },
        ],
        "a lost focus must release the button then the key (injected: {injected:?})"
    );
}

#[test]
fn keeping_focus_releases_nothing() {
    // The negative control: only a LOSS of focus releases.
    let mut session = session();
    let mut injected = Vec::new();
    let mut on_input = |m| injected.push(m);

    session.receive_input(SHIFT, &mut on_input);
    session.release_on_focus_loss(&visibility(true), &mut on_input);

    assert_eq!(injected, vec![SHIFT]);
}

#[test]
fn a_release_already_received_is_not_sent_again() {
    let mut session = session();
    let mut injected = Vec::new();
    let mut on_input = |m| injected.push(m);

    session.receive_input(SHIFT, &mut on_input);
    let released = InputMessage::Key {
        scancode: 0x2a,
        pressed: false,
        extended: false,
    };
    session.receive_input(released, &mut on_input);
    session.release_on_focus_loss(&visibility(false), &mut on_input);

    assert_eq!(injected, vec![SHIFT, released]);
}
