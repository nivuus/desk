//! Hardware H.264 encoder through Media Foundation.
//!
//! Hardware MFTs are asynchronous: driving is done through events
//! (`METransformNeedInput` / `METransformHaveOutput`) and not through a synchronous
//! `ProcessInput`/`ProcessOutput` loop. This constraint is imposed by
//! Media Foundation, not by a design choice.
//!
//! **Gap from the original brief, verified empirically on the target VM**: the
//! capture (`crate::capture`) provides `DXGI_FORMAT_B8G8R8A8_UNORM` textures
//! (BGRA), but the `NVIDIA H.264 Encoder MFT` encoder ONLY advertises `NV12` as
//! an available input type (see `fabrique::log_supported_input_types`, which
//! logs
//! the real list returned by `GetInputAvailableType` at start-up — no
//! RGB/ARGB variant appears in it). Wrapping the BGRA texture directly in
//! a sample announced as NV12 would produce a `ProcessInput` failure or, worse,
//! a corrupted image interpreted with the wrong colour plane. We therefore insert
//! a GPU converter (`CLSID_VideoProcessorMFT`, category
//! `MFT_CATEGORY_VIDEO_PROCESSOR`) between the capture and the encoder: it is a
//! *synchronous* MFT (unlike the encoder), driven by a simple
//! `ProcessInput`/`ProcessOutput` pair, which converts BGRA→NV12 without leaving
//! the GPU (the D3D device manager is shared with it as with
//! the encoder).
//!
//! **Fix of 28/07: the ceiling at ~1 frame/s, then the total stop of the
//! pipeline, came from a COM reference leak on the output
//! samples.** See `mft::convertisseur::take_output_sample` for the exact
//! mechanism, and `mft::convertisseur::feed_converter` for the draining model
//! that follows from it. In short:
//! `MFT_OUTPUT_DATA_BUFFER::pSample` is a `ManuallyDrop` whose reference
//! was never released, so that the converter's samples never
//! returned to its `IMFVideoSampleAllocator`; past the pool's 10
//! samples, each `ProcessOutput` waited a whole second
//! before returning `MF_E_SAMPLEALLOCATOR_EMPTY`. The two previous
//! "fixes" (draining loop until `MF_E_TRANSFORM_NEED_MORE_INPUT`,
//! then deferring draining to the next round) treated the symptoms of this
//! leak and aggravated it up to a complete block.
//!
//! These conclusions are not deduced from the code but measured: the hot
//! path publishes its current step and its counters in `EncoderTelemetry`,
//! which a watchdog thread logs every second (see `diagnostics/capture.rs`,
//! `ENCODER_THROUGHPUT_TEST`). The measured throughput is recorded in the task
//! report.

#![cfg(windows)]

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, OnceLock};

use anyhow::{bail, Result};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Media::MediaFoundation::*;

use crate::capture::CapturedFrame;
use crate::h264::{group_access_units, AccessUnit};

mod arret;
// Extracted on 30 August 2026 (batch 31), BEFORE any addition: this file
// weighed 1536 lines. `fabrique` FINDS and ACTIVATES the MFTs, `reglages` SETS
// the settings on an obtained MFT. Ordinary children (a simple `mod`
// in their gated parent) and not `#[path]`: they never need to
// get out of the `#![cfg(windows)]` above — see the child module
// convention in `docs/claude/module-conventions.md`.
mod fabrique;
mod reglages;

// The Media Foundation path, extracted on 30 August 2026 (batch 31) BEFORE the
// wiring it prepares. `H264Encoder` is RE-EXPORTED just below:
// the repository's ~10 callers keep writing `crate::encode::H264Encoder`,
// and none of the eight verbs changed signature.
mod mft;
mod natif;

/// Event identifiers of asynchronous MFTs (taken from the constants
/// provided by the crate rather than hardcoded duplicates, as suggested by the
/// brief).
const ME_TRANSFORM_NEED_INPUT: u32 = METransformNeedInput.0 as u32;
const ME_TRANSFORM_HAVE_OUTPUT: u32 = METransformHaveOutput.0 as u32;

