//! La session NVENC : charger la porte, l'ouvrir sur notre périphérique
//! D3D11, encoder une texture, relire le flux.
//!
//! 🔴 **CE MODULE N'A JAMAIS TOURNÉ.** Il compile pour la cible ; il n'a été
//! exécuté sur aucune machine. La VM appartenait à un autre lot au moment où
//! il a été écrit. **Tout ce qu'il affirme sur le comportement de NVENC vient
//! de l'en-tête ou d'une mesure faite sur Apollo, jamais d'une mesure faite
//! sur CE code.** Ce que la recette du document de résultats doit trancher
//! est exactement cela.
//!
//! ⚠️ **`#[cfg(windows)]` chez son parent, qui est pur** : c'est l'inverse du
//! motif habituel, et c'est voulu. Tout le chemin NVENC vit sous
//! `encode_nvenc` — la règle de choix, l'ABI, les dispositions — pour que la
//! frontière d'attribution de la notice de licence reste **un seul
//! sous-arbre**. Seul ce fichier-ci a besoin de Windows ; le reste se teste
//! sur l'hôte.
//!
//! ## L'ordre des appels, et ce que chacun peut refuser
//!
//! 1. `NvEncodeAPIGetMaxSupportedVersion` — **avant tout le reste**, sinon un
//!    pilote trop ancien échoue plus tard et moins lisiblement.
//! 2. `NvEncodeAPICreateInstance` — remplit la table de fonctions.
//! 3. `nvEncOpenEncodeSessionEx` — sur notre `ID3D11Device`.
//! 4. `nvEncGetEncodePresetConfigEx` — **les DEUX versions posées**.
//! 5. `nvEncInitializeEncoder`.
//! 6. `nvEncCreateBitstreamBuffer`.
//! Puis, par image : enregistrer, projeter, encoder, verrouiller, **copier**,
//! déverrouiller, déprojeter.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::ffi::c_void;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};

use super::abi;
use super::porte::{verifier, Porte};
use super::structures::{
    Config, InitializeParams, OpenEncodeSessionExParams, PresetConfig, ReconfigureParams,
};
use super::tampons::{CreateBitstreamBuffer, LockBitstream, MapInputResource, PicParams, RegisterResource};

/// Une session d'encodage : un encodeur, son tampon de flux, et le cache des
/// textures déjà enregistrées.
pub struct SessionNvenc {
    porte: Porte,
    encodeur: *mut c_void,
    tampon_de_flux: *mut c_void,
    /// 🔴 **Une texture ne se réenregistre pas à chaque image.**
    /// `nvEncRegisterResource` est coûteux, et la duplication DXGI rend
    /// souvent la MÊME texture. La clé est le pointeur COM brut.
    /// ⚠️ **Ce cache suppose qu'un pointeur réutilisé désigne la même
    /// texture** — vrai tant que nous tenons une référence sur chacune, ce
    /// que `textures` garantit en les gardant vivantes.
    enregistrees: HashMap<*mut c_void, *mut c_void>,
    textures: Vec<ID3D11Texture2D>,
    largeur: u32,
    hauteur: u32,
    debit_bps: u32,
    /// Posé par `demander_image_cle`, consommé par la prochaine image.
    image_cle_demandee: bool,
    /// 🔴 **Gardés parce que `nvEncReconfigureEncoder` les re-exige.**
    /// Reconfigurer, c'est re-soumettre l'initialisation entière avec le
    /// débit changé ; sans copie de l'originale, `regler_debit` devrait la
    /// réinventer, et toute divergence deviendrait un changement de réglage
    /// silencieux.
    ///
    /// ⚠️ **La `Box` est indispensable** : `InitializeParams::encode_config`
    /// est un pointeur BRUT vers cette `Config`. Sur la pile, elle bougerait.
    config: Box<Config>,
    /// L'initialisation d'origine, **son pointeur de configuration remis à
    /// zéro** : il est reposé à chaque usage, plutôt que gardé — un champ
    /// qui pointe vers un frère de la même structure est exactement le
    /// motif auto-référentiel que Rust ne garantit pas.
    init: InitializeParams,
}

