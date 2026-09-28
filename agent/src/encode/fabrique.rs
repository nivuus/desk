//! What FINDS and ACTIVATES the MFTs: the hardware H.264 encoder, the
//! colour converter, the DXGI device manager shared with them,
//! and the fallback NV12 sample.
//!
//! Extracted from `encode.rs` on 30 August 2026 (batch 31), **before** adding anything
//! to it: the file weighed 1536 lines, three times the ceiling of 500,
//! and the repository's doctrine is to extract in a DEDICATED task before the one
//! that adds — never to compress. The counterpart of this module is
//! `encode::reglages`, which SETS the settings on an MFT once obtained.
//!
//! ⚠️ **THIS IS NO LONGER THE DEFAULT PATH ON THE TARGET VM, SINCE
//! 30 AUGUST 2026 (batch 31)** — the sentence that opened this paragraph said
//! "this is where the product fails today", and it became
//! FALSE the day native NVENC moved ahead: `H264Encoder::new`
//! first tries the native door when an NVIDIA adapter is present, and
//! **it succeeds** (1195 access units measured). This MFT remains the
//! **generic** back end — Intel Quick Sync, AMD VCE, and session 0 where it
//! works — and what follows describes what it does *when we get to it*.
//!
//! 🔴 **WHAT REMAINS TRUE, AND EXPLAINS WHY IT IS NO LONGER
//! FIRST.** `find_hardware_encoder` enumerates a single MFT — `NVIDIA H.264 Encoder
//! MFT` — and its `ActivateObject` returns `0x8000FFFF` ("Catastrophic
//! failure") in **session 1**, whereas the same call succeeds in
//! **session 0**, in the same binary and in the same minute. Measured on
//! 30 August 2026, with two green controls set up alongside (the
//! SOFTWARE H.264 encoder and the SOFTWARE video processor do activate in both
//! sessions): the Media Foundation machinery works, only the NVIDIA hardware
//! MFT refuses. Three cheap remedies are **refuted by
//! measurement** — setting `MFT_ENUM_ADAPTER_LUID`, holding a live NVIDIA D3D11
//! device before activation, and binding the virtual display to the NVIDIA
//! GPU (already true here). See
//! `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`. (policy: allow-fr, real file path)
//!
//! ⚠️ **AND `find_hardware_video_processor` FINDS NOTHING ON THIS
//! MACHINE, IN BOTH SESSIONS** (same measurement): `create_color_converter`
//! therefore **always** falls back on its `CoCreateInstance`, that is on the
//! **software** `Microsoft Video Processor MFT`. The BGRA → NV12 conversion
//! goes through the CPU in production, and the header comment of `encode.rs`
//! that says it happens "without leaving the GPU" is wrong here. The fallback logs it,
//! but at `debug!`, a level production does not emit.
//!
//! **Pure move: no call, no order, no value has changed.**
//! The only differences from the original text are the visibilities
//! (`pub(super)` for the five functions having a caller outside this
//! file; `format_subtype` and `find_hardware_video_processor` stay
//! private, no one else calls them), the qualification of
//! `pack_u64` — which lives in the sibling `reglages` — and the imports, which never
//! follow on their own.

use anyhow::{anyhow, bail, Context, Result};
use windows::core::{Interface, GUID, PWSTR};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};

use super::reglages;
use crate::encode_nvenc;

/// Logs the input types actually advertised by the encoder, before
/// any configuration. Serves as empirical proof on the BGRA/NV12 question
/// (see the module comment of `encode.rs`, NOT that of this
/// file): on the target VM, only NV12 appears.
pub(super) fn log_supported_input_types(transform: &IMFTransform) {
    let mut index = 0u32;
    loop {
        let media_type = match unsafe { transform.GetInputAvailableType(0, index) } {
            Ok(t) => t,
            Err(_) => break, // MF_E_NO_MORE_TYPES: end of the enumeration.
        };
        let subtype = unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) };
        match subtype {
            Ok(guid) => tracing::info!(
                index,
                subtype = %format_subtype(guid),
                "input type announced by the encoder"
            ),
            Err(_) => tracing::info!(index, "input type announced (unreadable subtype)"),
        }
        index += 1;
    }
}

/// Translates the most common video subtype GUIDs into readable text,
/// for the logs. Unrelated to the conversion logic itself.
fn format_subtype(guid: GUID) -> String {
    if guid == MFVideoFormat_NV12 {
        "NV12".to_string()
    } else if guid == MFVideoFormat_ARGB32 {
        "ARGB32 (BGRA)".to_string()
    } else if guid == MFVideoFormat_RGB32 {
        "RGB32 (BGRX)".to_string()
    } else if guid == MFVideoFormat_YUY2 {
        "YUY2".to_string()
    } else if guid == MFVideoFormat_YV12 {
        "YV12".to_string()
    } else if guid == MFVideoFormat_IYUV {
        "IYUV".to_string()
    } else {
        format!("{guid:?}")
    }
}

