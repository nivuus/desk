//! The **native NVENC** back end behind the `H264Encoder` facade.
//!
//! This file does not talk to NVENC: it translates. On one side the eight verbs
//! the repository has always consumed (`submit`, `poll_output`, …), on
//! the other `encode_nvenc::session::SessionNvenc`. Everything touching the ABI
//! lives under `encode_nvenc`, and **nothing of the licence notice spills
//! over here**.
//!
//! 🟢 **THIS PATH HAS ENCODED**, measured on the VM on 30 August 2026: **1195
//! access units in 10 s** on the real capture device, carried by
//! the NVIDIA adapter — where the MFT returned `0x8000FFFF` and **no**
//! unit. (§ 11.1 of the results document.)
//!
//! 🟢 **AND ITS IMAGES REACH THE BROWSER** — `framesDecoded` **+494** and
//! **+484** over 25 s, two runs, versus **0** on a static source
//! whose audio was nevertheless flowing in the same survey.
//! 🔴 **THIS MEASUREMENT IS THAT OF BATCH 32, NOT OF BATCH 31**: it was played
//! with TWO remedies in place — this native path, and the output designation
//! of the neighbouring batch. It establishes that this path produces images that
//! get through; it is not to be credited to this file alone.
//!
//! ⚠️ **WHAT IS STILL NOT ESTABLISHED**, and must not be read into
//! the lines above: the ceiling at **N windows** is not measured — the
//! bench only opens one encoder — and **no one has looked at an image**.
//! `framesDecoded` counts decoded images, it says **nothing** about the
//! correctness of what is displayed.
//!
//! ## Two fundamental differences from the MFT path, and why they are safe
//!
//! 1. **No converter.** NVENC accepts `NV_ENC_BUFFER_FORMAT_ARGB`,
//!    that is exactly what DXGI duplication returns
//!    (`DXGI_FORMAT_B8G8R8A8_UNORM`). The BGRA→NV12 stage disappears from the hot
//!    path. ⚠️ **It is not a measured gain**: no one put a figure on what
//!    this stage cost, and batch 31 only established that it is
//!    **software** on this machine, for lack of a hardware video processing
//!    MFT. The gain is **plausible, not measured**, and must not be
//!    announced otherwise.
//! 2. **No event queue.** The session is opened in synchronous
//!    mode: a submitted image returns its output right away, or signals
//!    that it was buffered. `submit` therefore stores the output, and
//!    `poll_output` returns it — which fulfils the facade's contract without
//!    any caller changing.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::capture::CapturedFrame;
use crate::encode::{EncoderTelemetry, PHASE_IDLE};
use crate::encode_nvenc::session::SessionNvenc;
use crate::h264::{group_access_units, AccessUnit};

/// How many access units we keep between `submit` and `poll_output`.
///
/// ⚠️ **Only one**, like the MFT path's `MAX_PENDING_NV12` and for the same
/// reason: serving a stale image is paying in latency for the throughput just
/// gained. Overflow is **counted**, not silent.
const MAX_UNITES_EN_ATTENTE: usize = 1;

pub struct EncodeurNatif {
    session: SessionNvenc,
    en_attente: VecDeque<AccessUnit>,
    telemetry: Arc<EncoderTelemetry>,
    fps: u32,
}

impl EncodeurNatif {
    pub fn new(
        peripherique: &ID3D11Device,
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        let session = SessionNvenc::ouvrir(peripherique, encode.0, encode.1, fps, bitrate)?;
        Ok(Self {
            session,
            en_attente: VecDeque::new(),
            telemetry: Arc::new(EncoderTelemetry::default()),
            fps: fps.max(1),
        })
    }

    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        Arc::clone(&self.telemetry)
    }

    pub fn encode_size(&self) -> (u32, u32) {
        self.session.size()
    }

    pub fn request_keyframe(&mut self) -> Result<()> {
        self.session.demander_image_cle();
        Ok(())
    }

    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        if bitrate == self.session.debit() {
            return Ok(());
        }
        self.session.regler_debit(bitrate)
    }

    /// ⚠️ **Not applicable here, and it is NOT an oversight.** This verb exists for
    /// the asynchronous MFT, whose inputs wait for a
    /// `METransformNeedInput` event. Synchronous mode has nothing waiting: a
    /// submitted image is encoded at the call. Returning `Ok(())` is therefore the
    /// RIGHT behaviour, not a complacent silence.
    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        Ok(())
    }

    /// ⚠️ **Not applicable here either**: this verb clogs the work queue
    /// the asynchronous MFT imposes, and synchronous mode has none.
    pub fn eprouver_file(&self, _duree: std::time::Duration) {}

    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        self.telemetry.submit_calls.fetch_add(1, Ordering::Relaxed);
        // Media Foundation counted in 100 ns; NVENC takes what it is
        // given and returns it as is. We keep the facade's unit — 1/90000 s
        // — so that `poll_output` has nothing to convert back.
        let octets = self.session.encoder(&frame.texture, pts_90k)?;
        self.telemetry
            .encoder_inputs
            .fetch_add(1, Ordering::Relaxed);
        crate::encode::ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);

        let Some(octets) = octets else {
            // The encoder buffered the image: normal case, not a loss.
            return Ok(());
        };
        let mut unites = group_access_units(&octets, self.fps);
        if unites.is_empty() {
            return Ok(());
        }
        let mut unite = unites.remove(0);
        unite.pts_90k = pts_90k;
        self.en_attente.push_back(unite);
        self.telemetry
            .encoder_outputs
            .fetch_add(1, Ordering::Relaxed);

        // Keep only the most recent, and COUNT what we discard: a
        // silent rejection would make whoever looks at the counters read "the encoder keeps up",
        // while it is falling behind.
        while self.en_attente.len() > MAX_UNITES_EN_ATTENTE {
            self.en_attente.pop_front();
            self.telemetry
                .dropped_stale_nv12
                .fetch_add(1, Ordering::Relaxed);
            crate::encode::DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        Ok(self.en_attente.pop_front())
    }
}
