//! The **converter** half of the pump: BGRA → NV12, through a
//! *synchronous* MFT driven by a `ProcessInput`/`ProcessOutput` pair.
//!
//! Extracted from `encode.rs` with the rest of the MFT path (batch 31). Its counterpart is
//! `super::encodeur`, which drives the *asynchronous* MFT. The boundary between the
//! two is that of the two transforms, not an arbitrary cut.
//!
//! ⚠️ **`feed_converter` and `take_output_sample` are `pub(super)`**, and they
//! alone: they are the only two elements `super::encodeur` calls —
//! established by surveying call sites, not by assumption. Everything else
//! stays private to this file.

use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use windows::core::Interface;

// ⚠️ **Glob import, and it is a motivated choice.** This file is the
// CONTINUATION of `encode.rs`: phases, public counters and tuning
// constants live there, and listing them here would create a second list to
// keep up to date — the one that gets out of sync. The repository's rule targets
// copied assertions; a glob import copies none.
use crate::encode::*;
// The MFT back end's type comes from the PARENT, not from the glob: `crate::encode`
// exports the FACADE, which is not what we implement here.
use super::EncodeurMft;
use crate::capture::CapturedFrame;

/// Result of a `ProcessOutput` call on the BGRA→NV12 converter (see
/// `EncodeurMft::drain_converter_output`).
enum ConverterPoll {
    /// A converted sample is available.
    Sample(IMFSample),
    /// `MF_E_TRANSFORM_NEED_MORE_INPUT`: nothing more to produce for
    /// now, the converter is ready for a new input.
    NeedMoreInput,
    /// `MF_E_SAMPLEALLOCATOR_EMPTY`: output pool momentarily exhausted.
    /// Back-pressure signal documented by Media Foundation, not a
    /// failure (see `drain_converter_output`).
    Busy,
}

