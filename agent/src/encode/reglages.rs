//! The settings we SET on an MFT: input and output media
//! types, rate control, and the two `VARIANT` factories that
//! rate control requires.
//!
//! Extracted from `encode.rs` on 30 August 2026 (batch 31), **before** adding anything
//! to it: the file weighed 1536 lines, three times the ceiling of 500,
//! and the repository's doctrine is to extract in a DEDICATED task before the one
//! that adds — never to compress. The counterpart of this module is
//! `encode::fabrique`, which FINDS and ACTIVATES the MFTs; here we only
//! tune them once obtained.
//!
//! **Pure move: no call, no order, no value has changed.**
//! The only differences from the original text are the visibilities
//! (`pub(super)`, the six functions all having a caller outside this
//! file) and the imports, which never follow on their own.

use anyhow::{Context, Result};
use windows::core::Interface;
// windows-rs 0.62 API gap, moved here with its import from
// `encode.rs` (batch 31): `VARIANT_TRUE`/`VARIANT_FALSE` live in
// `Win32::Foundation` (`VARIANT_BOOL` constants), not in
// `Win32::System::Variant` where one would expect them next to the rest of the
// `VARIANT` type — confirmed by reading the crate's sources on the VM.
use windows::Win32::Foundation::{VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4,
};

/// Builds a `VT_UI4` `VARIANT` manually: this version of
/// windows-rs provides no `From<u32>` for `VARIANT` (gap from the brief,
/// checked by reading the crate's sources on the VM — no `impl From<`
/// exists for this type in `Win32::System::Variant`).
pub(super) fn variant_u32(value: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: value },
            }),
        },
    }
}

/// Builds a `VT_BOOL` `VARIANT` manually (same reason as
/// `variant_u32`).
pub(super) fn variant_bool(value: bool) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    boolVal: if value { VARIANT_TRUE } else { VARIANT_FALSE },
                },
            }),
        },
    }
}

pub(super) fn configure_output(
    transform: &IMFTransform,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        media_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_u64(1, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        // Baseline avoids B frames: decoding order = display order.
        media_type.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
        transform
            .SetOutputType(0, &media_type, 0)
            .context("configuration du type de sortie de l'encodeur H.264 (transform matériel)")?;
    }
    Ok(())
}

pub(super) fn configure_input(
    transform: &IMFTransform,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        transform
            .SetInputType(0, &media_type, 0)
            .context("configuration du type d'entrée de l'encodeur H.264 (transform matériel)")?;
    }
    Ok(())
}

pub(super) fn configure_rate_control(transform: &IMFTransform, bitrate: u32) -> Result<()> {
    let codec: ICodecAPI = transform.cast()?;
    unsafe {
        // Constant bitrate: predictable latency, indispensable for interactive use.
        let mode = variant_u32(eAVEncCommonRateControlMode_CBR.0 as u32);
        codec.SetValue(&CODECAPI_AVEncCommonRateControlMode, &mode)?;
        let rate = variant_u32(bitrate);
        codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate)?;
        // No closed group of pictures: key frames are requested on the
        // fly (`H264Encoder::request_keyframe`, wired from
        // str0m's `Event::KeyframeRequest` in `transport/evenements.rs`). The return
        // of `SetValue` is checked rather than thrown away: a silent refusal by the
        // driver would suggest the contract is honoured while a closed group
        // of pictures would make on-demand key frames inoperative.
        let gop = variant_u32(0);
        if let Err(e) = codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &gop) {
            tracing::warn!(
                erreur = %e,
                "réglage CODECAPI_AVEncMPVGOPSize (groupe d'images ouvert) refusé par le pilote"
            );
        }
        let low_latency = variant_bool(true);
        let _ = codec.SetValue(&CODECAPI_AVLowLatencyMode, &low_latency);
    }
    Ok(())
}

/// Packs two 32-bit integers into the 64-bit attribute MF expects.
pub(super) fn pack_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}
