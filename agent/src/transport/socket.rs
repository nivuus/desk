//! The socket and waiting: classification of receive errors, exponential
//! backoff, Windows timer resolution, and computation of the bounded wait
//! between two loop rounds.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::Input;

use super::tick::Tick;
use super::Session;

/// Maximum slice of a wait without data on the socket, in the
/// non-blocking polling loop of `act_on_timeout` (branch c).
///
/// Replaces `UdpSocket::set_read_timeout`, whose delay overshoots massively
/// under Windows (independent measurement: mean overshoot +12.7 ms, up to
/// +37 ms; a requested delay of 617 µs was honoured after 31,758 µs — five
/// times the budget of a whole frame at 60 Hz). `recv_from` thus consumed
/// up to ~90% of the loop time available at each round, time that
/// should have gone back to capture and encoding.
///
/// **Comparison figure withdrawn (07/28):** this measurement originally cited
/// an isolated capture/encoding ceiling of 47-51 fps against an observed
/// end-to-end throughput of 23-25 fps. Milestone 1's acceptance run
/// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, "What was
/// learned" section) establishes that this 47-51 fps figure came from an isolated
/// harness (`ENCODER_THROUGHPUT_TEST`), which does not go through `Session::run` and
/// is therefore not comparable with an end-to-end measurement — and that it was
/// stale anyway: capture, remeasured since in isolation
/// (`CAPTURE_TEST`), sustains ~90 fps on the same VM. The throughput ceiling
/// really established by the acceptance run sits on the hardware encoder side
/// (`METransformNeedInput` is only accepted at ~30 Hz, see
/// `windows_source.rs` and `encode.rs`), with no demonstrated link to the imprecision
/// of `recv_from` documented above, which stays a valid measurement in itself.
///
/// A non-blocking socket polled in a loop without ever sleeping would consume a
/// whole processor core for nothing — unacceptable for an agent meant
/// to run in the background. Conversely, a single `sleep` covering the whole
/// wait would reproduce the measured imprecision (the defect is not specific to
/// `recv_from`: it is the granularity of the underlying Windows timer). The
/// compromise chosen rechecks the socket at short, fixed intervals: the
/// oversleep of a given wake-up, if it happens, stays bounded to this interval
/// rather than to the total wait duration. 1 ms is much finer than
/// the frame interval (16.67 ms) while letting the thread sleep most
/// of the time.
pub(super) const RECV_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Outcome of classifying a UDP receive error (see
/// `classify_recv_error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecvErrorAction {
    /// Known transient error, which indicates no lasting corruption of the
    /// socket: we keep receiving, after a backoff (see
    /// `recv_error_backoff`) so as not to turn a burst of such
    /// errors into a tight loop.
    RetryWithBackoff,
    /// Error with no reason to resolve by itself (permissions,
    /// socket in an invalid state, network interface gone...):
    /// continuing to loop on it would only mask a real problem
    /// without ever solving it. We close the session cleanly rather than
    /// logging indefinitely.
    Fatal,
}

/// Classifies a `UdpSocket::recv_from` error (other than `WouldBlock`/`TimedOut`,
/// already handled separately as normal deadlines) according to whether it
/// justifies a new attempt or the end of the session.
///
/// The motivating case: under Windows, the target platform, an unconnected
/// UDP socket receives `WSAECONNRESET` when an ICMP "port unreachable" message
/// comes back — typically after the browser tab is closed abruptly,
/// before ICE has had time to detect the disconnection.
/// `std::io::ErrorKind::ConnectionReset` is the portable variant to
/// which Rust normalises `WSAECONNRESET` (see `std::io::Error::kind`):
/// we test this cross-platform name, never a Windows-specific error
/// value, so that this file stays independent of the build
/// platform. On Linux, with an unconnected socket like this one, this
/// variant is in practice never produced for this scenario — the test
/// therefore covers the classification itself, not a behaviour observable
/// only under Windows. `Interrupted` (signal received during the blocking
/// call) follows the same logic: retrying is the standard behaviour
/// documented by `std::io::Error`.
///
/// Any other error (permissions, closed socket, invalid argument...) is
/// classified fatal: nothing indicates it will resolve by itself, and
/// looping on it endlessly would mask a real problem rather than
/// report it.
pub(super) fn classify_recv_error(kind: std::io::ErrorKind) -> RecvErrorAction {
    use std::io::ErrorKind::{ConnectionReset, Interrupted};
    match kind {
        ConnectionReset | Interrupted => RecvErrorAction::RetryWithBackoff,
        _ => RecvErrorAction::Fatal,
    }
}

