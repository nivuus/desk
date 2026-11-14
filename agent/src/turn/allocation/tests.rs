//! Tests of the TURN allocation state machine (`allocation.rs`), kept in
//! their own file for the 500-lines-per-file limit.

use super::*;
use crate::turn::fixtures::{allouee, reponse, serveur, t0, trans_id_de};
use crate::turn::messages::{
    sha1_hmac, ATTR_ERROR_CODE, ATTR_NONCE, ATTR_REALM, METHODE_ALLOCATE, METHODE_CHANNEL_BIND,
    METHODE_REFRESH,
};
use std::time::Duration;

#[test]
fn the_first_emission_is_a_bare_allocation() {
    let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
    let paquet = c.poll_transmit().expect("an allocation must go out");
    assert_eq!(&paquet[0..2], &[0x00, 0x03], "Allocate expected");
    // Bare: nothing after REQUESTED-TRANSPORT.
    assert_eq!(paquet.len(), 28);
    // Nothing else as long as no response has arrived.
    assert!(c.poll_transmit().is_none(), "no burst of allocations");
    assert!(c.allocation().is_none());
}

#[test]
fn a_401_triggers_a_signed_allocation() {
    let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
    let nue = c.poll_transmit().expect("allocation nue");

    let refus = reponse(
        METHODE_ALLOCATE,
        false,
        trans_id_de(&nue),
        &[
            (ATTR_ERROR_CODE, vec![0, 0, 4, 1, b'U', b'n', b'a', b'u']),
            (ATTR_REALM, b"example.org".to_vec()),
            (ATTR_NONCE, b"nonce1".to_vec()),
        ],
    );
    assert!(c.handle_packet(&refus).expect("401 handled").is_none());

    let signee = c.poll_transmit().expect("signed allocation expected");
    let message = is::stun::StunMessage::parse(&signee).expect("relue");
    assert_eq!(message.realm(), Some("example.org"));
    assert_eq!(message.nonce(), Some("nonce1"));
    assert!(message.verify(&cle_longue_duree("u", "example.org", "p"), sha1_hmac));
}

#[test]
fn a_success_returns_the_relayed_address_and_the_reflexive_address() {
    let c = allouee();
    let a = c.allocation().expect("allocation obtenue");
    assert_eq!(a.relayee, "192.0.2.15:50000".parse::<SocketAddr>().unwrap());
    assert_eq!(a.reflexive, Some("203.0.113.4:41234".parse().unwrap()));
}

#[test]
fn a_stale_nonce_is_replayed_not_abandoned() {
    // 438 "Stale Nonce" is the error case ACTUALLY encountered:
    // coturn rotates its nonces. Treating it as a failure
    // would end the allocation after a few minutes.
    let mut c = allouee();
    c.avancer(t0() + Duration::from_secs(300));
    let refresh = c.poll_transmit().expect("refresh expected");

    let perime = reponse(
        METHODE_REFRESH,
        false,
        trans_id_de(&refresh),
        &[
            (ATTR_ERROR_CODE, vec![0, 0, 4, 38, b'x']),
            (ATTR_REALM, b"r".to_vec()),
            (ATTR_NONCE, b"n2".to_vec()),
        ],
    );
    c.handle_packet(&perime).expect("438 handled");

    let rejoue = c.poll_transmit().expect("the request must go out again");
    let message = is::stun::StunMessage::parse(&rejoue).expect("relue");
    assert_eq!(message.nonce(), Some("n2"), "the new nonce must be used");
    assert!(c.allocation().is_some(), "the allocation must not be lost");
}

#[test]
fn a_channel_bind_answer_does_not_push_back_the_lease_deadline() {
    // Finding of the acceptance run of 07/30/2026, obtained through a periodic
    // state reading: the agent's allocation expired at 600 s without having been
    // refreshed. `handle_packet` reset the lease deadline at EVERY
    // success response from the server — yet responses to `CreatePermission`
    // and `ChannelBind` carry no lease. As soon as channel bindings
    // were reaffirmed periodically, their responses pushed the
    // lease refresh back to infinity, and the allocation died.
    let mut c = allouee();

    // Success response to a ChannelBind: neither LIFETIME nor relayed address.
    let reponse_bind = reponse(METHODE_CHANNEL_BIND, true, [9u8; 12], &[]);
    c.avancer(t0() + Duration::from_secs(200));
    c.handle_packet(&reponse_bind).expect("answer handled");

    // The lease deadline stays the one set by the allocation: 300 s.
    c.avancer(t0() + Duration::from_secs(300));
    let paquet = c
        .poll_transmit()
        .expect("the lease refresh must go out despite the interleaved answer");
    assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh expected");
}

#[test]
fn the_refresh_falls_at_half_the_lease() {
    let mut c = allouee();
    // 600 s lease: nothing before 300 s.
    c.avancer(t0() + Duration::from_secs(299));
    assert!(c.poll_transmit().is_none(), "refresh too early");
    c.avancer(t0() + Duration::from_secs(300));
    let paquet = c.poll_transmit().expect("refresh expected");
    assert_eq!(&paquet[0..2], &[0x00, 0x04], "Refresh expected");
}
