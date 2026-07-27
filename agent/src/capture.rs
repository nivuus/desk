//! Capture d'une fenêtre via Windows.Graphics.Capture.
//!
//! Les images restent sur le GPU : `next_texture` renvoie une `ID3D11Texture2D`
//! que l'encodeur consomme directement, sans aller-retour en mémoire centrale.

#![cfg(windows)]

use std::sync::mpsc::{channel, Receiver, TryRecvError};

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

/// Une image capturée, encore résidente sur le GPU.
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
}

pub struct WindowCapture {
    device: ID3D11Device,
    _context: ID3D11DeviceContext,
    _session: GraphicsCaptureSession,
    frame_pool: Direct3D11CaptureFramePool,
    frames: Receiver<()>,
}

impl WindowCapture {
    pub fn new(hwnd: HWND) -> Result<Self> {
        // Le runtime WinRT doit être initialisé sur le thread appelant avant
        // toute utilisation de Windows.Graphics.Capture : sans cela,
        // `IGraphicsCaptureItemInterop::CreateForWindow` échoue avec
        // RPC_S_SERVER_UNAVAILABLE (0x800706BE), un message trompeur qui ne
        // mentionne jamais COM/WinRT. MTA convient ici puisque
        // `CreateFreeThreaded` évite justement de dépendre d'une pompe de
        // messages STA. `RPC_E_CHANGED_MODE` signifie que le thread est déjà
        // initialisé dans un autre mode : ce n'est pas une erreur pour nous.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .or_else(|e| {
                if e.code() == windows::Win32::Foundation::RPC_E_CHANGED_MODE {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .context("initialisation du runtime WinRT (RoInitialize)")?;

        let (device, context) = create_d3d_device()?;
        let direct3d_device = wrap_device_for_winrt(&device)?;

        // Interop WinRT : obtenir un GraphicsCaptureItem pour une HWND Win32.
        let interop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .context("fabrique IGraphicsCaptureItemInterop")?;
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForWindow(hwnd) }.context("capture de la fenêtre")?;

        let size = item.Size()?;
        // CreateFreeThreaded évite d'avoir à faire tourner une pompe de messages.
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &direct3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2, // deux tampons : capture et encodage se recouvrent
            size,
        )
        .context("création du pool d'images")?;

        // Le rappel signale seulement l'arrivée d'une image ; la texture est
        // récupérée dans next_texture, sur le fil de la boucle principale.
        let (notify, frames) = channel::<()>();
        // écart d'API windows-rs 0.62 : `TypedEventHandler::new` attend des
        // paramètres `windows::core::Ref<T>` (une référence empruntée
        // compatible ABI), plus `&Option<T>` comme dans les versions
        // antérieures de windows-rs.
        frame_pool.FrameArrived(&TypedEventHandler::new(
            move |_pool: windows::core::Ref<'_, Direct3D11CaptureFramePool>,
                  _frame: windows::core::Ref<'_, windows::core::IInspectable>| {
                let _ = notify.send(());
                Ok(())
            },
        ))?;

        let session = frame_pool
            .CreateCaptureSession(&item)
            .context("création de la session de capture")?;
        // Supprime la bordure jaune de capture sur Windows 11 ; échoue en silence
        // sur les versions antérieures, ce qui est acceptable.
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().context("démarrage de la capture")?;

        Ok(Self {
            device,
            _context: context,
            _session: session,
            frame_pool,
            frames,
        })
    }

    /// Récupère l'image la plus récente, ou `None` si aucune n'est disponible.
    ///
    /// Les images en retard sont volontairement écartées : en streaming, une
    /// image périmée n'a aucune valeur face à celle qui la suit.
    pub fn next_texture(&mut self) -> Result<Option<CapturedFrame>> {
        let mut available = false;
        loop {
            match self.frames.try_recv() {
                Ok(()) => available = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err(anyhow!("la capture s'est arrêtée"))
                }
            }
        }
        if !available {
            return Ok(None);
        }

        let mut latest = None;
        while let Ok(frame) = self.frame_pool.TryGetNextFrame() {
            let surface = frame.Surface()?;
            let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
            let texture: ID3D11Texture2D = unsafe { access.GetInterface() }?;
            let size = frame.ContentSize()?;
            latest = Some(CapturedFrame {
                texture,
                width: size.Width.max(0) as u32,
                height: size.Height.max(0) as u32,
            });
        }
        Ok(latest)
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }
}

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            // écart d'API windows-rs 0.62 : `software` est un `HMODULE` non
            // optionnel (plus `Option<HMODULE>`) ; un module nul signifie
            // « pas de rasteriseur logiciel », ce qui est le comportement
            // voulu ici puisqu'on demande un pilote matériel.
            HMODULE::default(),
            // BGRA_SUPPORT est obligatoire pour l'interopérabilité WinRT.
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .context("création du périphérique D3D11")?;
    }
    Ok((
        device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?,
        context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?,
    ))
}

fn wrap_device_for_winrt(
    device: &ID3D11Device,
) -> Result<windows::Graphics::DirectX::Direct3D11::IDirect3DDevice> {
    let dxgi: IDXGIDevice = device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .context("conversion du périphérique pour WinRT")?;
    Ok(inspectable.cast()?)
}
