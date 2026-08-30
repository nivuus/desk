//! Ce qui TROUVE et ACTIVE les MFT : l'encodeur H.264 matériel, le
//! convertisseur de couleur, le gestionnaire de périphérique DXGI qu'on leur
//! partage, et l'échantillon NV12 de repli.
//!
//! Extrait d'`encode.rs` le 30 août 2026 (lot 31), **avant** d'y ajouter quoi
//! que ce soit : le fichier pesait 1536 lignes, trois fois le plafond de 500,
//! et la doctrine du dépôt est d'extraire dans une tâche DÉDIÉE avant celle
//! qui ajoute — jamais de comprimer. Le pendant de ce module est
//! `encode::reglages`, qui POSE les réglages sur une MFT une fois obtenue.
//!
//! 🔴 **C'EST ICI QUE LE PRODUIT ÉCHOUE AUJOURD'HUI SUR LA VM CIBLE.**
//! `find_hardware_encoder` énumère une seule MFT — `NVIDIA H.264 Encoder
//! MFT` — et son `ActivateObject` rend `0x8000FFFF` (« Catastrophic
//! failure ») en **session 1**, alors que le même appel réussit en
//! **session 0**, dans le même binaire et à la même minute. Mesuré le
//! 30 août 2026, avec deux témoins verts posés à côté (l'encodeur H.264
//! LOGICIEL et le processeur vidéo LOGICIEL s'activent, eux, dans les deux
//! sessions) : la machinerie Media Foundation fonctionne, seule la MFT
//! matérielle NVIDIA refuse. Trois remèdes bon marché sont **réfutés par la
//! mesure** — poser `MFT_ENUM_ADAPTER_LUID`, tenir un périphérique D3D11
//! NVIDIA vivant avant l'activation, et lier l'affichage virtuel au GPU
//! NVIDIA (déjà vrai chez nous). Voir
//! `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`.
//!
//! ⚠️ **ET `find_hardware_video_processor` NE TROUVE RIEN SUR CETTE
//! MACHINE, DANS LES DEUX SESSIONS** (même mesure) : `create_color_converter`
//! retombe donc **toujours** sur son `CoCreateInstance`, c'est-à-dire sur le
//! `Microsoft Video Processor MFT` **logiciel**. La conversion BGRA → NV12
//! passe par le CPU en production, et le commentaire d'en-tête d'`encode.rs`
//! qui la dit « sans quitter le GPU » est faux ici. Le repli le journalise,
//! mais en `debug!`, un niveau que la production n'émet pas.
//!
//! **Déplacement pur : aucun appel, aucun ordre, aucune valeur n'a changé.**
//! Les seules différences avec le texte d'origine sont les visibilités
//! (`pub(super)` pour les cinq fonctions ayant un appelant hors de ce
//! fichier ; `format_subtype` et `find_hardware_video_processor` restent
//! privées, personne d'autre ne les appelle), la qualification de
//! `pack_u64` — qui vit chez le frère `reglages` — et les imports, qui ne
//! suivent jamais tout seuls.

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

/// Journalise les types d'entrée réellement annoncés par l'encodeur, avant
/// toute configuration. Sert de preuve empirique à la question BGRA/NV12
/// (voir le commentaire de module d'`encode.rs`, PAS celui de ce
/// fichier-ci) : sur la VM cible, seul NV12 apparaît.
pub(super) fn log_supported_input_types(transform: &IMFTransform) {
    let mut index = 0u32;
    loop {
        let media_type = match unsafe { transform.GetInputAvailableType(0, index) } {
            Ok(t) => t,
            Err(_) => break, // MF_E_NO_MORE_TYPES : fin de l'énumération.
        };
        let subtype = unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) };
        match subtype {
            Ok(guid) => tracing::info!(
                index,
                subtype = %format_subtype(guid),
                "type d'entrée annoncé par l'encodeur"
            ),
            Err(_) => tracing::info!(index, "type d'entrée annoncé (sous-type illisible)"),
        }
        index += 1;
    }
}

/// Traduit les GUID de sous-type vidéo les plus courants en texte lisible,
/// pour les journaux. Sans rapport avec la logique de conversion elle-même.
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