/// Beyond this duration, a Media Foundation call of the hot path is
/// logged: at 60 fps, a whole frame fits in ~16 ms, so any
/// call exceeding this threshold is already an incident, not noise.
const SLOW_CALL: std::time::Duration = std::time::Duration::from_millis(50);

/// Maximum number of outputs removed in a row to make the converter
/// ready to take a new input (see `mft::convertisseur::feed_converter`).
/// Strictly bounded:
/// this MFT permanently advertises an output ready, so an unbounded loop
/// would empty its pool and block a whole second on
/// `MF_E_SAMPLEALLOCATOR_EMPTY`. In the nominal regime, one iteration is enough.
const MAX_CONVERTER_COLLECTS: usize = 4;

/// Number of NV12 samples kept in advance, ready to be handed to
/// the encoder as soon as it claims one.
///
/// **This is the fix for the ~30 fps throughput ceiling** (see `submit`). The
/// capture (Desktop Duplication) and the hardware encoder have two independent
/// rhythms: keeping one image converted in advance is what makes it possible to
/// serve an input request that arrived at a round where the desktop has not
/// changed.
///
/// Why 1 and not more: each image kept waiting is an image
/// displayed one round late. At 60 Hz, 1 is enough to cover the
/// phase shift between the two rhythms (at most one round apart) without adding
/// more than 16.7 ms to the latency budget. Beyond that, we would no longer buy
/// throughput, only latency.
///
/// **Checked on 28/07**: raising this queue to 4 only yields ~3 frames/s (47.5
/// → 51). So it was not the limiting factor either — depth 1
/// is kept, because it is the one that costs the least latency.
const MAX_PENDING_NV12: usize = 1;

/// Steps of the hot path, published in `EncoderTelemetry::phase` before
/// each Media Foundation call and reset to `PHASE_IDLE` right after.
///
/// Rationale: a Media Foundation call that never returns is
/// invisible to any trace placed *around* it (the "after" line is
/// never reached, the "before" line drowns in the flow). Publishing the current
/// step in an atomic integer lets an external watchdog thread
/// name precisely the blocked call while it is still blocked.
pub const PHASE_IDLE: u64 = 0;
pub const PHASE_SUBMIT_DRAIN_EVENTS: u64 = 1;
pub const PHASE_CONVERTER_PROCESS_INPUT: u64 = 2;
pub const PHASE_CONVERTER_PROCESS_OUTPUT: u64 = 3;
pub const PHASE_ENCODER_PROCESS_INPUT: u64 = 4;
pub const PHASE_POLL_DRAIN_EVENTS: u64 = 5;
pub const PHASE_ENCODER_PROCESS_OUTPUT: u64 = 6;
pub const PHASE_ENCODER_READ_BUFFER: u64 = 7;
/// Outside the encoder: the caller is in image acquisition. Set from the
/// diagnostic loop (`diagnostics/capture.rs`) so that the watchdog thread distinguishes
/// "blocked in the encoder" from "blocked in the capture" — otherwise the
/// two look alike: frozen counters, step at rest.
pub const PHASE_CAPTURE: u64 = 8;
/// Sub-steps of `DesktopCapture::next_frame`, published from `capture.rs`.
///
/// `PHASE_CAPTURE` alone is not enough: it covers three Windows calls with
/// very different failure modes (DXGI acquisition, GPU copy, image
/// release). Without distinguishing them, fixing a block in this function
/// would amount to fixing blindly.
pub const PHASE_CAPTURE_ACQUIRE: u64 = 9;
pub const PHASE_CAPTURE_CROP: u64 = 10;
pub const PHASE_CAPTURE_RELEASE: u64 = 11;

