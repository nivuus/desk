//! The NVENC structure layouts configured **once** per
//! session: opening, initialisation, settings, presets.
//!
//! 🔴 **LICENCE NOTICE, PROVENANCE AND REREAD COMMAND: see
//! `super::abi`.** This file extends the same transcription and falls under the
//! same notice; it is not copied here so that there is **only one**
//! place to keep up to date, but the attribution boundary does encompass this
//! file. Its counterpart is `super::tampons`, which carries what is exchanged
//! **per image**.
//!
//! ## What makes this transcription checkable
//!
//! Each structure carries a **size** and **alignment** assertion
//! evaluated at compile time, and the structures one of whose fields is reached by
//! computation also carry **offset** assertions. The values come
//! from a measurement made **on the target** `x86_64-pc-windows-gnu` by
//! `x86_64-w64-mingw32-gcc` on the real header — not from a host measurement
//! assumed to carry over.
//!
//! 🔴 **Each assertion's message NAMES the structure**, otherwise a
//! red would send one searching.
//!
//! ## What these assertions catch, and what it is USELESS to expect from them
//!
//! 🔴 **A correction I nearly wrote the wrong way round, and which is worth
//! recording as is.** Trying to make this check go red, I
//! reduced `NV_ENC_CONFIG::reserved` from 278 to 277 `u32`: **nothing went red**,
//! neither size nor offset. I first concluded that the check was
//! weak, and I wrote here that it "stayed green on a wrong
//! layout". **That was false, and measured as such** (`equiv.c`, compiled):
//!
//! ```text
//! 278 : size=3584 offset_reserved2=3072
//! 277 : size=3584 offset_reserved2=3072   <-- IDENTICAL
//! 276 : size=3576 offset_reserved2=3064   <-- caught
//! ```
//!
//! The array ends at 3072 and `reserved2` is aligned on 8: the 4 bytes
//! removed are **entirely taken back by padding**, and the resulting layout
//! is **byte for byte the same**. It is therefore not a defect the
//! check lets through — **it is a non-defect**, and staying green is the
//! right answer. The lesson is not "strengthen the check" but **"a
//! red that stays green gets DIAGNOSED, it does not get filed"**.
//!
//! ✅ **What IS caught, and checked**: a missing or moved field in the
//! middle of a structure (`mv_precision` removed ⇒ *"wrong ABI offset for
//! NV_ENC_CONFIG.rcParams"*), and any gap in reserved fields large enough to
//! cross the alignment boundary (276 instead of 278).
//!
//! ⚠️ **The offsets of the trailing reserved fields are asserted anyway**: they
//! cost nothing, and they close the "gap of more than one word" case.
//!
//! ## Three transcription conventions, and why
//!
//! 1. **C enumerations become `u32`s.** They weigh 4 bytes in
//!    this header, and a Rust `enum` with missing variants would be
//!    undefined behaviour as soon as a more recent driver returned an
//!    unknown one.
//! 2. **Bit fields become ONE `u32` and named masks.** Rust
//!    has no bit fields, and above all: the bit allocation order
//!    is not guaranteed by the language. An explicit `u32` puts this order in
//!    our hands, where it is re-readable.
//! 3. **Reserved fields keep their real count.** Sizing them "so that
//!    the size comes out right" would make the size assertion **tautological**,
//!    hence unable to fail — a check that cannot go red is not
//!    one.

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

/// The header's `GUID`. **Transcribed rather than borrowed from `windows-core`**:
/// this module must compile on the Linux host, where that crate does not exist.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}
forme!(Guid, 16, 4, "GUID");

/// `NV_ENC_CODEC_H264_GUID` — `{6BC82762-4E63-4ca4-AA85-1E50F321F6BF}`.
pub const CODEC_H264: Guid = Guid {
    data1: 0x6bc8_2762,
    data2: 0x4e63,
    data3: 0x4ca4,
    data4: [0xaa, 0x85, 0x1e, 0x50, 0xf3, 0x21, 0xf6, 0xbf],
};

/// `NV_ENC_PRESET_P1_GUID` — `{FC0A8D3E-45F8-4CF8-80C7-298871590EBF}`.
///
/// ⚠️ **P1, the fastest.** The header says it: quality goes up and
/// performance goes down from P1 to P7. It is also the preset Apollo
/// uses on this machine — noted in its log:
/// `NvEnc: created encoder H.264 P1 async two-pass rfi`. **It is not a
/// calibration**: nothing in this repository has measured P1 against P4.
pub const PRESET_P1: Guid = Guid {
    data1: 0xfc0a_8d3e,
    data2: 0x45f8,
    data3: 0x4cf8,
    data4: [0x80, 0xc7, 0x29, 0x88, 0x71, 0x59, 0x0e, 0xbf],
};

