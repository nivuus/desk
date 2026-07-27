//! Capture de l'écran par DXGI Desktop Duplication, recadrée sur une fenêtre.
//!
//! `Windows.Graphics.Capture` aurait permis de capturer directement la fenêtre,
//! mais cette API est inutilisable sur Windows Server 2022 : le service système
//! qui l'implémente plante sur `CreateForWindow`. On duplique donc la sortie
//! écran et on recadre. Les images restent sur le GPU : le recadrage se fait
//! par `CopySubresourceRegion`, sans aller-retour en mémoire centrale.

#![cfg(windows)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

use crate::geometry::Rect;

/// Une image capturée et recadrée, résidente sur le GPU.
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
}

pub struct DesktopCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    desktop_width: u32,
    desktop_height: u32,
    /// Texture de destination, réallouée seulement quand la taille change.
    target: Option<(ID3D11Texture2D, u32, u32)>,
    /// Vrai tant qu'une image acquise n'a pas été relâchée.
    frame_held: bool,
    /// Étape courante publiée pour le fil de surveillance (voir
    /// `encode::PHASE_CAPTURE_*`). Absente hors mode diagnostic.
    phase: Option<Arc<AtomicU64>>,
}

impl DesktopCapture {
    pub fn new() -> Result<Self> {
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;

        // On retient le premier adaptateur possédant une sortie attachée au
        // bureau : c'est celui qui compose l'écran, et donc le seul duplicable.
        // Sur la VM cible c'est la RTX 4070, ce qui donne du même coup le bon
        // périphérique pour l'encodeur matériel de la tâche 10.
        let (adapter, output) = find_desktop_output(&factory)?;

        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                &adapter,
                // Un adaptateur explicite impose le type « inconnu ».
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .context("création du périphérique D3D11")?;
        }
        let device = device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?;
        let context = context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?;

        let duplication = unsafe { output.DuplicateOutput(&device) }
            .context("duplication de la sortie écran")?;

        // écart d'API windows-rs 0.62 : `GetDesc` ne prend plus de paramètre
        // de sortie ; elle renvoie directement la structure (par valeur pour
        // `IDXGIOutputDuplication`, dans un `Result` pour `IDXGIOutput1` et
        // `IDXGIAdapter1` juste plus bas, ces deux dernières pouvant échouer).
        let desc = unsafe { duplication.GetDesc() };
        let desktop_width = desc.ModeDesc.Width;
        let desktop_height = desc.ModeDesc.Height;
        tracing::info!(desktop_width, desktop_height, "duplication de sortie établie");

        Ok(Self {
            device,
            context,
            duplication,
            desktop_width,
            desktop_height,
            target: None,
            frame_held: false,
            phase: None,
        })
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    /// Branche le marqueur d'étape partagé avec le fil de surveillance.
    ///
    /// Sans lui, un blocage dans `next_frame` reste anonyme : les trois appels
    /// Windows qu'elle enchaîne se confondent en une seule étape.
    pub fn set_phase_marker(&mut self, phase: Arc<AtomicU64>) {
        self.phase = Some(phase);
    }

    fn set_phase(&self, value: u64) {
        if let Some(phase) = &self.phase {
            phase.store(value, Ordering::Relaxed);
        }
    }

    pub fn desktop_size(&self) -> (u32, u32) {
        (self.desktop_width, self.desktop_height)
    }

    /// Acquiert l'image suivante et la recadre sur `region`.
    ///
    /// Renvoie `Ok(None)` si aucune image nouvelle n'est disponible — cas
    /// courant et normal : le bureau ne change pas à chaque appel.
    pub fn next_frame(&mut self, region: Rect) -> Result<Option<CapturedFrame>> {
        self.release_frame();

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        // Attente nulle : la cadence est pilotée par la boucle appelante, pas
        // par un blocage ici.
        self.set_phase(crate::encode::PHASE_CAPTURE_ACQUIRE);
        let acquired = unsafe { self.duplication.AcquireNextFrame(0, &mut info, &mut resource) };
        self.set_phase(crate::encode::PHASE_CAPTURE);

        if let Err(e) = acquired {
            if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Ok(None);
            }
            return Err(anyhow!("acquisition d'image : {e}"));
        }
        self.frame_held = true;

