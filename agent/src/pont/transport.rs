//! The bridge's transport: a `PeerConnection` **without media**, carrying a
//! single `files` data channel, reliable and ordered.
//!
//! **Mixed, but without Windows**: the str0m loop and the UDP socket are
//! portable, and this module is therefore entirely tested on the host. It knows
//! **neither ProjFS nor Windows** — it carries opaque, correlated bytes, and
//! nothing else. That is what allows exercising it with a real str0m peer in
//! local loopback, without any virtualisation root.
//!
//! **Why a DEDICATED connection** (decision D4): the video session already
//! carries `control` and `input`, and the signaling relay only accepts one
//! `agent` and one `client` per identifier. Above all, mixing the files channel with
//! the video session would mean a reconnection of one would carry away the other —
//! which principle 4 of the framing forbids.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

/// The label of the bridge's data channel. **It is the browser that creates the
/// channel** (`createDataChannel('files')`); the agent is the responder.
pub const FILES_LABEL: &str = "fichiers";

/// UDP receive buffer size. A bridge frame fits in
/// `MAX_FRAME_SIZE` plus its header, but SCTP fragments: this buffer bounds
/// a datagram, not an application message.
const TAMPON_UDP: usize = 2048;

/// Maximum wait of a loop turn when str0m has no close
/// deadline. Bounds the latency of taking into account a request dropped into
/// `sortant` — without it, a request arriving just after a `recv_timeout`
/// would wait for the next str0m deadline.
const ATTENTE_MAX: Duration = Duration::from_millis(20);

/// What the bridge sends to the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersNavigateur {
    Requete { correlation: u32, trame: Vec<u8> },
}

/// What the bridge receives from the browser, or learns of the channel state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuNavigateur {
    Reponse {
        correlation: u32,
        trame: Vec<u8>,
    },
    CanalOuvert,
    /// The channel is gone: tab closed, page reloaded, WebRTC down.
    /// The caller translates to [`crate::pont::errors::Error::CanalFerme`],
    /// that is, `ERROR_IO_DEVICE` — "the standard I/O error" of the framing.
    CanalFerme,
}

/// Builds the UDP socket and the `Rtc` of a **data-only** endpoint.
///
/// Compared to `transport::initialisation::construire_rtc`, **dropped** are:
/// `enable_h264`, `enable_opus`, `enable_bwe`, `set_stats_interval` and
/// `set_desired_bitrate` — there is no track, hence nothing to estimate or
/// probe. **Kept** are the non-blocking UDP socket (whose reason is measured:
/// `set_read_timeout`'s delay overshoots massively under Windows), the setting of the
/// cryptographic provider, and the host candidate.
///
/// ⚠️ **`clear_codecs()` without any `enable_*` is deliberate, and it works**:
/// exercised by this module's tests, which really negotiate and exchange
/// on a data channel with a str0m peer without any codec being
/// enabled on either side. The browser offers no track; there is therefore nothing
/// to match.
pub fn build_data_rtc(local_ip: IpAddr) -> Result<(UdpSocket, Rtc)> {
    let socket =
        UdpSocket::bind(SocketAddr::new(local_ip, 0)).context("opening the bridge UDP socket")?;
    socket
        .set_nonblocking(true)
        .context("switching the bridge UDP socket to non-blocking")?;
    let addr = socket.local_addr()?;
    tracing::info!(%addr, "file bridge UDP socket");

    // Idempotent: `OnceLock::set` silently ignores a second call. The
    // bridge is a separate process, so in practice it is the first — but
    // this module's tests build several.
    str0m::crypto::from_feature_flags().install_process_default();

    let mut rtc = Rtc::builder().clear_codecs().build(Instant::now());
    rtc.add_local_candidate(
        Candidate::host(addr, "udp").map_err(|e| anyhow!("invalid host candidate: {e}"))?,
    );
    Ok((socket, rtc))
}

/// The transport loop. Returns `Ok(())` when the connection ends or the
/// caller drops `sortant`.
///
/// `sortant` carries the requests to emit, `entrant` returns the responses and the
/// channel state changes.
pub fn tourner(
    mut rtc: Rtc,
    socket: UdpSocket,
    sortant: Receiver<VersNavigateur>,
    entrant: Sender<DuNavigateur>,
) -> Result<()> {
    let adresse = socket
        .local_addr()
        .context("local address of the bridge socket")?;
    let mut canal: Option<ChannelId> = None;
    let mut tampon = vec![0u8; TAMPON_UDP];

    loop {
        if !rtc.is_alive() {
            // The channel goes with the connection: saying so explicitly, otherwise
            // commands in flight would wait for a delay rather than an error.
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Ok(());
        }

        // Drain `poll_output` until `Output::Timeout`: same invariant as
        // the video loop (`transport.rs`), and for the same reason — every
        // mutation of `Rtc` must be followed by a complete drain.
        let echeance = loop {
            match rtc
                .poll_output()
                .map_err(|e| anyhow!("bridge poll_output: {e}"))?
            {
                Output::Timeout(t) => break t,
                Output::Transmit(t) => {
                    // A failing write is not fatal: str0m
                    // will retransmit. Logging it per datagram would be —
                    // "never trace per packet in the transport
                    // loop" is a lesson this repository paid for with a
                    // whole session (18,619 lines in a few seconds,
                    // written to a CIFS share).
                    let _ = socket.send_to(&t.contents, t.destination);
                }
                Output::Event(evenement) => {
                    if let Some(fin) = traiter(evenement, &mut canal, &entrant) {
                        return fin;
                    }
                }
            }
        };

        let maintenant = Instant::now();
        let attente = echeance
            .saturating_duration_since(maintenant)
            .min(ATTENTE_MAX);

        // A request to emit? We wait at most until the str0m deadline.
        match sortant.recv_timeout(attente) {
            Ok(VersNavigateur::Requete { correlation, trame }) => {
                emettre(&mut rtc, canal, correlation, &trame);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("nobody emits requests any more: stopping the bridge transport");
                return Ok(());
            }
        }

        // Then the socket, without blocking (it is non-blocking), and finally the
        // passing time.
        match socket.recv_from(&mut tampon) {
            Ok((size, source)) => {
                let recu = Receive::new(Protocol::Udp, source, adresse, &tampon[..size])
                    .map_err(|e| anyhow!("unreadable datagram: {e}"))?;
                rtc.handle_input(Input::Receive(Instant::now(), recu))
                    .map_err(|e| anyhow!("bridge handle_input: {e}"))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                rtc.handle_input(Input::Timeout(Instant::now()))
                    .map_err(|e| anyhow!("bridge handle_input(Timeout): {e}"))?;
            }
            Err(e) => return Err(e).context("reading the bridge UDP socket"),
        }
    }
}

