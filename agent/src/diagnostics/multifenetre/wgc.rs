//! Path 1: `Windows.Graphics.Capture`, honest re-test.
//!
//! Abandoned at milestone 1 (commit `4493b24`): `captureservice.dll` crashed
//! with `0xc0000005` deterministically and `IsSupported()` raised
//! `E_OUTOFMEMORY` with 13 GB free. Yet it is the only path that gives
//! off-screen capture for free — ruling it out without a re-test would cost
//! work stream D dearly.
//!
//! This probe runs ALONE in its process: if the service crashes again, it
//! takes down this process and no other.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::core::Interface;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
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
    // `IsSupported` first, and logged even on success: it is
    // the call that raised `E_OUTOFMEMORY` at milestone 1.
    let supporte = match GraphicsCaptureSession::IsSupported() {
        Ok(valeur) => valeur,
        Err(erreur) => {
            tracing::error!(
                causes = %causes(erreur),
                "verdict WGC : ÉLIMINÉE — IsSupported a échoué"
            );
            return Ok(());
        }
    };
    tracing::info!(supporte, "GraphicsCaptureSession::IsSupported");
    if !supporte {
        tracing::error!("verdict WGC : ÉLIMINÉE — l'API se déclare non supportée");
        return Ok(());
    }

    // Two test patterns side by side: the one below is the observed window, the one
    // above will come to cover it. It is the only test that distinguishes WGC
    // from a desktop crop.
    //
    // A bench failure, not a verdict on WGC: if the desktop cannot be
    // split into two slots or if the test pattern windows do not open,
    // no measurement is possible — it is not WGC that is at fault. These
    // failures are therefore still propagated by `?`, unlike what follows.
    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect {
            x: 0,
            y: 0,
            width: largeur,
            height: hauteur,
        },
        2,
    )
    .context("deux places sur ce bureau")?;
    let mut mires = Mires::ouvrir(capture.device(), &places)?;
    mires.peindre()?;
    mires.pomper();

    // From here on, any failure IS a measurement result on WGC: fix
    // round 1 found that a failure of `CreateForWindow`
    // went up through `?` to `main()` without ever uttering one of the
    // "verdict WGC : …" messages below — a reader of the log
    // could not rely on them to find the verdict. `preparer_session`
    // therefore isolates the whole measurement zone, and its failure becomes here a
    // logged ÉLIMINÉE verdict, rather than a propagated error.
    let (pool, _session) = match preparer_session(&capture, &mires) {
        Ok(paire) => paire,
        Err(erreur) => {
            tracing::error!(
                causes = %causes(erreur),
                "verdict WGC : ÉLIMINÉE — la préparation de la capture a échoué"
            );
            return Ok(());
        }
    };

    // Covering: test pattern 1 goes over test pattern 0. WGC must keep
    // rendering test pattern 0 — that is the whole question. A failure here
    // (`SetWindowPos`) remains a bench failure, independent of WGC.
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

    tracing::info!(
        recues,
        ?dernier_verdict,
        "trames WGC reçues sous recouvrement"
    );
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

/// Prepares the WGC capture session proper: WinRT device
/// from the shared DXGI device, `GraphicsCaptureItem` interop,
/// `CreateForWindow`, frame pool, session, start.
///
/// Isolated in its own function so that `eprouver` can convert any
/// failure from here into an ÉLIMINÉE verdict rather than a propagated error: it is the
/// measurement zone proper (what WGC can or cannot do),
/// whereas what surrounds it in `eprouver` (opening of the test patterns,
/// covering) remains a bench failure if it fails.
///
/// Returns the session with the pool: `GraphicsCaptureSession` must stay alive
/// during the whole capture (the caller keeps it bound, even without ever
/// using it directly again) — letting it drop here would end it.
fn preparer_session(
    capture: &crate::capture::DesktopCapture,
    mires: &Mires,
) -> Result<(Direct3D11CaptureFramePool, GraphicsCaptureSession)> {
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
    let session = pool
        .CreateCaptureSession(&item)
        .context("session de capture")?;
    session.StartCapture().context("démarrage de la capture")?;

    Ok((pool, session))
}

/// Joins the full chain of an error's causes on a single log
/// line ("outermost cause: next cause: …").
///
/// `Display` on an error wrapped by `.context(...)` only shows the
/// outermost context message: fix round 2
/// found in the persisted log that the native HRESULT (`0x800706BE`)
/// disappeared from it entirely whenever the failure was caught here rather
/// than propagated up to `main()` (which, for its part, displays the full chain through
/// `Debug`). `impl Into<anyhow::Error>` accepts both an error already
/// wrapped by `anyhow` (like that of `preparer_session`, where the
/// conversion is the identity) and a raw `windows::core::Error` error
/// never wrapped (like that of `IsSupported()`) — the same function thus serves
/// both verdicts without duplicating the joining logic.
fn causes(erreur: impl Into<anyhow::Error>) -> String {
    erreur
        .into()
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" : ")
}
