//! Les réglages que l'on POSE sur une MFT : types de média d'entrée et de
//! sortie, contrôle de débit, et les deux fabriques de `VARIANT` que le
//! contrôle de débit exige.
//!
//! Extrait d'`encode.rs` le 30 août 2026 (lot 31), **avant** d'y ajouter quoi
//! que ce soit : le fichier pesait 1536 lignes, trois fois le plafond de 500,
//! et la doctrine du dépôt est d'extraire dans une tâche DÉDIÉE avant celle
//! qui ajoute — jamais de comprimer. Le pendant de ce module est
//! `encode::fabrique`, qui TROUVE et ACTIVE les MFT ; ici on ne fait que les
//! régler une fois obtenues.
//!
//! **Déplacement pur : aucun appel, aucun ordre, aucune valeur n'a changé.**
//! Les seules différences avec le texte d'origine sont les visibilités
//! (`pub(super)`, les six fonctions ayant toutes un appelant hors de ce
//! fichier) et les imports, qui ne suivent jamais tout seuls.

use anyhow::{Context, Result};
use windows::core::Interface;
// écart d'API windows-rs 0.62, déplacé ici avec son import depuis
// `encode.rs` (lot 31) : `VARIANT_TRUE`/`VARIANT_FALSE` vivent dans
// `Win32::Foundation` (constantes `VARIANT_BOOL`), pas dans
// `Win32::System::Variant` où on les attendrait à côté du reste du type
// `VARIANT` — confirmé en lisant les sources de la crate sur la VM.
use windows::Win32::Foundation::{VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4,
};

/// Construit une `VARIANT` `VT_UI4` manuellement : cette version de
/// windows-rs ne fournit pas de `From<u32>` pour `VARIANT` (écart au brief,
/// vérifié en lisant les sources de la crate sur la VM — aucun `impl From<`
/// n'existe pour ce type dans `Win32::System::Variant`).
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

/// Construit une `VARIANT` `VT_BOOL` manuellement (même raison que
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
        // Baseline évite les images B : ordre de décodage = ordre d'affichage.
        media_type.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
        transform
            .SetOutputType(0, &media_type, 0)
            .context("configuration du type de sortie de l'encodeur H.264 (transform matériel)")?;
    }
    Ok(())
}

pub(super) fn configure_input(transform: &IMFTransform, width: u32, height: u32, fps: u32) -> Result<()> {
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
        // Débit constant : latence prévisible, indispensable en interactif.
        let mode = variant_u32(eAVEncCommonRateControlMode_CBR.0 as u32);
        codec.SetValue(&CODECAPI_AVEncCommonRateControlMode, &mode)?;
        let rate = variant_u32(bitrate);
        codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate)?;
        // Pas de groupe d'images fermé : on demande les images clés à la
        // volée (`H264Encoder::request_keyframe`, câblé depuis
        // `Event::KeyframeRequest` de str0m dans `transport/evenements.rs`). Le retour
        // de `SetValue` est vérifié plutôt que jeté : un refus silencieux du
        // pilote laisserait croire le contrat honoré alors qu'un groupe
        // d'images fermé rendrait les images clés à la demande inopérantes.
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

/// Empaquette deux entiers 32 bits dans l'attribut 64 bits attendu par MF.
pub(super) fn pack_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}
