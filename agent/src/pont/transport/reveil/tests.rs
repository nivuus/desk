//! Host tests of the bridge's socket reader, through [`super::read_from`]:
//! a scripted `recv_from` hands it the errors a Windows socket returns.

use std::collections::VecDeque;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::sync_channel;
use std::sync::Arc;

use super::{read_from, Reveil};

/// Runs the reader over `script`, then reports what it emitted. When the
/// script runs dry, the fake socket fails with a FATAL error, so the reader
/// always stops on its own.
fn read_script(script: Vec<std::io::Result<Vec<u8>>>) -> Vec<Reveil> {
    let source: SocketAddr = "127.0.0.1:9".parse().unwrap();
    let mut script: VecDeque<_> = script.into();
    let (sender, received) = sync_channel(64);
    read_from(
        |buffer| match script.pop_front() {
            Some(Ok(bytes)) => {
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok((bytes.len(), source))
            }
            Some(Err(e)) => Err(e),
            None => Err(Error::new(ErrorKind::PermissionDenied, "end of script")),
        },
        sender,
        Arc::new(AtomicBool::new(false)),
    );
    received.try_iter().collect()
}

#[test]
fn a_connection_reset_does_not_stop_the_reader() {
    // On Windows, an ICMP "port unreachable" (a tab closed abruptly) makes
    // the next `recv_from` of an unconnected UDP socket fail with
    // `WSAECONNRESET`, which Rust maps to `ConnectionReset`. The socket is
    // still sound: the datagram that follows must reach the loop.
    let wakeups = read_script(vec![
        Err(Error::from(ErrorKind::ConnectionReset)),
        Ok(vec![0x16, 0xFE, 0xFD]),
    ]);

    let datagrams = wakeups
        .iter()
        .filter(|r| matches!(r, Reveil::Datagram { .. }))
        .count();
    assert_eq!(
        datagrams, 1,
        "the datagram after the reset must be delivered (emitted: {wakeups:?})"
    );
    assert!(
        matches!(wakeups.first(), Some(Reveil::Datagram { .. })),
        "the reset itself must not be reported as a socket failure (emitted: {wakeups:?})"
    );
}

#[test]
fn a_fatal_error_still_stops_the_reader() {
    // The negative control: classifying `ConnectionReset` as transient must
    // not turn EVERY error into a retry.
    let wakeups = read_script(vec![Err(Error::from(ErrorKind::PermissionDenied))]);
    assert!(
        matches!(wakeups.as_slice(), [Reveil::Socket(e)] if e.kind() == ErrorKind::PermissionDenied),
        "a fatal error must surface, once (emitted: {wakeups:?})"
    );
}