/// `NV_ENC_H264_PROFILE_BASELINE_GUID` — `{0727BCAA-78C4-4c83-8C2F-EF3DFF267C6A}`.
///
/// ⚠️ **The header writes it `0x727bcaa`, without the leading zero**; the value is
/// indeed `0x0727BCAA`. Several bytes of `data4` are also written short there
/// (`0x3` for `0x03`). C does not care, a hand transcription does.
/// The `{…}` form of the comment is the cross-check.
pub const PROFILE_H264_BASELINE: Guid = Guid {
    data1: 0x0727_bcaa,
    data2: 0x78c4,
    data3: 0x4c83,
    data4: [0x8c, 0x2f, 0xef, 0x3d, 0xff, 0x26, 0x7c, 0x6a],
};

/// `NV_ENC_QP`.
///
/// ⚠️ The header says it itself: these fields are `uint32_t` "for legacy
/// reasons" and must be **treated as signed** when a negative
/// value is intended.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Qp {
    pub inter_p: u32,
    pub inter_b: u32,
    pub intra: u32,
}
forme!(Qp, 12, 4, "NV_ENC_QP");

/// `NV_ENC_RC_PARAMS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RcParams {
    pub version: u32,
    pub rate_control_mode: u32,
    pub const_qp: Qp,
    pub average_bit_rate: u32,
    pub max_bit_rate: u32,
    pub vbv_buffer_size: u32,
    pub vbv_initial_delay: u32,
    /// Champ de bits : `enableMinQP`(1) … `reservedBitFields`(15).
    pub drapeaux: u32,
    pub min_qp: Qp,
    pub max_qp: Qp,
    pub initial_rc_qp: Qp,
    pub temporal_layer_idx_mask: u32,
    pub temporal_layer_qp: [u8; 8],
    pub target_quality: u8,
    pub target_quality_lsb: u8,
    pub lookahead_depth: u16,
    pub low_delay_key_frame_scale: u8,
    pub y_dc_qp_index_offset: i8,
    pub u_dc_qp_index_offset: i8,
    pub v_dc_qp_index_offset: i8,
    pub qp_map_mode: u32,
    pub multi_pass: u32,
    pub alpha_layer_bitrate_ratio: u32,
    pub cb_qp_index_offset: i8,
    pub cr_qp_index_offset: i8,
    pub reserved2: u16,
    pub lookahead_level: u32,
    pub reserved: [u32; 3],
}
forme!(RcParams, 128, 4, "NV_ENC_RC_PARAMS");
deport!(RcParams, reserved, 116, "NV_ENC_RC_PARAMS.reserved");

/// `NV_ENC_CONFIG_H264_VUI_PARAMETERS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConfigH264Vui {
    pub overscan_info_present_flag: u32,
    pub overscan_info: u32,
    pub video_signal_type_present_flag: u32,
    pub video_format: u32,
    pub video_full_range_flag: u32,
    pub colour_description_present_flag: u32,
    pub colour_primaries: u32,
    pub transfer_characteristics: u32,
    pub colour_matrix: u32,
    pub chroma_sample_location_flag: u32,
    pub chroma_sample_location_top: u32,
    pub chroma_sample_location_bot: u32,
    pub bitstream_restriction_flag: u32,
    pub timing_info_present_flag: u32,
    pub num_unit_in_ticks: u32,
    pub time_scale: u32,
    pub reserved: [u32; 12],
}
forme!(ConfigH264Vui, 112, 4, "NV_ENC_CONFIG_H264_VUI_PARAMETERS");

