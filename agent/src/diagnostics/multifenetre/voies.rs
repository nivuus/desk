//! The trait the bench measures, and the paths that implement it.
//!
//! The bench is written ONCE and exercised on each path: without this trait, we
//! would write it once per path and would no longer compare the same things.
//! It is also the seam work stream D will need to make
//! capture substitutable.

use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Storage::Xps::PrintWindow;

use crate::capture::{CapturedFrame, DesktopCapture};
use crate::geometry::Rect;

/// Open capture paths, one per window, and the region each one keeps: the
/// dimensions its frames will carry.
pub(super) type VoiesOuvertes = (Vec<Box<dyn VoieDeCapture>>, Vec<Rect>);

pub(super) trait VoieDeCapture {
    /// Opens a stream on a window. `region` is its place on screen, which
    /// the cropping paths need and which the per-window paths
    /// ignore.
    fn ouvrir(&mut self, hwnd: HWND, region: Rect) -> Result<()>;
    /// Returns the next image, or `None` if none is available.
    ///
    /// `tour` identifies the bench's current tick (see `Mires::trame`, whose
    /// value the bench reads after each `peindre()`). The paths that
    /// share a single source between several windows (see
    /// `SourceDuplication`) use it to acquire that source
    /// only ONCE per round, whatever the number of paths that
    /// crop it afterwards — otherwise the first path queried in a
    /// round consumes the only change the source signals, and the
    /// following ones harvest nothing more (see the comment of
    /// `SourceDuplication`, which documents this starvation as measured
    /// before the fix). The paths without a shared source (`VoiePrintWindow`)
    /// ignore it: each window is captured independently there.
    ///
    /// **Lifetime contract, not guaranteed beyond one call.** The
    /// texture carried by the returned `CapturedFrame` is the cropping
    /// texture SPECIFIC to this path (see `create_crop_texture`):
    /// the next call to `prochaine_image` on the SAME path overwrites it (through
    /// `CopySubresourceRegion` or `UpdateSubresource` depending on the
    /// implementation). It is only valid until that next call.
    /// No effect in this bench, which is synchronous (each image is read or encoded
    /// before the next call) — but any asynchronous consumer of
    /// work stream D would see its image silently rewritten if it
    /// kept a reference to it beyond one round.
    fn prochaine_image(&mut self, tour: u64) -> Result<Option<CapturedFrame>>;
    /// D3D11 device owning the textures returned by this path.
    /// The bench needs it to read a pixel and to create the encoder: a
    /// texture cannot be read from a device other than its own.
    fn device(&self) -> ID3D11Device;
}

/// Allocates a destination D3D11 texture for a crop: format and
/// usage expected by the encoder (`encode/mft/convertisseur.rs::feed_converter`,
/// which wraps
/// the texture through `MFCreateDXGISurfaceBuffer` in non-sRGB BGRA).
///
/// Each path owns ITS OWN, never a texture shared with another
/// path nor the texture internal to `DesktopCapture` (`next_frame` reuses a
/// single slot, whatever the caller: handing it as is to N
/// paths of the same size — the common case here, `disposition::tuiles` produces
/// uniform slots — would make the next path's crop
/// overwrite the previous one's before it consumed it, silently,
/// since `ID3D11Texture2D::clone()` does not copy the content, only the
/// COM reference).
fn create_crop_texture(device: &ID3D11Device, region: Rect) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: region.width,
        Height: region.height,
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
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("allocation d'une texture de recadrage")?;
    texture.ok_or_else(|| anyhow!("texture de recadrage absente"))
}

/// State shared between all `VoieDuplication` instances of the same bench:
/// THE DXGI duplication (only one per output — measured, not assumed: the
/// second `DuplicateOutput` fails with `0x80070057`, see
/// `banc::executer`), and the cache of the current round's desktop image.
///
/// **Fix round 1.** The first version of this file made
/// `next_frame(region)` — acquisition + crop + release in
/// a single call — be called once PER PATH and per round. Measured: at N≥2, a single
/// window stayed fed (~100 fps) and all the others fell below
/// 1.3 fps, whatever N. Cause: `AcquireNextFrame` only signals a
/// desktop change ONCE; the first path of the bench's loop
/// to call it after a change consumes that signal and immediately releases
/// the image; the following paths, called the next microsecond in
/// the SAME round, find nothing new anymore. It was not Desktop
/// Duplication degrading with N, but the measurement architecture:
/// contending for it path by path rather than priming it once and for all.
///
/// The fix: `amorcer` only calls `next_frame` once per value of
/// `tour`, on the WHOLE desktop; each path then makes its own
/// GPU sub-crop (`CopySubresourceRegion`, cheap) from this
/// common image into ITS texture. All paths of the same round thus receive
/// the same freshness — either all a new image, or all nothing,
/// never only one out of N.
pub(super) struct SourceDuplication {
    capture: DesktopCapture,
    bureau: Rect,
    contexte: ID3D11DeviceContext,
    /// Round number for which `last_desktop` was primed. `None`
    /// before the first call.
    last_round: Option<u64>,
    /// Whole desktop image primed for `last_round`. `None` if the
    /// desktop had nothing new at that instant — the common case, not an
    /// error: `DesktopCapture::next_frame` only returns an image when
    /// the desktop changed since the last call.
    last_desktop: Option<CapturedFrame>,
}

