//! Binary codec of input messages (mouse, keyboard).
//!
//! Format: `version: u8 | type: u8 | payload`, integers in little-endian.
//! Each message is self-contained and of fixed size: the transport channel is
//! unreliable and unordered, no message depends on another.

/// Version of the input protocol. Increment on any format change.
///
/// v2 (workstream B): added `MouseMoveRelative` (type 5) and `Gamepad`
/// (type 6). Agent and client being deployed together, mutual rejection of
/// versions is the desirable behaviour — a v1 client talking to
/// a v2 agent would have no way anyway to announce a relative
/// mode.
pub const PROTOCOL_VERSION: u8 = 2;

const TYPE_MOUSE_MOVE: u8 = 1;
const TYPE_MOUSE_BUTTON: u8 = 2;
const TYPE_WHEEL: u8 = 3;
const TYPE_KEY: u8 = 4;
const TYPE_MOUSE_MOVE_RELATIVE: u8 = 5;
const TYPE_GAMEPAD_STATE: u8 = 6;

/// Complete state of a gamepad, modelled on `XINPUT_GAMEPAD`: no
/// conversion on the agent side, hence no chance of getting the convention wrong.
///
/// `seq` grows from one message to the next. The channel is unordered: it makes it possible
/// to reject an older state that arrived after a more recent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamepadState {
    pub seq: u16,
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    fn to_u8(self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }

    fn from_u8(v: u8) -> Result<Self, DecodeError> {
        match v {
            0 => Ok(MouseButton::Left),
            1 => Ok(MouseButton::Right),
            2 => Ok(MouseButton::Middle),
            other => Err(DecodeError::UnknownButton(other)),
        }
    }
}

/// Input message from the client to the agent.
///
/// The `x`/`y` coordinates are normalised on `0..=65535` relative to the
/// video area: this space is independent of the current resolution and
/// maps directly onto the absolute mode of `SendInput`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMessage {
    MouseMove {
        x: u16,
        y: u16,
    },
    MouseButton {
        button: MouseButton,
        pressed: bool,
        x: u16,
        y: u16,
    },
    Wheel {
        delta_x: i16,
        delta_y: i16,
    },
    Key {
        scancode: u16,
        pressed: bool,
        extended: bool,
    },
    /// Relative movement, in raw pixels. Emitted under Pointer Lock, when
    /// the agent announced a hidden cursor.
    MouseMoveRelative {
        dx: i16,
        dy: i16,
    },
    Gamepad(GamepadState),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u8),
    #[error("unknown message type: {0}")]
    UnknownType(u8),
    #[error("unknown mouse button: {0}")]
    UnknownButton(u8),
    #[error("truncated message: {actual} bytes received, {expected} expected")]
    Truncated { expected: usize, actual: usize },
}

