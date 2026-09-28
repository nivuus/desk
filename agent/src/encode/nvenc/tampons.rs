//! The NVENC structure layouts exchanged **per image**:
//! registering a texture, mapping it, obtaining a stream buffer, encoding,
//! reading the stream back.
//!
//! 🔴 **LICENCE NOTICE, PROVENANCE AND REREAD COMMAND: see
//! `super::abi`.** This file extends the same transcription and falls under the
//! same notice. Its counterpart is `super::structures`, which carries what is
//! configured **once** per session; the split follows that boundary,
//! and not the 500-line ceiling (which it incidentally serves).
//!
//! Same transcription conventions and same assertions as
//! `super::structures` — **including what these assertions do not have to
//! catch**, a case measured and explained there.

#![allow(dead_code)]

use core::ffi::c_void;

macro_rules! forme {
    ($t:ty, $size:expr, $alignment:expr, $name:literal) => {
        const _: () = assert!(
            core::mem::size_of::<$t>() == $size,
            concat!("wrong ABI size for ", $name)
        );
        const _: () = assert!(
            core::mem::align_of::<$t>() == $alignment,
            concat!("wrong ABI alignment for ", $name)
        );
    };
}

macro_rules! deport {
    ($t:ty, $field:ident, $value:expr, $name:literal) => {
        const _: () = assert!(
            core::mem::offset_of!($t, $field) == $value,
            concat!("wrong ABI offset for ", $name)
        );
    };
}

/// `NV_ENC_REGISTER_RESOURCE` — declares a D3D11 texture to the encoder.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RegisterResource {
    pub version: u32,
    pub resource_type: u32,
    pub width: u32,
    pub height: u32,
    /// ⚠️ **`0` for a D3D11 texture**: the pitch is the texture's, and
    /// it is the driver that knows it.
    pub pitch: u32,
    pub sub_resource_index: u32,
    /// L'`ID3D11Texture2D*`.
    pub resource_to_register: *mut c_void,
    /// **[out]** the registered resource.
    pub registered_resource: *mut c_void,
    pub buffer_format: u32,
    pub buffer_usage: u32,
    /// ⚠️ **`null` in D3D11** — this synchronisation point is a D3D12 object.
    pub p_input_fence_point: *mut c_void,
    pub chroma_offset: [u32; 2],
    pub reserved1: [u32; 246],
    pub reserved2: [*mut c_void; 61],
}
forme!(RegisterResource, 1536, 8, "NV_ENC_REGISTER_RESOURCE");
deport!(
    RegisterResource,
    resource_to_register,
    24,
    "NV_ENC_REGISTER_RESOURCE.resourceToRegister"
);
deport!(
    RegisterResource,
    chroma_offset,
    56,
    "NV_ENC_REGISTER_RESOURCE.chromaOffset"
);
deport!(
    RegisterResource,
    reserved2,
    1048,
    "NV_ENC_REGISTER_RESOURCE.reserved2"
);

/// `NV_ENC_MAP_INPUT_RESOURCE` — maps a registered resource for an
/// image, and returns the input pointer `PicParams` expects.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MapInputResource {
    pub version: u32,
    /// ⚠️ **Deprecated** by the header; left at zero.
    pub sub_resource_index: u32,
    /// ⚠️ **Deprecated** by the header; left null.
    pub input_resource: *mut c_void,
    pub registered_resource: *mut c_void,
    /// **[out]** what is passed to `PicParams::input_buffer`.
    pub mapped_resource: *mut c_void,
    /// **[sortie]** le format que l'encodeur a retenu.
    pub mapped_buffer_fmt: u32,
    pub reserved1: [u32; 251],
    pub reserved2: [*mut c_void; 63],
}
forme!(MapInputResource, 1544, 8, "NV_ENC_MAP_INPUT_RESOURCE");
deport!(
    MapInputResource,
    registered_resource,
    16,
    "NV_ENC_MAP_INPUT_RESOURCE.registeredResource"
);
deport!(
    MapInputResource,
    mapped_resource,
    24,
    "NV_ENC_MAP_INPUT_RESOURCE.mappedResource"
);
deport!(
    MapInputResource,
    reserved2,
    1040,
    "NV_ENC_MAP_INPUT_RESOURCE.reserved2"
);