/// Backoff applied after `consecutive_errors` transient UDP receive
/// errors in a row: bounded exponential backoff (1 ms, 2 ms, 4
/// ms, ... up to `RECV_ERROR_BACKOFF_MAX`).
///
/// Without this bound, a burst of `WSAECONNRESET` (one ICMP "port
/// unreachable" per packet sent back while ICE has not yet detected the
/// disconnection, which takes several seconds) would spin in a tight loop
/// — `recv_from` returning the error immediately, without ever waiting for the
/// requested read delay — logging at each round and consuming a
/// processor core until ICE detection. The ceiling is chosen low
/// enough not to noticeably delay receiving a legitimate packet
/// that would arrive meanwhile (nor ICE detection itself, which does not depend
/// on this loop but on `Rtc`'s deadlines).
const RECV_ERROR_BACKOFF_BASE: Duration = Duration::from_millis(1);
const RECV_ERROR_BACKOFF_MAX: Duration = Duration::from_millis(200);

pub(super) fn recv_error_backoff(consecutive_errors: u32) -> Duration {
    // `1u32 << exponent` would overflow beyond 31: bound the exponent before
    // the shift, rather than relying on `saturating_mul` alone, which
    // operates on `Duration`s (no arithmetic overflow there), but whose
    // operand `2^exponent` would already have silently overflowed as a `u32`
    // before being passed to it.
    let exponent = consecutive_errors.min(31);
    RECV_ERROR_BACKOFF_BASE
        .saturating_mul(1u32 << exponent)
        .min(RECV_ERROR_BACKOFF_MAX)
}

// `timeBeginPeriod`/`timeEndPeriod` (winmm.dll) are declared by hand:
// the `windows` 0.62 crate (even with the
// `Win32_Media_Multimedia` feature enabled) does not generate them — checked by
// exhaustive search in the crate's vendored sources, no
// occurrence of `timeBeginPeriod`/`BeginPeriod`. The API is stable and
// documented by Microsoft since Windows XP; declaring it directly avoids
// depending on a missing feature. (`//`, not `///`: rustdoc does not
// document `extern` blocks, and such a comment would attach
// anyway to the next item rather than to this one.)
#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn timeBeginPeriod(uperiod: u32) -> u32;
    fn timeEndPeriod(uperiod: u32) -> u32;
}

/// RAII guard pairing `timeBeginPeriod`/`timeEndPeriod` (winmm) for the
/// lifetime of a `Session`.
///
/// Without this call, `std::thread::sleep` under Windows inherits the system
/// timer's default resolution — typically 15.6 ms as long as no
/// process has asked for better. Measured experimentally on this agent:
/// `RECV_POLL_INTERVAL` (1 ms) without this guard barely reduced
/// throughput (~24 fps, against ~23 fps before any fix) — the polling
/// loop inherited the same granularity defect as the one measured on
/// `recv_from`, just moved to `sleep`. With the resolution brought down to
/// 1 ms, `sleep` does honour waits of the order of a
/// millisecond. `timeBeginPeriod`/`timeEndPeriod` must be paired
/// (Microsoft documentation): this guard does it even on early
/// return (`?`) or panic, never through a code path that could
/// be skipped.
///
/// Only exists under Windows: under Linux (used by the tests), the SDK
/// does not expose `timeBeginPeriod` and the measured defect does not apply.
#[cfg(windows)]
pub(super) struct TimerResolutionGuard;

