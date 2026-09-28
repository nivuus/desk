//! Probe 1 of workstream E (spec §12): does str0m 0.21 depacketize INCOMING
//! Opus and expose it through `Event::MediaData`?
//!
//! **Blocking**: if the answer is no, the workstream changes shape —
//! depacketization to write, or fallback to a data channel.
//!
//! No product code is exercised here: two bare `Rtc`s, one offers a
//! `SendOnly` audio track, the other accepts it and must see the exact
//! payload arrive. The end-to-end test **through `Session`** exists
//! in `piste_micro.rs` — the two have distinct purposes and coexist:
//! this one answers about str0m, that one about our transport.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{Direction, Frequency, MediaKind, MediaTime};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Event, Input, Output, Rtc};

use super::fixtures::{local_ip, local_peer};

/// Recognisable, non-trivial payload. The leading byte `0x78` is a
/// plausible Opus TOC, but **the probe does not depend on it**: `OpusDepacketizer`
/// is a pass-through (`str0m-0.21.0/src/packet/opus.rs:50-60`), and that is
/// precisely what we check.
const CHARGE: &[u8] = &[0x78, 0xDE, 0xAD, 0xBE, 0xEF];

/// Hands `rtc` the first readable datagram on `socket`, or a mere
/// deadline if nothing arrives before `attente`.
///
/// Distinct from `fixtures::poll_peer_socket`, which assumes a single peer facing
/// a `Session`: here BOTH ends are bare `Rtc`s driven by the
/// test, and each needs the same service.
fn pomper(rtc: &mut Rtc, socket: &UdpSocket, adresse: SocketAddr, attente: Duration) {
    if attente.is_zero() {
        let _ = rtc.handle_input(Input::Timeout(Instant::now()));
        return;
    }
    socket.set_read_timeout(Some(attente)).unwrap();
    let mut tampon = vec![0u8; 2000];
    match socket.recv_from(&mut tampon) {
        Ok((n, source)) => {
            if let Ok(contenu) = DatagramRecv::try_from(&tampon[..n]) {
                let recu = Receive {
                    proto: Protocol::Udp,
                    source,
                    destination: adresse,
                    contents: contenu,
                };
                let _ = rtc.handle_input(Input::Receive(Instant::now(), recu));
            }
        }
        Err(_) => {
            let _ = rtc.handle_input(Input::Timeout(Instant::now()));
        }
    }
}

#[test]
fn str0m_expose_l_opus_montant_via_media_data() {
    let ip = local_ip();
    // BOTH peers enable Opus: without `enable_opus`, no Opus PT is
    // offered and the probe would answer "no" for the wrong reason — it is
    // the trap of `initialisation.rs`, already paid for in workstream A.
    let (socket_e, adresse_e, mut emetteur) = local_peer(ip, true);
    let (socket_r, adresse_r, mut recepteur) = local_peer(ip, true);

    let mut api = emetteur.sdp_api();
    // `add_media` RETURNS the `Mid`, and it is the only way to get it on the
    // OFFERING side: str0m only emits `Event::MediaAdded` on the side that ACCEPTS
    // the offer — found by this very test, whose first draft waited
    // in vain for a `MediaAdded` on the emitter (the agent, for its part, always accepts,
    // which is why `evenements.rs` makes do with it).
    let mid_emission = api.add_media(MediaKind::Audio, Direction::SendOnly, None, None, None);
    let (offre, en_attente) = api.apply().expect("offre non vide");

    let reponse = recepteur
        .sdp_api()
        .accept_offer(offre)
        .expect("offre acceptée par le récepteur");
    emetteur
        .sdp_api()
        .accept_answer(en_attente, reponse)
        .expect("réponse acceptée par l'émetteur");

    socket_e.set_nonblocking(false).unwrap();
    socket_r.set_nonblocking(false).unwrap();

    let echeance = Instant::now() + Duration::from_secs(15);
    let mut written = false;
    let mut recu: Option<(Vec<u8>, Codec, u32)> = None;
    let mut horodatage = 0u64;

    while recu.is_none() {
        let maintenant = Instant::now();
        assert!(
            maintenant < echeance,
            "aucun `Event::MediaData` reçu en 15 s : str0m ne délivre pas l'Opus montant, \
             ou la connexion ne s'est pas établie (au moins une écriture tentée : {written})"
        );

        // ---- the emitter ------------------------------------------------
        match emetteur.poll_output().expect("poll_output de l'émetteur") {
            Output::Timeout(t) => {
                {
                    let mid = mid_emission;
                    // Write at each deadline, not just once: the
                    // first write may precede the receiver's SRTP
                    // establishment, and a single lost packet would make a probe
                    // whose answer is "yes" answer "no".
                    if let Some(writer) = emetteur.writer(mid) {
                        let pt = writer
                            .payload_params()
                            .find(|p| p.spec().codec == Codec::Opus)
                            .map(|p| p.pt());
                        if let Some(pt) = pt {
                            let temps = MediaTime::new(horodatage, Frequency::FORTY_EIGHT_KHZ);
                            if emetteur
                                .writer(mid)
                                .unwrap()
                                .write(pt, Instant::now(), temps, CHARGE.to_vec())
                                .is_ok()
                            {
                                written = true;
                                horodatage += 960;
                            }
                        }
                    }
                }
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(Duration::from_millis(5));
                pomper(&mut emetteur, &socket_e, adresse_e, attente);
            }
            Output::Transmit(t) => {
                let _ = socket_e.send_to(&t.contents, t.destination);
            }
            Output::Event(_) => {}
        }

        // ---- the receiver -----------------------------------------------
        match recepteur.poll_output().expect("poll_output du récepteur") {
            Output::Timeout(t) => {
                let attente = t
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(5));
                pomper(&mut recepteur, &socket_r, adresse_r, attente);
            }
            Output::Transmit(t) => {
                let _ = socket_r.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::MediaData(d)) => {
                recu = Some((d.data.to_vec(), d.params.spec().codec, d.time.denom()));
            }
            Output::Event(_) => {}
        }
    }

    let (data, codec, denominateur) = recu.unwrap();
    eprintln!(
        "SONDE 1 : MediaData reçue — {} octets, codec {codec:?}, horloge RTP {denominateur} Hz",
        data.len()
    );

    assert_eq!(
        data, CHARGE,
        "la charge utile n'a pas traversé octet pour octet : str0m ne se comporte pas en \
         passe-plat sur l'Opus entrant"
    );
    assert_eq!(codec, Codec::Opus, "le codec délivré n'est pas Opus");
    assert_eq!(
        denominateur, 48_000,
        "l'horloge RTP délivrée n'est pas celle d'Opus"
    );
}
