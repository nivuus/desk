//! Les dispositions de structures NVENC que l'on configure **une fois** par
//! session : ouverture, initialisation, réglages, préréglages.
//!
//! 🔴 **NOTICE DE LICENCE, PROVENANCE ET COMMANDE DE RELECTURE : voir
//! `super::abi`.** Ce fichier prolonge la même transcription et relève de la
//! même notice ; elle n'est pas recopiée ici pour qu'il n'y ait **qu'un seul**
//! endroit à tenir à jour, mais la frontière d'attribution englobe bien ce
//! fichier. Son pendant est `super::tampons`, qui porte ce qui s'échange
//! **par image**.
//!
//! ## Ce qui rend cette transcription vérifiable
//!
//! Chaque structure porte une assertion de **taille** et d'**alignement**
//! évaluée à la compilation, et les structures dont un champ est atteint par
//! calcul portent en plus des assertions de **déport**. Les valeurs viennent
//! d'une mesure faite **sur la cible** `x86_64-pc-windows-gnu` par
//! `x86_64-w64-mingw32-gcc` sur l'en-tête réel — pas d'une mesure d'hôte
//! supposée transposable.
//!
//! 🔴 **Le message de chaque assertion NOMME la structure**, sans quoi un
//! rouge ferait chercher.
//!
//! ## Ce que ces assertions attrapent, et ce qu'il est INUTILE d'en attendre
//!
//! 🔴 **Une correction que j'ai failli écrire à l'envers, et qui vaut d'être
//! consignée telle quelle.** En cherchant à faire rougir ce contrôle, j'ai
//! ramené `NV_ENC_CONFIG::reserved` de 278 à 277 `u32` : **rien n'a rougi**,
//! ni la taille ni le déport. J'ai d'abord conclu que le contrôle était
//! faible, et j'ai écrit ici qu'il « restait vert sur une disposition
//! fausse ». **C'était faux, et mesuré comme tel** (`equiv.c`, compilé) :
//!
//! ```text
//! 278 : taille=3584 deport_reserved2=3072
//! 277 : taille=3584 deport_reserved2=3072   <-- IDENTIQUE
//! 276 : taille=3576 deport_reserved2=3064   <-- attrapé
//! ```
//!
//! Le tableau finit à 3072 et `reserved2` est aligné sur 8 : les 4 octets
//! retirés sont **entièrement repris par le remplissage**, et la disposition
//! obtenue est **octet pour octet la même**. Ce n'est donc pas un défaut que
//! le contrôle laisse passer — **c'est un non-défaut**, et rester vert est la
//! bonne réponse. La leçon n'est pas « renforcer le contrôle » mais **« une
//! rouge qui reste verte se DIAGNOSTIQUE, elle ne se classe pas »**.
//!
//! ✅ **Ce qui EST attrapé, et vérifié** : un champ manquant ou déplacé au
//! milieu d'une structure (`mv_precision` retiré ⇒ *« déport ABI faux pour
//! NV_ENC_CONFIG.rcParams »*), et tout écart de réservés assez grand pour
//! franchir la frontière d'alignement (276 au lieu de 278).
//!
//! ⚠️ **Les déports des réservés de queue sont assertés quand même** : ils ne
//! coûtent rien, et ils ferment le cas « écart de plus d'un mot ».
//!
//! ## Trois conventions de transcription, et pourquoi
//!
//! 1. **Les énumérations C deviennent des `u32`.** Elles pèsent 4 octets dans
//!    cet en-tête, et un `enum` Rust avec des variantes manquantes serait un
//!    comportement indéfini dès qu'un pilote plus récent en rendrait une
//!    inconnue.
//! 2. **Les champs de bits deviennent UN `u32` et des masques nommés.** Rust
//!    n'a pas de champs de bits, et surtout : l'ordre d'attribution des bits
//!    n'est pas garanti par le langage. Un `u32` explicite met cet ordre entre
//!    nos mains, où il est relisible.
//! 3. **Les réservés gardent leur compte réel.** Les dimensionner « pour que
//!    la taille tombe juste » rendrait l'assertion de taille **tautologique**,
//!    donc incapable d'échouer — un contrôle qui ne peut pas rougir n'en est
//!    pas un.

#![allow(dead_code)]

use core::ffi::c_void;

macro_rules! forme {
    ($t:ty, $taille:expr, $alignement:expr, $nom:literal) => {
        const _: () = assert!(
            core::mem::size_of::<$t>() == $taille,
            concat!("taille ABI fausse pour ", $nom)
        );
        const _: () = assert!(
            core::mem::align_of::<$t>() == $alignement,
            concat!("alignement ABI faux pour ", $nom)
        );
    };
}

macro_rules! deport {
    ($t:ty, $champ:ident, $valeur:expr, $nom:literal) => {
        const _: () = assert!(
            core::mem::offset_of!($t, $champ) == $valeur,
            concat!("déport ABI faux pour ", $nom)
        );
    };
}

