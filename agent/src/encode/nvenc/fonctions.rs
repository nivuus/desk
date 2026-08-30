//! `NV_ENCODE_API_FUNCTION_LIST` — la table de fonctions que le pilote
//! remplit, et les signatures de celles que ce produit appelle.
//!
//! 🔴 **NOTICE DE LICENCE, PROVENANCE ET COMMANDE DE RELECTURE : voir
//! `super::abi`.** Même transcription, même notice.
//!
//! 🔴 **L'ORDRE DES CHAMPS EST L'ABI.** Cette table n'est pas une commodité :
//! le pilote écrit des pointeurs à des déports fixes, et un champ déplacé
//! d'un cran fait appeler une fonction pour une autre — avec des arguments
//! d'une troisième. C'est pour cela que **les quarante-cinq emplacements sont
//! tous déclarés dans l'ordre**, y compris ceux qu'on n'appelle jamais.
//!
//! ⚠️ **Trente emplacements sont volontairement laissés OPAQUES**
//! (`*const c_void`). Un emplacement de pointeur de fonction pèse la taille
//! d'un pointeur, quel que soit son type : les laisser opaques **préserve
//! exactement l'ABI** et retire trente signatures à transcrire — donc trente
//! occasions de se tromper — pour des fonctions que rien n'appelle. Le jour
//! où l'une sert, elle se type **à ce moment-là**, et son déport asserté le
//! prouvera restée en place.
//!
//! ⚠️ **Un piège de l'en-tête, relevé et sans effet ici** : les emplacements
//! `nvEncGetEncodeProfileGUIDCount` et `…GUIDs` y sont déclarés avec les
//! *typedefs des préréglages*, pas ceux des profils. C'est une coquille amont
//! — corrigée au tag `n13.1.15.0` — qui **ne change aucun déport**. Les deux
//! sont opaques ici de toute façon.

#![allow(dead_code)]

use core::ffi::c_void;

use super::structures::{
    Config, Guid, InitializeParams, OpenEncodeSessionExParams, PresetConfig,
};
use super::tampons::{CreateBitstreamBuffer, LockBitstream, MapInputResource, PicParams, RegisterResource};

macro_rules! deport {
    ($champ:ident, $valeur:expr) => {
        const _: () = assert!(
            core::mem::offset_of!(ListeDeFonctions, $champ) == $valeur,
            concat!("déport ABI faux pour NV_ENCODE_API_FUNCTION_LIST.", stringify!($champ))
        );
    };
}

/// Le code de retour de toute fonction NVENC (`NVENCSTATUS`).
///
/// ⚠️ **`u32` et non un `enum` Rust** : les valeurs de l'en-tête sont
/// **implicites** (0, 1, 2… sans `= n`), donc insérer une variante décalerait
/// tout ce qui suit — et un pilote futur peut en ajouter. Voir
/// `super::abi::SUCCESS` et `super::abi::ERR_INVALID_VERSION`.
pub type Statut = u32;

/// ⚠️ **`extern "system"` et non `extern "C"`** : l'en-tête déclare
/// `__stdcall` sur Windows. Sur x86-64 les deux coïncident, mais l'écrire
/// juste coûte le même prix et reste correct si la cible change.
pub type OuvrirSessionEx =
    unsafe extern "system" fn(*mut OpenEncodeSessionExParams, *mut *mut c_void) -> Statut;
pub type PreregleageConfigEx = unsafe extern "system" fn(
    *mut c_void,
    Guid,
    Guid,
    u32,
    *mut PresetConfig,
) -> Statut;
pub type InitialiserEncodeur =
    unsafe extern "system" fn(*mut c_void, *mut InitializeParams) -> Statut;
pub type CreerTamponDeFlux =
    unsafe extern "system" fn(*mut c_void, *mut CreateBitstreamBuffer) -> Statut;
pub type DetruireTamponDeFlux = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type EnregistrerRessource =
    unsafe extern "system" fn(*mut c_void, *mut RegisterResource) -> Statut;
pub type DesenregistrerRessource = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type ProjeterRessource =
    unsafe extern "system" fn(*mut c_void, *mut MapInputResource) -> Statut;
pub type DeprojeterRessource = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type EncoderImage = unsafe extern "system" fn(*mut c_void, *mut PicParams) -> Statut;
pub type VerrouillerFlux = unsafe extern "system" fn(*mut c_void, *mut LockBitstream) -> Statut;
pub type DeverrouillerFlux = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;
pub type DetruireEncodeur = unsafe extern "system" fn(*mut c_void) -> Statut;
/// ⚠️ **Ne rend PAS un `Statut`** mais une chaîne C — seule exception de la
/// table, et elle est facile à transcrire de travers.
pub type DerniereErreur = unsafe extern "system" fn(*mut c_void) -> *const core::ffi::c_char;
/// `NV_ENC_RECONFIGURE_PARAMS` n'est pas transcrite : ce chemin passera
/// probablement par une reconstruction d'encodeur, comme le fait déjà la MFT.
/// L'emplacement reste typé « opaque mais nommé » pour qu'on le retrouve.
pub type ReconfigurerEncodeur = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Statut;

