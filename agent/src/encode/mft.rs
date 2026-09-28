//! The **Media Foundation** path: the `EncodeurMft` object, its construction and
//! its destruction.
//!
//! Extracted from `encode.rs` on 30 August 2026 (batch 31), in a DEDICATED task and
//! **before** the wiring it prepares — the repository's doctrine: *extract,
//! never compress*, and *the extraction played BEFORE the one that adds*.
//!
//! 🔴 **WHY THREE FILES AND NOT TWO.** The pump's methods alone
//! weigh **487 lines** (measured, not estimated): they do not fit
//! under the 500 ceiling with their header and imports. And Rust's
//! visibility rule forbids housing them anywhere other than **under** the module
//! that defines the structure — a sibling does not see private fields. The
//! pump is therefore split **by responsibility** — `convertisseur` (BGRA→NV12)
//! and `encodeur` (the asynchronous MFT) — and not cut at random.
//!
//! ⚠️ **This module is NOT the facade.** `encode.rs` re-exports
//! `EncodeurMft`: the repository's ~10 callers have not moved by one line, and
//! none of the eight verbs changed signature.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

// Same reason as in its children: this file is the continuation
// of `encode.rs`, not an independent module that would consume its interface.
use crate::encode::*;
use crate::encode::{arret, fabrique, reglages};

mod convertisseur;
mod encodeur;

pub struct EncodeurMft {
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    device_manager: IMFDXGIDeviceManager,
    /// BGRA→NV12 converter (synchronous MFT). Always present: the capture
    /// only produces BGRA, the encoder only accepts NV12 (see the
    /// module comment).
    converter: IMFTransform,
    /// True if the converter allocates its output samples itself
    /// (`MFT_OUTPUT_STREAM_PROVIDES_SAMPLES`). Determined once at
    /// construction: providing a sample while the flag is set
    /// (or the reverse) is a `ProcessOutput` error documented by MF.
    converter_provides_samples: bool,
    /// The capture's D3D11 device, kept to allocate NV12 output
    /// textures when the converter does not self-allocate.
    device: ID3D11Device,
    /// Timestamps (time, duration) of the BGRA inputs submitted to the converter
    /// but whose output has not yet been retrieved, in submission
    /// order. A video converter never reorders images:
    /// the oldest output not yet retrieved always corresponds
    /// to the oldest input not yet come out (see fix
    /// round 1/5 — the converter may return, while draining
    /// an input, the output of an earlier input still pending;
    /// it must then be associated with ITS original timestamp, not that of
    /// the input just submitted).
    pending_conversion_timestamps: VecDeque<(i64, i64)>,
    /// NV12 samples already produced by the converter but not yet
    /// submitted to the encoder (it was not claiming any yet).
    pending_nv12: VecDeque<IMFSample>,
    /// True if the converter must still return the output of an already
    /// consumed input. Purely diagnostic since 28/07 (published as
    /// `awaiting_drain`): driving now relies on `GetInputStatus`,
    /// which describes the converter's real state instead of deducing it.
    converter_output_pending: bool,
    /// Size actually encoded and sent: the converter's output and the
    /// encoder's input. It can be smaller than the captured textures; it is
    /// the adaptive-resolution lever, and it never touches the Windows window
    /// (unlike `WindowsSource::resize`).
    encode: (u32, u32),
    fps: u32,
    /// Number of input requests not yet satisfied.
    pending_input_requests: u32,
    /// Number of images ready to be retrieved.
    pending_outputs: u32,
    /// Diagnostic counter: occasions where the converter's output pool
    /// was momentarily exhausted (see `collect_converter_output`). Exposed
    /// to measure the real extent of this workaround, not consumed by
    /// the driving logic itself.
    skipped_busy: u64,
    /// Counters and current step, readable from another thread (see
    /// `EncoderTelemetry`).
    telemetry: Arc<EncoderTelemetry>,
    /// Serialised work queue imposed on the encoder MFT, and barrier of its
    /// putting at rest (see `arret::FileMft`; the converter has none, and
    /// `arret::put_to_rest` says why).
    ///
    /// **Declared last on purpose**: fields are destroyed in
    /// declaration order, after `Drop for EncodeurMft` has run.
    /// The queue must only be released once the MFT holding it has been released.
    file_encodeur: arret::FileMft,
}

impl EncodeurMft {
    pub fn new(
        device: &ID3D11Device,
        capture: (u32, u32),
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        start_media_foundation()?;

        // Allocated HERE, before any MFT: locals are destroyed in
        // REVERSE declaration order, so this one is destroyed last if a
        // `?` further down interrupts the construction. A queue released before the MFT
        // holding it would be exactly the inversion the field order
        // above avoids. See `arret::FileMft::allouer`.
        let mut file_encodeur = arret::FileMft::allouer();

        let transform = fabrique::find_hardware_encoder()?;
        let attributes = unsafe { transform.GetAttributes() }?;

        // Unlock asynchronous mode: mandatory for any hardware MFT.
        let is_async = unsafe { attributes.GetUINT32(&MF_TRANSFORM_ASYNC) }.unwrap_or(0);
        if is_async == 0 {
            bail!("l'encodeur trouvé n'est pas asynchrone : configuration inattendue");
        }
        unsafe { attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) }?;
        // Low-latency mode: no multi-frame buffering.
        unsafe { attributes.SetUINT32(&MF_LOW_LATENCY, 1) }?;

        // Empirical proof (see module comment): the real list of
        // input types advertised by this encoder, before any configuration.
        fabrique::log_supported_input_types(&transform);

        // Share the D3D11 device to receive GPU textures.
        let device_manager = fabrique::share_device(device)?;
        unsafe {
            transform.ProcessMessage(
                MFT_MESSAGE_SET_D3D_MANAGER,
                device_manager.as_raw() as usize,
            )
        }
        .context("partage du périphérique D3D avec l'encodeur")?;