impl InputMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8);
        out.push(PROTOCOL_VERSION);
        match *self {
            InputMessage::MouseMove { x, y } => {
                out.push(TYPE_MOUSE_MOVE);
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
            InputMessage::MouseButton {
                button,
                pressed,
                x,
                y,
            } => {
                out.push(TYPE_MOUSE_BUTTON);
                out.push(button.to_u8());
                out.push(pressed as u8);
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
            InputMessage::Wheel { delta_x, delta_y } => {
                out.push(TYPE_WHEEL);
                out.extend_from_slice(&delta_x.to_le_bytes());
                out.extend_from_slice(&delta_y.to_le_bytes());
            }
            InputMessage::Key {
                scancode,
                pressed,
                extended,
            } => {
                out.push(TYPE_KEY);
                out.extend_from_slice(&scancode.to_le_bytes());
                out.push(pressed as u8);
                out.push(extended as u8);
            }
            InputMessage::MouseMoveRelative { dx, dy } => {
                out.push(TYPE_MOUSE_MOVE_RELATIVE);
                out.extend_from_slice(&dx.to_le_bytes());
                out.extend_from_slice(&dy.to_le_bytes());
            }
            InputMessage::Gamepad(state) => {
                out.push(TYPE_GAMEPAD_STATE);
                out.extend_from_slice(&state.seq.to_le_bytes());
                out.extend_from_slice(&state.buttons.to_le_bytes());
                out.push(state.left_trigger);
                out.push(state.right_trigger);
                out.extend_from_slice(&state.thumb_lx.to_le_bytes());
                out.extend_from_slice(&state.thumb_ly.to_le_bytes());
                out.extend_from_slice(&state.thumb_rx.to_le_bytes());
                out.extend_from_slice(&state.thumb_ry.to_le_bytes());
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let header = take(bytes, 0, 2)?;
        if header[0] != PROTOCOL_VERSION {
            return Err(DecodeError::UnsupportedVersion(header[0]));
        }
        match header[1] {
            TYPE_MOUSE_MOVE => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::MouseMove {
                    x: le_u16(p, 0),
                    y: le_u16(p, 2),
                })
            }
            TYPE_MOUSE_BUTTON => {
                let p = take(bytes, 2, 6)?;
                Ok(InputMessage::MouseButton {
                    button: MouseButton::from_u8(p[0])?,
                    pressed: p[1] != 0,
                    x: le_u16(p, 2),
                    y: le_u16(p, 4),
                })
            }
            TYPE_WHEEL => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::Wheel {
                    delta_x: le_u16(p, 0) as i16,
                    delta_y: le_u16(p, 2) as i16,
                })
            }
            TYPE_KEY => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::Key {
                    scancode: le_u16(p, 0),
                    pressed: p[2] != 0,
                    extended: p[3] != 0,
                })
            }
            TYPE_MOUSE_MOVE_RELATIVE => {
                let p = take(bytes, 2, 4)?;
                Ok(InputMessage::MouseMoveRelative {
                    dx: le_u16(p, 0) as i16,
                    dy: le_u16(p, 2) as i16,
                })
            }
            TYPE_GAMEPAD_STATE => {
                let p = take(bytes, 2, 14)?;
                Ok(InputMessage::Gamepad(GamepadState {
                    seq: le_u16(p, 0),
                    buttons: le_u16(p, 2),
                    left_trigger: p[4],
                    right_trigger: p[5],
                    thumb_lx: le_u16(p, 6) as i16,
                    thumb_ly: le_u16(p, 8) as i16,
                    thumb_rx: le_u16(p, 10) as i16,
                    thumb_ry: le_u16(p, 12) as i16,
                }))
            }
            other => Err(DecodeError::UnknownType(other)),
        }
    }
}