/// Readable name of a step, for the watchdog logs.
pub fn phase_name(phase: u64) -> &'static str {
    match phase {
        PHASE_IDLE => "repos",
        PHASE_SUBMIT_DRAIN_EVENTS => "submit/GetEvent",
        PHASE_CONVERTER_PROCESS_INPUT => "convertisseur/ProcessInput",
        PHASE_CONVERTER_PROCESS_OUTPUT => "convertisseur/ProcessOutput",
        PHASE_ENCODER_PROCESS_INPUT => "encodeur/ProcessInput",
        PHASE_POLL_DRAIN_EVENTS => "poll_output/GetEvent",
        PHASE_ENCODER_PROCESS_OUTPUT => "encodeur/ProcessOutput",
        PHASE_ENCODER_READ_BUFFER => "encoder/buffer read",
        PHASE_CAPTURE => "capture/next_frame",
        PHASE_CAPTURE_ACQUIRE => "capture/AcquireNextFrame",
        PHASE_CAPTURE_CROP => "capture/CopySubresourceRegion",
        PHASE_CAPTURE_RELEASE => "capture/ReleaseFrame",
        _ => "unknown",
    }
}

/// Counters of the hot path, all atomic to stay readable **while**
/// a Media Foundation call is in progress — which is exactly the case we
/// are trying to diagnose. The cost is nil in practice (`Relaxed` writes
/// to integers already in cache).
#[derive(Default)]
pub struct EncoderTelemetry {
    /// Current step (see the `PHASE_*` constants).
    ///
    /// Behind its own `Arc` to be shareable with `DesktopCapture`,
    /// which publishes its own sub-steps without knowing anything about the rest of the
    /// encoder's telemetry.
    pub phase: Arc<AtomicU64>,
    /// Number of `submit` calls entered (not necessarily exited).
    pub submit_calls: AtomicU64,
    /// Images BGRA effectivement remises au convertisseur.
    pub converter_inputs: AtomicU64,
    /// NV12 samples actually obtained from the converter.
    pub converter_outputs: AtomicU64,
    /// NV12 images actually handed to the encoder.
    pub encoder_inputs: AtomicU64,
    /// Access units actually obtained from the encoder.
    pub encoder_outputs: AtomicU64,
    /// `METransformNeedInput` events received since start-up.
    pub need_input_events: AtomicU64,
    /// `METransformHaveOutput` events received since start-up.
    pub have_output_events: AtomicU64,
    /// Current length of `pending_nv12`.
    pub queued_nv12: AtomicU64,
    /// Current value of `pending_input_requests`.
    pub pending_input_requests: AtomicU64,
    /// Occurrences de `MF_E_SAMPLEALLOCATOR_EMPTY`.
    pub skipped_busy: AtomicU64,
    /// 1 if a converter output remains to be removed.
    pub awaiting_drain: AtomicU64,
    /// Last `GetInputStatus` of the converter: bit 0 = `ACCEPT_DATA`,
    /// `u64::MAX` if the method is not implemented by this MFT.
    pub converter_input_status: AtomicU64,
    /// Last `GetOutputStatus` of the converter: bit 0 = `SAMPLE_READY`,
    /// `u64::MAX` if the method is not implemented by this MFT.
    pub converter_output_status: AtomicU64,
    /// Inputs refused by the converter (`MF_E_NOTACCEPTING`).
    pub converter_not_accepting: AtomicU64,
    /// NV12 samples converted then discarded because a more
    /// recent image was available before the encoder claimed them (see
    /// `MAX_PENDING_NV12`). Expected close to zero in the nominal regime: a
    /// rising value means that the encoder no longer keeps up with the capture.
    pub dropped_stale_nv12: AtomicU64,
}

/// Process-wide diagnostic counters, doubling two fields of
/// `EncoderTelemetry` (see `SOURCE_TRACE` in `demarrage.rs`).
///
/// Seemingly redundant, but `EncoderTelemetry` is owned by
/// the encoder, which `resize` replaces: a handle taken at start-up stops
/// being fed as of the first rebuild — that is as of the
/// first resize requested by the browser, hence in all
/// real sessions. These statics survive rebuilds.
pub static NEED_INPUT_EVENTS: AtomicU64 = AtomicU64::new(0);
pub static ENCODER_INPUTS: AtomicU64 = AtomicU64::new(0);
pub static DROPPED_STALE: AtomicU64 = AtomicU64::new(0);