#[cfg(windows)]
impl TimerResolutionGuard {
    pub(super) fn new() -> Self {
        // Return ignored: `TIMERR_NOERROR` (success) or `TIMERR_NOCANDO` (already
        // at maximum resolution, or out of bounds) — in both cases, nothing
        // actionable to do here; a silent failure would at worst degrade
        // to the previous behaviour (default resolution), never to
        // a functional error.
        unsafe {
            timeBeginPeriod(1);
        }
        Self
    }
}

#[cfg(windows)]
impl Drop for TimerResolutionGuard {
    fn drop(&mut self) {
        unsafe {
            timeEndPeriod(1);
        }
    }
}

/// Under Linux (tests), no equivalent to call: the measured granularity
/// is a defect specific to the Windows timer.
#[cfg(not(windows))]
pub(super) struct TimerResolutionGuard;

#[cfg(not(windows))]
impl TimerResolutionGuard {
    pub(super) fn new() -> Self {
        Self
    }
}

/// Duration to wait before the next wake-up, bounded by the nearer of
/// two deadlines: the one `Rtc` asks for (`rtc_deadline`) and, if a video
/// track is negotiated and the session is not closing,
/// `next_frame_at`.
///
/// This is where the cadence failure lodged (C1 of the review): without a
/// bound on the frame deadline, the wait lasted until the deadline
/// `Rtc` asks for — up to a whole second, imposed by the RTCP report
/// or statistics interval, as soon as nothing else was due. The
/// sending rhythm was then dictated by str0m's wake-ups, not by
/// `FRAME_INTERVAL`.
pub(super) fn bounded_wait(
    now: Instant,
    rtc_deadline: Instant,
    next_frame_at: Option<Instant>,
    cap: Option<Duration>,
) -> Duration {
    let mut wait = rtc_deadline.saturating_duration_since(now);
    if let Some(next_frame_at) = next_frame_at {
        wait = wait.min(next_frame_at.saturating_duration_since(now));
    }
    // Audio ceiling: packets arrive from ANOTHER thread, with no deadline
    // this loop can predict. Only a regular wake-up lets us
    // pick them up in time. A ceiling only SHORTENS the wait, never
    // lengthens it.
    if let Some(cap) = cap {
        wait = wait.min(cap);
    }
    wait
}