/// Crée le convertisseur GPU BGRA→NV12 (Video Processor MFT de Media
/// Foundation, `CLSID_VideoProcessorMFT`). Contrairement à l'encodeur, cette
/// MFT est synchrone : pas d'événements à suivre, `ProcessInput` suivi de
/// `ProcessOutput` suffit.
pub(super) fn create_color_converter(
    device_manager: &IMFDXGIDeviceManager,
    capture: (u32, u32),
    encode: (u32, u32),
    fps: u32,
) -> Result<IMFTransform> {
    // Essai (ronde de correction 1/5, investigation du débit) :
    // `CoCreateInstance(CLSID_VideoProcessorMFT)` instancie l'implémentation
    // par défaut de ce CLSID, qui pourrait être un chemin logiciel/mixte
    // plutôt qu'une implémentation matérielle. On tente d'abord de trouver
    // un convertisseur explicitement enregistré comme matériel via
    // `MFTEnumEx`, comme pour l'encodeur — repli sur `CoCreateInstance` si
    // rien n'est trouvé.
    let converter: IMFTransform = match find_hardware_video_processor() {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!(erreur = %e, "aucun convertisseur vidéo matériel énuméré, repli sur CLSID_VideoProcessorMFT");
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER) }
                .context("création du convertisseur vidéo (Video Processor MFT)")?
        }
    };

    // Essai : le mode faible latence n'était appliqué qu'à l'encodeur, pas au
    // convertisseur — potentiellement lié à l'attente d'~1 s observée dans
    // `mft::convertisseur::drain_converter_output` (voir son commentaire).
    // `GetAttributes` peut
    // échouer si le convertisseur n'expose pas d'attributs modifiables ; dans
    // ce cas on continue sans bloquer la construction.
    if let Ok(converter_attributes) = unsafe { converter.GetAttributes() } {
        let _ = unsafe { converter_attributes.SetUINT32(&MF_LOW_LATENCY, 1) };
    }

    unsafe {
        converter.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, device_manager.as_raw() as usize)
    }
    .context("partage du périphérique D3D avec le convertisseur")?;

    // Piège rencontré à l'essai : avec le pool par défaut, la séquence
    // documentée « ProcessOutput jusqu'à MF_E_TRANSFORM_NEED_MORE_INPUT »
    // échoue avec `MF_E_SAMPLEALLOCATOR_EMPTY` (0xC00D4A3E) après exactement 5
    // images — l'encodeur matériel en garde plusieurs « en vol » avant d'en
    // libérer, et le pool par défaut n'a pas cette marge.
    //
    // Premier correctif tenté, insuffisant : `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`
    // seul, avec les valeurs 4 puis 16 — dans les deux cas, échec au exactement
    // le même 5e appel, preuve que cet attribut seul n'a aucun effet ici.
    // Cause : notre flux est **progressif**
    // (`MF_MT_INTERLACE_MODE` = `MFVideoInterlace_Progressive`), et ce MFT
    // distingue apparemment deux attributs de taille de pool — un pour le
    // contenu entrelacé, un pour le progressif
    // (`MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE`) — seul ce second
    // attribut est honoré pour du contenu progressif. On positionne les deux
    // par prudence (documentation MF ambiguë sur ce point).
    let output_stream_attributes = unsafe { converter.GetOutputStreamAttributes(0) }
        .context("attributs du flux de sortie du convertisseur")?;
    unsafe {
        output_stream_attributes.SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT, 16)?;
        output_stream_attributes
            .SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE, 16)?;
    }

    let input_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        input_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        // Format d'entrée = ce que produit la capture (BGRA, avec alpha) ;
        // `MFVideoFormat_ARGB32` correspond à `DXGI_FORMAT_B8G8R8A8_UNORM`.
        input_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)?;
        input_type.SetUINT64(&MF_MT_FRAME_SIZE, reglages::pack_u64(capture.0, capture.1))?;
        input_type.SetUINT64(&MF_MT_FRAME_RATE, reglages::pack_u64(fps, 1))?;
        input_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter
            .SetInputType(0, &input_type, 0)
            .context("configuration du type d'entrée du convertisseur de couleur (Video Processor MFT)")?;
    }

    let output_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        output_type.SetUINT64(&MF_MT_FRAME_SIZE, reglages::pack_u64(encode.0, encode.1))?;
        output_type.SetUINT64(&MF_MT_FRAME_RATE, reglages::pack_u64(fps, 1))?;
        output_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter.SetOutputType(0, &output_type, 0).context(
            "configuration du type de sortie du convertisseur de couleur (Video Processor MFT)",
        )?;
    }

    Ok(converter)
}

