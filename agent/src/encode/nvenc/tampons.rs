//! Les dispositions de structures NVENC qui s'échangent **par image** :
//! enregistrer une texture, la projeter, obtenir un tampon de flux, encoder,
//! relire le flux.
//!
//! 🔴 **NOTICE DE LICENCE, PROVENANCE ET COMMANDE DE RELECTURE : voir
//! `super::abi`.** Ce fichier prolonge la même transcription et relève de la
//! même notice. Son pendant est `super::structures`, qui porte ce qu'on
//! configure **une fois** par session ; la scission suit cette frontière-là,
//! et pas le plafond de 500 lignes (qu'elle sert accessoirement).
//!
//! Mêmes conventions de transcription et mêmes assertions que
//! `super::structures` — **y compris ce que ces assertions n'ont pas à
//! attraper**, cas mesuré et expliqué là-bas.

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

/// `NV_ENC_REGISTER_RESOURCE` — déclare une texture D3D11 à l'encodeur.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RegisterResource {
    pub version: u32,
    pub resource_type: u32,
    pub width: u32,
    pub height: u32,
    /// ⚠️ **`0` pour une texture D3D11** : le pas est celui de la texture, et
    /// c'est le pilote qui le connaît.
    pub pitch: u32,
    pub sub_resource_index: u32,
    /// L'`ID3D11Texture2D*`.
    pub resource_to_register: *mut c_void,
    /// **[sortie]** la ressource enregistrée.
    pub registered_resource: *mut c_void,
    pub buffer_format: u32,
    pub buffer_usage: u32,
    /// ⚠️ **`null` en D3D11** — ce point de synchronisation est un objet D3D12.
    pub p_input_fence_point: *mut c_void,
    pub chroma_offset: [u32; 2],
    pub reserved1: [u32; 246],
    pub reserved2: [*mut c_void; 61],
}
forme!(RegisterResource, 1536, 8, "NV_ENC_REGISTER_RESOURCE");
deport!(RegisterResource, resource_to_register, 24, "NV_ENC_REGISTER_RESOURCE.resourceToRegister");
deport!(RegisterResource, chroma_offset, 56, "NV_ENC_REGISTER_RESOURCE.chromaOffset");
deport!(RegisterResource, reserved2, 1048, "NV_ENC_REGISTER_RESOURCE.reserved2");

/// `NV_ENC_MAP_INPUT_RESOURCE` — projette une ressource enregistrée pour une
/// image, et rend le pointeur d'entrée que `PicParams` attend.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MapInputResource {
    pub version: u32,
    /// ⚠️ **Déprécié** par l'en-tête ; laissé à zéro.
    pub sub_resource_index: u32,
    /// ⚠️ **Déprécié** par l'en-tête ; laissé nul.
    pub input_resource: *mut c_void,
    pub registered_resource: *mut c_void,
    /// **[sortie]** ce qu'on passe à `PicParams::input_buffer`.
    pub mapped_resource: *mut c_void,
    /// **[sortie]** le format que l'encodeur a retenu.
    pub mapped_buffer_fmt: u32,
    pub reserved1: [u32; 251],
    pub reserved2: [*mut c_void; 63],
}
forme!(MapInputResource, 1544, 8, "NV_ENC_MAP_INPUT_RESOURCE");
deport!(MapInputResource, registered_resource, 16, "NV_ENC_MAP_INPUT_RESOURCE.registeredResource");
deport!(MapInputResource, mapped_resource, 24, "NV_ENC_MAP_INPUT_RESOURCE.mappedResource");
deport!(MapInputResource, reserved2, 1040, "NV_ENC_MAP_INPUT_RESOURCE.reserved2");

/// `NV_ENC_CREATE_BITSTREAM_BUFFER` — le tampon où l'encodeur dépose le flux.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CreateBitstreamBuffer {
    pub version: u32,
    /// ⚠️ **Déprécié** — « Do not use », dit l'en-tête.
    pub size: u32,
    /// ⚠️ **Déprécié** — idem.
    pub memory_heap: u32,
    pub reserved: u32,
    /// **[sortie]** le tampon, à passer à `PicParams::output_bitstream`.
    pub bitstream_buffer: *mut c_void,
    /// **[sortie]** réservé — l'en-tête dit de ne pas s'en servir.
    pub bitstream_buffer_ptr: *mut c_void,
    pub reserved1: [u32; 58],
    pub reserved2: [*mut c_void; 64],
}
forme!(CreateBitstreamBuffer, 776, 8, "NV_ENC_CREATE_BITSTREAM_BUFFER");
deport!(CreateBitstreamBuffer, bitstream_buffer, 16, "NV_ENC_CREATE_BITSTREAM_BUFFER.bitstreamBuffer");
deport!(CreateBitstreamBuffer, reserved2, 264, "NV_ENC_CREATE_BITSTREAM_BUFFER.reserved2");