/// `NV_ENCODE_API_FUNCTION_LIST`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ListeDeFonctions {
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
    pub initialiser_encodeur: Option<InitialiserEncodeur>,
    pub creer_tampon_entree: *const c_void,
    pub detruire_tampon_entree: *const c_void,
    pub creer_tampon_de_flux: Option<CreerTamponDeFlux>,
    pub detruire_tampon_de_flux: Option<DetruireTamponDeFlux>,
    pub encoder_image: Option<EncoderImage>,
    pub verrouiller_flux: Option<VerrouillerFlux>,
    pub deverrouiller_flux: Option<DeverrouillerFlux>,
    pub verrouiller_tampon_entree: *const c_void,
    pub deverrouiller_tampon_entree: *const c_void,
    pub statistiques: *const c_void,
    pub parametres_de_sequence: *const c_void,
    pub enregistrer_evenement_async: *const c_void,
    pub desenregistrer_evenement_async: *const c_void,
    pub projeter_ressource: Option<ProjeterRessource>,
    pub deprojeter_ressource: Option<DeprojeterRessource>,
    pub detruire_encodeur: Option<DetruireEncodeur>,
    pub invalider_images_de_reference: *const c_void,
    pub ouvrir_session_ex: Option<OuvrirSessionEx>,
    pub enregistrer_ressource: Option<EnregistrerRessource>,
    pub desenregistrer_ressource: Option<DesenregistrerRessource>,
    pub reconfigurer_encodeur: Option<ReconfigurerEncodeur>,
    /// ⚠️ **Un vrai emplacement, pas du remplissage** : `void* reserved1`
    /// s'intercale entre `nvEncReconfigureEncoder` et `nvEncCreateMVBuffer`.
    pub reserved1: *const c_void,
    pub creer_tampon_mv: *const c_void,
    pub detruire_tampon_mv: *const c_void,
    pub estimation_de_mouvement_seule: *const c_void,
    pub derniere_erreur: Option<DerniereErreur>,
    pub flux_cuda: *const c_void,
    pub config_preregleage_ex: Option<PreregleageConfigEx>,
    pub parametres_de_sequence_ex: *const c_void,
    pub restaurer_etat: *const c_void,
    pub anticipation: *const c_void,
    pub reserved2: [*const c_void; 275],
}

const _: () = assert!(
    core::mem::size_of::<ListeDeFonctions>() == 2552,
    "taille ABI fausse pour NV_ENCODE_API_FUNCTION_LIST"
);
const _: () = assert!(
    core::mem::align_of::<ListeDeFonctions>() == 8,
    "alignement ABI faux pour NV_ENCODE_API_FUNCTION_LIST"
);
// Chaque emplacement TYPÉ est épinglé : c'est ce qui transforme « l'ordre est
// l'ABI » en une propriété que le compilateur vérifie.
deport!(initialiser_encodeur, 96);
deport!(creer_tampon_de_flux, 120);
deport!(detruire_tampon_de_flux, 128);
deport!(encoder_image, 136);
deport!(verrouiller_flux, 144);
deport!(deverrouiller_flux, 152);
deport!(projeter_ressource, 208);
deport!(deprojeter_ressource, 216);
deport!(detruire_encodeur, 224);
deport!(ouvrir_session_ex, 240);
deport!(enregistrer_ressource, 248);
deport!(desenregistrer_ressource, 256);
deport!(reconfigurer_encodeur, 264);
deport!(derniere_erreur, 304);
deport!(config_preregleage_ex, 320);
deport!(reserved2, 352);

/// `NvEncodeAPICreateInstance`, résolue dans la DLL du pilote.
pub type CreerInstance = unsafe extern "system" fn(*mut ListeDeFonctions) -> Statut;
/// `NvEncodeAPIGetMaxSupportedVersion`. ⚠️ **Son empaquetage n'est pas celui
/// de `NVENCAPI_VERSION`** — voir `super::abi::version_pilote_attendue`.
pub type VersionMaxSupportee = unsafe extern "system" fn(*mut u32) -> Statut;

/// Le nom de la DLL du pilote. **Nous ne la redistribuons pas** : elle est
/// installée par le pilote NVIDIA.
pub const DLL_NVENC: &str = "nvEncodeAPI64.dll";

/// Un `Config` mis à zéro, sa `version` posée. Pratique et **pur**, donc
/// éprouvable sur l'hôte.
///
/// ⚠️ `Config` porte des pointeurs bruts : `zeroed` est licite ici parce que
/// tous ses champs sont des entiers, des tableaux d'entiers ou des pointeurs,
/// et qu'un pointeur nul est une valeur valide pour eux.
pub fn config_vierge() -> Config {
    let mut c: Config = unsafe { core::mem::zeroed() };
    c.version = super::abi::CONFIG_VER;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 Ce que `NV_ENC_PRESET_CONFIG` exige et qui n'est écrit nulle part
    /// ailleurs : sa `preset_cfg.version` doit être posée **en plus** de la
    /// sienne. C'est l'échec de premier lancement le plus courant de cette
    /// API, et il se lit `NV_ENC_ERR_INVALID_VERSION`.
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
