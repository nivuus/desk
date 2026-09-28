//! Building the UDP socket and the str0m `Rtc` in their initial state,
//! before any SDP negotiation.
//!
//! **Extracted from `Session::new` (`transport.rs`)**: this code is entirely
//! self-contained — it reads and writes no field of `Session`, only
//! `local_ip` and `plafond_bps` — and its extraction is what gave back to
//! `transport.rs` the margin that the review of task 12 (sub-block D10, the
//! rebuild budget remedy) had taken from it: the file had
//! gone to 501 lines, one more than the project's 500 ceiling.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::bwe::Bitrate;
use str0m::{Candidate, Rtc};

use super::adaptation::ESTIMATION_INITIALE_BPS;

/// Opens the agent's UDP socket and builds the matching `Rtc`,
/// H.264/Opus codecs enabled, initial bandwidth estimate set,
/// and the only local (host) candidate added.
///
/// `local_ip` is the address through which the browser will reach the agent;
/// `plafond_bps` is the target the bandwidth probing seeks to
/// reach.
pub(super) fn construire_rtc(local_ip: IpAddr, plafond_bps: u32) -> Result<(UdpSocket, Rtc)> {
    let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).context("opening the UDP socket")?;
    // Non-blocking once and for all: `act_on_timeout` no longer depends on
    // `set_read_timeout`, whose delay overshoots massively under Windows
    // (measured: mean overshoot +12.7 ms, up to +37 ms on a requested
    // delay of 617 µs — see `poll_recv_or_timeout`). The waiting rhythm
    // is now entirely driven by our own polling loop,
    // independent of the socket timer's precision.
    socket
        .set_nonblocking(true)
        .context("passage du socket UDP en non bloquant")?;
    let addr = socket.local_addr()?;
    tracing::info!(%addr, "socket UDP de l'agent");

    // str0m 0.21 requires a cryptographic provider installed for the
    // process (checked in the crate's sources: the default Cargo feature
    // `aws-lc-rs` provides `from_feature_flags()`, and
    // `install_process_default(self)` is a consuming method on
    // `CryptoProvider`). Idempotent: `OnceLock::set` silently ignores
    // a second call, so calling `Session::new` several times per
    // process does not panic.
    str0m::crypto::from_feature_flags().install_process_default();

    // `enable_opus(true)`: without this line, no Opus payload type
    // is ever offered in the SDP answer, whatever the peer
    // negotiates on its side — `select_negotiated_opus_pt` would then never
    // find anything, and audio would stay silent even with a source opened
    // successfully. Absent from the original brief, added here: without it, the
    // audio track simply never negotiates (see the task
    // report).
    let mut rtc = Rtc::builder()
        .clear_codecs()
        .enable_h264(true)
        .enable_opus(true)
        // Without this call, `Event::EgressBitrateEstimate` is NEVER emitted and
        // the whole feedback control stays silent. The initial estimate is
        // deliberately modest: the subsystem probes upwards towards
        // `set_desired_bitrate` (set below), and starting too high would
        // saturate the link before the first correction.
        .enable_bwe(Some(Bitrate::bps(ESTIMATION_INITIALE_BPS as u64)))
        .set_stats_interval(Some(Duration::from_secs(1)))
        // Depth of the audio receive REORDERING buffer, brought down
        // from 15 (str0m's default, `config.rs`) to 2.
        //
        // This buffer adds NO latency in nominal operation — a contiguous
        // sequence goes out immediately (`packet/buffer_rx.rs`,
        // `wait_for_contiguity = !contiguous_seq && !more_than_hold_back`).
        // But ON A GAP it holds up to `reordering_size_audio`
        // segments, and at 20 ms per packet — Chrome's frame duration —
        // that makes up to **300 ms of retention**, which:
        //
        //   1. blow the 100 ms budget the microphone allows itself in total;
        //   2. **cancel in-band FEC**, whose whole mechanism is to
        //      rebuild a lost frame from the NEXT one — which
        //      str0m would then only deliver 300 ms later.
        //
        // Accepted cost: a burst of three or more consecutive losses is
        // delivered as a gap rather than waited for. That is INTENDED — the
        // decoder's PLC covers the gap, and 300 ms of awaited silence would be worse
        // than 40 ms of concealment.
        //
        // ⚠️ No effect on existing behaviour: it is a RECEIVE setting, and
        // before workstream E the agent received no media (the video as well as
        // workstream A's audio go from the agent to the browser).
        .set_reordering_size_audio(2)
        .build(Instant::now());

    // Target the probing seeks to reach: the configured ceiling.
    rtc.bwe()
        .set_desired_bitrate(Bitrate::bps(plafond_bps as u64));

    // `add_local_candidate` does not return a `Result`: it returns
    // `Option<&Candidate>` (the previous candidate if it was already known).
    // Only building the `Candidate` itself can fail.
    rtc.add_local_candidate(
        Candidate::host(addr, "udp").map_err(|e| anyhow!("invalid host candidate: {e}"))?,
    );

    Ok((socket, rtc))
}