/// `NV_ENC_CREATE_BITSTREAM_BUFFER` — the buffer where the encoder drops the stream.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CreateBitstreamBuffer {
    pub version: u32,
    /// ⚠️ **Deprecated** — "Do not use", says the header.
    pub size: u32,
    /// ⚠️ **Deprecated** — ditto.
    pub memory_heap: u32,
    pub reserved: u32,
    /// **[out]** the buffer, to be passed to `PicParams::output_bitstream`.
    pub bitstream_buffer: *mut c_void,
    /// **[out]** reserved — the header says not to use it.
    pub bitstream_buffer_ptr: *mut c_void,
    pub reserved1: [u32; 58],
    pub reserved2: [*mut c_void; 64],
}
forme!(
    CreateBitstreamBuffer,
    776,
    8,
    "NV_ENC_CREATE_BITSTREAM_BUFFER"
);
deport!(
    CreateBitstreamBuffer,
    bitstream_buffer,
    16,
    "NV_ENC_CREATE_BITSTREAM_BUFFER.bitstreamBuffer"
);
deport!(
    CreateBitstreamBuffer,
    reserved2,
    264,
    "NV_ENC_CREATE_BITSTREAM_BUFFER.reserved2"
);

/// `NV_ENC_CODEC_PIC_PARAMS`, reduced to its footprint.
///
/// ⚠️ **No member is transcribed, and it is deliberate**: on the current
/// path, **nothing** is written into this union — neither explicit slices,
/// nor SEI, nor long-term references. Transcribing `NV_ENC_PIC_PARAMS_H264` and
/// its fifty fields to write none of them would be fifty opportunities to
/// get it wrong for nothing in return. The day one of them is needed, it gets transcribed
/// **with its measurement**, not before.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CodecPicParams {
    /// 193 × 8 = 1544 bytes. The alignment of 8 is that of the pointers the
    /// real variants carry.
    pub _encombrement: [u64; 193],
}
forme!(CodecPicParams, 1544, 8, "NV_ENC_CODEC_PIC_PARAMS");

/// `NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE`, redeclared here so that this
/// file does not depend on the compilation order of its sibling.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MeHintCounts {
    pub drapeaux: u32,
    pub reserved1: [u32; 3],
}
forme!(
    MeHintCounts,
    16,
    4,
    "NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE (tampons)"
);

/// `NV_ENC_PIC_PARAMS` — an image to encode.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PicParams {
    pub version: u32,
    pub input_width: u32,
    pub input_height: u32,
    pub input_pitch: u32,
    /// Masque de `NV_ENC_PIC_FLAGS` — voir `super::abi::PIC_FLAG_*`.
    pub encode_pic_flags: u32,
    pub frame_idx: u32,
    pub input_time_stamp: u64,
    pub input_duration: u64,
    pub input_buffer: *mut c_void,
    pub output_bitstream: *mut c_void,
    pub completion_event: *mut c_void,
    pub buffer_fmt: u32,
    /// ⚠️ **`PIC_STRUCT_FRAME` is 1, not 0**: zeroing the structure
    /// and forgetting this field is an error, not a harmless default.
    pub picture_struct: u32,
    pub picture_type: u32,
    pub codec_pic_params: CodecPicParams,
    pub me_hint_counts_per_block: [MeHintCounts; 2],
    pub me_external_hints: *mut c_void,
    pub reserved2: [u32; 7],
    pub reserved5: [*mut c_void; 2],
    pub qp_delta_map: *mut i8,
    pub qp_delta_map_size: u32,
    pub reserved_bit_fields: u32,
    pub me_hint_ref_pic_dist: [u16; 2],
    pub reserved4: u32,
    pub alpha_buffer: *mut c_void,
    pub me_external_sb_hints: *mut c_void,
    pub me_sb_hints_count: u32,
    pub state_buffer_idx: u32,
    pub output_recon_buffer: *mut c_void,
    pub reserved3: [u32; 284],
    pub reserved6: [*mut c_void; 57],
}
forme!(PicParams, 3360, 8, "NV_ENC_PIC_PARAMS");
deport!(PicParams, input_buffer, 40, "NV_ENC_PIC_PARAMS.inputBuffer");
deport!(
    PicParams,
    codec_pic_params,
    80,
    "NV_ENC_PIC_PARAMS.codecPicParams"
);
deport!(
    PicParams,
    qp_delta_map,
    1712,
    "NV_ENC_PIC_PARAMS.qpDeltaMap"
);
deport!(PicParams, reserved3, 1768, "NV_ENC_PIC_PARAMS.reserved3");
deport!(PicParams, reserved6, 2904, "NV_ENC_PIC_PARAMS.reserved6");