        // À partir d'ici l'image est détenue : tout chemin de sortie doit la
        // relâcher, sans quoi la duplication refuse toute acquisition
        // ultérieure. `release_frame` en tête de la prochaine itération ne
        // couvre pas le cas d'une erreur qui remonte et arrête la boucle,
        // ni celui d'un appelant qui réessaie après avoir avalé l'erreur.
        let cropped = (|| {
            let resource = resource.ok_or_else(|| anyhow!("ressource d'image absente"))?;
            let desktop: ID3D11Texture2D = resource.cast()?;
            self.crop(&desktop, region)
        })();
        match cropped {
            Ok(frame) => Ok(Some(frame)),
            Err(e) => {
                self.release_frame();
                Err(e)
            }
        }
    }

    /// Copie la région demandée dans une texture dédiée, sur le GPU.
    fn crop(&mut self, source: &ID3D11Texture2D, region: Rect) -> Result<CapturedFrame> {
        let (width, height) = (region.width, region.height);
        if width == 0 || height == 0 {
            bail!("région de recadrage vide");
        }

        // Réallouer seulement si la taille a changé : un redimensionnement est
        // rare, une image ne l'est pas.
        let need_alloc = !matches!(self.target, Some((_, w, h)) if w == width && h == height);
        if need_alloc {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut texture: Option<ID3D11Texture2D> = None;
            unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }
                .context("allocation de la texture de recadrage")?;
            self.target = Some((
                texture.ok_or_else(|| anyhow!("texture de recadrage absente"))?,
                width,
                height,
            ));
        }

        let (texture, _, _) = self.target.as_ref().expect("texture allouée");
        let box_ = D3D11_BOX {
            left: region.x.max(0) as u32,
            top: region.y.max(0) as u32,
            front: 0,
            right: region.x.max(0) as u32 + width,
            bottom: region.y.max(0) as u32 + height,
            back: 1,
        };
        self.set_phase(crate::encode::PHASE_CAPTURE_CROP);
        unsafe {
            self.context
                .CopySubresourceRegion(texture, 0, 0, 0, 0, source, 0, Some(&box_));
        }
        self.set_phase(crate::encode::PHASE_CAPTURE);

        Ok(CapturedFrame { texture: texture.clone(), width, height })
    }

    fn release_frame(&mut self) {
        if self.frame_held {
            self.set_phase(crate::encode::PHASE_CAPTURE_RELEASE);
            // Un échec ici n'est pas récupérable et ne doit pas masquer la suite.
            let _ = unsafe { self.duplication.ReleaseFrame() };
            self.set_phase(crate::encode::PHASE_CAPTURE);
            self.frame_held = false;
        }
    }
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        self.release_frame();
    }
}

/// Trouve l'adaptateur et la sortie qui composent le bureau.
fn find_desktop_output(factory: &IDXGIFactory1) -> Result<(IDXGIAdapter1, IDXGIOutput1)> {
    let mut index = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
        index += 1;
        let mut out_index = 0;
        while let Ok(output) = unsafe { adapter.EnumOutputs(out_index) } {
            out_index += 1;
            let desc = match unsafe { output.GetDesc() } {
                Ok(desc) => desc,
                Err(_) => continue,
            };
            if desc.AttachedToDesktop.as_bool() {
                let name = match unsafe { adapter.GetDesc1() } {
                    Ok(adapter_desc) => String::from_utf16_lossy(&adapter_desc.Description)
                        .trim_end_matches('\0')
                        .to_string(),
                    Err(_) => "<inconnu>".to_string(),
                };
                tracing::info!(adaptateur = %name, "sortie attachée au bureau retenue");
                return Ok((adapter.clone(), output.cast()?));
            }
        }
    }
    bail!("aucune sortie attachée au bureau : la session est-elle interactive ?")
}