/// `NV_ENC_CONFIG_H264`. ⚠️ **No `version` field**: it is a union
/// member, versioned by its parent `Config`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConfigH264 {
    /// Bit field: `enableTemporalSVC`(1) … `reservedBitFields`(10).
    /// See the `H264_*` masks below.
    pub drapeaux: u32,
    pub level: u32,
    pub idr_period: u32,
    pub separate_colour_plane_flag: u32,
    pub disable_deblocking_filter_idc: u32,
    pub num_temporal_layers: u32,
    pub sps_id: u32,
    pub pps_id: u32,
    pub adaptive_transform_mode: u32,
    pub fmo_mode: u32,
    pub bdirect_mode: u32,
    pub entropy_coding_mode: u32,
    pub stereo_mode: u32,
    pub intra_refresh_period: u32,
    pub intra_refresh_cnt: u32,
    pub max_num_ref_frames: u32,
    pub slice_mode: u32,
    pub slice_mode_data: u32,
    pub vui: ConfigH264Vui,
    pub ltr_num_frames: u32,
    pub ltr_trust_mode: u32,
    pub chroma_format_idc: u32,
    pub max_temporal_layers: u32,
    pub use_b_frames_as_ref: u32,
    pub num_ref_l0: u32,
    pub num_ref_l1: u32,
    pub output_bit_depth: u32,
    pub input_bit_depth: u32,
    pub reserved1: [u32; 265],
    pub reserved2: [*mut c_void; 64],
}
forme!(ConfigH264, 1792, 8, "NV_ENC_CONFIG_H264");
deport!(ConfigH264, vui, 72, "NV_ENC_CONFIG_H264.h264VUIParameters");
deport!(ConfigH264, reserved1, 220, "NV_ENC_CONFIG_H264.reserved1");
deport!(ConfigH264, reserved2, 1280, "NV_ENC_CONFIG_H264.reserved2");

/// `outputAUD`, 7th bit of the `ConfigH264` bit field.
pub const H264_OUTPUT_AUD: u32 = 1 << 6;
/// `repeatSPSPPS`, 13th bit — **indispensable for broadcasting**: without it, a
/// peer arriving midway never gets SPS/PPS.
pub const H264_REPEAT_SPS_PPS: u32 = 1 << 12;

/// `NV_ENC_CODEC_CONFIG`, reduced to the only member this product writes.
///
/// ⚠️ **The upstream union carries FIVE members** (H.264, HEVC, AV1, and two
/// "MEOnly") plus a `reserved[320]`. We only transcribe one: a C union
/// is only aligned storage, and transcribing four variants that nothing
/// writes would be four opportunities to get it wrong for nothing in return. The
/// padding carries the size, and the assertion checks it.
#[repr(C)]
#[derive(Clone, Copy)]
pub union CodecConfig {
    pub h264: ConfigH264,
    /// `[u64; 224]` = 1792 bytes. ⚠️ **The alignment of 8 does NOT come from
    /// this padding** — I first wrote that, it was wrong: the alignment
    /// of a union is the maximum of its members', and `h264` already carries
    /// 8 (it contains pointers). A `[u32; 448]` would therefore give
    /// exactly the same union. The `u64` is here to state the intent, not
    /// to produce it.
    pub _remplissage: [u64; 224],
}
forme!(CodecConfig, 1792, 8, "NV_ENC_CODEC_CONFIG");

/// `NV_ENC_CONFIG`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Config {
    pub version: u32,
    pub profile_guid: Guid,
    pub gop_length: u32,
    /// ⚠️ **Signed** in the header.
    pub frame_interval_p: i32,
    pub mono_chrome_encoding: u32,
    pub frame_field_mode: u32,
    pub mv_precision: u32,
    pub rc_params: RcParams,
    pub encode_codec_config: CodecConfig,
    pub reserved: [u32; 278],
    pub reserved2: [*mut c_void; 64],
}
forme!(Config, 3584, 8, "NV_ENC_CONFIG");
deport!(Config, rc_params, 40, "NV_ENC_CONFIG.rcParams");
deport!(
    Config,
    encode_codec_config,
    168,
    "NV_ENC_CONFIG.encodeCodecConfig"
);
deport!(Config, reserved, 1960, "NV_ENC_CONFIG.reserved");
deport!(Config, reserved2, 3072, "NV_ENC_CONFIG.reserved2");

/// `NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MeHintCountsPerBlocktype {
    pub drapeaux: u32,
    pub reserved1: [u32; 3],
}
forme!(
    MeHintCountsPerBlocktype,
    16,
    4,
    "NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE"
);

