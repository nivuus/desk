//! Lecture de pixels d'une texture GPU, pour les modes diagnostic.
//!
//! Copie vers une texture « staging » accessible au CPU
//! (`D3D11_USAGE_STAGING`) : c'est ce qui permet de prouver que le recadrage
//! capture bien le contenu de la fenêtre, et pas seulement des dimensions
//! qui auraient l'air correctes sans l'être.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::{capture, geometry};

/// Lit un pixel BGRA d'une texture GPU en la copiant vers une texture
/// « staging » accessible au CPU (`D3D11_USAGE_STAGING`).
///
/// Sert uniquement au mode diagnostic `CAPTURE_TEST` : prouver que le
/// recadrage capture bien le contenu de la fenêtre, et pas juste des
/// dimensions qui auraient l'air correctes sans l'être (voir l'appelant).
/// Renvoie `(r, g, b, a)`.
pub(super) fn read_pixel(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    texture: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> Result<(u8, u8, u8, u8)> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }
        .context("allocation de la texture de lecture")?;
    let staging = staging.context("texture de lecture absente")?;

    let context = unsafe { device.GetImmediateContext() }.context("contexte immédiat")?;

    unsafe { context.CopyResource(&staging, texture) };

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
        .context("projection de la texture de lecture en mémoire CPU")?;

    let base = mapped.pData as *const u8;
    let offset = (y * mapped.RowPitch + x * 4) as isize;
    // Format BGRA : l'ordre des octets en mémoire est bleu, vert, rouge, alpha.
    let (b, g, r, a) = unsafe {
        (
            *base.offset(offset),
            *base.offset(offset + 1),
            *base.offset(offset + 2),
            *base.offset(offset + 3),
        )
    };

    unsafe { context.Unmap(&staging, 0) };

    Ok((r, g, b, a))
}

/// Acquiert une image pour `region` (en retentant jusqu'à `timeout`) et lit
/// le pixel en son centre.
pub(super) fn capture_center_pixel(
    capture: &mut capture::DesktopCapture,
    region: geometry::Rect,
    timeout: Duration,
) -> Result<(u32, u32, u8, u8, u8, u8)> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Some(frame) = capture.next_frame(region)? {
            let (r, g, b, a) = read_pixel(
                capture.device(),
                &frame.texture,
                frame.width,
                frame.height,
                frame.width / 2,
                frame.height / 2,
            )?;
            return Ok((frame.width, frame.height, r, g, b, a));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    anyhow::bail!("aucune image obtenue pour {region:?} en {timeout:?}")
}
