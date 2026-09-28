//! Reading pixels of a GPU texture, for the diagnostic modes.
//!
//! Copy into a CPU-accessible "staging" texture
//! (`D3D11_USAGE_STAGING`): it is what makes it possible to prove that the crop
//! does capture the window's content, and not only dimensions
//! that would look correct without being so.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::{capture, geometry};

/// Reads a BGRA pixel of a GPU texture by copying it into a
/// CPU-accessible "staging" texture (`D3D11_USAGE_STAGING`).
///
/// Only serves the `CAPTURE_TEST` diagnostic mode: proving that the
/// crop does capture the window's content, and not just
/// dimensions that would look correct without being so (see the caller).
/// Returns `(r, g, b, a)`.
pub(crate) fn read_pixel(
    device: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
    texture: &windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> Result<(u8, u8, u8, u8)> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut staging = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }
        .context("allocating the readback texture")?;
    let staging = staging.context("readback texture absent")?;

    let context = unsafe { device.GetImmediateContext() }.context("immediate context")?;

    unsafe { context.CopyResource(&staging, texture) };

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
        .context("mapping the readback texture into CPU memory")?;

    let base = mapped.pData as *const u8;
    let offset = (y * mapped.RowPitch + x * 4) as isize;
    // BGRA format: the byte order in memory is blue, green, red, alpha.
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

/// Acquires an image for `region` (retrying until `timeout`) and reads
/// the pixel at its centre.
pub(super) fn capture_center_pixel(
    capture: &mut capture::DesktopCapture,
    region: geometry::Rect,
    timeout: Duration,
) -> Result<(u32, u32, u8, u8, u8, u8)> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Some(frame) = capture
            .next_frame(region)
            .map_err(|e| anyhow::anyhow!("{e}"))?
        {
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
    anyhow::bail!("no image obtained for {region:?} within {timeout:?}")
}