impl Session {
    /// Branch `c` of the priority list (see `tick`): nothing to emit this
    /// round, we wait for an incoming packet.
    ///
    /// Always conclusive — it ends on exactly one mutation of
    /// `Rtc` (`handle_input`, receive or deadline) or on a backoff
    /// after a receive error, and gives control back in all cases.
    pub(super) fn brancher_attente(&mut self, deadline: Instant) -> Result<Tick> {
        // c) Nothing to emit this round: wait for an incoming packet, bounded
        //    both by `Rtc`'s deadline and by the next frame
        //    deadline (see `bounded_wait` — it is the fix for C1).
        let now = Instant::now();
        let next_frame_at =
            (self.video_mid.is_some() && !self.ending).then_some(self.next_frame_at);
        let mut wait = bounded_wait(now, deadline, next_frame_at, self.audio_wait_cap());
        // Never sleep beyond the TURN lease refresh. The media cadence
        // today bounds the wait well below the 300 s of a
        // half-lease, but depending on it would mean relying on luck: a
        // session without a negotiated video track would wait for `Rtc`'s deadline.
        if let Some(echeance_turn) = self.turn.as_ref().and_then(|t| t.poll_timeout()) {
            wait = wait.min(echeance_turn.saturating_duration_since(now));
        }

        if wait.is_zero() {
            self.rtc
                .handle_input(Input::Timeout(now))
                .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
            return Ok(Tick::Continue);
        }

        // Polls the socket (non-blocking since `Session::new`) in small
        // slices rather than entrusting the wait to `set_read_timeout`:
        // it is the fix for the measured defect (see the comment of
        // `RECV_POLL_INTERVAL`). No mutation of `Rtc` happens as long
        // as this loop has neither received a datagram nor reached
        // `poll_deadline` — a single mutation comes out of it, as the
        // method's docstring requires.
        let poll_deadline = now + wait;
        let mut buffer = vec![0u8; 2000];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((n, source_addr)) => {
                    // A loop round without a receive error ends
                    // any burst: the next error, if there is
                    // one, starts again from a minimal backoff rather than
                    // continuing the growth begun by a past
                    // burst.
                    self.consecutive_recv_errors = 0;

                    // Packet from the TURN server: it does not go through the
                    // ordinary path. `traiter_paquet_turn` makes it either a
                    // service message for the state machine, or a
                    // relayed payload presented to str0m as coming from the peer.
                    if self
                        .turn
                        .as_ref()
                        .is_some_and(|t| t.serveur() == source_addr)
                    {
                        self.traiter_paquet_turn(&buffer[..n])?;
                        return Ok(Tick::Continue);
                    }

                    let destination = self.socket.local_addr()?;
                    // I2: a datagram that is neither STUN, nor DTLS, nor
                    // RTP/RTCP (network noise, port probe, empty packet)
                    // makes this conversion fail. It must not bring
                    // the agent down — only be ignored.
                    match DatagramRecv::try_from(&buffer[..n]) {
                        Ok(contents) => {
                            let receive = Receive {
                                proto: Protocol::Udp,
                                source: source_addr,
                                destination,
                                contents,
                            };
                            self.rtc
                                .handle_input(Input::Receive(Instant::now(), receive))
                                .map_err(|e| anyhow!("handle_input receive : {e}"))?;
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "UDP packet ignored (unrecognised)");
                        }
                    }
                    return Ok(Tick::Continue);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available for now: the common
                    // case. Does not affect the burst counter (it is
                    // not an error).
                    self.consecutive_recv_errors = 0;
                    let remaining = poll_deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        // Deadline reached without data: give control back to
                        // `Rtc` through a timeout, exactly as the
                        // old blocking `recv_from` did on expiry of
                        // `set_read_timeout`.
                        self.rtc
                            .handle_input(Input::Timeout(Instant::now()))
                            .map_err(|e| anyhow!("handle_input timeout : {e}"))?;
                        return Ok(Tick::Continue);
                    }
                    // Neither a tight spin (would consume a whole core), nor a single
                    // sleep over the whole duration (would reproduce the measured
                    // imprecision): we sleep in small slices bounded by
                    // `RECV_POLL_INTERVAL`, rechecking the socket at
                    // each wake-up. The possible oversleep of a single call
                    // to `sleep` (same granularity defect as the one measured
                    // on `recv_from`) stays bounded to one polling
                    // interval, never to the whole of `wait`.
                    std::thread::sleep(remaining.min(RECV_POLL_INTERVAL));
                }
                Err(e) => match classify_recv_error(e.kind()) {
                    // I2: transient receive error — logged,
                    // not fatal. A burst would loop idle without this
                    // growing backoff (see `recv_error_backoff`) —
                    // the socket itself is not at fault, only the
                    // rhythm of new attempts is.
                    RecvErrorAction::RetryWithBackoff => {
                        self.consecutive_recv_errors =
                            self.consecutive_recv_errors.saturating_add(1);
                        let backoff = recv_error_backoff(self.consecutive_recv_errors);
                        tracing::warn!(
                            error = %e,
                            consecutives = self.consecutive_recv_errors,
                            backoff_ms = backoff.as_millis(),
                            "transient UDP receive failure, ignored"
                        );
                        std::thread::sleep(backoff);
                        return Ok(Tick::Continue);
                    }
                    // Error with no reason to resolve
                    // by itself: clean session close (like I5
                    // for an exhausted source), not an endless loop nor a
                    // process panic — only `Session::new`/
                    // `accept_offer` justify killing the whole process
                    // (see the module comment).
                    RecvErrorAction::Fatal => {
                        tracing::warn!(
                            error = %e,
                            "non-transient UDP receive failure, end of session"
                        );
                        self.begin_ending("non-transient UDP receive failure");
                        return Ok(Tick::Continue);
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- classify_recv_error / recv_error_backoff -------------------------
    //
    // No real socket here: provoking a deterministic WSAECONNRESET
    // would require a real Windows machine and a peer closing its
    // connection at the right moment, which the review explicitly rules out as
    // not reliably testable. So we test the pure logic of
    // classification and backoff, independently of any I/O.

    #[test]
    fn connection_reset_is_transient() {
        // The motivating case (I2 extended): `ConnectionReset` is the portable
        // variant to which Rust normalises `WSAECONNRESET`, received on
        // a Windows UDP socket after an ICMP "port unreachable".
        assert_eq!(
            classify_recv_error(std::io::ErrorKind::ConnectionReset),
            RecvErrorAction::RetryWithBackoff
        );
    }

    #[test]
    fn interrupted_is_transient() {
        assert_eq!(
            classify_recv_error(std::io::ErrorKind::Interrupted),
            RecvErrorAction::RetryWithBackoff
        );
    }

    #[test]
    fn unrecognised_errors_are_fatal() {
        // A representative selection of errors with no reason to
        // resolve by themselves: no exhaustive list needed,
        // only proof that the default classification is indeed fatal
        // (not transient), not the inverse of an open list of
        // exceptions.
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::NotConnected,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::Unsupported,
            std::io::ErrorKind::Other,
        ] {
            assert_eq!(
                classify_recv_error(kind),
                RecvErrorAction::Fatal,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn backoff_grows_with_the_number_of_consecutive_errors() {
        let un = recv_error_backoff(1);
        let deux = recv_error_backoff(2);
        let trois = recv_error_backoff(3);
        assert!(un < deux, "{un:?} should be < {deux:?}");
        assert!(deux < trois, "{deux:?} should be < {trois:?}");
    }

    #[test]
    fn backoff_stays_bounded_even_after_a_very_long_burst() {
        // Direct proof of the targeted defect: without a bound, a burst
        // of consecutive errors would make the delay grow without limit (or
        // would overflow the arithmetic). Here, even after a number of errors
        // that would overflow `1u32 << n` as a `u32` without the bound on
        // the exponent, the result stays finite and capped.
        assert_eq!(recv_error_backoff(1_000_000), RECV_ERROR_BACKOFF_MAX);
        assert!(recv_error_backoff(50) <= RECV_ERROR_BACKOFF_MAX);
    }

    #[test]
    fn backoff_is_non_zero_from_the_first_error() {
        // A single error is already enough to introduce a backoff: no
        // "free first hit" that would let one tight loop round
        // through before the mechanism engages.
        assert!(recv_error_backoff(1) > Duration::ZERO);
    }

    #[test]
    fn wait_bounded_by_the_nearest_frame_deadline() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_secs(1);
        let next_frame_at = now + Duration::from_micros(5_000);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at), None);
        assert_eq!(wait, Duration::from_micros(5_000));
    }

    #[test]
    fn wait_bounded_by_the_rtc_deadline_if_nearer() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_micros(2_000);
        let next_frame_at = now + Duration::from_secs(1);
        let wait = bounded_wait(now, rtc_deadline, Some(next_frame_at), None);
        assert_eq!(wait, Duration::from_micros(2_000));
    }

    #[test]
    fn wait_dictated_by_rtc_alone_without_a_video_track() {
        let now = Instant::now();
        let rtc_deadline = now + Duration::from_millis(10);
        assert_eq!(
            bounded_wait(now, rtc_deadline, None, None),
            Duration::from_millis(10)
        );
    }
}