/// Cumulative time (ns) in the three Media Foundation calls of the hot path,
/// to decide between the BGRA→NV12 converter and the encoder itself.
///
/// `submit` encompasses conversion AND submission; without this breakdown,
/// a slow `submit` does not say which of the two MFTs costs.
pub static CONVERT_NS: AtomicU64 = AtomicU64::new(0);
pub static ENC_IN_NS: AtomicU64 = AtomicU64::new(0);
pub static ENC_OUT_NS: AtomicU64 = AtomicU64::new(0);

/// Material balance of the BGRA→NV12 converter, process-wide.
///
/// Without these three, the image count does not add up: the trace showed
/// 68.5 images captured per second for 47.5 handed to the encoder and 12.5
/// declared stale, that is 8.5 vanished without trace. They are lost here —
/// a refused input (`converter_not_accepting`) or an uncollected output
/// (empty pool, `ConverterPoll::Busy`) makes `mft::convertisseur::feed_converter` exit
/// without having
/// stacked anything in `pending_nv12`, and the image is never offered again.
pub static CONVERTER_INPUTS: AtomicU64 = AtomicU64::new(0);
pub static CONVERTER_OUTPUTS: AtomicU64 = AtomicU64::new(0);
pub static CONVERTER_SKIPPED: AtomicU64 = AtomicU64::new(0);

/// Starts Media Foundation, ONLY ONCE for the life of the process, and
/// NEVER stops it.
///
/// **What this replaces.** A RAII guard paired `MFStartup` and
/// `MFShutdown` over the life of each `H264Encoder`: destroying the last
/// encoder therefore tore down the whole Media Foundation platform. It is
/// the instant where task 2bis had noted the fault — main thread in
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown` while a
/// work item of the NVIDIA MFT was still running.
///
/// **`MFShutdown` is nevertheless NOT necessary for the fault, and it is measured
/// here**: with this call entirely removed from the path, the fault came back,
/// identical stack, identical offset, and this time AFTER the complete
/// release of the encoder (1 recurrence out of 5 runs).
///
/// Exact scope of this survey, not to be exceeded: it establishes that the fault
/// **can occur without** `MFShutdown`, hence that this call is not a
/// necessary condition for it. It does NOT establish that its presence in the two
/// dumps of 2bis was inert — two paths leading to the same symptom can
/// coexist. The barrier of `arret::FileMft` is what handles the race; this
/// change does not handle it.
///
/// **Why keep it all the same.** No process gains anything from stopping
/// Media Foundation: `MFShutdown` only serves to give back resources just
/// before dying, and the system reclaims them anyway at the end of the
/// process. In return, no encoder destruction tears down and
/// then brings back up the platform anymore — which matters for the upcoming work stream, where
/// closing a window will destroy its encoder while others keep
/// encoding.
fn start_media_foundation() -> Result<()> {
    static DEMARRAGE: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    match DEMARRAGE.get_or_init(|| {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }.map_err(|err| err.to_string())
    }) {
        Ok(()) => Ok(()),
        // A failure is remembered: retrying it at each encoder would only
        // repeat the same failure, and `MFStartup` is not a call to retry.
        Err(message) => bail!("Media Foundation startup: {message}"),
    }
}

/// **The facade.** An H.264 encoder, whatever back end runs it.
///
/// 🔴 **THE EIGHT VERBS HAVE NOT CHANGED SIGNATURE**, and it is the
/// constraint that governed this design: the repository's ~10 callers
/// have not moved by one line.
///
/// ## The order of back ends, and WHY — with what to redo it
///
/// 1. **Native NVENC** when an NVIDIA adapter is present.
/// 2. **The MFT**, unchanged, for everything else.
///
/// 🔴 **THE MFT IS NOT A LAST RESORT, IT IS THE GENERIC BACK END.** `MFTEnumEx`
/// does not enumerate "the NVIDIA encoder": it enumerates **the hardware H.264
/// encoders**, Intel Quick Sync and AMD VCE included. A machine without NVIDIA
/// has no NVENC; taking it away would deprive it of **any** hardware
/// encoder. Three host tests pin this regression
/// (`encode_nvenc::tests`).
///
/// **Why NVENC first**: on the target VM, on 30 August 2026, the
/// `NVIDIA H.264 Encoder MFT` activates in session 0 and returns `0x8000FFFF` in
/// **session 1** — the one where the product runs — on the four arrangements
/// Media Foundation allows. Two green controls set up in the same
/// run (**software** H.264 encoder, **software** video processor)
/// establish that the machinery is not at fault. Apollo, on the same
/// machine and in the same session, builds six encoders through the native
/// door.
///
/// **Redo the measurement**, rather than taking my word for it:
///
/// ```text
/// (Get-Process sunshine).Modules | ? { $_.ModuleName -match 'mfplat|nvEnc' }
/// Select-String 'NvEnc: created encoder' 'C:\Program Files\Apollo\config\sunshine.log'
/// ```
///
/// Detail, raw surveys and **three remedies refuted by measurement**:
/// `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`. (policy: allow-fr, real file path)
pub enum H264Encoder {
    /// L'API NVENC native — la porte qu'Apollo emprunte.
    ///
    /// Boxed: the session holds the NVENC configuration inline (several KiB),
    /// which would make every `H264Encoder` that large.
    Natif(Box<natif::EncodeurNatif>),
    /// The Media Foundation MFT — the **generic** back end.
    Mft(mft::EncodeurMft),
}

