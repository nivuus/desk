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
    //
    // Panne du banc, pas verdict sur WGC : si le bureau ne peut pas être
    // découpé en deux places ou si les fenêtres de mire ne s'ouvrent pas,
    // aucune mesure n'est possible — ce n'est pas WGC qui est en cause. Ces
    // échecs restent donc propagés par `?`, contrairement à ce qui suit.
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

    // À partir d'ici, tout échec EST un résultat de mesure sur WGC : la
    // ronde de correction 1 a constaté qu'un échec de `CreateForWindow`
    // remontait par `?` jusqu'à `main()` sans jamais prononcer l'un des
    // messages « verdict WGC : … » ci-dessous — un lecteur du journal ne
    // pouvait pas s'y fier pour trouver le verdict. `preparer_session`
    // isole donc toute la zone de mesure, et son échec devient ici un
    // verdict ÉLIMINÉE journalisé, plutôt qu'une erreur propagée.
    let (pool, _session) = match preparer_session(&capture, &mires) {
        Ok(paire) => paire,
        Err(erreur) => {
            tracing::error!(
                %erreur,
                "verdict WGC : ÉLIMINÉE — la préparation de la capture a échoué"
            );
            return Ok(());
        }
    };

    // Recouvrement : la mire 1 passe par-dessus la mire 0. WGC doit continuer
    // de rendre la mire 0 — c'est toute la question. Un échec ici
    // (`SetWindowPos`) reste une panne du banc, indépendante de WGC.
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

/// Prépare la session de capture WGC proprement dite : périphérique WinRT
/// depuis le périphérique DXGI partagé, interop `GraphicsCaptureItem`,
/// `CreateForWindow`, pool de trames, session, démarrage.
///
/// Isolée dans sa propre fonction pour que `eprouver` puisse convertir tout
/// échec d'ici en verdict ÉLIMINÉE plutôt qu'en erreur propagée : c'est la
/// zone de mesure proprement dite (ce que WGC sait ou ne sait pas faire),
/// alors que ce qui l'entoure dans `eprouver` (ouverture des mires,
/// recouvrement) reste une panne du banc si ça échoue.
///
/// Rend la session avec le pool : `GraphicsCaptureSession` doit rester en vie
/// pendant toute la capture (l'appelant la garde liée, même sans plus jamais
/// s'en servir directement) — la laisser retomber ici y mettrait fin.
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
    let session = pool.CreateCaptureSession(&item).context("session de capture")?;
    session.StartCapture().context("démarrage de la capture")?;

    Ok((pool, session))
}
