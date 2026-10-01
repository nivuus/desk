//! Integration tests of the bridge's transport, on a real str0m peer in local
//! loopback.
//!
//! ⚠️ **LOCAL scaffolding, and it is deliberate**: `transport::fixtures` is
//! `pub(super)` — `pont/` cannot use it. Hoisting it would touch the
//! video path for a test need, which is exactly the kind of
//! trade-off this repository refuses. The scaffolding below is therefore minimal:
//! a second `Rtc` on loopback that creates its channels and exchanges the offer.

use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use str0m::change::SdpAnswer;
use str0m::channel::ChannelId;
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

use super::*;

/// Hard budget of a test. Generous: a DTLS+ICE handshake in local
/// loopback takes a few tens of milliseconds, but a failure must return
/// a telling message rather than a hang.
const BUDGET: Duration = Duration::from_secs(10);

/// The "browser" peer: its socket, its address, its `Rtc`, and the
/// identifiers of the channels it created.
pub(super) struct Pair {
    socket: UdpSocket,
    adresse: SocketAddr,
    rtc: Rtc,
    pub(super) canaux: Vec<ChannelId>,
}

/// Sets up a bridge and a peer, negotiates `labels` as data channels, and returns
/// the ready peer as well as both ends of the bridge's loop.
///
/// **No codec is enabled on either side** — it is the point the doc of
/// `build_data_rtc` announces as exercised: `clear_codecs()` without a single
/// `enable_*` does negotiate a data-only connection.
pub(super) fn monter(labels: &[&str]) -> (Pair, Sender<VersNavigateur>, Receiver<DuNavigateur>) {
    let local_ip = "127.0.0.1".parse().unwrap();
    let (socket_pont, mut rtc_pont) = build_data_rtc(local_ip).expect("bridge built");

    let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("peer socket");
    let adresse = socket.local_addr().unwrap();
    let mut rtc = Rtc::builder().clear_codecs().build(Instant::now());
    rtc.add_local_candidate(Candidate::host(adresse, "udp").unwrap());

    // It is the BROWSER that creates the channels: the bridge is the responder.
    let mut api = rtc.sdp_api();
    let canaux: Vec<ChannelId> = labels
        .iter()
        .map(|l| api.add_channel((*l).to_string()))
        .collect();
    let (offre, en_attente) = api.apply().expect("non-empty offer");

    let reponse = rtc_pont
        .sdp_api()
        .accept_offer(offre)
        .expect("the bridge accepts a data-only offer");
    let reponse = SdpAnswer::from_sdp_string(&reponse.to_sdp_string()).expect("valid SDP answer");
    rtc.sdp_api()
        .accept_answer(en_attente, reponse)
        .expect("answer accepted");

    let (tx_sortant, rx_sortant) = channel();
    let (tx_entrant, rx_entrant) = channel();
    thread::spawn(move || {
        let _ = tourner(rtc_pont, socket_pont, rx_sortant, tx_entrant);
    });

    (
        Pair {
            socket,
            adresse,
            rtc,
            canaux,
        },
        tx_sortant,
        rx_entrant,
    )
}

/// A message the peer received: its channel, whether it is binary, its bytes.
type Recu = (ChannelId, bool, Vec<u8>);

/// **THE** driver of these tests: it pumps the peer AND collects what the bridge
/// reports, in the SAME loop.
///
/// ⚠️ The two cannot be separated, and a first draft learned it
/// the hard way: waiting for a message on `entrant` in a separate
/// function BLOCKED the only thread pumping the peer, so ICE and DTLS never
/// completed, so the channel never opened. The four tests failed at the
/// same line with the same message — the signature of a scaffolding defect, and
/// not of four product defects.
///
/// `agir` runs at each turn, with the peer's `Rtc` and everything reported
/// so far. `fini` decides the stop. Returns what the bridge reported, and what
/// the peer received on its channels.
pub(super) fn echanger(
    pair: &mut Pair,
    entrant: &Receiver<DuNavigateur>,
    mut agir: impl FnMut(&mut Rtc, &[DuNavigateur]),
    mut fini: impl FnMut(&[DuNavigateur], &[Recu]) -> bool,
    quoi: &str,
) -> (Vec<DuNavigateur>, Vec<Recu>) {
    let limite = Instant::now() + BUDGET;
    let mut remontees: Vec<DuNavigateur> = Vec::new();
    let mut recus: Vec<Recu> = Vec::new();
    loop {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        if fini(&remontees, &recus) {
            return (remontees, recus);
        }
        let maintenant = Instant::now();
        assert!(
            maintenant < limite,
            "budget exceeded without {quoi} (surfaced: {remontees:?}, received: {})",
            recus.len()
        );
        agir(&mut pair.rtc, &remontees);
        match pair.rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(limite.saturating_duration_since(maintenant))
                    // Bounded short: the bridge thread may have something to
                    // tell us at any moment, and `entrant` is only reread at the
                    // top of this turn.
                    .min(Duration::from_millis(5));
                if attente.is_zero() {
                    let _ = pair.rtc.handle_input(Input::Timeout(maintenant));
                    continue;
                }
                pair.socket.set_read_timeout(Some(attente)).unwrap();
                let mut tampon = vec![0u8; 2000];
                match pair.socket.recv_from(&mut tampon) {
                    Ok((n, source)) => {
                        if let Ok(contenu) = DatagramRecv::try_from(&tampon[..n]) {
                            let recu = Receive {
                                proto: Protocol::Udp,
                                source,
                                destination: pair.adresse,
                                contents: contenu,
                            };
                            let _ = pair.rtc.handle_input(Input::Receive(Instant::now(), recu));
                        }
                    }
                    Err(_) => {
                        let _ = pair.rtc.handle_input(Input::Timeout(Instant::now()));
                    }
                }
            }
            Output::Transmit(t) => {
                let _ = pair.socket.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::ChannelData(data)) => {
                recus.push((data.id, data.binary, data.data.clone()));
            }
            Output::Event(_) => {}
        }
    }
}