impl H264Encoder {
    pub fn new(
        device: &ID3D11Device,
        capture: (u32, u32),
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        let adaptateurs = fabrique::adaptateurs_dxgi();
        if let crate::encode_nvenc::Voie::Nvenc(index) =
            crate::encode_nvenc::choisir_voie(&adaptateurs)
        {
            let vu = &adaptateurs[index];
            match natif::EncodeurNatif::new(device, encode, fps, bitrate) {
                Ok(encodeur) => {
                    tracing::info!(
                        adaptateur = %vu.nom,
                        luid = format!("{:08X}:{:08X}", vu.luid.0, vu.luid.1),
                        "encodeur NVENC natif retenu"
                    );
                    return Ok(Self::Natif(Box::new(encodeur)));
                }
                // 🔴 **The fallback is NOISY, on purpose.** Falling back silently
                // to the MFT would make whoever sees a session establish itself read "NVENC works",
                // while the back end running is the other one.
                Err(error) => tracing::warn!(
                    %error,
                    adaptateur = %vu.nom,
                    "native NVENC unavailable: falling back to the Media Foundation MFT"
                ),
            }
        }
        Ok(Self::Mft(mft::EncodeurMft::new(
            device, capture, encode, fps, bitrate,
        )?))
    }

    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        match self {
            Self::Natif(e) => e.telemetry(),
            Self::Mft(e) => e.telemetry(),
        }
    }

    pub fn eprouver_file(&self, duree: std::time::Duration) {
        match self {
            Self::Natif(e) => e.eprouver_file(duree),
            Self::Mft(e) => e.eprouver_file(duree),
        }
    }

    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        match self {
            Self::Natif(e) => e.submit(frame, pts_90k),
            Self::Mft(e) => e.submit(frame, pts_90k),
        }
    }

    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        match self {
            Self::Natif(e) => e.poll_output(),
            Self::Mft(e) => e.poll_output(),
        }
    }

    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        match self {
            Self::Natif(e) => e.flush_pending_inputs(),
            Self::Mft(e) => e.flush_pending_inputs(),
        }
    }

    pub fn request_keyframe(&mut self) -> Result<()> {
        match self {
            Self::Natif(e) => e.request_keyframe(),
            Self::Mft(e) => e.request_keyframe(),
        }
    }

    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        match self {
            Self::Natif(e) => e.set_bitrate(bitrate),
            Self::Mft(e) => e.set_bitrate(bitrate),
        }
    }

    pub fn encode_size(&self) -> (u32, u32) {
        match self {
            Self::Natif(e) => e.encode_size(),
            Self::Mft(e) => e.encode_size(),
        }
    }
}