/// `NV_ENC_CODEC_PIC_PARAMS`, réduite à son encombrement.
///
/// ⚠️ **Aucun membre n'est transcrit, et c'est délibéré** : sur le chemin
/// courant, **rien** n'est écrit dans cette union — ni tranches explicites,
/// ni SEI, ni références long terme. Transcrire `NV_ENC_PIC_PARAMS_H264` et
/// ses cinquante champs pour n'en écrire aucun serait cinquante occasions de
/// se tromper sans contrepartie. Le jour où l'un d'eux sert, il se transcrit
/// **avec sa mesure**, pas avant.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CodecPicParams {
    /// 193 × 8 = 1544 octets. L'alignement de 8 est celui des pointeurs que
    /// portent les variantes réelles.
    pub _encombrement: [u64; 193],
}
forme!(CodecPicParams, 1544, 8, "NV_ENC_CODEC_PIC_PARAMS");

/// `NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE`, redéclarée ici pour que ce
/// fichier ne dépende pas de l'ordre de compilation de son frère.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MeHintCounts {
    pub drapeaux: u32,
    pub reserved1: [u32; 3],
}
forme!(MeHintCounts, 16, 4, "NVENC_EXTERNAL_ME_HINT_COUNTS_PER_BLOCKTYPE (tampons)");

/// `NV_ENC_PIC_PARAMS` — une image à encoder.
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
    /// ⚠️ **`PIC_STRUCT_FRAME` vaut 1, pas 0** : mettre la structure à zéro
    /// et oublier ce champ est une erreur, pas un défaut inoffensif.
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
deport!(PicParams, codec_pic_params, 80, "NV_ENC_PIC_PARAMS.codecPicParams");
deport!(PicParams, qp_delta_map, 1712, "NV_ENC_PIC_PARAMS.qpDeltaMap");
deport!(PicParams, reserved3, 1768, "NV_ENC_PIC_PARAMS.reserved3");
deport!(PicParams, reserved6, 2904, "NV_ENC_PIC_PARAMS.reserved6");

/// `NV_ENC_LOCK_BITSTREAM` — relit le flux encodé.
///
/// 🔴 **`reserved_internal` est le DERNIER membre, après `reserved2`.** Le
/// laisser tomber raccourcirait la structure de 32 octets, et NVENC écrirait
/// **au-delà de notre allocation**. C'est le déport asserté plus bas qui
/// l'empêche de partir sans qu'on le voie.
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
    /// **[sortie]** la longueur utile de `bitstream_buffer_ptr`.
    pub bitstream_size_in_bytes: u32,
    pub output_time_stamp: u64,
    pub output_duration: u64,
    /// **[sortie]** les octets du flux. ⚠️ **Valides seulement entre
    /// `nvEncLockBitstream` et `nvEncUnlockBitstream`** : ce qu'on en garde
    /// doit être copié avant de rendre le verrou.
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
    /// ⚠️ **Signé**.
    pub average_mvx: i32,
    /// ⚠️ **Signé**.
    pub average_mvy: i32,
    pub alpha_layer_size_in_bytes: u32,
    pub output_stats_ptr_size: u32,
    pub reserved: u32,
    pub output_stats_ptr: *mut c_void,
    pub frame_idx_display: u32,
    pub reserved1: [u32; 219],
    pub reserved2: [*mut c_void; 63],
    /// 🔴 **Le dernier membre, et le plus facile à oublier.**
    pub reserved_internal: [u32; 8],
}
forme!(LockBitstream, 1544, 8, "NV_ENC_LOCK_BITSTREAM");
deport!(LockBitstream, bitstream_size_in_bytes, 36, "NV_ENC_LOCK_BITSTREAM.bitstreamSizeInBytes");
deport!(LockBitstream, bitstream_buffer_ptr, 56, "NV_ENC_LOCK_BITSTREAM.bitstreamBufferPtr");
deport!(LockBitstream, picture_type, 64, "NV_ENC_LOCK_BITSTREAM.pictureType");
deport!(LockBitstream, reserved2, 1008, "NV_ENC_LOCK_BITSTREAM.reserved2");
deport!(LockBitstream, reserved_internal, 1512, "NV_ENC_LOCK_BITSTREAM.reservedInternal");