/// True as soon as the bridge has announced its channel open.
pub(super) fn canal_ouvert(remontees: &[DuNavigateur]) -> bool {
    remontees.contains(&DuNavigateur::CanalOuvert)
}

#[test]
fn a_frame_emitted_by_the_bridge_reaches_the_peer_as_binary() {
    let (mut pair, sortant, entrant) = monter(&[FILES_LABEL]);

    let trame = proto::files::encoder(
        proto::files::TYPE_LIRE,
        0x1234_5678,
        r#"{"chemin":"a.txt"}"#,
        &[0x00, 0xFF],
    );
    let a_emettre = trame.clone();
    let mut emise = false;
    let (_, recus) = echanger(
        &mut pair,
        &entrant,
        |_, remontees| {
            // Emit ONLY once the channel is announced open: emitting
            // before would lose the request, and the test would only measure
            // its own race.
            if !emise && canal_ouvert(remontees) {
                emise = true;
                sortant
                    .send(VersNavigateur::Requete {
                        correlation: 0x1234_5678,
                        trame: a_emettre.clone(),
                        echeance: None,
                    })
                    .unwrap();
            }
        },
        |_, recus| !recus.is_empty(),
        "the bridge frame reaching the peer",
    );

    let (_, binaire, octets) = recus.first().expect("one frame received").clone();
    // ⚠️ `binary = true`, unlike the `control` channel which writes `false`: the
    // payload is made of raw bytes, and text mode would put it through
    // UTF-8 validation on the browser side — 0x00 and 0xFF would not survive it.
    assert!(binaire, "the frame must be written as BINARY");
    assert_eq!(octets, trame, "the frame must arrive byte for byte");
}

#[test]
fn a_peer_frame_surfaces_with_its_correlation() {
    let (mut pair, _sortant, entrant) = monter(&[FILES_LABEL]);
    let reponse = proto::files::encoder(proto::files::TYPE_DATA, 0x0BAD_F00D, "{}", &[1, 2, 3, 4]);

    let canal = pair.canaux[0];
    let a_envoyer = reponse.clone();
    let mut envoye = false;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !envoye && canal_ouvert(remontees) {
                if let Some(mut c) = rtc.channel(canal) {
                    envoye = c.write(true, &a_envoyer).is_ok();
                }
            }
        },
        |remontees, _| {
            remontees
                .iter()
                .any(|m| matches!(m, DuNavigateur::Reponse { .. }))
        },
        "the peer's answer surfacing",
    );

    let reponse_recue = remontees
        .iter()
        .find_map(|m| match m {
            DuNavigateur::Reponse { correlation, trame } => Some((*correlation, trame.clone())),
            _ => None,
        })
        .expect("one answer surfaced");
    assert_eq!(
        reponse_recue.0, 0x0BAD_F00D,
        "the correlation must pass through intact"
    );
    assert_eq!(
        reponse_recue.1, reponse,
        "the frame must surface byte for byte"
    );
}

