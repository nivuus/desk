//! Tests of the TURN allocation state machine (`allocation.rs`), kept in
//! their own file for the 500-lines-per-file limit.

use super::*;
use crate::turn::fixtures::{allouee, reponse, serveur, t0, trans_id_de};
use crate::turn::messages::{
    sha1_hmac, ATTR_ERROR_CODE, ATTR_LIFETIME, ATTR_NONCE, ATTR_REALM, METHODE_ALLOCATE,
    METHODE_CHANNEL_BIND, METHODE_CREATE_PERMISSION, METHODE_REFRESH,
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
    let (_, _, bind) = bind_channel(&mut c, "203.0.113.9:6000".parse().unwrap());

    // Success response to a ChannelBind: neither LIFETIME nor relayed address.
    let reponse_bind = reponse(METHODE_CHANNEL_BIND, true, trans_id_de(&bind), &[]);
    // Before the 150 s channel refresh, which would supersede this binding.
    c.avancer(t0() + Duration::from_secs(100));
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

/// Error response carrying a code, a realm and a nonce, as coturn sends them.
fn error_response(methode: u16, trans_id: [u8; 12], code: u16, nonce: &[u8]) -> Vec<u8> {
    let class = (code / 100) as u8;
    let number = (code % 100) as u8;
    reponse(
        methode,
        false,
        trans_id,
        &[
            (ATTR_ERROR_CODE, vec![0, 0, class, number, b'x']),
            (ATTR_REALM, b"r".to_vec()),
            (ATTR_NONCE, nonce.to_vec()),
        ],
    )
}

/// Binds a channel and returns the `CreatePermission` and `ChannelBind`
/// packets it emitted, after draining what was pending.
fn bind_channel(c: &mut TurnClient, pair: SocketAddr) -> (u16, Vec<u8>, Vec<u8>) {
    while c.poll_transmit().is_some() {}
    let canal = c.lier_canal(pair).expect("channel assigned");
    let permission = c.poll_transmit().expect("CreatePermission expected");
    let bind = c.poll_transmit().expect("ChannelBind expected");
    (canal, permission, bind)
}

#[test]
fn a_403_on_create_permission_keeps_the_allocation_and_drops_only_that_channel() {
    let mut c = allouee();
    let refused_peer: SocketAddr = "203.0.113.9:6000".parse().unwrap();
    let accepted_peer: SocketAddr = "203.0.113.10:6000".parse().unwrap();
    let (_, permission, _) = bind_channel(&mut c, refused_peer);
    bind_channel(&mut c, accepted_peer);

    let forbidden = error_response(
        METHODE_CREATE_PERMISSION,
        trans_id_de(&permission),
        403,
        b"n1",
    );
    c.handle_packet(&forbidden).expect("403 handled");

    assert!(
        c.allocation().is_some(),
        "a refused peer must not end the relay"
    );
    assert!(
        c.encapsuler(refused_peer, &[1]).is_none(),
        "the refused peer's channel is dropped"
    );
    assert!(
        c.encapsuler(accepted_peer, &[1]).is_some(),
        "the other peers keep their channel"
    );
    assert!(
        c.poll_transmit().is_none(),
        "nothing is replayed after a 403"
    );
    // The transport asks for a channel before every packet it relays: a
    // refused peer must not turn that into one request per packet.
    assert!(
        c.lier_canal(refused_peer).is_none(),
        "a refused peer is not bound again"
    );
    assert!(c.poll_transmit().is_none(), "no request for a refused peer");
}

#[test]
fn a_438_on_channel_bind_replays_that_channel_bind_and_never_reallocates() {
    let mut c = allouee();
    let pair: SocketAddr = "203.0.113.9:6000".parse().unwrap();
    let (canal, _, bind) = bind_channel(&mut c, pair);

    c.handle_packet(&error_response(
        METHODE_CHANNEL_BIND,
        trans_id_de(&bind),
        438,
        b"n2",
    ))
    .expect("438 handled");

    let rejoue = c
        .poll_transmit()
        .expect("the ChannelBind must go out again");
    assert_eq!(
        &rejoue[0..2],
        &[0x00, 0x09],
        "ChannelBind expected, not Allocate"
    );
    let message = is::stun::StunMessage::parse(&rejoue).expect("relue");
    assert_eq!(message.nonce(), Some("n2"), "the new nonce must be used");
    assert_eq!(message.channel_number(), Some(canal));
    assert_eq!(message.xor_peer_address(), Some(pair));
    assert!(c.poll_transmit().is_none(), "a single replay");
    assert!(c.allocation().is_some());
}

#[test]
fn nonce_rotations_answered_by_successful_refreshes_never_abandon_the_relay() {
    // Each rotation costs one 438; a successful refresh in between proves
    // the credentials are good, so the refusals must not add up.
    let mut c = allouee();
    let mut maintenant = t0();
    for round in 0..10u8 {
        maintenant += Duration::from_secs(300);
        c.avancer(maintenant);
        let refresh = c.poll_transmit().expect("refresh expected");
        let nonce = format!("rot{round}");
        c.handle_packet(&error_response(
            METHODE_REFRESH,
            trans_id_de(&refresh),
            438,
            nonce.as_bytes(),
        ))
        .expect("438 handled");
        let rejoue = c.poll_transmit().expect("refresh replayed");
        assert_eq!(&rejoue[0..2], &[0x00, 0x04], "Refresh expected");
        let success = reponse(
            METHODE_REFRESH,
            true,
            trans_id_de(&rejoue),
            &[(ATTR_LIFETIME, 600u32.to_be_bytes().to_vec())],
        );
        c.handle_packet(&success).expect("success handled");
        assert!(
            c.allocation().is_some(),
            "relay abandoned at rotation {round}"
        );
    }
}

#[test]
fn a_response_to_no_pending_request_is_ignored() {
    let mut c = allouee();
    while c.poll_transmit().is_some() {}
    let orphan = error_response(METHODE_ALLOCATE, [7u8; 12], 437, b"n1");
    c.handle_packet(&orphan).expect("handled");
    assert!(
        c.allocation().is_some(),
        "an unsolicited error must not end the relay"
    );
    assert!(c.poll_transmit().is_none(), "nothing is emitted for it");
}

#[test]
fn repeated_authentication_refusals_of_the_same_request_still_give_up() {
    // The bound that protects against wrong credentials survives the routing:
    // a request refused again and again ends the allocation.
    let mut c = TurnClient::new(serveur(), "u".into(), "p".into(), t0());
    for _ in 0..20 {
        let Some(paquet) = c.poll_transmit() else {
            break;
        };
        c.handle_packet(&error_response(
            METHODE_ALLOCATE,
            trans_id_de(&paquet),
            401,
            b"n1",
        ))
        .expect("401 handled");
    }
    assert!(c.allocation().is_none());
    assert!(c.poll_transmit().is_none(), "the client gave up");
}
