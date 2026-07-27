//! Codec binaire des messages d'entrée (souris, clavier).
//!
//! Format : `version: u8 | type: u8 | charge utile`, entiers en petit-boutiste.
//! Chaque message est autonome et de taille fixe : le canal de transport est
//! non fiable et non ordonné, aucun message ne dépend d'un autre.

/// Version du protocole d'entrée. Incrémenter à tout changement de format.
pub const PROTOCOL_VERSION: u8 = 1;

const TYPE_MOUSE_MOVE: u8 = 1;
const TYPE_MOUSE_BUTTON: u8 = 2;
const TYPE_WHEEL: u8 = 3;
const TYPE_KEY: u8 = 4;

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

/// Message d'entrée du client vers l'agent.
///
/// Les coordonnées `x`/`y` sont normalisées sur `0..=65535` par rapport à la
/// zone vidéo : cet espace est indépendant de la résolution courante et
/// correspond directement au mode absolu de `SendInput`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMessage {
    MouseMove { x: u16, y: u16 },
    MouseButton { button: MouseButton, pressed: bool, x: u16, y: u16 },
    Wheel { delta_x: i16, delta_y: i16 },
    Key { scancode: u16, pressed: bool, extended: bool },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("version de protocole non supportée : {0}")]
    UnsupportedVersion(u8),
    #[error("type de message inconnu : {0}")]
    UnknownType(u8),
    #[error("bouton de souris inconnu : {0}")]
    UnknownButton(u8),
    #[error("message tronqué : {actual} octets reçus, {expected} attendus")]
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
            InputMessage::MouseButton { button, pressed, x, y } => {
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
            InputMessage::Key { scancode, pressed, extended } => {
                out.push(TYPE_KEY);
                out.extend_from_slice(&scancode.to_le_bytes());
                out.push(pressed as u8);
                out.push(extended as u8);
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
                Ok(InputMessage::MouseMove { x: le_u16(p, 0), y: le_u16(p, 2) })
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
            other => Err(DecodeError::UnknownType(other)),
        }
    }
}

fn take(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], DecodeError> {
    bytes
        .get(offset..offset + len)
        .ok_or(DecodeError::Truncated { expected: offset + len, actual: bytes.len() })
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(msg: InputMessage) {
        let encoded = msg.encode();
        let decoded = InputMessage::decode(&encoded).expect("décodage réussi");
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
        round_trip(InputMessage::Wheel { delta_x: 0, delta_y: 120 });
        round_trip(InputMessage::Wheel { delta_x: -240, delta_y: -120 });
    }

    #[test]
    fn round_trip_key() {
        round_trip(InputMessage::Key { scancode: 0x1E, pressed: true, extended: false });
        round_trip(InputMessage::Key { scancode: 0x48, pressed: false, extended: true });
    }

    #[test]
    fn encodage_petit_boutiste() {
        // MouseMove x=0x0201, y=0x0403 : version, type, puis octets faibles en tête.
        let encoded = InputMessage::MouseMove { x: 0x0201, y: 0x0403 }.encode();
        assert_eq!(encoded, vec![PROTOCOL_VERSION, 1, 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn rejette_version_inconnue() {
        let err = InputMessage::decode(&[99, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnsupportedVersion(99)));
    }

    #[test]
    fn rejette_type_inconnu() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 42, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownType(42)));
    }

    #[test]
    fn rejette_message_tronque() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 1, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::Truncated { .. }));
    }

    #[test]
    fn rejette_message_vide() {
        assert!(matches!(
            InputMessage::decode(&[]).unwrap_err(),
            DecodeError::Truncated { .. }
        ));
    }

    #[test]
    fn rejette_bouton_inconnu() {
        let err = InputMessage::decode(&[PROTOCOL_VERSION, 2, 9, 1, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, DecodeError::UnknownButton(9)));
    }
}
