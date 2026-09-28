//! The video track: negotiation of the H.264 payload type, anchoring of the capture
//! instant on the session's clock origin, and writing access units
//! to str0m.

use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::clock::instant_from_pts;
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};

/// Video source polling cadence: once every 10 ms (100 Hz).
///
/// It is NOT the emission cadence: `VideoSource::next_frame` only returns an
/// access unit if one is ready, and returns `None` otherwise (the common
/// and normal case, see `windows_source`). The real emission cadence is
/// therefore the source's, bounded by this one.
///
/// **Why 100 Hz and not 60 (07/28).** At 60 Hz, capture only retrieved
/// 46 frames/s from a desktop that, for its part, updates at 68.5 Hz — measured
/// directly through `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`
/// (`desktop_updates_hz` in the `SOURCE_TRACE` trace). Each round can only
/// bring back one frame, whatever the number of updates DXGI has
/// merged meanwhile: polling a 68.5 Hz source only 60 times
/// per second mechanically loses part of it. Polling more often than
/// the source produces lifts this bound at no cost when there is nothing
/// to take — `AcquireNextFrame` is called with a ZERO timeout, so an empty
/// round comes down to an immediate DXGI round trip.
///
/// The 60 fps ceiling the milestone aims at stays, for its part, the content's: nothing
/// here fabricates frames that do not exist.
///
/// **Checked on 07/28**: polling 5× faster (2 ms) changes nothing in throughput
/// — `produced_hz` stays at 47.5 for a desktop at 68.5 Hz. The polling
/// cadence was therefore not the limiting factor; it was the
/// `MF_MT_FRAME_RATE` announced to the MFTs (see `demarrage.rs`).
pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(10);

/// Minimal view of a negotiated payload profile, independent of str0m
/// to stay testable without a real RTC session: the fields of
/// `str0m::format::PayloadParams` (including `pt`) are `pub(crate)` on str0m's side,
/// hence impossible to build from this crate for a test.
#[derive(Debug, Clone, Copy)]
struct CandidatePt {
    codec: Codec,
    packetization_mode: Option<u8>,
    pt: Pt,
}

/// Selects the payload type to use to send H.264.
///
/// `enable_h264(true)` negotiates seven profiles (packetization modes 0 and 1,
/// four compatibility profiles): taking the first one in the list, as
/// the initial version did, guarantees nothing about what the
/// encoder will produce. We explicitly filter on packetization mode 1
/// (non-interleaved, RFC 6184 §6.2) — the only one the following tasks
/// will produce. The exact profile (constrained-baseline, etc.) is not
/// discriminated more finely here: that is only checkable with a real
/// browser and a real encoder, not before tasks 8/11.
fn select_h264_pt(candidates: impl Iterator<Item = CandidatePt>) -> Option<Pt> {
    candidates
        .filter(|p| p.codec == Codec::H264 && p.packetization_mode == Some(1))
        .map(|p| p.pt)
        .next()
}

/// Deadline of the next frame, computed from the *previous*
/// deadline rather than from the current instant, so as not to accumulate
/// drift: a slight delay on one frame does not systematically delay
/// all the following ones. Bounded to a catch-up interval: beyond it, we
/// abandon the computation based on `previous` (which would produce a burst
/// of frames to catch up all the delay at once) and restart one
/// interval after `now`.
pub(super) fn next_frame_deadline(previous: Instant, now: Instant, interval: Duration) -> Instant {
    let candidate = previous + interval;
    if now.saturating_duration_since(candidate) > interval {
        now + interval
    } else {
        candidate
    }
}

impl Session {
    /// Branch `b` of the priority list (see `tick`): emits a video
    /// frame if its deadline is reached and the track negotiated.
    ///
    /// Returns `Some(Tick::Continue)` when the deadline was reached — the round
    /// is then concluded, whether a frame was written or not: the attempt
    /// itself is the round's action, and a successful write is a
    /// mutation of `Rtc` that must be followed by the deferred drain of
    /// branch `a0`. Returns `None` when the deadline is not reached or
    /// the track is not negotiated, without having mutated anything.
    pub(super) fn brancher_video(&mut self) -> Option<Tick> {
        let mid = self.video_mid?;
        let now = Instant::now();
        if now < self.next_frame_at {
            return None;
        }
        self.next_frame_at = next_frame_deadline(self.next_frame_at, now, FRAME_INTERVAL);
        match self.source.next_frame() {
            Some(unit) => {
                // `writer.write()` only pushes the frame onto str0m's internal
                // `to_payload` queue — it is
                // `Rtc::handle_input(Input::Timeout(..))` that actually pops it
                // into RTP packets (`do_payload`), never
                // `poll_output()` alone (see str0m's `session.rs`).
                // Calling it HERE would be a second mutation in the same
                // call to `act_on_timeout`, without `poll_output` between the
                // two — exactly the violation this mechanism must
                // avoid (fix round 1). So we set a flag:
                // the NEXT invocation of `act_on_timeout` handles it with
                // absolute priority (branch `a0`). The non-empty payload queue
                // makes `poll_output()` return an immediate deadline,
                // so `run()` calls again at once.
                if self.write_frame(mid, unit) {
                    self.video_write_pending_drain = true;
                }
            }
            None => {
                // Fix round 1: the absence of a new frame is the
                // common and normal case of live capture (still
                // desktop) — not a session end. Only a really
                // exhausted source (window closed, unrecoverable
                // error) justifies it, through `VideoSource::is_exhausted`.
                // `FileSource` never returns `None` and therefore never reaches
                // this path.
                if self.source.is_exhausted() {
                    self.begin_ending("source vidéo épuisée");
                }
            }
        }
        // Child-side cadence counter, the counterpart of the sensor's —
        // see `cadence_video.rs` (extracted from this file, task 8: the addition
        // exceeded this file's 500-line ceiling).
        self.compter_la_cadence_video();
        Some(Tick::Continue)
    }