impl SessionNvenc {
    /// Ouvre une session sur le périphérique D3D11 **de la capture**.
    ///
    /// ⚠️ **`enable_encode_async = 0`** : mode synchrone. C'est un choix, pas
    /// une contrainte — le mode asynchrone de NVENC exige un événement Win32
    /// par image et un fil pour l'attendre, là où la façade `H264Encoder`
    /// expose déjà un `submit` / `poll_output` que le mode synchrone remplit
    /// directement. **Non mesuré** : rien ici n'établit ce que le mode
    /// asynchrone changerait à la latence.
    pub fn ouvrir(
        peripherique: &ID3D11Device,
        largeur: u32,
        hauteur: u32,
        fps: u32,
        debit_bps: u32,
    ) -> Result<Self> {
        let porte = Porte::ouvrir()?;

        let mut params: OpenEncodeSessionExParams = unsafe { std::mem::zeroed() };
        params.version = abi::OPEN_ENCODE_SESSION_EX_PARAMS_VER;
        params.device_type = abi::DEVICE_TYPE_DIRECTX;
        params.device = peripherique.as_raw();
        // ⚠️ `NVENCAPI_VERSION`, PAS une version de structure.
        params.api_version = abi::VERSION_API;

        let mut encodeur: *mut c_void = std::ptr::null_mut();
        let ouvrir = porte
            .fonctions
            .ouvrir_session_ex
            .ok_or_else(|| anyhow!("emplacement nvEncOpenEncodeSessionEx vide"))?;
        verifier(
            unsafe { ouvrir(&mut params, &mut encodeur) },
            "nvEncOpenEncodeSessionEx",
        )?;

        let mut session = Self {
            porte,
            encodeur,
            tampon_de_flux: std::ptr::null_mut(),
            enregistrees: HashMap::new(),
            textures: Vec::new(),
            largeur,
            hauteur,
            debit_bps,
            image_cle_demandee: false,
            config: Box::new(unsafe { std::mem::zeroed() }),
            init: unsafe { std::mem::zeroed() },
        };
        session.initialiser(fps, debit_bps)?;
        Ok(session)
    }

