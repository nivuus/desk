//! The NVENC session: load the door, open it on our D3D11
//! device, encode a texture, read the stream back.
//!
//! 🟢 **THIS MODULE HAS RUN**, on 30 August 2026, on the target VM: the door
//! opens, the session initialises, and encoding returns **1195 access
//! units in 10 s**. This file's header carried "this module has never
//! run" between its writing and this measurement — it was true then, and
//! erasing it without saying so would have made disappear the only thing that
//! distinguished a design from a fact.
//!
//! ⚠️ **WHAT IS STILL NOT TESTED HERE**: synchronous mode with
//! **several encoders** (the bench only opens one), bitrate
//! reconfiguration (`regler_debit` has **never** been called on the machine), and the
//! error path of each call. What ran is the NOMINAL path.
//!
//! ⚠️ **`#[cfg(windows)]` in its parent, which is pure**: it is the reverse of the
//! usual pattern, and it is intended. The whole NVENC path lives under
//! `encode_nvenc` — the choice rule, the ABI, the layouts — so that the
//! attribution boundary of the licence notice remains **a single
//! subtree**. Only this file needs Windows; the rest is tested
//! on the host.
//!
//! ## The order of the calls, and what each one can refuse
//!
//! 1. `NvEncodeAPIGetMaxSupportedVersion` — **before everything else**, otherwise a
//!    driver too old fails later and less readably.
//! 2. `NvEncodeAPICreateInstance` — fills the function table.
//! 3. `nvEncOpenEncodeSessionEx` — on our `ID3D11Device`.
//! 4. `nvEncGetEncodePresetConfigEx` — **BOTH versions set**.
//! 5. `nvEncInitializeEncoder`.
//! 6. `nvEncCreateBitstreamBuffer`.
//!
//! Then, per image: register, map, encode, lock, **copy**,
//! unlock, unmap.

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
use super::tampons::{
    CreateBitstreamBuffer, LockBitstream, MapInputResource, PicParams, RegisterResource,
};

/// An encoding session: an encoder, its stream buffer, and the cache of
/// already registered textures.
pub struct SessionNvenc {
    porte: Porte,
    encodeur: *mut c_void,
    tampon_de_flux: *mut c_void,
    /// 🔴 **A texture is not re-registered at each image.**
    /// `nvEncRegisterResource` is costly, and DXGI duplication often returns
    /// the SAME texture. The key is the raw COM pointer.
    /// ⚠️ **This cache assumes that a reused pointer designates the same
    /// texture** — true as long as we hold a reference on each, which
    /// `textures` guarantees by keeping them alive.
    enregistrees: HashMap<*mut c_void, *mut c_void>,
    textures: Vec<ID3D11Texture2D>,
    largeur: u32,
    hauteur: u32,
    debit_bps: u32,
    /// Set by `demander_image_cle`, consumed by the next image.
    image_cle_demandee: bool,
    /// 🔴 **Kept because `nvEncReconfigureEncoder` requires them again.**
    /// Reconfiguring means resubmitting the whole initialisation with the
    /// changed bitrate; without a copy of the original, `regler_debit` would have to
    /// reinvent it, and any divergence would become a silent settings
    /// change.
    ///
    /// ⚠️ **The `Box` is indispensable**: `InitializeParams::encode_config`
    /// is a RAW pointer to this `Config`. On the stack, it would move.
    config: Box<Config>,
    /// The original initialisation, **its configuration pointer reset to
    /// zero**: it is set again at each use, rather than kept — a field
    /// pointing to a sibling of the same structure is exactly the
    /// self-referential pattern Rust does not guarantee.
    init: InitializeParams,
}

impl SessionNvenc {
    /// Opens a session on the D3D11 device **of the capture**.
    ///
    /// ⚠️ **`enable_encode_async = 0`**: synchronous mode. It is a choice, not
    /// a constraint — NVENC's asynchronous mode requires a Win32 event
    /// per image and a thread to wait for it, whereas the `H264Encoder` facade
    /// already exposes a `submit` / `poll_output` that synchronous mode fills
    /// directly. **Not measured**: nothing here establishes what asynchronous
    /// mode would change to latency.
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
        // ⚠️ `NVENCAPI_VERSION`, NOT a structure version.
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
        // ① Start from the preset, never from a blank configuration: NVIDIA
        // puts default values there that we have no reason to
        // reinvent.
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
        // Open group of pictures: key frames are requested on the fly,
        // as on the MFT path (`CODECAPI_AVEncMPVGOPSize` at 0).
        config.gop_length = u32::MAX;
        // No B frames: decoding order = display order, which
        // interactive use requires.
        config.frame_interval_p = 1;
        config.rc_params.version = abi::RC_PARAMS_VER;
        config.rc_params.rate_control_mode = abi::RC_MODE_CBR;
        config.rc_params.average_bit_rate = debit_bps;
        config.rc_params.max_bit_rate = debit_bps;
        // VBV buffer of a single image: it is what bounds latency.
        // ⚠️ **NOT CALIBRATED** — like all the constants of this repository.
        config.rc_params.vbv_buffer_size = debit_bps / fps.max(1);
        config.rc_params.vbv_initial_delay = config.rc_params.vbv_buffer_size;

