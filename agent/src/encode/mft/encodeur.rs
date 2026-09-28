//! The **encoder** half of the pump: the *asynchronous* H.264 MFT, driven
//! by events (`METransformNeedInput` / `METransformHaveOutput`).
//!
//! Extracted from `encode.rs` with the rest of the MFT path (batch 31). Its counterpart is
//! `super::convertisseur`, whose `feed_converter` and
//! `take_output_sample` it calls — only those two, and that is why they are the only two
//! to be `pub(super)` there.

use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};

// ⚠️ **Glob import, and it is a motivated choice.** This file is the
// CONTINUATION of `encode.rs`: phases, public counters and tuning
// constants live there, and listing them here would create a second list to
// keep up to date — the one that gets out of sync. The repository's rule targets
// copied assertions; a glob import copies none.
use crate::encode::*;
// The MFT back end's type comes from the PARENT, not from the glob: `crate::encode`
// exports the FACADE, which is not what we implement here.
use super::EncodeurMft;
// The only two elements this file borrows from its counterpart.
use super::convertisseur::take_output_sample;
impl EncodeurMft {
    /// Drains the available events without blocking.
    fn drain_events(&mut self) -> Result<()> {
        loop {
            // MF_EVENT_FLAG_NO_WAIT: returns immediately if there is nothing.
            let event = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(event) => event,
                Err(_) => break, // file vide
            };
            let kind = unsafe { event.GetType() }?;
            match kind {
                ME_TRANSFORM_NEED_INPUT => {
                    self.pending_input_requests += 1;
                    self.telemetry
                        .need_input_events
                        .fetch_add(1, Ordering::Relaxed);
                    NEED_INPUT_EVENTS.fetch_add(1, Ordering::Relaxed);
                    tracing::trace!(
                        pending_input_requests = self.pending_input_requests,
                        "événement METransformNeedInput reçu"
                    );
                }
                ME_TRANSFORM_HAVE_OUTPUT => {
                    self.pending_outputs += 1;
                    self.telemetry
                        .have_output_events
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::trace!(
                        pending_outputs = self.pending_outputs,
                        "événement METransformHaveOutput reçu"
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Converts the captured image and hands it to the encoder as soon as it
    /// claims one.
    ///
    /// **Fix of the ~30 fps throughput ceiling (28/07).** The previous
    /// version only converted the image if `pending_nv12` was not already
    /// enough to satisfy the pending input requests:
    ///
    /// ```ignore
    /// if (self.pending_nv12.len() as u32) < self.pending_input_requests {
    ///     self.feed_converter(frame, sample_time, duration)?;
    /// }
    /// ```
    ///
    /// The intent ("do not convert in advance, it would only be
    /// latency") was right for a source that can be queried again at
    /// will — it is wrong for this one. `AcquireNextFrame` only signals
    /// content **once**: the image this guard discarded was not
    /// postponed, it was **permanently lost**. At the next
    /// round, when the encoder finally claimed an input, the capture
    /// had nothing left to give (the desktop had not changed again), and the
    /// request stayed pending until the round after. Hence an
    /// anti-phase lock at one image every two rounds of
    /// `Session::run`: 60 Hz / 2 = **exactly the ~30 fps ceiling**
    /// measured end to end, six times, by the previous rounds.
    ///
    /// This guard also explains why the two experiments that should have
    /// settled it showed nothing: `ENCODER_THROUGHPUT_TEST`/`ENCODE_TEST`
    /// (`diagnostics/capture.rs`) resubmit **the same texture** in a loop, so that
    /// throwing an image away there costs nothing — hence the ~80 fps that seemed to
    /// exonerate the code and accuse the NVENC driver; and forcing the capture to
    /// 60 Hz (`remesure-debit.md`, step 3a) did not move the throughput, the
    /// extra captures all falling back into this same guard.
    ///
    /// The correct driving decouples the two rhythms: we systematically
    /// convert what the capture gave, we keep in advance only
    /// `MAX_PENDING_NV12` of them (the most recent — an older image is
    /// stale for an interactive stream), and we serve the input requests
    /// with what is ready.
    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        self.telemetry.submit_calls.fetch_add(1, Ordering::Relaxed);
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_SUBMIT_DRAIN_EVENTS, Ordering::Relaxed);
        let drained = self.drain_events();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        drained?;
        self.publish_state();
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "drainage des événements de l'encodeur lent");
        }

        // Media Foundation counts in 100 ns units; our timestamps are
        // in 1/90000 s. 90000 Hz → 10,000,000 Hz: factor 1000/9.
        let sample_time = (pts_90k as i64) * 1000 / 9;
        let duration = 10_000_000 / self.fps.max(1) as i64;

        // Convert unconditionally: the image will never be offered again by the
        // capture (see the method comment).
        let t_convert = std::time::Instant::now();
        let converted = self.feed_converter(frame, sample_time, duration);
        CONVERT_NS.fetch_add(t_convert.elapsed().as_nanos() as u64, Ordering::Relaxed);
        converted?;

        // Keep only the most recent. `feed_converter` pushes at the tail,
        // so the stale ones are at the head. Removing them here rather than
        // letting the queue grow avoids serving the encoder an image already
        // outdated at the moment it claims it — that would be paying in latency for the
        // throughput just gained.
        while self.pending_nv12.len() > MAX_PENDING_NV12 {
            self.pending_nv12.pop_front();
            self.telemetry
                .dropped_stale_nv12
                .fetch_add(1, Ordering::Relaxed);
            DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
        }

        while self.pending_input_requests > 0 {
            let Some(nv12_sample) = self.pending_nv12.pop_front() else {
                break;
            };
            let t = std::time::Instant::now();
            self.telemetry
                .phase
                .store(PHASE_ENCODER_PROCESS_INPUT, Ordering::Relaxed);
            let fed = unsafe { self.transform.ProcessInput(0, &nv12_sample, 0) };
            self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
            ENC_IN_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
            fed.context("soumission de l'image NV12 à l'encodeur")?;
            self.telemetry
                .encoder_inputs
                .fetch_add(1, Ordering::Relaxed);
            ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);
            let elapsed = t.elapsed();
            if elapsed > SLOW_CALL {
                tracing::warn!(?elapsed, "ProcessInput de l'encodeur lent");
            }
            self.pending_input_requests -= 1;
        }
        self.publish_state();
        Ok(())
    }

    /// Retrieves an encoded access unit if one is available.
    ///
    /// **Fix round 1/5**: the reviewer asked to check,
    /// rather than assume, that the same shortcut (stopping after a single
    /// sample) does not also affect the draining of the encoder. MF's async
    /// model: one `METransformHaveOutput` event corresponds to
    /// exactly one `ProcessOutput` call — but the
    /// `MFT_OUTPUT_DATA_BUFFER_INCOMPLETE` flag (set in `dwStatus`) explicitly
    /// signals, when present, that output remains for THIS
    /// stream without a new event being guaranteed. We now check it
    /// explicitly rather than ignore it: if it is set, we
    /// reschedule an entry in `pending_outputs` so that the
    /// caller's loop (`while let Some(unit) = poll_output()?`) asks again
    /// immediately, without waiting for an event that might not come.
    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.telemetry
            .phase
            .store(PHASE_POLL_DRAIN_EVENTS, Ordering::Relaxed);
        let drained = self.drain_events();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        drained?;
        if self.pending_outputs == 0 {
            return Ok(None);
        }
        self.pending_outputs -= 1;

        // Hardware MFTs allocate their output samples themselves.
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: std::mem::ManuallyDrop::new(None),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        }];
        let mut status = 0u32;

        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_ENCODER_PROCESS_OUTPUT, Ordering::Relaxed);
        let produced = unsafe { self.transform.ProcessOutput(0, &mut buffers, &mut status) };
        self.telemetry
            .phase
            .store(PHASE_ENCODER_READ_BUFFER, Ordering::Relaxed);
        // Take the reference back BEFORE any error propagation, as
        // `take_output_sample` requires and as
        // `drain_converter_output` already does. Exiting through `?` first would leak
        // the sample if the MFT had dropped one despite the failure — this
        // driver does not seem to do so, but it is exactly the class of
        // leak fixed in this file, and nothing guarantees it elsewhere.
        // `dwStatus` is read before, since the buffer must no longer be read after.
        let incomplete = buffers[0].dwStatus & MFT_OUTPUT_DATA_BUFFER_INCOMPLETE.0 as u32 != 0;
        let taken = unsafe { take_output_sample(&mut buffers[0]) };
        ENC_OUT_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        produced.context("récupération de l'image encodée")?;
        self.telemetry
            .encoder_outputs
            .fetch_add(1, Ordering::Relaxed);
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessOutput de l'encodeur lent");
        }

        if incomplete {
            self.pending_outputs += 1;
        }

        let sample = taken.ok_or_else(|| anyhow!("échantillon de sortie absent"))?;

        let media_buffer = unsafe { sample.ConvertToContiguousBuffer() }?;
        let mut data_ptr: *mut u8 = std::ptr::null_mut();
        let mut length = 0u32;
        unsafe { media_buffer.Lock(&mut data_ptr, None, Some(&mut length))? };
        let bytes = unsafe { std::slice::from_raw_parts(data_ptr, length as usize) }.to_vec();
        unsafe { media_buffer.Unlock()? };

        // The output is in Annex-B; we pass it again through grouping to
        // obtain the key frame indicator consistently with the rest.
        let mut units = group_access_units(&bytes, self.fps.max(1));
        if units.is_empty() {
            return Ok(None);
        }
        let mut unit = units.remove(0);
        // The timestamp comes from the sample, not from the position in the stream.
        let sample_time = unsafe { sample.GetSampleTime() }.unwrap_or(0);
        unit.pts_90k = (sample_time.max(0) as u64) * 9 / 1000;
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        Ok(Some(unit))
    }

    /// Hands the encoder the NV12 samples already ready, for each
    /// input request it has issued since the last call.
    ///
    /// Exists to break a serialisation measured in `Session::run`: the
    /// cycle "submit → the encoder produces → retrieve the output →
    /// the encoder frees a slot → it asks for an input again" only
    /// crossed ONE step per loop round, since `submit` and
    /// `poll_output` were only called once each per 16.7 ms round.
    /// Throughput was thereby capped at the loop cadence divided by the
    /// number of steps — measured at ~23 Hz for a 60 Hz loop, whereas the
    /// same encoder sustains 66 Hz when solicited in a tight loop
    /// (`ENCODE_TEST`), where these steps chain in a few microseconds.
    ///
    /// Called after draining the outputs: it is this draining that frees
    /// the input slots, so it is right after it that the
    /// corresponding request becomes available.
    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        self.drain_events()?;
        while self.pending_input_requests > 0 {
            let Some(nv12_sample) = self.pending_nv12.pop_front() else {
                break;
            };
            let t = std::time::Instant::now();
            let fed = unsafe { self.transform.ProcessInput(0, &nv12_sample, 0) };
            ENC_IN_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
            fed.context("soumission différée de l'image NV12 à l'encodeur")?;
            self.telemetry
                .encoder_inputs
                .fetch_add(1, Ordering::Relaxed);
            ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);
            self.pending_input_requests -= 1;
        }
        self.publish_state();
        Ok(())
    }
}