/// `NV_ENC_INITIALIZE_PARAMS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InitializeParams {
    pub version: u32,
    pub encode_guid: Guid,
    pub preset_guid: Guid,
    pub encode_width: u32,
    pub encode_height: u32,
    pub dar_width: u32,
    pub dar_height: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub enable_encode_async: u32,
    pub enable_ptd: u32,
    /// Champ de bits : `reportSliceOffsets`(1) … `reservedBitFields`(19).
    pub drapeaux: u32,
    pub priv_data_size: u32,
    pub reserved: u32,
    pub priv_data: *mut c_void,
    pub encode_config: *mut Config,
    pub max_encode_width: u32,
    pub max_encode_height: u32,
    pub max_me_hint_counts_per_block: [MeHintCountsPerBlocktype; 2],
    pub tuning_info: u32,
    pub buffer_format: u32,
    pub num_state_buffers: u32,
    pub output_stats_level: u32,
    pub reserved1: [u32; 284],
    pub reserved2: [*mut c_void; 64],
}
forme!(InitializeParams, 1800, 8, "NV_ENC_INITIALIZE_PARAMS");
deport!(
    InitializeParams,
    encode_guid,
    4,
    "NV_ENC_INITIALIZE_PARAMS.encodeGUID"
);
deport!(
    InitializeParams,
    encode_config,
    88,
    "NV_ENC_INITIALIZE_PARAMS.encodeConfig"
);
deport!(
    InitializeParams,
    tuning_info,
    136,
    "NV_ENC_INITIALIZE_PARAMS.tuningInfo"
);
deport!(
    InitializeParams,
    buffer_format,
    140,
    "NV_ENC_INITIALIZE_PARAMS.bufferFormat"
);
deport!(
    InitializeParams,
    reserved1,
    152,
    "NV_ENC_INITIALIZE_PARAMS.reserved1"
);
deport!(
    InitializeParams,
    reserved2,
    1288,
    "NV_ENC_INITIALIZE_PARAMS.reserved2"
);

/// `NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenEncodeSessionExParams {
    pub version: u32,
    pub device_type: u32,
    pub device: *mut c_void,
    pub reserved: *mut c_void,
    /// ⚠️ **`NVENCAPI_VERSION`, not a structure version.**
    pub api_version: u32,
    pub reserved1: [u32; 253],
    pub reserved2: [*mut c_void; 64],
}
forme!(
    OpenEncodeSessionExParams,
    1552,
    8,
    "NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS"
);
deport!(
    OpenEncodeSessionExParams,
    reserved1,
    28,
    "NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS.reserved1"
);
deport!(
    OpenEncodeSessionExParams,
    reserved2,
    1040,
    "NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS.reserved2"
);

/// `NV_ENC_PRESET_CONFIG`.
///
/// 🔴 **Its `presetCfg.version` must be set IN ADDITION to its own**, otherwise
/// `nvEncGetEncodePresetConfigEx` returns `NV_ENC_ERR_INVALID_VERSION`.
/// It is the most common first-launch failure of this API.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetConfig {
    pub version: u32,
    pub reserved: u32,
    pub preset_cfg: Config,
    pub reserved1: [u32; 256],
    pub reserved2: [*mut c_void; 64],
}
forme!(PresetConfig, 5128, 8, "NV_ENC_PRESET_CONFIG");
deport!(
    PresetConfig,
    reserved1,
    3592,
    "NV_ENC_PRESET_CONFIG.reserved1"
);
deport!(
    PresetConfig,
    reserved2,
    4616,
    "NV_ENC_PRESET_CONFIG.reserved2"
);

/// `NV_ENC_RECONFIGURE_PARAMS` — changes the bitrate of a LIVE encoder.
///
/// 🔴 **Transcribed rather than leaving `set_bitrate` without effect.** The repository
/// drives the video bitrate through this path (`transport/adaptation.rs`); a
/// second back end that accepted the call without doing anything would make all
/// bandwidth adaptation **invisibly inoperative** — exactly the silent
/// failure.
///
/// ⚠️ It carries a WHOLE `InitializeParams`: reconfiguring means
/// resubmitting the initialisation, with the bitrate modified. Hence the session
/// keeping its original configuration.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ReconfigureParams {
    pub version: u32,
    pub reserved: u32,
    pub re_init_encode_params: InitializeParams,
    /// Champ de bits : `resetEncoder`(1), `forceIDR`(1), `reserved1`(30).
    pub drapeaux: u32,
    pub reserved2: u32,
}
forme!(ReconfigureParams, 1816, 8, "NV_ENC_RECONFIGURE_PARAMS");
deport!(
    ReconfigureParams,
    re_init_encode_params,
    8,
    "NV_ENC_RECONFIGURE_PARAMS.reInitEncodeParams"
);

/// `resetEncoder`, 1ᵉʳ bit.
pub const RECONFIGURE_RESET: u32 = 1 << 0;
/// `forceIDR`, 2ᵉ bit.
pub const RECONFIGURE_FORCE_IDR: u32 = 1 << 1;