    fn initialiser(&mut self, fps: u32, debit_bps: u32) -> Result<()> {
        // ① Partir du préréglage, jamais d'une configuration vierge : NVIDIA
        // y met des valeurs par défaut que nous n'avons aucune raison de
        // réinventer.
        let mut preregleage: PresetConfig = unsafe { std::mem::zeroed() };
        preregleage.version = abi::PRESET_CONFIG_VER;
        // 🔴 LES DEUX VERSIONS. Sans celle-ci : NV_ENC_ERR_INVALID_VERSION.
        preregleage.preset_cfg.version = abi::CONFIG_VER;
        let config_preregleage = self
            .porte
            .fonctions
            .config_preregleage_ex
            .ok_or_else(|| anyhow!("emplacement nvEncGetEncodePresetConfigEx vide"))?;
        verifier(
            unsafe {
                config_preregleage(
                    self.encodeur,
                    super::structures::CODEC_H264,
                    super::structures::PRESET_P1,
                    abi::TUNING_ULTRA_LOW_LATENCY,
                    &mut preregleage,
                )
            },
            "nvEncGetEncodePresetConfigEx",
        )?;

        let mut config: Config = preregleage.preset_cfg;
        config.version = abi::CONFIG_VER;
        config.profile_guid = super::structures::PROFILE_H264_BASELINE;
        // Groupe d'images ouvert : les images clés se demandent à la volée,
        // comme sur le chemin MFT (`CODECAPI_AVEncMPVGOPSize` à 0).
        config.gop_length = u32::MAX;
        // Aucune image B : ordre de décodage = ordre d'affichage, ce que
        // l'interactif exige.
        config.frame_interval_p = 1;
        config.rc_params.version = abi::RC_PARAMS_VER;
        config.rc_params.rate_control_mode = abi::RC_MODE_CBR;
        config.rc_params.average_bit_rate = debit_bps;
        config.rc_params.max_bit_rate = debit_bps;
        // Tampon VBV d'une seule image : c'est ce qui borne la latence.
        // ⚠️ **NON CALIBRÉ** — comme toutes les constantes de ce dépôt.
        config.rc_params.vbv_buffer_size = debit_bps / fps.max(1);
        config.rc_params.vbv_initial_delay = config.rc_params.vbv_buffer_size;

        // SAFETY : l'union ne porte que du H.264 sur ce chemin, et le
        // préréglage vient d'être écrit par le pilote pour ce même codec.
        let h264 = unsafe { &mut config.encode_codec_config.h264 };
        // 🔴 Sans ceci, un pair qui arrive en cours de route n'a JAMAIS de
        // SPS/PPS et ne décode rien.
        h264.drapeaux |= super::structures::H264_REPEAT_SPS_PPS;
        h264.idr_period = u32::MAX;

        let mut init: InitializeParams = unsafe { std::mem::zeroed() };
        init.version = abi::INITIALIZE_PARAMS_VER;
        init.encode_guid = super::structures::CODEC_H264;
        init.preset_guid = super::structures::PRESET_P1;
        init.encode_width = self.largeur;
        init.encode_height = self.hauteur;
        init.dar_width = self.largeur;
        init.dar_height = self.hauteur;
        init.frame_rate_num = fps.max(1);
        init.frame_rate_den = 1;
        init.enable_encode_async = 0;
        // Décision de type d'image confiée à l'encodeur : c'est ce que
        // `NV_ENC_PIC_FLAG_FORCEIDR` exige pour être honoré.
        init.enable_ptd = 1;
        *self.config = config;
        init.encode_config = &mut *self.config;
        init.tuning_info = abi::TUNING_ULTRA_LOW_LATENCY;
        // 🔴 ARGB, pas ABGR : c'est `DXGI_FORMAT_B8G8R8A8_UNORM`, ce que rend
        // la duplication. Voir `abi::BUFFER_FORMAT_ARGB`.
        init.buffer_format = abi::BUFFER_FORMAT_ARGB;

        let initialiser = self
            .porte
            .fonctions
            .initialiser_encodeur
            .ok_or_else(|| anyhow!("emplacement nvEncInitializeEncoder vide"))?;
        verifier(
            unsafe { initialiser(self.encodeur, &mut init) },
            "nvEncInitializeEncoder",
        )?;
        // Garder l'initialisation SANS son pointeur : il est reposé à chaque
        // usage, sur la `Box` qui, elle, ne bouge pas.
        init.encode_config = std::ptr::null_mut();
        self.init = init;

        let mut tampon: CreateBitstreamBuffer = unsafe { std::mem::zeroed() };
        tampon.version = abi::CREATE_BITSTREAM_BUFFER_VER;
        let creer_tampon = self
            .porte
            .fonctions
            .creer_tampon_de_flux
            .ok_or_else(|| anyhow!("emplacement nvEncCreateBitstreamBuffer vide"))?;
        verifier(
            unsafe { creer_tampon(self.encodeur, &mut tampon) },
            "nvEncCreateBitstreamBuffer",
        )?;
        self.tampon_de_flux = tampon.bitstream_buffer;

        tracing::info!(
            largeur = self.largeur,
            hauteur = self.hauteur,
            fps,
            debit_bps,
            "session NVENC native initialisée (P1, ultra faible latence, CBR)"
        );
        Ok(())
    }