/// Creates the BGRA→NV12 GPU converter (Media Foundation's Video Processor MFT,
/// `CLSID_VideoProcessorMFT`). Unlike the encoder, this
/// MFT is synchronous: no events to follow, `ProcessInput` followed by
/// `ProcessOutput` is enough.
pub(super) fn create_color_converter(
    device_manager: &IMFDXGIDeviceManager,
    capture: (u32, u32),
    encode: (u32, u32),
    fps: u32,
) -> Result<IMFTransform> {
    // Attempt (fix round 1/5, throughput investigation):
    // `CoCreateInstance(CLSID_VideoProcessorMFT)` instantiates the default
    // implementation of this CLSID, which could be a software/mixed path
    // rather than a hardware implementation. We first try to find
    // a converter explicitly registered as hardware through
    // `MFTEnumEx`, as for the encoder — falling back on `CoCreateInstance` if
    // nothing is found.
    let converter: IMFTransform = match find_hardware_video_processor() {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!(error = %e, "no hardware video converter enumerated, falling back to CLSID_VideoProcessorMFT");
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER) }
                .context("creating the video converter (Video Processor MFT)")?
        }
    };

    // Attempt: low-latency mode was only applied to the encoder, not to the
    // converter — potentially linked to the ~1 s wait observed in
    // `mft::convertisseur::drain_converter_output` (see its comment).
    // `GetAttributes` may
    // fail if the converter does not expose modifiable attributes; in
    // that case we continue without blocking the construction.
    if let Ok(converter_attributes) = unsafe { converter.GetAttributes() } {
        let _ = unsafe { converter_attributes.SetUINT32(&MF_LOW_LATENCY, 1) };
    }

    unsafe {
        converter.ProcessMessage(
            MFT_MESSAGE_SET_D3D_MANAGER,
            device_manager.as_raw() as usize,
        )
    }
    .context("sharing the D3D device with the converter")?;

    // Trap met during the attempt: with the default pool, the documented
    // sequence "ProcessOutput until MF_E_TRANSFORM_NEED_MORE_INPUT"
    // fails with `MF_E_SAMPLEALLOCATOR_EMPTY` (0xC00D4A3E) after exactly 5
    // images — the hardware encoder keeps several "in flight" before
    // releasing any, and the default pool does not have that margin.
    //
    // First fix attempted, insufficient: `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`
    // alone, with the values 4 then 16 — in both cases, failure at exactly
    // the same 5th call, proof that this attribute alone has no effect here.
    // Cause: our stream is **progressive**
    // (`MF_MT_INTERLACE_MODE` = `MFVideoInterlace_Progressive`), and this MFT
    // apparently distinguishes two pool size attributes — one for
    // interlaced content, one for progressive
    // (`MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE`) — only this second
    // attribute is honoured for progressive content. We set both
    // out of caution (MF documentation ambiguous on this point).
    let output_stream_attributes = unsafe { converter.GetOutputStreamAttributes(0) }
        .context("converter output stream attributes")?;
    unsafe {
        output_stream_attributes.SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT, 16)?;
        output_stream_attributes.SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE, 16)?;
    }

    let input_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        input_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        // Input format = what the capture produces (BGRA, with alpha);
        // `MFVideoFormat_ARGB32` corresponds to `DXGI_FORMAT_B8G8R8A8_UNORM`.
        input_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)?;
        input_type.SetUINT64(&MF_MT_FRAME_SIZE, reglages::pack_u64(capture.0, capture.1))?;
        input_type.SetUINT64(&MF_MT_FRAME_RATE, reglages::pack_u64(fps, 1))?;
        input_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter
            .SetInputType(0, &input_type, 0)
            .context("configuring the colour converter input type (Video Processor MFT)")?;
    }

    let output_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        output_type.SetUINT64(&MF_MT_FRAME_SIZE, reglages::pack_u64(encode.0, encode.1))?;
        output_type.SetUINT64(&MF_MT_FRAME_RATE, reglages::pack_u64(fps, 1))?;
        output_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter
            .SetOutputType(0, &output_type, 0)
            .context("configuring the colour converter output type (Video Processor MFT)")?;
    }

    Ok(converter)
}

/// Enumerates the video converters (BGRA→NV12) explicitly registered
/// as hardware, and activates the first — same logic as
/// `find_hardware_encoder`, with the same memory release
/// precautions (see its comment).
fn find_hardware_video_processor() -> Result<IMFTransform> {
    let input_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_ARGB32,
    };
    let output_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;

    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_PROCESSOR,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input_info),
            Some(&output_info),
            &mut activates,
            &mut count,
        )
        .context("enumerating the hardware video converters")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!("no hardware video converter registered");
    }

    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
    }
    let first = first.ok_or_else(|| anyhow!("converter activator missing"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    if unsafe {
        first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len)
    }
    .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(convertisseur = %name, "hardware video converter retained");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let transform: IMFTransform = unsafe { first.ActivateObject() }
        .context("activating the hardware video converter (ActivateObject)")?;
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    Ok(transform)
}

