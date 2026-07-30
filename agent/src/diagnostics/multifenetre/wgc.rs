//! Voie 1 : `Windows.Graphics.Capture`, re-test honnête.
//!
//! Abandonnée au jalon 1 (commit `4493b24`) : `captureservice.dll` plantait
//! en `0xc0000005` de façon déterministe et `IsSupported()` levait
//! `E_OUTOFMEMORY` avec 13 Go libres. C'est pourtant la seule voie qui donne
//! la capture hors-écran gratuitement — l'écarter sans re-test coûterait cher
//! au chantier D.
//!
//! Cette sonde tourne SEULE dans son processus : si le service replante, elle
//! emporte ce processus et aucun autre.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::core::Interface;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;

pub(super) fn eprouver() -> Result<()> {
    // `IsSupported` d'abord, et journalisé même en cas de succès : c'est
    // l'appel qui levait `E_OUTOFMEMORY` au jalon 1.
    let supporte = match GraphicsCaptureSession::IsSupported() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            tracing::error!(%erreur, "verdict WGC : ÉLIMINÉE — IsSupported a échoué");
            return Ok(());
        }
    };
    tracing::info!(supporte, "GraphicsCaptureSession::IsSupported");
    if !supporte {
        tracing::error!("verdict WGC : ÉLIMINÉE — l'API se déclare non supportée");
        return Ok(());
    }

    // Deux mires côte à côte : celle du dessous est la fenêtre observée, celle
    // du dessus viendra la recouvrir. C'est le seul test qui distingue WGC
    // d'un recadrage de bureau.
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect { x: 0, y: 0, width: largeur, height: hauteur },
        2,
    )
    .context("deux places sur ce bureau")?;
    let mut mires = Mires::ouvrir(capture.device(), &places)?;
    mires.peindre()?;
    mires.pomper();

    let dxgi: IDXGIDevice = capture.device().cast().context("IDXGIDevice")?;
    let winrt = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .context("périphérique WinRT depuis le périphérique DXGI")?;
    let winrt: IDirect3DDevice = winrt.cast().context("IDirect3DDevice")?;

    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .context("fabrique d'interop GraphicsCaptureItem")?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(mires.hwnd(0)?) }
        .context("CreateForWindow sur la mire observée")?;
    tracing::info!("CreateForWindow a réussi — le service de capture a répondu");

    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        item.Size()?,
    )
    .context("création du pool de trames")?;
    let session = pool.CreateCaptureSession(&item).context("session de capture")?;
    session.StartCapture().context("démarrage de la capture")?;

    // Recouvrement : la mire 1 passe par-dessus la mire 0. WGC doit continuer
    // de rendre la mire 0 — c'est toute la question.
    mires.recouvrir(1, 0)?;

    let mut recues = 0usize;
    let mut dernier_verdict = mire::Verdict::Inconnue;
    let echeance = Instant::now() + Duration::from_secs(8);
    while Instant::now() < echeance {
        mires.peindre()?;
        mires.pomper();
        if let Ok(trame) = pool.TryGetNextFrame() {
            let surface = trame.Surface()?;
            let acces: IDirect3DDxgiInterfaceAccess = surface.cast()?;
            let texture: ID3D11Texture2D = unsafe { acces.GetInterface() }?;
            let taille = trame.ContentSize()?;
            let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
                capture.device(),
                &texture,
                taille.Width as u32,
                taille.Height as u32,
                taille.Width as u32 / 2,
                taille.Height as u32 / 2,
            )?;
            dernier_verdict = mire::verdict(0, (r, g, b));
            recues += 1;
        }
        std::thread::sleep(Duration::from_millis(8));
    }

    tracing::info!(recues, ?dernier_verdict, "trames WGC reçues sous recouvrement");
    match (recues > 0, dernier_verdict) {
        (true, mire::Verdict::Juste) => tracing::info!(
            "verdict WGC : VIABLE — la fenêtre recouverte reste capturée correctement"
        ),
        (true, autre) => tracing::error!(
            ?autre,
            "verdict WGC : ÉLIMINÉE — des trames arrivent mais pas le bon contenu"
        ),
        (false, _) => tracing::error!("verdict WGC : ÉLIMINÉE — aucune trame en 8 s"),
    }
    Ok(())
}