/// Handles a str0m event. Returns `Some(..)` when the loop must stop.
fn traiter(
    evenement: Event,
    canal: &mut Option<ChannelId>,
    entrant: &Sender<DuNavigateur>,
) -> Option<Result<()>> {
    match evenement {
        Event::Connected => {
            tracing::info!("file bridge connected to the browser");
        }
        // 🔴 **Defect found by the closing test, not by
        // review.** Without these two arms, the loop only noticed the peer's
        // departure at ICE expiry — that is, tens of seconds after
        // the tab closed, during which any command in flight
        // would have waited for its DELAY instead of returning `ERROR_IO_DEVICE` right
        // away. It is exactly fix I1 of `transport/evenements.rs`,
        // which had had to be made there for the same reason; replaying it here
        // would have been the fifth time this repository paid for an event fallen
        // into a catch-all arm.
        Event::Closed => {
            tracing::info!("bridge connection closed by the peer (DTLS close_notify)");
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Some(Ok(()));
        }
        Event::IceConnectionStateChange(str0m::IceConnectionState::Disconnected) => {
            tracing::warn!("ICE disconnected on the file bridge");
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Some(Ok(()));
        }
        Event::ChannelOpen(id, label) => {
            // ⚠️ **ROUTING IS BY LABEL, FROM DAY ONE.**
            //
            // ❌ *This comment described IN THE PRESENT TENSE a defect of
            // `transport/evenements.rs::dispatch_channel_data` — "routes
            // on `data.binary` alone and ignores `data.id`". Task 17 of
            // the SAME branch fixed it: it now calls
            // `destination(data.id, …)`, so it looks at the channel.* The reason
            // for this block does not change: keeping the id of the expected label, and
            // refusing
            // everything else, costs three lines now and a whole acceptance
            // run later.
            if label == FILES_LABEL {
                tracing::info!(%label, "file bridge channel open");
                *canal = Some(id);
                let _ = entrant.send(DuNavigateur::CanalOuvert);
            } else {
                tracing::warn!(
                    %label, ?id,
                    "data channel ignored: the bridge only serves the expected label"
                );
            }
        }
        Event::ChannelClose(id) => {
            if *canal == Some(id) {
                tracing::info!(?id, "file bridge channel closed");
                *canal = None;
                let _ = entrant.send(DuNavigateur::CanalFerme);
            }
        }
        Event::ChannelData(data) => {
            if *canal != Some(data.id) {
                // Naming the id: without it, this `warn!` does not allow saying
                // WHICH channel spoke, and the trace would be unusable in
                // an acceptance run. "An unattributable trace costs a
                // re-attribution" (D6).
                tracing::warn!(
                    id = ?data.id, attendu = ?canal, octets = data.data.len(),
                    "data received on a channel that is not the bridge's: ignored"
                );
                return None;
            }
            match proto::files::decoder(&data.data) {
                Ok(trame) => {
                    let correlation = trame.correlation;
                    if entrant
                        .send(DuNavigateur::Reponse {
                            correlation,
                            trame: data.data.to_vec(),
                        })
                        .is_err()
                    {
                        tracing::info!("nobody reads the answers any more: stopping the transport");
                        return Some(Ok(()));
                    }
                }
                // An unreadable frame is THROWN AWAY, never guessed: its
                // correlation is precisely what cannot be read, so
                // nothing would allow linking it to a command.
                Err(error) => tracing::warn!(%error, "unreadable browser frame, dropped"),
            }
        }
        _ => {}
    }
    None
}

/// Writes a request on the channel, if the channel exists.
fn emettre(rtc: &mut Rtc, canal: Option<ChannelId>, correlation: u32, trame: &[u8]) {
    let Some(id) = canal else {
        // It is not a programming anomaly: the channel can go down
        // between a command's registration and its emission. The caller
        // will learn it through its table's expiry — it is what the table
        // exists to cover.
        tracing::warn!(correlation, "request not emitted: no bridge channel open");
        return;
    };
    let Some(mut sortie) = rtc.channel(id) else {
        tracing::warn!(
            correlation,
            ?id,
            "request not emitted: channel not found on the str0m side"
        );
        return;
    };
    // ⚠️ `binary = true`, **unlike the `control` channel** which writes `false`:
    // a files frame's payload is made of raw bytes, and writing it
    // in text mode would put it through UTF-8 validation on the browser side.
    if let Err(error) = sortie.write(true, trame) {
        tracing::warn!(%error, correlation, "writing a bridge request failed");
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_fermeture;