impl EncodeurMft {
    /// Converts **one** captured BGRA texture into **one** NV12 sample,
    /// stacked in `pending_nv12`. Unlike the encoder, this transform is
    /// driven by a classic `ProcessInput`/`ProcessOutput` pair: no
    /// events to follow.
    ///
    /// **Fix of 28/07 — draining model revised, with measurements to back it.**
    /// The previous round had replaced a single `ProcessOutput` with a
    /// loop "until actually observing `MF_E_TRANSFORM_NEED_MORE_INPUT`",
    /// relying on Microsoft's "Basic MFT Processing Model". The
    /// telemetry (see `EncoderTelemetry`) shows that this MFT **never returns
    /// this code**: with only 2 inputs submitted, the loop pulled
    /// 10 output samples, then 9 more per second — that is
    /// as many samples as its `IMFVideoSampleAllocator` contains,
    /// after which `ProcessOutput` blocks a whole second before returning
    /// `MF_E_SAMPLEALLOCATOR_EMPTY`. Looping therefore does not "drain" this
    /// converter: it empties its pool, it duplicates images that were
    /// never submitted, and it imposes a one-second wait per round.
    ///
    /// The correct driving for this 1-input/1-output transform is the
    /// original one: one `ProcessOutput` per `ProcessInput`. What made
    /// that version fail with `MF_E_NOTACCEPTING` was not the
    /// model but the reference leak fixed in `take_output_sample` —
    /// pool exhausted, hence a converter unable to accept one more
    /// input.
    pub(super) fn feed_converter(
        &mut self,
        frame: &CapturedFrame,
        sample_time: i64,
        duration: i64,
    ) -> Result<()> {
        // 1. Remove the pending outputs until the converter
        //    declares itself ready to take an input.
        //
        // It is `GetInputStatus` — and not `GetOutputStatus` — that serves as the
        // stop condition, for a measured reason: after having consumed
        // an input, this MFT keeps announcing "output ready"
        // permanently (each extra `ProcessOutput` succeeds by
        // replaying the last converted image), so that a loop
        // "drain until it announces nothing anymore" never
        // ends and ends up emptying its `IMFVideoSampleAllocator`. On the
        // other hand it refuses any new input as long as its output has not been
        // taken back: the strict "one output per input" pairing therefore blocks
        // just as surely (measured: only 1 image converted in 30 s).
        // Removing outputs until it is ready again is the only one
        // of the three drivings that really gets new images through —
        // in the nominal regime, a single iteration is enough.
        let mut collected = 0usize;
        while !self.converter_accepts_input() {
            if collected >= MAX_CONVERTER_COLLECTS || !self.collect_converter_output()? {
                // Still not ready (or pool momentarily empty): we skip
                // this image and will retry. A skipped image is better
                // than a dead pipeline — and better than a `MF_E_NOTACCEPTING`
                // taken as an error.
                self.telemetry
                    .converter_not_accepting
                    .fetch_add(1, Ordering::Relaxed);
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
                self.publish_state();
                return Ok(());
            }
            collected += 1;
        }

        let bgra_sample = unsafe { MFCreateSample() }?;
        let bgra_buffer =
            unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &frame.texture, 0, false) }
                .context("wrapping the BGRA texture for the converter")?;
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_CONVERTER_PROCESS_INPUT, Ordering::Relaxed);
        let result = unsafe {
            bgra_sample.AddBuffer(&bgra_buffer)?;
            bgra_sample.SetSampleTime(sample_time)?;
            bgra_sample.SetSampleDuration(duration)?;
            self.converter.ProcessInput(0, &bgra_sample, 0)
        };
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        // Safety net: `GetInputStatus` just said yes, but if the
        // converter changes its mind, `MF_E_NOTACCEPTING` remains
        // back-pressure and not a failure — we skip the image.
        if let Err(e) = &result {
            if e.code() == MF_E_NOTACCEPTING {
                self.telemetry
                    .converter_not_accepting
                    .fetch_add(1, Ordering::Relaxed);
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
                self.publish_state();
                return Ok(());
            }
        }
        result.context("submitting the frame to the BGRA→NV12 converter")?;
        self.telemetry
            .converter_inputs
            .fetch_add(1, Ordering::Relaxed);
        CONVERTER_INPUTS.fetch_add(1, Ordering::Relaxed);
        self.converter_output_pending = true;
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessInput du convertisseur lent");
        }
        self.pending_conversion_timestamps
            .push_back((sample_time, duration));

        self.collect_converter_output()?;
        Ok(())
    }

    /// Does the converter declare itself ready to accept an input?
    ///
    /// **Portability risk to be aware of**: this driving relies on a
    /// documented API, but its adoption comes from observing an abnormal
    /// behaviour found on **a single driver (NVIDIA) and a single machine**.
    /// `GetInputStatus`/`GetOutputStatus` are optional in `IMFTransform`
    /// and nothing guarantees that an Intel or AMD converter behaves the
    /// same. The fallback below (attempting `ProcessInput` when the method is
    /// not implemented) covers the most likely case, not all of them; to be revalidated
    /// on the first other GPU encountered.
    ///
    /// Publishes both flags to the telemetry along the way. If the MFT
    /// does not implement `GetInputStatus` (`u64::MAX`), we answer yes: better
    /// to attempt `ProcessInput` and handle a possible `MF_E_NOTACCEPTING`
    /// than never to submit anything.
    fn converter_accepts_input(&self) -> bool {
        let input = converter_status(&self.converter, true);
        self.telemetry
            .converter_input_status
            .store(input, Ordering::Relaxed);
        self.telemetry
            .converter_output_status
            .store(converter_status(&self.converter, false), Ordering::Relaxed);
        input == u64::MAX || input & MFT_INPUT_STATUS_ACCEPT_DATA.0 as u64 != 0
    }

    /// Removes **one** converted sample and stacks it in `pending_nv12`,
    /// with the timestamp of ITS original input (popped from
    /// `pending_conversion_timestamps`, FIFO — see its field comment).
    ///
    /// Returns `true` if the due output was indeed removed (the converter
    /// accepts an input again), `false` if we must retry later
    /// (`MF_E_SAMPLEALLOCATOR_EMPTY`, `0xC00D4A3E` — pool momentarily empty,
    /// a back-pressure signal and not a failure).
    fn collect_converter_output(&mut self) -> Result<bool> {
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_CONVERTER_PROCESS_OUTPUT, Ordering::Relaxed);
        let poll = self.drain_converter_output();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        let poll = poll?;
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessOutput du convertisseur lent");
        }
        let ready = match poll {
            ConverterPoll::Sample(sample) => {
                self.telemetry
                    .converter_outputs
                    .fetch_add(1, Ordering::Relaxed);
                CONVERTER_OUTPUTS.fetch_add(1, Ordering::Relaxed);
                let (time, duration) = self
                    .pending_conversion_timestamps
                    .pop_front()
                    .unwrap_or((0, 0));
                unsafe {
                    let _ = sample.SetSampleTime(time);
                    let _ = sample.SetSampleDuration(duration);
                }
                self.pending_nv12.push_back(sample);
                self.converter_output_pending = false;
                true
            }
            // Nothing ready: the converter is available for an input.
            ConverterPoll::NeedMoreInput => {
                self.converter_output_pending = false;
                true
            }
            ConverterPoll::Busy => {
                self.converter_output_pending = true;
                self.skipped_busy += 1;
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
                false
            }
        };
        self.publish_state();
        Ok(ready)
    }

    /// One `ProcessOutput` call on the converter.
    ///
    /// Trap met during the attempt, beyond the `MF_E_NOTACCEPTING` documented
    /// above: even after enlarging the output pool through
    /// `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`/`_PROGRESSIVE` (tried up to 16),
    /// the "confirmation" call fails reproducibly after
    /// exactly 5 images with `MF_E_SAMPLEALLOCATOR_EMPTY` — identical
    /// whatever the requested pool size, which proves that this
    /// attribute is not honoured by this MFT for this use case. The MF error
    /// message associated with this code ("empty due to outstanding
    /// requests") corresponds to a back-pressure signal documented by
    /// Media Foundation for `IMFVideoSampleAllocator` — not a failure — and
    /// is handled normally by retrying later, once a previous
    /// sample has been released by the downstream encoder.
    fn drain_converter_output(&mut self) -> Result<ConverterPoll> {
        let mut buffer = MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            // A new sample at each round in the case where the
            // converter does not self-allocate: several outputs can
            // be queued simultaneously (`pending_nv12`), so
            // reusing a single one would make them all point at the same texture
            // — each overwriting the previous one. This path is not taken
            // on the target VM (`converter_provides_samples` is `true` there),
            // but it must not be wrong for all that.
            pSample: std::mem::ManuallyDrop::new(if self.converter_provides_samples {
                None
            } else {
                Some(fabrique::create_nv12_sample(
                    &self.device,
                    self.encode.0,
                    self.encode.1,
                )?)
            }),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        };
        let mut status = 0u32;
        let result = unsafe {
            self.converter
                .ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status)
        };
        // `take_output_sample` MUST be called on all paths, including
        // error ones: see its comment (the reference dropped into
        // `pSample` by the MFT belongs to no one but us).
        let sample = unsafe { take_output_sample(&mut buffer) };
        match result {
            Ok(()) => Ok(sample
                .map(ConverterPoll::Sample)
                .unwrap_or(ConverterPoll::NeedMoreInput)),
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => {
                Ok(ConverterPoll::NeedMoreInput)
            }
            Err(e) if e.code() == MF_E_SAMPLEALLOCATOR_EMPTY => Ok(ConverterPoll::Busy),
            Err(e) => Err(e).context("retrieving the frame converted to NV12"),
        }
    }
}

