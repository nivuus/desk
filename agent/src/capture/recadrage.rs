//! Cropping: copy the requested region of a duplicated frame into a dedicated
//! texture, on the GPU, never going through main memory.
//!
//! Split out of `capture.rs` when `cargo fmt` pushed that file past 500 lines.

use super::*;

impl DesktopCapture {
    /// Copie la région demandée dans une texture dédiée, sur le GPU.
    pub(super) fn crop(&mut self, source: &ID3D11Texture2D, region: Rect) -> Result<CapturedFrame> {
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
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
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

        Ok(CapturedFrame {
            texture: texture.clone(),
            width,
            height,
        })
    }
}