/// `NV_ENC_LOCK_BITSTREAM` — reads the encoded stream back.
///
/// 🔴 **`reserved_internal` is the LAST member, after `reserved2`.**
/// Dropping it would shorten the structure by 32 bytes, and NVENC would write
/// **beyond our allocation**. It is the offset asserted below that
/// prevents it from going away unnoticed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LockBitstream {
    pub version: u32,
    /// Champ de bits : `doNotWait`(1), `ltrFrame`(1), `getRCStats`(1),
    /// `reservedBitFields`(29).
    pub drapeaux: u32,
    pub output_bitstream: *mut c_void,
    pub slice_offsets: *mut u32,
    pub frame_idx: u32,
    pub hw_encode_status: u32,
    pub num_slices: u32,
    /// **[out]** the useful length of `bitstream_buffer_ptr`.
    pub bitstream_size_in_bytes: u32,
    pub output_time_stamp: u64,
    pub output_duration: u64,
    /// **[out]** the stream's bytes. ⚠️ **Only valid between
    /// `nvEncLockBitstream` and `nvEncUnlockBitstream`**: whatever we keep of them
    /// must be copied before giving back the lock.
    pub bitstream_buffer_ptr: *mut c_void,
    pub picture_type: u32,
    pub picture_struct: u32,
    pub frame_avg_qp: u32,
    pub frame_satd: u32,
    pub ltr_frame_idx: u32,
    pub ltr_frame_bitmap: u32,
    pub temporal_id: u32,
    pub intra_mb_count: u32,
    pub inter_mb_count: u32,
    /// ⚠️ **Signed**.
    pub average_mvx: i32,
    /// ⚠️ **Signed**.
    pub average_mvy: i32,
    pub alpha_layer_size_in_bytes: u32,
    pub output_stats_ptr_size: u32,
    pub reserved: u32,
    pub output_stats_ptr: *mut c_void,
    pub frame_idx_display: u32,
    pub reserved1: [u32; 219],
    pub reserved2: [*mut c_void; 63],
    /// 🔴 **The last member, and the easiest to forget.**
    pub reserved_internal: [u32; 8],
}
forme!(LockBitstream, 1544, 8, "NV_ENC_LOCK_BITSTREAM");
deport!(
    LockBitstream,
    bitstream_size_in_bytes,
    36,
    "NV_ENC_LOCK_BITSTREAM.bitstreamSizeInBytes"
);
deport!(
    LockBitstream,
    bitstream_buffer_ptr,
    56,
    "NV_ENC_LOCK_BITSTREAM.bitstreamBufferPtr"
);
deport!(
    LockBitstream,
    picture_type,
    64,
    "NV_ENC_LOCK_BITSTREAM.pictureType"
);
deport!(
    LockBitstream,
    reserved2,
    1008,
    "NV_ENC_LOCK_BITSTREAM.reserved2"
);
deport!(
    LockBitstream,
    reserved_internal,
    1512,
    "NV_ENC_LOCK_BITSTREAM.reservedInternal"
);