/// Takes back ownership of the sample (and events) an MFT has just
/// dropped into an `MFT_OUTPUT_DATA_BUFFER`, leaving the structure empty.
///
/// **This is the root cause of the block of 28/07, fixed here** (see the
/// task report). The two fields `pSample`/`pEvents` of
/// `MFT_OUTPUT_DATA_BUFFER` are `ManuallyDrop<Option<...>>`: windows-rs
/// deliberately refuses to free them on its own, since their ownership
/// depends on the direction of the call. `ProcessOutput` drops a COM reference into them of which
/// **the caller becomes the owner**; reading it through `.as_ref().cloned()`
/// adds a second reference without ever giving back the first, and the
/// `ManuallyDrop` takes the latter to the grave at the end of the block. Each
/// encoded image therefore leaked a reference.
///
/// Observed consequence, much more serious than a mere memory leak: the
/// output samples of the `Video Processor MFT` come from a
/// fixed-size `IMFVideoSampleAllocator` (10 on this VM). A sample
/// never released never returns to the pool. After exactly 10 images the
/// pool was permanently empty, and each following `ProcessOutput` waited
/// a whole second for a free sample before returning
/// `MF_E_SAMPLEALLOCATOR_EMPTY` — hence the "ceiling at ~1 frame/s" then, as soon
/// as draining demanded a confirmation through
/// `MF_E_TRANSFORM_NEED_MORE_INPUT` (previous round), the total stop of the
/// pipeline. `ManuallyDrop::take` moves the reference out of the structure:
/// it is then owned normally, and released as soon as the caller is
/// done with it — which returns the sample to the pool.
///
/// # Safety
///
/// The buffer must no longer be read after this call (its two COM fields are
/// left in a moved state). All callers use it as a local
/// variable and no longer touch it afterwards.
/// Queries the converter: `true` for `GetInputStatus` (can it accept
/// an input), `false` for `GetOutputStatus` (is an output ready).
///
/// Returns the raw flags, or `u64::MAX` if the method is not
/// implemented by this MFT — both are optional in `IMFTransform`, and
/// the distinction "answers no" / "does not answer" is precisely what we
/// need to know.
fn converter_status(converter: &IMFTransform, input: bool) -> u64 {
    // windows-rs 0.62 API gap: these two methods return the flags as a
    // return value (`Result<u32>`), where the C signature writes them into an
    // output parameter.
    let result = if input {
        unsafe { converter.GetInputStatus(0) }
    } else {
        unsafe { converter.GetOutputStatus() }
    };
    match result {
        Ok(flags) => flags as u64,
        Err(_) => u64::MAX,
    }
}

pub(super) unsafe fn take_output_sample(buffer: &mut MFT_OUTPUT_DATA_BUFFER) -> Option<IMFSample> {
    // `pEvents` is almost always null, but when an MFT drops an event queue
    // into it, it belongs to us in exactly the same way.
    drop(std::mem::ManuallyDrop::take(&mut buffer.pEvents));
    std::mem::ManuallyDrop::take(&mut buffer.pSample)
}
