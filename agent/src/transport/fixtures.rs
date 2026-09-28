//! Shared scaffolding for `transport`'s integration tests: the local
//! address, the test video source, and the str0m peer on local loopback that
//! plays the browser's role.
//!
//! `#[cfg(test)]`: none of this is compiled in `release`.
//!
//! Only the items really redeclared by several tests appear here —
//! `DummyAudioSource`, `CountingSource` and `SourceRefusant` each stay
//! with the single test that defines them, they are not shared
//! scaffolding.
//!
//! The `use`s here are explicit rather than a `use super::*`: `transport.rs` now
//! only carries the structure and the loop, and therefore no longer imports
//! by itself everything this scaffolding needs.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use str0m::media::Mid;
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::stats::MediaEgressStats;
use str0m::{Candidate, Input, Rtc};

/// Local address used by all test peers (loopback).
pub(super) fn local_ip() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

/// Opens the test H.264 stream shared by `transport`'s integration
/// tests: `testdata/testsrc.264`, 1280x720 at 60 fps.
pub(super) fn video_test_source() -> crate::source::FileSource {
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("chargement du flux de test")
}

/// Builds the second str0m `Rtc` representing the "browser" peer on
/// local loopback: UDP socket bound and host candidate added. The SDP
/// negotiation (tracks, channels) stays specific to each test, which negotiates
/// different combinations of tracks.
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

/// Minimal emission statistics for track `mid`: only `mid`, `rtt`
/// and `loss` are read by `handle_event`, the rest only has to exist.
///
/// Shared between `adaptation` and `part` (sub-block D6): both modules
/// need it to bring the controller to observe a real estimate.
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

/// Receives a datagram from the peer and passes it to its `Rtc`, respecting
/// `wait` — already bounded by the caller according to its own deadlines (measurement
/// window, hard deadline...). Returns `true` if the caller must resume
/// its loop round immediately (delay already elapsed): it is up to the caller
/// to `continue`, this function cannot do it in its place.
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