#[test]
fn a_channeldata_from_another_channel_is_refused_and_logged() {
    // ⚠️ This test can only be seen RED if TWO channels are negotiated: without
    // the second, there is nothing to send on the wrong one, and the test would be
    // VACUOUS — it would pass on a bridge that routes on nothing at all.
    let (mut pair, _sortant, entrant) = monter(&["autre-canal", FILES_LABEL]);
    let intrus = proto::files::encoder(proto::files::TYPE_ECHEC, 0xDEAD_0000, "{}", b"non");
    let legitime = proto::files::encoder(proto::files::TYPE_META, 0x0000_BEEF, "{}", b"oui");

    let (mauvais, bon) = (pair.canaux[0], pair.canaux[1]);
    let (a, b) = (intrus.clone(), legitime.clone());
    let mut etape = 0u8;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !canal_ouvert(remontees) {
                return;
            }
            // The INTRUDER FIRST, the legitimate frame AFTER. The channel being
            // reliable and ordered, if the legitimate one comes up while the intruder
            // never appeared, it is that the intruder was THROWN AWAY — and not
            // merely that it has not arrived yet. Without this order, the test
            // would only prove an absence within a time window.
            if etape == 0 {
                if let Some(mut c) = rtc.channel(mauvais) {
                    if c.write(true, &a).is_ok() {
                        etape = 1;
                    }
                }
            } else if etape == 1 {
                if let Some(mut c) = rtc.channel(bon) {
                    if c.write(true, &b).is_ok() {
                        etape = 2;
                    }
                }
            }
        },
        |remontees, _| {
            remontees
                .iter()
                .any(|m| matches!(m, DuNavigateur::Reponse { .. }))
        },
        "the legitimate frame surfacing",
    );

    let correlations: Vec<u32> = remontees
        .iter()
        .filter_map(|m| match m {
            DuNavigateur::Reponse { correlation, .. } => Some(*correlation),
            _ => None,
        })
        .collect();
    assert_eq!(
        correlations,
        vec![0x0000_BEEF],
        "only the frame of the `fichiers` channel must surface: the presence of \
         0xDEAD0000 would mean that the wrong channel was processed"
    );
    // …and the intruder does not arrive afterwards either.
    assert!(
        entrant.recv_timeout(Duration::from_millis(200)).is_err(),
        "no other frame must surface: the intruder was dropped"
    );
}

#[test]
fn a_single_channel_is_retained_out_of_two_and_it_is_the_label_one() {
    // 🔴 **THE label routing test, and it was born from a SURVIVING
    // MUTATION.** Replacing `if label == FILES_LABEL` with `if true`
    // left the first four tests GREEN: the intruder one negotiates
    // `autre-canal` then `files`, and since the last `ChannelOpen` overwrites
    // the previous one, `canal` ended up on the right one anyway — by the ORDER
    // of opening, not by the label. The test did not measure what it
    // announced.
    //
    // This one depends on no order: on TWO negotiated channels, the bridge must
    // announce ONLY ONE opening. Without the label filter, it
    // announces two, whatever the order in which they arrive.
    let (mut pair, _sortant, entrant) = monter(&[FILES_LABEL, "autre-canal"]);

    // We wait for BOTH channels to be open ON THE PEER SIDE — otherwise we
    // would conclude "a single opening" while the second has not
    // arrived yet, and the test would become vacuous again.
    //
    // ⚠️ A first draft entrusted this count to `echanger`, whose
    // stop condition read a counter it never increments — a
    // check unable to SUCCEED, exactly the counterpart of the check
    // unable to fail. `echanger` does not report the peer's `ChannelOpen`s;
    // this test therefore pumps itself.
    let mut ouverts = 0usize;
    let limite = Instant::now() + BUDGET;
    let mut remontees: Vec<DuNavigateur> = Vec::new();
    while ouverts < 2 && Instant::now() < limite {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        match pair.rtc.poll_output().expect("poll_output du pair") {
            Output::Transmit(t) => {
                let _ = pair.socket.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::ChannelOpen(..)) => ouverts += 1,
            Output::Event(_) => {}
            Output::Timeout(_) => {
                pair.socket
                    .set_read_timeout(Some(Duration::from_millis(5)))
                    .unwrap();
                let mut tampon = vec![0u8; 2000];
                match pair.socket.recv_from(&mut tampon) {
                    Ok((n, source)) => {
                        if let Ok(contenu) = DatagramRecv::try_from(&tampon[..n]) {
                            let recu = Receive {
                                proto: Protocol::Udp,
                                source,
                                destination: pair.adresse,
                                contents: contenu,
                            };
                            let _ = pair.rtc.handle_input(Input::Receive(Instant::now(), recu));
                        }
                    }
                    Err(_) => {
                        let _ = pair.rtc.handle_input(Input::Timeout(Instant::now()));
                    }
                }
            }
        }
    }
    assert_eq!(ouverts, 2, "the peer must have opened its TWO channels");

    // Give the bridge time to announce a possible second opening.
    let fin = Instant::now() + Duration::from_millis(300);
    while Instant::now() < fin {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let ouvertures = remontees
        .iter()
        .filter(|m| **m == DuNavigateur::CanalOuvert)
        .count();
    assert_eq!(
        ouvertures, 1,
        "two channels negotiated, ONE single opening announced: \
         seeing two means the label is not filtered (surfaced: {remontees:?})"
    );
}