    /// Selects the H.264 payload type negotiated for `mid`, if there is
    /// one. A call separate from `write_frame` so that the borrow on `self`
    /// through `Rtc::writer` ends before any later `&mut self` call
    /// (the warning log, notably).
    fn select_negotiated_h264_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        select_h264_pt(writer.payload_params().map(|p| CandidatePt {
            codec: p.spec().codec,
            packetization_mode: p.spec().format.packetization_mode,
            pt: p.pt(),
        }))
    }

    /// Real instant at which the frame with timestamp `pts_90k` was captured.
    ///
    /// It is this value that `write_frame` announces to str0m as `wallclock`.
    /// Extracted into a method to be checkable directly: the write
    /// itself requires a negotiated session, the conversion does not.
    fn capture_instant(&self, pts_90k: u64) -> Instant {
        instant_from_pts(self.clock_origin, pts_90k, CLOCK_RATE_HZ as u32)
    }

    /// Writes an access unit on the video track. Mutation issued from
    /// inside `run()`'s loop (see `act_on_timeout`), hence
    /// followed by an immediate return to `poll_output` — compliant with str0m's
    /// drain rule.
    ///
    /// Returns `true` if `writer.write()` was actually called and
    /// succeeded (hence an entry was indeed pushed onto `to_payload` and
    /// requires the deferred drain — see `video_write_pending_drain`),
    /// `false` if the write did not happen (incomplete negotiation,
    /// track unavailable) or failed: in both cases, no entry
    /// was added to `to_payload`, setting the drain flag would be
    /// wrong and would cause a useless `handle_input(Timeout)`.
    pub(super) fn write_frame(&mut self, mid: Mid, unit: AccessUnit) -> bool {
        let Some(pt) = self.select_negotiated_h264_pt(mid) else {
            // I4: incomplete negotiation (no H.264 profile in packetization
            // mode 1) — without this log, the frame is dropped
            // silently, producing a silent black screen indefinitely
            // without the slightest hint in the logs.
            self.warn_negotiation_once(
                "aucun type de charge utile H.264 négocié (mode de paquetisation 1) : images jetées",
            );
            return false;
        };
        // str0m's `wallclock` is "the real world time that corresponds
        // to the MediaTime" — the CAPTURE instant, not the write one.
        // Passing `Instant::now()` here folded the whole capture and hardware
        // encoding delay into the announced correspondence, which stayed
        // invisible as long as video was alone. With an audio track, whose
        // path is much shorter, audio would run ahead of video by all
        // that delay and lip sync would be wrong by construction.
        //
        // The timestamp makes the round trip through Media Foundation without loss
        // (`encode.rs`), so the capture instant is rebuilt exactly
        // from the shared origin. Computed before borrowing `writer`:
        // that one holds `&mut self.rtc`, incompatible with the immutable
        // borrow of `self.clock_origin` that `capture_instant` requires.
        let capture_at = self.capture_instant(unit.pts_90k);
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_negotiation_once("piste vidéo plus accessible en écriture : images jetées");
            return false;
        };
        match writer.write(
            pt,
            capture_at,
            MediaTime::from_90khz(unit.pts_90k),
            unit.data,
        ) {
            Ok(()) => {
                // It is HERE, and only here, that the write really
                // happened — see `compter_la_cadence_video`, which logs
                // this count, never a loop round nor an empty
                // `next_frame`.
                self.unites_video_ecrites += 1;
                true
            }
            Err(e) => {
                // Application write failure (e.g. unknown RID): we close the
                // session rather than propagate the error up to the
                // process. Only `Session::new` and `accept_offer` — before
                // a session really exists — justify killing the
                // whole process.
                tracing::warn!(error = %e, "échec d'écriture de l'image, fin de session");
                self.begin_ending("échec d'écriture vidéo");
                false
            }
        }
    }

    fn warn_negotiation_once(&mut self, message: &str) {
        if !self.warned_negotiation {
            self.warned_negotiation = true;
            tracing::warn!(
                message,
                "négociation vidéo incomplète (avertissement unique)"
            );
        }
    }
}

#[cfg(test)]
mod tests;