/// Énumère les convertisseurs vidéo (BGRA→NV12) explicitement enregistrés
/// comme matériels, et active le premier — même logique que
/// `find_hardware_encoder`, avec les mêmes précautions de libération
/// mémoire (voir son commentaire).
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
        .context("énumération des convertisseurs vidéo matériels")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!("aucun convertisseur vidéo matériel enregistré");
    }

    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
    }
    let first = first.ok_or_else(|| anyhow!("activateur de convertisseur absent"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    if unsafe { first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len) }
        .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(convertisseur = %name, "convertisseur vidéo matériel retenu");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let transform: IMFTransform = unsafe { first.ActivateObject() }
        .context("activation du convertisseur vidéo matériel (ActivateObject)")?;
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    Ok(transform)
}

/// Alloue une texture NV12 GPU et l'enveloppe dans un échantillon Media
/// Foundation réutilisable, pour les cas où le convertisseur ne s'auto-alloue
/// pas (`MFT_OUTPUT_STREAM_PROVIDES_SAMPLES` absent — voir
/// `super::H264Encoder::new`).
pub(super) fn create_nv12_sample(device: &ID3D11Device, width: u32, height: u32) -> Result<IMFSample> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("allocation de la texture NV12 intermédiaire")?;
    let texture = texture.ok_or_else(|| anyhow!("texture NV12 absente"))?;

    let sample = unsafe { MFCreateSample() }?;
    let buffer = unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &texture, 0, false) }
        .context("enveloppement de la texture NV12")?;
    unsafe { sample.AddBuffer(&buffer) }?;
    Ok(sample)
}

/// Énumère les encodeurs H.264 matériels et active le premier.
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
        .context("énumération des encodeurs H.264 matériels")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!(
            "aucun encodeur H.264 matériel trouvé sur cette machine. \
             Vérifier le pilote GPU ; le jalon 1 n'a pas de repli logiciel."
        );
    }

    // Récupérer les objets AVANT de libérer le tableau alloué par CoTaskMemAlloc.
    //
    // Ronde de correction 1/5 — fuite corrigée ici : la version précédente
    // ne relâchait que le premier `IMFActivate` (par `.clone()`, qui ajoute
    // une référence sans jamais libérer celle que `MFTEnumEx` a placée dans
    // la case du tableau). `CoTaskMemFree` ne libère que la mémoire brute du
    // tableau, pas les références COM qu'il contient : chaque entrée, y
    // compris la première, fuyait donc une référence. `slot.take()` déplace
    // chaque entrée hors du tableau (remplacée par `None`) ; les entrées
    // qu'on ne garde pas sont droppées immédiatement (donc relâchées), la
    // première est conservée dans `first` sans référence supplémentaire.
    // Latent tant qu'un seul encodeur est présent, mais réel dès qu'il y en
    // aurait plusieurs.
    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
        // Sinon : `activate` est droppé ici, relâchant sa référence COM.
    }
    let first = first.ok_or_else(|| anyhow!("activateur d'encodeur absent"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    // écart d'API windows-rs 0.62 : `GetStringAlloc` n'existe pas sur
    // `IMFAttributes` dans cette version ; la méthode s'appelle
    // `GetAllocatedString` (mémoire allouée par `CoTaskMemAlloc`, à libérer
    // explicitement après usage).
    if unsafe { first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len) }
        .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(encodeur = %name, "encodeur matériel retenu");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let active = unsafe { first.ActivateObject::<IMFTransform>() };
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    match active {
        Ok(transform) => Ok(transform),
        Err(erreur) => Err(anyhow!(
            "{}",
            // La COMPOSITION du message est pure et vit chez
            // `encode_nvenc`, où elle est testée sur l'hôte : ici on ne
            // fait que lui donner le code et ce que la machine porte.
            encode_nvenc::diagnostic_activation(erreur.code().0, &erreur.to_string(), &adaptateurs_dxgi())
        )),
    }
}

/// Les adaptateurs DXGI, réduits à ce que la règle PURE de
/// `crate::encode_nvenc` sait lire.
///
/// ⚠️ **Rend une liste VIDE plutôt qu'une erreur** : cette fonction ne sert
/// qu'à enrichir un diagnostic et à choisir une voie. La faire échouer
/// remplacerait un message utile par un autre message d'erreur, et masquerait
/// la panne qu'on essaie justement de décrire.
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
        .context("liaison du périphérique D3D11 au gestionnaire DXGI")?;
    Ok(manager)
}