/// `GUID` de l'en-tête. **Transcrit plutôt qu'emprunté à `windows-core`** :
/// ce module doit compiler sur l'hôte Linux, où ce crate n'existe pas.
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
/// ⚠️ **P1, le plus rapide.** L'en-tête le dit : la qualité monte et la
/// performance descend de P1 vers P7. C'est aussi le préréglage qu'Apollo
/// emploie sur cette machine — relevé dans son journal :
/// `NvEnc: created encoder H.264 P1 async two-pass rfi`. **Ce n'est pas une
/// calibration** : rien dans ce dépôt n'a mesuré P1 contre P4.
pub const PRESET_P1: Guid = Guid {
    data1: 0xfc0a_8d3e,
    data2: 0x45f8,
    data3: 0x4cf8,
    data4: [0x80, 0xc7, 0x29, 0x88, 0x71, 0x59, 0x0e, 0xbf],
};

/// `NV_ENC_H264_PROFILE_BASELINE_GUID` — `{0727BCAA-78C4-4c83-8C2F-EF3DFF267C6A}`.
///
/// ⚠️ **L'en-tête l'écrit `0x727bcaa`, sans le zéro de tête** ; la valeur est
/// bien `0x0727BCAA`. Plusieurs octets de `data4` y sont aussi écrits courts
/// (`0x3` pour `0x03`). C ne s'en soucie pas, une transcription à la main si.
/// La forme `{…}` du commentaire est le recoupement.
pub const PROFILE_H264_BASELINE: Guid = Guid {
    data1: 0x0727_bcaa,
    data2: 0x78c4,
    data3: 0x4c83,
    data4: [0x8c, 0x2f, 0xef, 0x3d, 0xff, 0x26, 0x7c, 0x6a],
};

/// `NV_ENC_QP`.
///
/// ⚠️ L'en-tête le dit lui-même : ces champs sont `uint32_t` « for legacy
/// reasons » et doivent être **traités comme signés** quand une valeur
/// négative est visée.
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

/// `NV_ENC_CONFIG_H264`. ⚠️ **Aucun champ `version`** : c'est un membre
/// d'union, versionné par son parent `Config`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConfigH264 {
    /// Champ de bits : `enableTemporalSVC`(1) … `reservedBitFields`(10).
    /// Voir les masques `H264_*` plus bas.
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

/// `outputAUD`, 7ᵉ bit du champ de bits de `ConfigH264`.
pub const H264_OUTPUT_AUD: u32 = 1 << 6;
/// `repeatSPSPPS`, 13ᵉ bit — **indispensable en diffusion** : sans lui, un
/// pair qui arrive en cours de route n'a jamais de SPS/PPS.
pub const H264_REPEAT_SPS_PPS: u32 = 1 << 12;

/// `NV_ENC_CODEC_CONFIG`, réduite au seul membre que ce produit écrit.
///
/// ⚠️ **L'union amont porte CINQ membres** (H.264, HEVC, AV1, et deux
/// « MEOnly ») plus un `reserved[320]`. On n'en transcrit qu'un : une union C
/// n'est que du stockage aligné, et transcrire quatre variantes que rien
/// n'écrit serait quatre occasions de se tromper sans contrepartie. Le
/// remplissage porte la taille, et l'assertion la vérifie.
#[repr(C)]
#[derive(Clone, Copy)]
pub union CodecConfig {
    pub h264: ConfigH264,
    /// `[u64; 224]` = 1792 octets. ⚠️ **L'alignement de 8 ne vient PAS de
    /// ce remplissage** — je l'ai d'abord écrit, c'était faux : l'alignement
    /// d'une union est le maximum de celui de ses membres, et `h264` porte
    /// déjà 8 (il contient des pointeurs). Un `[u32; 448]` donnerait donc
    /// exactement la même union. Le `u64` est ici pour dire l'intention, pas
    /// pour la produire.
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
    /// ⚠️ **Signé** dans l'en-tête.
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
deport!(Config, encode_codec_config, 168, "NV_ENC_CONFIG.encodeCodecConfig");
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
deport!(InitializeParams, encode_guid, 4, "NV_ENC_INITIALIZE_PARAMS.encodeGUID");
deport!(InitializeParams, encode_config, 88, "NV_ENC_INITIALIZE_PARAMS.encodeConfig");
deport!(InitializeParams, tuning_info, 136, "NV_ENC_INITIALIZE_PARAMS.tuningInfo");
deport!(InitializeParams, buffer_format, 140, "NV_ENC_INITIALIZE_PARAMS.bufferFormat");
deport!(InitializeParams, reserved1, 152, "NV_ENC_INITIALIZE_PARAMS.reserved1");
deport!(InitializeParams, reserved2, 1288, "NV_ENC_INITIALIZE_PARAMS.reserved2");

/// `NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenEncodeSessionExParams {
    pub version: u32,
    pub device_type: u32,
    pub device: *mut c_void,
    pub reserved: *mut c_void,
    /// ⚠️ **`NVENCAPI_VERSION`, pas une version de structure.**
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
deport!(OpenEncodeSessionExParams, reserved1, 28, "NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS.reserved1");
deport!(OpenEncodeSessionExParams, reserved2, 1040, "NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS.reserved2");

/// `NV_ENC_PRESET_CONFIG`.
///
/// 🔴 **Sa `presetCfg.version` doit être posée EN PLUS de la sienne**, sans
/// quoi `nvEncGetEncodePresetConfigEx` rend `NV_ENC_ERR_INVALID_VERSION`.
/// C'est l'échec de premier lancement le plus courant de cette API.
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
deport!(PresetConfig, reserved1, 3592, "NV_ENC_PRESET_CONFIG.reserved1");
deport!(PresetConfig, reserved2, 4616, "NV_ENC_PRESET_CONFIG.reserved2");
