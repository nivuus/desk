//! `impl VideoSource for WindowsSource`: one capture attempt per call, and the
//! short bounded retry kept for the encoder's very first start.

use super::*;

/// Budget granted to waiting for the output of a frame **that has just been
/// submitted** to the encoder, **only as long as it has produced nothing yet**
/// (see `WindowsSource::encoder_warmed_up`) — never to waiting for a
/// new capture, and never either once the encoder is established as
/// able to respond.
///
/// The hardware encoder is asynchronous (see `encode.rs`): after the very
/// first `submit()`, `poll_output()` almost never has a result yet
/// on the first try, the `METransformHaveOutput` event taking one or two
/// cycles to arrive. Without this short retry, the very first frame (hence the
/// first keyframe) was only retrieved at the next round of `Session::run`
/// (~16.7 ms later) at best. Bounded to a few tens of milliseconds:
/// largely enough for this startup, without ever approaching the
/// second that starved `Session::run` (fix round 1).
///
/// **Diagnostic round (throughput capped at ~25-30 fps):** spotted on rereading
/// that the inverse of the value then in force (40 ms) fell exactly on
/// the observed ceiling — hypothesis of an ARTIFICIAL ceiling if this retry
/// blocked `act_on_timeout` (hence all of `Session::run`, capture AND
/// transport) on almost every round in steady state. Measured
/// directly (temporary instrumentation, removed): FALSE on this VM. The
/// retry systematically resolved in 3-5 ms (the 40 ms budget never
/// reached, over hundreds of frames), and reducing the budget from 40 ms to
/// 2 ms (twenty times less) changed strictly nothing in the throughput measured on the
/// browser side (297/10 s in both cases, identical content). At this stage of the
/// diagnosis, the real ceiling was attributed upstream: `DesktopCapture::next_frame`
/// (hence `AcquireNextFrame`, non-blocking) only signalled a new frame at
/// ~30 Hz, whereas the loop polls it at exactly 60 Hz (measured by
/// counting — see also `docs/superpowers/plans/fix-debit-socket-report.md`),
/// which had made the desktop's own composition/duplication cadence
/// suspected as the real limit.
///
/// **Hypothesis ruled out since**, by milestone 1's acceptance run
/// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, criterion 2):
/// measured in isolation (`CAPTURE_TEST`), capture sustains ~90 fps on this
/// same VM — desktop composition/duplication is not the bottleneck. The
/// real ceiling sits on the hardware encoder side: `H264Encoder::submit`, in
/// `encode.rs`, only receives new input requests
/// (`METransformNeedInput`) at ~30 Hz, whereas the same encoder, driven
/// in a tight loop (`ENCODE_TEST`), sustains ~80 fps — an unresolved
/// interaction between the fixed submission rhythm (16.7 ms) and the hardware
/// MFT's own rhythm, not a limit of capture nor, as such, of the
/// NVIDIA GPU/driver (details of the trials that successively rule out the
/// competing hypotheses in
/// `docs/superpowers/plans/diagnostic-plafond-debit.md` and
/// `docs/superpowers/plans/remesure-debit.md`).
///
/// `SUBMIT_POLL_BUDGET` has nothing to do with it — but
/// since it therefore never cost anything except at the very first startup
/// (never rechecked once the encoder is warm), it is now limited to
/// that single case: on hardware where the encoder would respond more slowly in
/// steady state, the old version could really have throttled `run()`
/// up to that budget at each frame — which this restriction eliminates
/// structurally, without changing anything in the throughput measured here.
const SUBMIT_POLL_BUDGET: std::time::Duration = std::time::Duration::from_millis(40);
const SUBMIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1);

