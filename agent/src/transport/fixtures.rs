//! Échafaudages partagés des tests d'intégration de `transport` : l'adresse
//! locale, la source vidéo de test, et le pair str0m en boucle locale qui
//! joue le rôle du navigateur.
//!
//! `#[cfg(test)]` : rien de ceci n'est compilé en `release`.
//!
//! N'y figurent que les éléments réellement redéclarés par plusieurs tests —
//! `DummyAudioSource`, `CountingSource` et `SourceRefusant` restent chacune
//! avec l'unique test qui les définit, ce ne sont pas des échafaudages
//! partagés.
//!
//! Les `use` y sont explicites plutôt qu'un `use super::*` : `transport.rs` ne
//! porte plus que la structure et la boucle, et n'importe donc plus de
//! lui-même tout ce dont ces échafaudages ont besoin.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use str0m::media::Mid;
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::stats::MediaEgressStats;
use str0m::{Candidate, Input, Rtc};

/// Adresse locale utilisée par tous les pairs de test (loopback).
pub(super) fn local_ip() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

/// Ouvre le flux H.264 de test partagé par les tests d'intégration de
/// `transport` : `testdata/testsrc.264`, 1280x720 à 60 im/s.
pub(super) fn video_test_source() -> crate::source::FileSource {
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("chargement du flux de test")
}

/// Construit le second `Rtc` str0m représentant le pair « navigateur » en
/// boucle locale : socket UDP lié et candidate hôte ajoutée. La négociation
/// SDP (pistes, canaux) reste propre à chaque test, qui négocie des
/// combinaisons différentes de pistes.
pub(super) fn local_peer(local_ip: IpAddr, enable_opus: bool) -> (UdpSocket, SocketAddr, Rtc) {
    let peer_socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket du pair");
    let peer_addr = peer_socket.local_addr().unwrap();
    let mut builder = Rtc::builder().clear_codecs().enable_h264(true);
    if enable_opus {
        builder = builder.enable_opus(true);
    }
    let mut peer_rtc = builder.build(Instant::now());
    peer_rtc.add_local_candidate(Candidate::host(peer_addr, "udp").unwrap());
    (peer_socket, peer_addr, peer_rtc)
}

/// Statistiques d'émission minimales pour la piste `mid` : seuls `mid`, `rtt`
/// et `loss` sont lus par `handle_event`, le reste n'a qu'à exister.
///
/// Partagée entre `adaptation` et `part` (sous-bloc D6) : les deux modules en
/// ont besoin pour amener le contrôleur à observer une estimation réelle.
pub(super) fn stats_video(mid: Mid) -> MediaEgressStats {
    MediaEgressStats {
        mid,
        rid: None,
        bytes: 0,
        packets: 0,
        firs: 0,
        plis: 0,
        nacks: 0,
        rtt: Some(Duration::from_millis(20)),
        loss: Some(0.0),
        timestamp: Instant::now(),
        remote: None,
    }
}

/// Reçoit un datagramme du pair et le transmet à son `Rtc`, en respectant
/// `wait` — déjà borné par l'appelant selon ses propres échéances (fenêtre
/// de mesure, échéance dure...). Renvoie `true` si l'appelant doit reprendre
/// son tour de boucle immédiatement (délai déjà écoulé) : c'est à l'appelant
/// de faire `continue`, cette fonction ne peut pas le faire à sa place.
pub(super) fn poll_peer_socket(
    peer_rtc: &mut Rtc,
    peer_socket: &UdpSocket,
    peer_addr: SocketAddr,
    now: Instant,
    wait: Duration,
) -> bool {
    if wait.is_zero() {
        let _ = peer_rtc.handle_input(Input::Timeout(now));
        return true;
    }
    peer_socket.set_read_timeout(Some(wait)).unwrap();
    let mut buffer = vec![0u8; 2000];
    match peer_socket.recv_from(&mut buffer) {
        Ok((n, source_addr)) => {
            if let Ok(contents) = DatagramRecv::try_from(&buffer[..n]) {
                let receive = Receive {
                    proto: Protocol::Udp,
                    source: source_addr,
                    destination: peer_addr,
                    contents,
                };
                let _ = peer_rtc.handle_input(Input::Receive(Instant::now(), receive));
            }
        }
        Err(_) => {
            let _ = peer_rtc.handle_input(Input::Timeout(Instant::now()));
        }
    }
    false
}