        // SAFETY: the union only carries H.264 on this path, and the
        // preset has just been written by the driver for this same codec.
        let h264 = unsafe { &mut config.encode_codec_config.h264 };
        // 🔴 Without this, a peer arriving midway NEVER gets
        // SPS/PPS and decodes nothing.
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
        // Picture type decision entrusted to the encoder: it is what
        // `NV_ENC_PIC_FLAG_FORCEIDR` requires to be honoured.
        init.enable_ptd = 1;
        *self.config = config;
        init.encode_config = &mut *self.config;
        init.tuning_info = abi::TUNING_ULTRA_LOW_LATENCY;
        // 🔴 ARGB, not ABGR: it is `DXGI_FORMAT_B8G8R8A8_UNORM`, what
        // duplication returns. See `abi::BUFFER_FORMAT_ARGB`.
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
        // Keep the initialisation WITHOUT its pointer: it is set again at each
        // use, on the `Box` which, for its part, does not move.
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

    /// Registers a texture if it is not already, and returns its resource.
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
        // Keep a live reference: the cache is indexed by POINTER, and
        // a pointer reused after being freed would designate something else.
        self.textures.push(texture.clone());
        self.enregistrees.insert(cle, demande.registered_resource);
        Ok(demande.registered_resource)
    }

    /// Requests a key frame on the next submitted image.
    pub fn demander_image_cle(&mut self) {
        self.image_cle_demandee = true;
    }

    pub fn taille(&self) -> (u32, u32) {
        (self.largeur, self.hauteur)
    }

    pub fn debit(&self) -> u32 {
        self.debit_bps
    }

    /// Changes the bitrate of a live encoder.
    ///
    /// 🔴 **A real call, not a silence.** The repository drives the video bitrate
    /// through this path (`transport/adaptation.rs`); accepting the call without
    /// doing anything would make all bandwidth adaptation
    /// **invisibly inoperative**.
    ///
    /// ⚠️ **Without `resetEncoder`**: resetting would reset the temporal
    /// reference and force a key frame at each bitrate adjustment,
    /// that is a burst at the precise moment we try to reduce
    /// traffic. The bitrate is changed on the fly.
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

    /// Encodes a texture and returns the stream's bytes.
    ///
    /// ⚠️ **Returns `Ok(None)` if the encoder asked for more input** — a normal
    /// case of this API, and not an error.
    pub fn encoder(
        &mut self,
        texture: &ID3D11Texture2D,
        pts_100ns: u64,
    ) -> Result<Option<Vec<u8>>> {
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

        // Unmap IN ALL CASES: a mapped resource that is never unmapped
        // makes the next mapping fail, and the symptom would be
        // attributed to the wrong image.
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
        // ⚠️ Is 1, not 0: a zeroed structure would be wrong here.
        image.picture_struct = abi::PIC_STRUCT_FRAME;
        image.input_time_stamp = pts_100ns;
        if std::mem::take(&mut self.image_cle_demandee) {
            // Both together: an IDR without SPS/PPS is useless to a
            // peer that just arrived.
            image.encode_pic_flags = abi::PIC_FLAG_FORCEIDR | abi::PIC_FLAG_OUTPUT_SPSPPS;
        }

        let encoder = self
            .porte
            .fonctions
            .encoder_image
            .ok_or_else(|| anyhow!("emplacement nvEncEncodePicture vide"))?;
        let statut = unsafe { encoder(self.encodeur, &mut image) };
        // 17 = NV_ENC_ERR_NEED_MORE_INPUT: the encoder buffered the
        // image. It is NOT an error.
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

        // 🔴 COPY DURING THE LOCK. The bytes are only valid between
        // lock and unlock; keeping a borrow of them would be a
        // use-after-free that nothing would report.
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
        // The order is that of the header, the reverse of construction.
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