    /// Enregistre une texture si elle ne l'est pas déjà, et rend sa ressource.
    fn ressource(&mut self, texture: &ID3D11Texture2D) -> Result<*mut c_void> {
        let cle = texture.as_raw();
        if let Some(deja) = self.enregistrees.get(&cle) {
            return Ok(*deja);
        }
        let mut demande: RegisterResource = unsafe { std::mem::zeroed() };
        demande.version = abi::REGISTER_RESOURCE_VER;
        demande.resource_type = abi::INPUT_RESOURCE_TYPE_DIRECTX;
        demande.width = self.largeur;
        demande.height = self.hauteur;
        demande.pitch = 0;
        demande.resource_to_register = cle;
        demande.buffer_format = abi::BUFFER_FORMAT_ARGB;
        demande.buffer_usage = abi::BUFFER_USAGE_INPUT_IMAGE;
        let enregistrer = self
            .porte
            .fonctions
            .enregistrer_ressource
            .ok_or_else(|| anyhow!("emplacement nvEncRegisterResource vide"))?;
        verifier(
            unsafe { enregistrer(self.encodeur, &mut demande) },
            "nvEncRegisterResource",
        )?;
        // Garder une référence vivante : le cache est indexé par POINTEUR, et
        // un pointeur réutilisé après libération désignerait autre chose.
        self.textures.push(texture.clone());
        self.enregistrees.insert(cle, demande.registered_resource);
        Ok(demande.registered_resource)
    }

    /// Demande une image clé sur la prochaine image soumise.
    pub fn demander_image_cle(&mut self) {
        self.image_cle_demandee = true;
    }