/// Allocates a GPU NV12 texture and wraps it in a reusable Media
/// Foundation sample, for the cases where the converter does not self-allocate
/// (`MFT_OUTPUT_STREAM_PROVIDES_SAMPLES` absent — see
/// `super::H264Encoder::new`).
pub(super) fn create_nv12_sample(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IMFSample> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("allocating the intermediate NV12 texture")?;
    let texture = texture.ok_or_else(|| anyhow!("texture NV12 absente"))?;

    let sample = unsafe { MFCreateSample() }?;
    let buffer = unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &texture, 0, false) }
        .context("wrapping the NV12 texture")?;
    unsafe { sample.AddBuffer(&buffer) }?;
    Ok(sample)
}

/// Enumerates the hardware H.264 encoders and activates the first.
pub(super) fn find_hardware_encoder() -> Result<IMFTransform> {
    let input_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let output_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;

    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input_info),
            Some(&output_info),
            &mut activates,
            &mut count,
        )
        .context("enumerating the hardware H.264 encoders")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!(
            "no hardware H.264 encoder found on this machine. \
             Check the GPU driver; milestone 1 has no software fallback."
        );
    }

    // Retrieve the objects BEFORE freeing the array allocated by CoTaskMemAlloc.
    //
    // Fix round 1/5 — leak fixed here: the previous version
    // only released the first `IMFActivate` (through `.clone()`, which adds
    // a reference without ever releasing the one `MFTEnumEx` placed in
    // the array slot). `CoTaskMemFree` only frees the raw memory of the
    // array, not the COM references it contains: each entry,
    // including the first, therefore leaked a reference. `slot.take()` moves
    // each entry out of the array (replaced by `None`); the entries
    // we do not keep are dropped immediately (hence released), the
    // first is kept in `first` without an extra reference.
    // Latent as long as a single encoder is present, but real as soon as there
    // would be several.
    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
        // Otherwise: `activate` is dropped here, releasing its COM reference.
    }
    let first = first.ok_or_else(|| anyhow!("activateur d'encodeur absent"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    // windows-rs 0.62 API gap: `GetStringAlloc` does not exist on
    // `IMFAttributes` in this version; the method is called
    // `GetAllocatedString` (memory allocated by `CoTaskMemAlloc`, to be freed
    // explicitly after use).
    if unsafe {
        first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len)
    }
    .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(encodeur = %name, "hardware encoder retained");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let active = unsafe { first.ActivateObject::<IMFTransform>() };
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    match active {
        Ok(transform) => Ok(transform),
        Err(error) => Err(anyhow!(
            "{}",
            // The COMPOSITION of the message is pure and lives in
            // `encode_nvenc`, where it is tested on the host: here we only
            // give it the code and what the machine carries.
            encode_nvenc::diagnostic_activation(
                error.code().0,
                &error.to_string(),
                &adaptateurs_dxgi()
            )
        )),
    }
}

/// The DXGI adapters, reduced to what the PURE rule of
/// `crate::encode_nvenc` can read.
///
/// ⚠️ **Returns an EMPTY list rather than an error**: this function only serves
/// to enrich a diagnostic and to choose a path. Making it fail
/// would replace a useful message with another error message, and would mask
/// the very failure we are trying to describe.
pub(super) fn adaptateurs_dxgi() -> Vec<encode_nvenc::Adaptateur> {
    let Ok(fabrique) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return Vec::new();
    };
    let mut vus = Vec::new();
    let mut index = 0u32;
    while let Ok(adaptateur) = unsafe { fabrique.EnumAdapters1(index) } {
        if let Ok(desc) = unsafe { adaptateur.GetDesc1() } {
            vus.push(encode_nvenc::Adaptateur {
                nom: String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\0')
                    .to_string(),
                vendeur: desc.VendorId,
                luid: (desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart),
            });
        }
        index += 1;
    }
    vus
}

pub(super) fn share_device(device: &ID3D11Device) -> Result<IMFDXGIDeviceManager> {
    let mut token = 0u32;
    let mut manager: Option<IMFDXGIDeviceManager> = None;
    unsafe {
        MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
    }
    let manager = manager.ok_or_else(|| anyhow!("gestionnaire DXGI absent"))?;
    unsafe { manager.ResetDevice(device, token) }
        .context("binding the D3D11 device to the DXGI manager")?;
    Ok(manager)
}