impl VideoSource for WindowsSource {
    /// A single capture attempt per call, never waiting for a
    /// new frame — with a short bounded retry, reserved for the encoder's very
    /// first startup, to retrieve its output.
    ///
    /// **Fix round 1 (review), 1st fix:** a first
    /// version of this method retried in a loop (1 ms sleep)
    /// for up to TWO SECONDS before giving up, **whether something had been
    /// captured or not**. Two defects followed from it, both measured by
    /// rereading: (1) it did not distinguish "the encoder is starting" (the
    /// real bug targeted) from "nothing moved on screen" (the nominal case of a
    /// live capture — `DesktopCapture::next_frame` itself documents
    /// this case as "common and normal"); any static page or any
    /// moment without movement for more than two seconds therefore cut the stream;
    /// (2) while it looped, the single thread of `Session::run` no longer
    /// handled ICE, nor RTCP, nor the data channels — observed in
    /// practice by a spontaneous ICE disconnection ~20 s after
    /// negotiation.
    ///
    /// **2nd fix, after measurement:** removing ALL retry (a single
    /// attempt, whatever happens) did fix both defects above,
    /// but strongly degraded the throughput observed on the browser side at the very
    /// start: the external cadence of `Session::run` (~16.7 ms) is too
    /// coarse to catch in time the output of the very first frame
    /// submitted, before the encoder has proven it responds quickly. The
    /// retry therefore reappears, but bounded to `SUBMIT_POLL_BUDGET`.
    ///
    /// **3rd fix, after diagnosing the throughput ceiling (see
    /// `SUBMIT_POLL_BUDGET`):** this retry had ended up applying to
    /// *every* submitted frame, not only the first — without
    /// measured consequence on this VM (it never consumed its budget
    /// in steady state) but remaining a latent risk on slower
    /// hardware, where it would really have throttled `run()` to `1/SUBMIT_POLL_BUDGET`.
    /// Now reserved for the startup phase (`encoder_warmed_up`):
    /// once the encoder is proven able to respond, each submission only
    /// makes a single immediate attempt, exactly like the "nothing
    /// new to capture" case below — an output not yet ready comes out at the
    /// next round, 16.7 ms later, without ever blocking this one.
    ///
    /// When nothing was captured (normal case, still desktop), immediate
    /// return, without loop or wait, as the review requires. `None` therefore never
    /// means "nothing this time"; it stays possible for
    /// two really final causes (`is_exhausted` informs
    /// the caller): the window disappeared, or an unrecoverable capture error
    /// occurred.
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.telemetrie.tick();
        if self.fatal {
            // A rebuild of the chain by `resize` may have failed to the
            // point of leaving no valid backup capture either
            // (see `RebuildOutcome::Fatal` in `resize`): `self.capture`
            // is then `None` for good. NEVER call
            // `capture_mut()` in that case — `is_exhausted()` (already true through
            // `self.fatal`) will make the session close cleanly at the next
            // round, rather than a panic on the empty field.
            return None;
        }

        // Feed the encoder with the most recent frame, if the desktop has
        // changed since the last call (Desktop Duplication only returns a
        // frame on change — common and normal case, see
        // capture.rs).
        let mut submitted = false;
        let region = self.region;
        let t_capture = std::time::Instant::now();
        let captured = self.capture_mut().next_frame(region);
        CAPTURE_NS.fetch_add(
            t_capture.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        match captured {
            Ok(Some(frame)) => {
                self.telemetrie.capturee();
                // Timestamp read from the real clock BEFORE submission:
                // it is the capture instant that dates the frame, not the one
                // when the encoder is willing to accept it (see `next_pts_90k`).
                let pts = self.next_pts_90k();
                let t_submit = std::time::Instant::now();
                let fed = self
                    .encoder_mut()
                    .and_then(|encoder| encoder.submit(&frame, pts));
                SUBMIT_NS.fetch_add(
                    t_submit.elapsed().as_nanos() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
                if let Err(e) = fed {
                    tracing::warn!(erreur = %crate::cause::chaine(&e), "soumission à l'encodeur échouée");
                } else {
                    submitted = true;
                }
            }
            Ok(None) => {}
            Err(e) => {
                // An access loss has already gone through the resumptions of
                // `next_frame`: receiving it here means they were not
                // enough. Legitimate end in both cases.
                tracing::error!(erreur = %e, "capture interrompue, source déclarée épuisée");
                self.fatal = true;
                return None;
            }
        }

        if !submitted || self.encoder_warmed_up {
            // Either nothing new to capture this round (normal case), or
            // the encoder has already proven it responds quickly (see the doc of
            // `SUBMIT_POLL_BUDGET`): in both cases, no wait —
            // but we drain everything ALREADY ready, without ever sleeping.
            return self.drain_ready_output();
        }

        // Encoder not warm yet: its very first output may take
        // a little more than a round to arrive (see the doc of
        // `SUBMIT_POLL_BUDGET`) — we wait for it briefly rather than
        // delaying the very first keyframe.
        let deadline = std::time::Instant::now() + SUBMIT_POLL_BUDGET;
        loop {
            match self.drain_ready_output() {
                Some(unit) => {
                    return Some(unit);
                }
                None => {
                    if std::time::Instant::now() >= deadline {
                        // Not ready yet: it will come out at a later
                        // call. Not a failure, just a latency a little
                        // longer than normal at this very first round.
                        return None;
                    }
                    std::thread::sleep(SUBMIT_POLL_INTERVAL);
                }
            }
        }
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Exhausted for good only if the window disappeared or an
    /// unrecoverable capture error was observed — never for a
    /// mere still desktop (see `next_frame`).
    fn is_exhausted(&self) -> bool {
        self.fatal || !self.is_alive()
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::resize(self, width, height)
    }

    fn is_alive(&self) -> bool {
        WindowsSource::is_alive(self)
    }

    /// Relays to the hardware encoder (see `WindowsSource::request_keyframe`
    /// and the comment of `VideoSource::request_keyframe`).
    fn request_keyframe(&mut self) -> Result<()> {
        WindowsSource::request_keyframe(self)
    }

    fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        // Stored even on failure: it is this bitrate that a later
        // rebuild of the encoder will have to take up.
        self.bitrate = bitrate;
        self.encoder_mut()?.set_bitrate(bitrate)
    }

    fn set_encode_size(&mut self, width: u32, height: u32) -> Result<()> {
        WindowsSource::set_encode_size(self, width, height)
    }
}
