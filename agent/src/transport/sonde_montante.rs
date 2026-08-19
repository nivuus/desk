//! Sonde 1 du chantier E (spec §12) : str0m 0.21 dépaquetise-t-il l'Opus
//! ENTRANT et l'expose-t-il via `Event::MediaData` ?
//!
//! **Bloquante** : si la réponse est non, le chantier change de forme —
//! dépaquetisation à écrire, ou repli sur un canal de données.
//!
//! Aucun code de produit n'est exercé ici : deux `Rtc` nus, l'un offre une
//! piste audio `SendOnly`, l'autre l'accepte et doit voir arriver la charge
//! utile exacte. Le test de bout en bout **à travers `Session`** existe, lui,
//! en `piste_micro.rs` — les deux ont des objets distincts et coexistent :
//! celui-ci répond sur str0m, celui-là sur notre transport.

use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{Direction, Frequency, MediaKind, MediaTime};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Event, Input, Output, Rtc};

use super::fixtures::{local_ip, local_peer};

/// Charge utile reconnaissable et non triviale. L'octet de tête `0x78` est un
/// TOC Opus plausible, mais **la sonde n'en dépend pas** : `OpusDepacketizer`
/// est un passe-plat (`str0m-0.21.0/src/packet/opus.rs:50-60`), et c'est
/// justement ce qu'on vérifie.
const CHARGE: &[u8] = &[0x78, 0xDE, 0xAD, 0xBE, 0xEF];

/// Remet à `rtc` le premier datagramme lisible sur `socket`, ou une simple
/// échéance si rien n'arrive avant `attente`.
///
/// Distincte de `fixtures::poll_peer_socket`, qui suppose un pair unique face
/// à une `Session` : ici les DEUX bouts sont des `Rtc` nus pilotés par le
/// test, et chacun a besoin du même service.
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
    // Les DEUX pairs activent Opus : sans `enable_opus`, aucun PT Opus n'est
    // proposé et la sonde répondrait « non » pour la mauvaise raison — c'est
    // le piège de `initialisation.rs`, déjà payé au chantier A.
    let (socket_e, adresse_e, mut emetteur) = local_peer(ip, true);
    let (socket_r, adresse_r, mut recepteur) = local_peer(ip, true);

    let mut api = emetteur.sdp_api();
    // `add_media` REND le `Mid`, et c'est la seule façon de l'obtenir côté
    // OFFRANT : str0m n'émet `Event::MediaAdded` que du côté qui ACCEPTE
    // l'offre — relevé par ce test même, dont la première rédaction attendait
    // en vain un `MediaAdded` sur l'émetteur (l'agent, lui, accepte toujours,
    // c'est pourquoi `evenements.rs` s'en contente).
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
    let mut ecrit = false;
    let mut recu: Option<(Vec<u8>, Codec, u32)> = None;
    let mut horodatage = 0u64;

    while recu.is_none() {
        let maintenant = Instant::now();
        assert!(
            maintenant < echeance,
            "aucun `Event::MediaData` reçu en 15 s : str0m ne délivre pas l'Opus montant, \
             ou la connexion ne s'est pas établie (au moins une écriture tentée : {ecrit})"
        );

        // ---- l'émetteur -------------------------------------------------
        match emetteur.poll_output().expect("poll_output de l'émetteur") {
            Output::Timeout(t) => {
                {
                    let mid = mid_emission;
                    // Écrire à chaque échéance, pas une seule fois : la
                    // première écriture peut précéder l'établissement SRTP du
                    // récepteur, et un unique paquet perdu ferait répondre
                    // « non » à une sonde dont la réponse est « oui ».
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
                                ecrit = true;
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

        // ---- le récepteur -----------------------------------------------
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

    let (donnees, codec, denominateur) = recu.unwrap();
    eprintln!(
        "SONDE 1 : MediaData reçue — {} octets, codec {codec:?}, horloge RTP {denominateur} Hz",
        donnees.len()
    );

    assert_eq!(
        donnees, CHARGE,
        "la charge utile n'a pas traversé octet pour octet : str0m ne se comporte pas en \
         passe-plat sur l'Opus entrant"
    );
    assert_eq!(codec, Codec::Opus, "le codec délivré n'est pas Opus");
    assert_eq!(
        denominateur, 48_000,
        "l'horloge RTP délivrée n'est pas celle d'Opus"
    );
}