impl SourceDuplication {
    /// Dimensions of the TEXTURE that the acquisition of this output returns — not
    /// those announced by DXGI, from which they may differ by a DPI factor.
    pub(super) fn dimensions_bureau(&self) -> (u32, u32) {
        (self.bureau.width, self.bureau.height)
    }

    /// Primes the whole desktop for `tour`, only once per value of
    /// `tour` whatever the number of paths that call this method.
    fn amorcer(&mut self, tour: u64) -> Result<()> {
        if self.last_round == Some(tour) {
            return Ok(());
        }
        self.last_desktop = self
            .capture
            .next_frame(self.bureau)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.last_round = Some(tour);
        Ok(())
    }
}

/// Reference path: Desktop Duplication of the desktop, cropped to the
/// window. It is the current production behaviour — the one already known
/// to get polluted under covering. It serves as a yardstick: a path that does not
/// do better than it brings nothing, and if it did NOT get polluted on the
/// bench, it is the bench that would have to be suspected.
///
/// Is NOT faithful to production in the strict sense: production
/// today only captures ONE window per session (see `CLAUDE.md`,
/// "Known Constraints"). Sharing a single duplication between N
/// crops, measured and fixed here (see `SourceDuplication`), is a
/// behaviour work stream D will have to build, not one that already exists.
pub(super) struct VoieDuplication {
    source: Rc<RefCell<SourceDuplication>>,
    region: Rect,
    /// Texture specific to this path (see `create_crop_texture`), allocated
    /// at `ouvrir()`.
    texture: Option<ID3D11Texture2D>,
}

impl VoieDuplication {
    /// Creates the shared duplication, once for the whole bench, on the designated
    /// DXGI output — or on the desktop's if none is designated.
    ///
    /// The desktop returned by `desktop_size()` is that of THIS output, in
    /// its MODE dimensions — that is the physical dimensions, whereas
    /// `DXGI_OUTPUT_DESC::DesktopCoordinates` gives the dimensions scaled
    /// by DPI. The regions passed to `ouvrir()` must therefore be
    /// expressed in the texture's frame of reference, not in that of the windows:
    /// see `moniteurs_virtuels::vers_texture`, which `banc::executer` uses
    /// to convert.
    pub(super) fn partagee_sur(sortie: Option<&str>) -> Result<Rc<RefCell<SourceDuplication>>> {
        let capture = match sortie {
            Some(nom) => DesktopCapture::sur_sortie(nom)?,
            None => DesktopCapture::new()?,
        };
        let (largeur, hauteur) = capture.desktop_size();
        let contexte = unsafe { capture.device().GetImmediateContext() }
            .context("contexte immédiat pour les sous-recadrages partagés")?;
        Ok(Rc::new(RefCell::new(SourceDuplication {
            capture,
            bureau: Rect {
                x: 0,
                y: 0,
                width: largeur,
                height: hauteur,
            },
            contexte,
            last_round: None,
            last_desktop: None,
        })))
    }

    pub(super) fn new(source: Rc<RefCell<SourceDuplication>>) -> Self {
        Self {
            source,
            region: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            texture: None,
        }
    }
}

impl VoieDeCapture for VoieDuplication {
    fn ouvrir(&mut self, _hwnd: HWND, region: Rect) -> Result<()> {
        self.region = region;
        let device = self.source.borrow().capture.device().clone();
        self.texture = Some(create_crop_texture(&device, region)?);
        Ok(())
    }

    fn prochaine_image(&mut self, tour: u64) -> Result<Option<CapturedFrame>> {
        let mut source = self.source.borrow_mut();
        source.amorcer(tour)?;
        let Some(bureau) = source.last_desktop.as_ref() else {
            return Ok(None);
        };

        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("texture de recadrage non ouverte (ouvrir() jamais appelée)"))?;
        // GPU sub-crop, cheap, from the common desktop image
        // already primed by `SourceDuplication::amorcer` — not a new
        // `AcquireNextFrame`.
        let box_ = D3D11_BOX {
            left: self.region.x.max(0) as u32,
            top: self.region.y.max(0) as u32,
            front: 0,
            right: self.region.x.max(0) as u32 + self.region.width,
            bottom: self.region.y.max(0) as u32 + self.region.height,
            back: 1,
        };
        unsafe {
            source.contexte.CopySubresourceRegion(
                texture,
                0,
                0,
                0,
                0,
                &bureau.texture,
                0,
                Some(&box_),
            );
        }

        Ok(Some(CapturedFrame {
            texture: texture.clone(),
            width: self.region.width,
            height: self.region.height,
        }))
    }

    fn device(&self) -> ID3D11Device {
        // `ID3D11Device` is a reference-counted COM pointer:
        // cloning it does not duplicate the device, it increments a counter.
        // Returning a value rather than a reference spares the bench from holding a
        // borrow on the path while it calls it.
        self.source.borrow().capture.device().clone()
    }
}