fn take(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], DecodeError> {
    bytes
        .get(offset..offset + len)
        .ok_or(DecodeError::Truncated {
            expected: offset + len,
            actual: bytes.len(),
        })
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(msg: InputMessage) {
        let encoded = msg.encode();
        let decoded = InputMessage::decode(&encoded).expect("successful decoding");
        assert_eq!(msg, decoded);
    }

    #[test]
    fn round_trip_mouse_move() {
        round_trip(InputMessage::MouseMove { x: 0, y: 0 });
        round_trip(InputMessage::MouseMove { x: 65535, y: 32768 });
    }

    #[test]
    fn round_trip_mouse_button() {
        round_trip(InputMessage::MouseButton {
            button: MouseButton::Left,
            pressed: true,
            x: 100,
            y: 200,
        });
        round_trip(InputMessage::MouseButton {
            button: MouseButton::Middle,
            pressed: false,
            x: 65535,
            y: 65535,
        });
    }

    #[test]
    fn round_trip_wheel() {
        round_trip(InputMessage::Wheel {
            delta_x: 0,
            delta_y: 120,
        });
        round_trip(InputMessage::Wheel {
            delta_x: -240,
            delta_y: -120,
        });
    }

    #[test]
    fn round_trip_key() {
        round_trip(InputMessage::Key {
            scancode: 0x1E,
            pressed: true,
            extended: false,
        });
        round_trip(InputMessage::Key {
            scancode: 0x48,
            pressed: false,
            extended: true,
        });
    }

    #[test]
    fn little_endian_encoding() {
        // MouseMove x=0x0201, y=0x0403: version, type, then low bytes first.
        let encoded = InputMessage::MouseMove {
            x: 0x0201,
            y: 0x0403,
        }
        .encode();
        assert_eq!(encoded, vec![PROTOCOL_VERSION, 1, 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn rejects_unknown_version() {
        let err = InputMessage::decode(&[99, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnsupportedVersion(99)));
    }

    #[test]
    fn rejects_unknown_type() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 42, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownType(42)));
    }

    #[test]
    fn rejects_truncated_message() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 1, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::Truncated { .. }));
    }

    #[test]
    fn rejects_empty_message() {
        assert!(matches!(
            InputMessage::decode(&[]).unwrap_err(),
            DecodeError::Truncated { .. }
        ));
    }

    #[test]
    fn rejects_unknown_button() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 2, 9, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownButton(9)));
    }

    #[test]
    fn round_trip_relative_motion() {
        round_trip(InputMessage::MouseMoveRelative { dx: 0, dy: 0 });
        round_trip(InputMessage::MouseMoveRelative {
            dx: -32768,
            dy: 32767,
        });
    }

    #[test]
    fn round_trip_gamepad() {
        round_trip(InputMessage::Gamepad(GamepadState {
            seq: 65535,
            buttons: 0xF00D,
            left_trigger: 255,
            right_trigger: 1,
            thumb_lx: -32768,
            thumb_ly: 32767,
            thumb_rx: 0,
            thumb_ry: -1,
        }));
    }

    #[test]
    fn rejects_version_1_now_obsolete() {
        let err = InputMessage::decode(&[1, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnsupportedVersion(1)));
    }

    #[test]
    fn conformance_to_the_shared_vectors() {
        let raw = include_str!("../vectors.json");
        let doc: serde_json::Value = serde_json::from_str(raw).expect("valid vectors.json");
        let cases = doc["cases"].as_array().expect("array of cases");
        assert!(!cases.is_empty(), "at least one vector expected");

        for case in cases {
            let name = case["name"].as_str().unwrap();
            let expected: Vec<u8> = case["bytes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();

            let msg = match case["kind"].as_str().unwrap() {
                "mouse_move" => InputMessage::MouseMove {
                    x: case["x"].as_u64().unwrap() as u16,
                    y: case["y"].as_u64().unwrap() as u16,
                },
                "mouse_button" => InputMessage::MouseButton {
                    button: MouseButton::from_u8(case["button"].as_u64().unwrap() as u8).unwrap(),
                    pressed: case["pressed"].as_bool().unwrap(),
                    x: case["x"].as_u64().unwrap() as u16,
                    y: case["y"].as_u64().unwrap() as u16,
                },
                "wheel" => InputMessage::Wheel {
                    delta_x: case["delta_x"].as_i64().unwrap() as i16,
                    delta_y: case["delta_y"].as_i64().unwrap() as i16,
                },
                "key" => InputMessage::Key {
                    scancode: case["scancode"].as_u64().unwrap() as u16,
                    pressed: case["pressed"].as_bool().unwrap(),
                    extended: case["extended"].as_bool().unwrap(),
                },
                "mouse_move_relative" => InputMessage::MouseMoveRelative {
                    dx: case["dx"].as_i64().unwrap() as i16,
                    dy: case["dy"].as_i64().unwrap() as i16,
                },
                "gamepad_state" => InputMessage::Gamepad(GamepadState {
                    seq: case["seq"].as_u64().unwrap() as u16,
                    buttons: case["buttons"].as_u64().unwrap() as u16,
                    left_trigger: case["left_trigger"].as_u64().unwrap() as u8,
                    right_trigger: case["right_trigger"].as_u64().unwrap() as u8,
                    thumb_lx: case["thumb_lx"].as_i64().unwrap() as i16,
                    thumb_ly: case["thumb_ly"].as_i64().unwrap() as i16,
                    thumb_rx: case["thumb_rx"].as_i64().unwrap() as i16,
                    thumb_ry: case["thumb_ry"].as_i64().unwrap() as i16,
                }),
                other => panic!("unknown vector type: {other}"),
            };

            assert_eq!(msg.encode(), expected, "encoding of vector « {name} »");
            assert_eq!(
                InputMessage::decode(&expected).expect("decoding of the vector"),
                msg,
                "decoding of vector « {name} »"
            );
        }
    }
}