        reglages::configure_output(&transform, encode.0, encode.1, fps, bitrate)?;
        reglages::configure_input(&transform, encode.0, encode.1, fps)?;
        reglages::configure_rate_control(&transform, bitrate)?;

        let events: IMFMediaEventGenerator = transform.cast()?;

        // BEFORE any stream start: impose on the MFT the queue on which
        // it will drop its asynchronous work, the only way to later obtain
        // a barrier on that work (see `arret::FileMft`).
        file_encodeur.confier(&transform, "encodeur");

        // `NOTIFY_BEGIN_STREAMING` is the point where a hardware MFT reserves
        // its GPU session resources: a candidate for refusal when several
        // encoders coexist, not to be confused with the two others.
        unsafe {
            transform
                .ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)
                .context("purge initiale de l'encodeur H.264 (transform matériel)")?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .context("démarrage du flux de l'encodeur H.264 (transform matériel)")?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .context("début de flux de l'encodeur H.264 (transform matériel)")?;
        }

        // BGRA→NV12 converter, sharing the same D3D device.
        let converter = fabrique::create_color_converter(&device_manager, capture, encode, fps)?;
        let converter_stream_info = unsafe { converter.GetOutputStreamInfo(0) }
            .context("interrogation du flux de sortie du convertisseur")?;
        let converter_provides_samples =
            converter_stream_info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
        tracing::info!(
            converter_provides_samples,
            "convertisseur BGRA→NV12 (Video Processor MFT) configuré"
        );
        // Attempt made (throughput investigation): systematically provide our
        // own output sample, including when
        // `converter_provides_samples` is true, to see whether it avoids
        // the ~1 s wait measured in `drain_converter_output`. Rejected
        // immediately by the converter (`Output Sample is Invalid`,
        // `0x80070057`): the documented contract (never provide a buffer
        // when this flag is set) must be respected, there is no possible
        // workaround here.
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0) }
            .context("démarrage du flux du convertisseur de couleur (Video Processor MFT)")?;
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0) }
            .context("début de flux du convertisseur de couleur (Video Processor MFT)")?;

        Ok(Self {
            transform,
            events,
            device_manager,
            converter,
            converter_provides_samples,
            device: device.clone(),
            pending_conversion_timestamps: VecDeque::new(),
            pending_nv12: VecDeque::new(),
            converter_output_pending: false,
            skipped_busy: 0,
            telemetry: Arc::new(EncoderTelemetry::default()),
            file_encodeur,
            encode,
            fps,
            pending_input_requests: 0,
            pending_outputs: 0,
        })
    }

    /// Telemetry handle, to be shared with a watchdog thread.
    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        self.telemetry.clone()
    }

    /// Copies the "state" counters (queue lengths) into the
    /// telemetry. Called at the breathing points of the hot path.
    fn publish_state(&self) {
        self.telemetry
            .queued_nv12
            .store(self.pending_nv12.len() as u64, Ordering::Relaxed);
        self.telemetry
            .pending_input_requests
            .store(self.pending_input_requests as u64, Ordering::Relaxed);
        self.telemetry
            .skipped_busy
            .store(self.skipped_busy, Ordering::Relaxed);
        self.telemetry
            .awaiting_drain
            .store(self.converter_output_pending as u64, Ordering::Relaxed);
    }

    /// Occupies the work queue imposed on the encoder MFT for `duree`.
    ///
    /// **Measurement probe, never called in operation**: it tests whether the
    /// MFT's asynchronous work really goes through the queue imposed on
    /// it. If so, clogging it must stop the encoder; if the encoder
    /// continues, the barrier of `arret::FileMft` bars nothing and we must
    /// know it. See the residual failure mode documented on `arret::FileMft`.
    pub fn eprouver_file(&self, duree: std::time::Duration) {
        self.file_encodeur.bloquer(duree);
    }

    /// Forces the production of a key frame on the next image.
    pub fn request_keyframe(&mut self) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let value = reglages::variant_bool(true);
        unsafe { codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &value) }?;
        Ok(())
    }

    /// Changes the target bitrate without rebuilding the encoder.
    ///
    /// Hot `ICodecAPI::SetValue` is already tested on this driver by
    /// `request_keyframe`, which writes `AVEncVideoForceKeyFrame` during a
    /// session on this same object.
    ///
    /// A refusal by the driver is returned to the caller rather than logged here:
    /// it is `WindowsSource` that knows whether it must continue with the resolution
    /// (see task 7).
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let rate = reglages::variant_u32(bitrate);
        unsafe { codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate) }
            .context("réglage à chaud du débit d'encodage")?;
        Ok(())
    }

    /// Size actually encoded. Distinct from the captured size since
    /// the resolution adapts to the link.
    pub fn encode_size(&self) -> (u32, u32) {
        self.encode
    }
}

impl Drop for EncodeurMft {
    fn drop(&mut self) {
        if self.skipped_busy > 0 {
            tracing::debug!(
                skipped_busy = self.skipped_busy,
                "images renoncées faute de confirmation du convertisseur (diagnostic)"
            );
        }
        // Putting at rest BEFORE releasing the COM references: nothing
        // ever asked the hardware MFT to stop its asynchronous
        // processing, and that is the race task 2bis noted. See
        // `arret::put_to_rest` for the detail and the survey that motivates it.
        arret::put_to_rest(&self.converter, &self.transform, &self.file_encodeur);

        // `MFShutdown` is called nowhere, and it is deliberate: see
        // `start_media_foundation`. The line below is INERT (borrow
        // immediately thrown away, zero machine code): to be removed outside the measurement branch.
        let _ = &self.device_manager;
    }
}