/// Path 4: `PrintWindow(PW_RENDERFULLCONTENT)`, behind the trait.
///
/// Verdict of phase 1 (`replis.rs`): CONDITIONAL — the image returned under
/// covering is correct (`verdict=Juste`, exact pixel), but the path is
/// CPU, not GPU. Wired here to put a figure on THAT cost at N windows rather than
/// leaving it theoretical: it is the most useful information that remained to
/// produce for the next work stream.
///
/// Unlike `VoieDuplication`, each instance contends for no
/// resource limited in number: `PrintWindow` addresses a
/// HWND directly, without the ceiling of ONE single DXGI duplication per output. No
/// equivalent `SourceDuplication` here, and `prochaine_image` ignores its
/// `tour` parameter: each window is captured independently, there is
/// nothing to amortise between paths. The D3D11 device is nevertheless created
/// only once for the whole bench and cloned into each path (a COM clone
/// only increments a reference counter): eight
/// independent devices would bring nothing and would complicate consistency with
/// the encoder, which must run on THE device of its path.
pub(super) struct VoiePrintWindow {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    hwnd: HWND,
    region: Rect,
    /// Upload texture, allocated at opening once the size is
    /// known, then reused at each image (`UpdateSubresource`): it is
    /// the CPU→GPU transfer this path must measure, not an
    /// allocation at each frame.
    texture: Option<ID3D11Texture2D>,
}

/// Creates a hardware D3D11 device with BGRA support.
///
/// `pub(super)` because `paralleles.rs` needs it for its test patterns: it
/// cannot borrow the one of a provisional `DesktopCapture`, DXGI
/// allowing only ONE duplication per output — the provisional one would make
/// the real one fail with 0x80070057.
pub(super) fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .context("périphérique D3D11 pour la voie printwindow")?;
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?;
    let context = context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?;
    Ok((device, context))
}

impl VoiePrintWindow {
    /// Creates the shared D3D11 device, once for the whole bench.
    ///
    /// Without an explicit adapter target: as for the test patterns
    /// (`capture::DesktopCapture::new`), the default adapter is the one
    /// that carries the VM's real GPU, the only one able to then host
    /// the hardware encoder.
    pub(super) fn partagee() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
        create_device()
    }

    pub(super) fn new(device: ID3D11Device, context: ID3D11DeviceContext) -> Self {
        Self {
            device,
            context,
            hwnd: HWND::default(),
            region: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            texture: None,
        }
    }
}

impl VoieDeCapture for VoiePrintWindow {
    fn ouvrir(&mut self, hwnd: HWND, region: Rect) -> Result<()> {
        self.hwnd = hwnd;
        self.region = region;
        self.texture = Some(create_crop_texture(&self.device, region)?);
        Ok(())
    }

    fn prochaine_image(&mut self, _tour: u64) -> Result<Option<CapturedFrame>> {
        let (largeur, hauteur) = (self.region.width, self.region.height);

        // GDI capture into a memory DC, as in phase 1 (`replis.rs`):
        // it is precisely this transfer into main memory that this
        // path must put a figure on, not bypass.
        let ecran = unsafe { GetDC(None) };
        let memoire = unsafe { CreateCompatibleDC(Some(ecran)) };
        let bitmap = unsafe { CreateCompatibleBitmap(ecran, largeur as i32, hauteur as i32) };
        let ancien = unsafe { SelectObject(memoire, bitmap.into()) };

        let rendu = unsafe { PrintWindow(self.hwnd, memoire, super::replis::PW_RENDERFULLCONTENT) }
            .as_bool();

        let mut tampon = vec![0u8; (largeur as usize) * (hauteur as usize) * 4];
        let mut entete = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: largeur as i32,
                // Negative: top-down bitmap, same row order as a
                // D3D11 texture — otherwise the uploaded image would be
                // flipped vertically.
                biHeight: -(hauteur as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lignes_lues = unsafe {
            GetDIBits(
                memoire,
                bitmap,
                0,
                hauteur,
                Some(tampon.as_mut_ptr() as *mut _),
                &mut entete,
                DIB_RGB_COLORS,
            )
        };

        unsafe {
            SelectObject(memoire, ancien);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memoire);
            ReleaseDC(None, ecran);
        }

        // `PrintWindow` failed or `GetDIBits` copied no row:
        // no image available, like a DXGI capture that has nothing
        // new — not a bench failure.
        if !rendu || lignes_lues == 0 {
            return Ok(None);
        }

        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("texture printwindow non ouverte (ouvrir() jamais appelée)"))?;
        // The cost measured by this path: uploading the CPU bitmap to the
        // GPU texture the encoder will consume.
        unsafe {
            self.context.UpdateSubresource(
                texture,
                0,
                None,
                tampon.as_ptr() as *const _,
                largeur * 4,
                0,
            );
        }

        Ok(Some(CapturedFrame {
            texture: texture.clone(),
            width: largeur,
            height: hauteur,
        }))
    }

    fn device(&self) -> ID3D11Device {
        self.device.clone()
    }
}
