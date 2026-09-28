//! Shared scaffolding for the `turn` tests: the starting clock, the server
//! address, the response factory that replaces coturn, and the client already
//! brought to the allocated state.
//!
//! `#[cfg(test)]`: none of this is compiled in `release`.
//!
//! Only the items really shared by `allocation` and
//! `canaux` appear here — same criterion as `transport/fixtures.rs`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::allocation::TurnClient;
use super::messages::{
    write_attribute, xor_adresse, ATTR_ERROR_CODE, ATTR_LIFETIME, ATTR_NONCE, ATTR_REALM,
    ATTR_XOR_MAPPED_ADDRESS, ATTR_XOR_RELAYED_ADDRESS, MAGIC, METHODE_ALLOCATE,
};

/// Reference instant of the tests, in the past: `avancer` can thus move
/// the clock forward several minutes without ever exceeding `Instant::now()`.
pub(super) fn t0() -> Instant {
    Instant::now() - Duration::from_secs(3600)
}

pub(super) fn serveur() -> SocketAddr {
    "192.0.2.1:3478".parse().unwrap()
}

/// Fabricates the response a server would produce, to drive the client
/// without a network. It is what replaces coturn in these tests.
pub(super) fn reponse(
    methode: u16,
    classe_succes: bool,
    trans_id: [u8; 12],
    attributs: &[(u16, Vec<u8>)],
) -> Vec<u8> {
    let mut corps = Vec::new();
    for (type_, value) in attributs {
        write_attribute(&mut corps, *type_, value);
    }
    // Success class = 0b10 → bits 0x0100; error class = 0b11 → 0x0110.
    let type_fil = methode | if classe_succes { 0x0100 } else { 0x0110 };
    let mut paquet = Vec::new();
    paquet.extend_from_slice(&type_fil.to_be_bytes());
    paquet.extend_from_slice(&((corps.len()) as u16).to_be_bytes());
    paquet.extend_from_slice(&MAGIC);
    paquet.extend_from_slice(&trans_id);
    paquet.extend_from_slice(&corps);
    paquet
}

pub(super) fn trans_id_de(paquet: &[u8]) -> [u8; 12] {
    paquet[8..20].try_into().unwrap()
}

/// Brings a client to the allocated state, for the tests that start from there.
pub(super) fn allouee() -> TurnClient {
    let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
    let nue = c.poll_transmit().unwrap();
    let refus = reponse(
        METHODE_ALLOCATE,
        false,
        trans_id_de(&nue),
        &[
            (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'x']),
            (ATTR_REALM, b"r".to_vec()),
            (ATTR_NONCE, b"n1".to_vec()),
        ],
    );
    c.handle_packet(&refus).unwrap();
    let signee = c.poll_transmit().unwrap();
    let succes = reponse(
        METHODE_ALLOCATE,
        true,
        trans_id_de(&signee),
        &[
            (
                ATTR_XOR_RELAYED_ADDRESS,
                xor_adresse("192.0.2.15:50000".parse().unwrap(), &trans_id_de(&signee)),
            ),
            (
                ATTR_XOR_MAPPED_ADDRESS,
                xor_adresse("203.0.113.4:41234".parse().unwrap(), &trans_id_de(&signee)),
            ),
            (ATTR_LIFETIME, 600u32.to_be_bytes().to_vec()),
        ],
    );
    c.handle_packet(&succes).unwrap();
    c
}