    pub fn taille(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    pub fn debit(&self) -> u32 {
        self.debit_bps
    }

    /// Change le débit d'un encodeur vivant.
    ///
    /// 🔴 **Un vrai appel, pas un silence.** Le dépôt pilote le débit vidéo
    /// par cette voie (`transport/adaptation.rs`) ; accepter l'appel sans
    /// rien faire rendrait toute l'adaptation de bande passante
    /// **invisiblement inopérante**.
    ///
    /// ⚠️ **Sans `resetEncoder`** : réinitialiser rendrait la référence
    /// temporelle et forcerait une image clé à chaque ajustement de débit,
    /// c'est-à-dire une rafale à l'instant précis où l'on essaie de réduire
    /// le trafic. Le débit se change à chaud.
    pub fn regler_debit(&mut self, bps: u32) -> Result<()> {
        self.config.rc_params.average_bit_rate = bps;
        self.config.rc_params.max_bit_rate = bps;

        let mut params: ReconfigureParams = unsafe { std::mem::zeroed() };
        params.version = abi::RECONFIGURE_PARAMS_VER;
        params.re_init_encode_params = self.init;
        params.re_init_encode_params.encode_config = &mut *self.config;

        let reconfigurer = self
            .porte
            .fonctions
            .reconfigurer_encodeur
            .ok_or_else(|| anyhow!("emplacement nvEncReconfigureEncoder vide"))?;
        verifier(
            unsafe { reconfigurer(self.encodeur, &mut params) },
            "nvEncReconfigureEncoder",
        )?;
        self.debit_bps = bps;
        Ok(())
    }

    /// Encode une texture et rend les octets du flux.
    ///
    /// ⚠️ **Rend `Ok(None)` si l'encodeur a demandé plus d'entrée** — cas
    /// normal de cette API, et non une erreur.
    pub fn encoder(&mut self, texture: &ID3D11Texture2D, pts_100ns: u64) -> Result<Option<Vec<u8>>> {
        let ressource = self.ressource(texture)?;

        let mut projection: MapInputResource = unsafe { std::mem::zeroed() };
        projection.version = abi::MAP_INPUT_RESOURCE_VER;
        projection.registered_resource = ressource;
        let projeter = self
            .porte
            .fonctions
            .projeter_ressource
            .ok_or_else(|| anyhow!("emplacement nvEncMapInputResource vide"))?;
        verifier(
            unsafe { projeter(self.encodeur, &mut projection) },
            "nvEncMapInputResource",
        )?;

        let issue = self.encoder_projetee(&projection, pts_100ns);

        // Déprojeter DANS TOUS LES CAS : une ressource projetée qui ne l'est
        // jamais fait échouer la projection suivante, et le symptôme serait
        // attribué à la mauvaise image.
        if let Some(deprojeter) = self.porte.fonctions.deprojeter_ressource {
            let _ = unsafe { deprojeter(self.encodeur, projection.mapped_resource) };
        }
        issue
    }

    fn encoder_projetee(
        &mut self,
        projection: &MapInputResource,
        pts_100ns: u64,
    ) -> Result<Option<Vec<u8>>> {
        let mut image: PicParams = unsafe { std::mem::zeroed() };
        image.version = abi::PIC_PARAMS_VER;
        image.input_width = self.largeur;
        image.input_height = self.hauteur;
        image.input_buffer = projection.mapped_resource;
        image.output_bitstream = self.tampon_de_flux;
        image.buffer_fmt = abi::BUFFER_FORMAT_ARGB;
        // ⚠️ Vaut 1, pas 0 : une structure mise à zéro serait fausse ici.
        image.picture_struct = abi::PIC_STRUCT_FRAME;
        image.input_time_stamp = pts_100ns;
        if std::mem::take(&mut self.image_cle_demandee) {
            // Les deux ensemble : une IDR sans SPS/PPS ne sert à rien à un
            // pair qui vient d'arriver.
            image.encode_pic_flags = abi::PIC_FLAG_FORCEIDR | abi::PIC_FLAG_OUTPUT_SPSPPS;
        }

        let encoder = self
            .porte
            .fonctions
            .encoder_image
            .ok_or_else(|| anyhow!("emplacement nvEncEncodePicture vide"))?;
        let statut = unsafe { encoder(self.encodeur, &mut image) };
        // 17 = NV_ENC_ERR_NEED_MORE_INPUT : l'encodeur a mis l'image en
        // tampon. Ce n'est PAS une erreur.
        if statut == 17 {
            return Ok(None);
        }
        verifier(statut, "nvEncEncodePicture")?;

        let mut verrou: LockBitstream = unsafe { std::mem::zeroed() };
        verrou.version = abi::LOCK_BITSTREAM_VER;
        verrou.output_bitstream = self.tampon_de_flux;
        let verrouiller = self
            .porte
            .fonctions
            .verrouiller_flux
            .ok_or_else(|| anyhow!("emplacement nvEncLockBitstream vide"))?;
        verifier(
            unsafe { verrouiller(self.encodeur, &mut verrou) },
            "nvEncLockBitstream",
        )?;

        // 🔴 COPIER PENDANT LE VERROU. Les octets ne sont valides qu'entre le
        // verrouillage et le déverrouillage ; en garder un emprunt serait une
        // lecture après libération que rien ne signalerait.
        let octets = unsafe {
            std::slice::from_raw_parts(
                verrou.bitstream_buffer_ptr as *const u8,
                verrou.bitstream_size_in_bytes as usize,
            )
        }
        .to_vec();

        if let Some(deverrouiller) = self.porte.fonctions.deverrouiller_flux {
            verifier(
                unsafe { deverrouiller(self.encodeur, self.tampon_de_flux) },
                "nvEncUnlockBitstream",
            )?;
        }
        Ok(Some(octets))
    }
}

impl Drop for SessionNvenc {
    fn drop(&mut self) {
        // L'ordre est celui de l'en-tête, à l'envers de la construction.
        for (_, ressource) in self.enregistrees.drain() {
            if let Some(desenregistrer) = self.porte.fonctions.desenregistrer_ressource {
                let _ = unsafe { desenregistrer(self.encodeur, ressource) };
            }
        }
        if !self.tampon_de_flux.is_null() {
            if let Some(detruire) = self.porte.fonctions.detruire_tampon_de_flux {
                let _ = unsafe { detruire(self.encodeur, self.tampon_de_flux) };
            }
        }
        if !self.encodeur.is_null() {
            if let Some(detruire) = self.porte.fonctions.detruire_encodeur {
                let _ = unsafe { detruire(self.encodeur) };
            }
        }
    }
}
