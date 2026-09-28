//! `NV_ENCODE_API_FUNCTION_LIST` — the function table the driver
//! fills, and the signatures of those this product calls.
//!
//! 🔴 **LICENCE NOTICE, PROVENANCE AND REREAD COMMAND: see
//! `super::abi`.** Same transcription, same notice.
//!
//! 🔴 **THE FIELD ORDER IS THE ABI.** This table is not a convenience:
//! the driver writes pointers at fixed offsets, and a field shifted
//! by one slot makes one function be called for another — with the arguments
//! of a third. That is why **all forty-five slots are
//! declared in order**, including those never called.
//!
//! ⚠️ **Thirty slots are deliberately left OPAQUE**
//! (`*const c_void`). A function pointer slot weighs the size
//! of a pointer, whatever its type: leaving them opaque **preserves
//! exactly the ABI** and removes thirty signatures to transcribe — hence thirty
//! opportunities to get it wrong — for functions nothing calls. The day
//! one of them is needed, it gets typed **at that moment**, and its asserted offset will
//! prove it stayed in place.
//!
//! ⚠️ **A trap of the header, noted and without effect here**: the slots
//! `nvEncGetEncodeProfileGUIDCount` and `…GUIDs` are declared there with the
//! *preset typedefs*, not the profile ones. It is an upstream typo
//! — fixed at tag `n13.1.15.0` — which **changes no offset**. Both
//! are opaque here anyway.

#![allow(dead_code)]

use core::ffi::c_void;

use super::structures::{
    Config, Guid, InitializeParams, OpenEncodeSessionExParams, PresetConfig, ReconfigureParams,
};
use super::tampons::{
    CreateBitstreamBuffer, LockBitstream, MapInputResource, PicParams, RegisterResource,
};

macro_rules! deport {
    ($champ:ident, $valeur:expr) => {
        const _: () = assert!(
            core::mem::offset_of!(FunctionList, $champ) == $valeur,
            concat!(
                "déport ABI faux pour NV_ENCODE_API_FUNCTION_LIST.",
                stringify!($champ)
            )
        );
    };
}

/// The return code of any NVENC function (`NVENCSTATUS`).
///
/// ⚠️ **`u32` and not a Rust `enum`**: the header's values are
/// **implicit** (0, 1, 2… without `= n`), so inserting a variant would shift
/// everything that follows — and a future driver may add some. See
/// `super::abi::SUCCESS` and `super::abi::ERR_INVALID_VERSION`.
pub type Statut = u32;

/// ⚠️ **`extern "system"` and not `extern "C"`**: the header declares
/// `__stdcall` on Windows. On x86-64 the two coincide, but writing it
/// right costs the same and stays correct if the target changes.
pub type OuvrirSessionEx =
    unsafe extern "system" fn(*mut OpenEncodeSessionExParams, *mut *mut c_void) -> Statut;
pub type PreregleageConfigEx =
    unsafe extern "system" fn(*mut c_void, Guid, Guid, u32, *mut PresetConfig) -> Statut;
pub type InitializeEncoderFn =
    unsafe extern "system" fn(*mut c_void, *mut InitializeParams) -> Statut;
pub type CreateBitstreamBufferFn =
    unsafe extern "system" fn(*mut c_void, *mut CreateBitstreamBuffer) -> Statut;
pub type DetruireTamponDeFlux = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type RegisterResourceFn =
    unsafe extern "system" fn(*mut c_void, *mut RegisterResource) -> Statut;
pub type DesenregistrerRessource = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type ProjeterRessource =
    unsafe extern "system" fn(*mut c_void, *mut MapInputResource) -> Statut;
pub type DeprojeterRessource = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type EncoderImage = unsafe extern "system" fn(*mut c_void, *mut PicParams) -> Statut;
pub type VerrouillerFlux = unsafe extern "system" fn(*mut c_void, *mut LockBitstream) -> Statut;
pub type DeverrouillerFlux = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type DetruireEncodeur = unsafe extern "system" fn(*mut c_void) -> Statut;
/// ⚠️ **Does NOT return a `Statut`** but a C string — the only exception in the
/// table, and it is easy to transcribe wrongly.
pub type LastErrorFn = unsafe extern "system" fn(*mut c_void) -> *const core::ffi::c_char;
pub type ReconfigurerEncodeur =
    unsafe extern "system" fn(*mut c_void, *mut ReconfigureParams) -> Statut;

/// `NV_ENCODE_API_FUNCTION_LIST`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FunctionList {
    pub version: u32,
    pub reserved: u32,
    pub ouvrir_session: *const c_void,
    pub compte_guid_encodage: *const c_void,
    pub compte_guid_profil: *const c_void,
    pub guids_profil: *const c_void,
    pub guids_encodage: *const c_void,
    pub compte_formats_entree: *const c_void,
    pub formats_entree: *const c_void,
    pub capacites: *const c_void,
    pub compte_preregleages: *const c_void,
    pub guids_preregleages: *const c_void,
    pub config_preregleage: *const c_void,
    pub initialize_encoder: Option<InitializeEncoderFn>,
    pub create_input_buffer: *const c_void,
    pub detruire_tampon_entree: *const c_void,
    pub create_bitstream_buffer: Option<CreateBitstreamBufferFn>,
    pub detruire_tampon_de_flux: Option<DetruireTamponDeFlux>,
    pub encoder_image: Option<EncoderImage>,
    pub verrouiller_flux: Option<VerrouillerFlux>,
    pub deverrouiller_flux: Option<DeverrouillerFlux>,
    pub verrouiller_tampon_entree: *const c_void,
    pub deverrouiller_tampon_entree: *const c_void,
    pub statistiques: *const c_void,
    pub sequence_params: *const c_void,
    pub register_async_event: *const c_void,
    pub desenregistrer_evenement_async: *const c_void,
    pub projeter_ressource: Option<ProjeterRessource>,
    pub deprojeter_ressource: Option<DeprojeterRessource>,
    pub detruire_encodeur: Option<DetruireEncodeur>,
    pub invalider_images_de_reference: *const c_void,
    pub ouvrir_session_ex: Option<OuvrirSessionEx>,
    pub register_resource: Option<RegisterResourceFn>,
    pub desenregistrer_ressource: Option<DesenregistrerRessource>,
    pub reconfigurer_encodeur: Option<ReconfigurerEncodeur>,
    /// ⚠️ **A real slot, not padding**: `void* reserved1`
    /// sits between `nvEncReconfigureEncoder` and `nvEncCreateMVBuffer`.
    pub reserved1: *const c_void,
    pub create_mv_buffer: *const c_void,
    pub detruire_tampon_mv: *const c_void,
    pub estimation_de_mouvement_seule: *const c_void,
    pub last_error: Option<LastErrorFn>,
    pub flux_cuda: *const c_void,
    pub config_preregleage_ex: Option<PreregleageConfigEx>,
    pub sequence_params_ex: *const c_void,
    pub restaurer_etat: *const c_void,
    pub anticipation: *const c_void,
    pub reserved2: [*const c_void; 275],
}

const _: () = assert!(
    core::mem::size_of::<FunctionList>() == 2552,
    "taille ABI fausse pour NV_ENCODE_API_FUNCTION_LIST"
);
const _: () = assert!(
    core::mem::align_of::<FunctionList>() == 8,
    "alignement ABI faux pour NV_ENCODE_API_FUNCTION_LIST"
);
// Each TYPED slot is pinned: it is what turns "the order is
// the ABI" into a property the compiler checks.
deport!(initialize_encoder, 96);
deport!(create_bitstream_buffer, 120);
deport!(detruire_tampon_de_flux, 128);
deport!(encoder_image, 136);
deport!(verrouiller_flux, 144);
deport!(deverrouiller_flux, 152);
deport!(projeter_ressource, 208);
deport!(deprojeter_ressource, 216);
deport!(detruire_encodeur, 224);
deport!(ouvrir_session_ex, 240);
deport!(register_resource, 248);
deport!(desenregistrer_ressource, 256);
deport!(reconfigurer_encodeur, 264);
deport!(last_error, 304);
deport!(config_preregleage_ex, 320);
deport!(reserved2, 352);

/// `NvEncodeAPICreateInstance`, resolved in the driver's DLL.
pub type CreateInstanceFn = unsafe extern "system" fn(*mut FunctionList) -> Statut;
/// `NvEncodeAPIGetMaxSupportedVersion`. ⚠️ **Its packing is not that
/// of `NVENCAPI_VERSION`** — see `super::abi::version_pilote_attendue`.
pub type VersionMaxSupportee = unsafe extern "system" fn(*mut u32) -> Statut;

/// The name of the driver's DLL. **We do not redistribute it**: it is
/// installed by the NVIDIA driver.
pub const DLL_NVENC: &str = "nvEncodeAPI64.dll";

/// A zeroed `Config`, its `version` set. Convenient and **pure**, hence
/// testable on the host.
///
/// ⚠️ `Config` carries raw pointers: `zeroed` is lawful here because
/// all its fields are integers, integer arrays or pointers,
/// and a null pointer is a valid value for them.
pub fn config_vierge() -> Config {
    let mut c: Config = unsafe { core::mem::zeroed() };
    c.version = super::abi::CONFIG_VER;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 What `NV_ENC_PRESET_CONFIG` requires and which is written nowhere
    /// else: its `preset_cfg.version` must be set **in addition** to its
    /// own. It is the most common first-launch failure of this
    /// API, and it reads `NV_ENC_ERR_INVALID_VERSION`.
    #[test]
    fn un_preregleage_vierge_porte_les_deux_versions() {
        let mut p: PresetConfig = unsafe { core::mem::zeroed() };
        p.version = super::super::abi::PRESET_CONFIG_VER;
        p.preset_cfg.version = super::super::abi::CONFIG_VER;
        assert_eq!(p.version, 0xF205_000C);
        assert_eq!(p.preset_cfg.version, 0xF209_000C);
        assert_ne!(p.version, p.preset_cfg.version, "deux versions distinctes");
    }

    #[test]
    fn une_config_vierge_porte_sa_version_et_rien_d_autre() {
        let c = config_vierge();
        assert_eq!(c.version, 0xF209_000C);
        assert_eq!(c.gop_length, 0);
        assert_eq!(c.rc_params.average_bit_rate, 0);
    }
}
