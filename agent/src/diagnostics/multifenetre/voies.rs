//! Le trait que le banc mesure, et les voies qui l'implémentent.
//!
//! Le banc est écrit UNE FOIS et exercé sur chaque voie : sans ce trait, on
//! l'écrirait une fois par voie et l'on ne comparerait plus les mêmes choses.
//! C'est aussi la couture dont le chantier D aura besoin pour rendre la
//! capture substituable.

use anyhow::{anyhow, Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_BIND_RENDER_TARGET, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Storage::Xps::PrintWindow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::capture::{CapturedFrame, DesktopCapture};
use crate::geometry::Rect;

pub(super) trait VoieDeCapture {
    fn nom(&self) -> &'static str;
    /// Ouvre un flux sur une fenêtre. `region` est sa place à l'écran, dont
    /// les voies par recadrage ont besoin et que les voies par fenêtre
    /// ignorent.
    fn ouvrir(&mut self, hwnd: HWND, region: Rect) -> Result<()>;
    /// Rend l'image suivante, ou `None` si aucune n'est disponible.
    fn prochaine_image(&mut self) -> Result<Option<CapturedFrame>>;
    /// Périphérique D3D11 propriétaire des textures rendues par cette voie.
    /// Le banc en a besoin pour lire un pixel et pour créer l'encodeur : une
    /// texture ne se lit pas depuis un autre périphérique que le sien.
    fn device(&self) -> ID3D11Device;
}

/// Voie de référence : Desktop Duplication du bureau, recadrée sur la fenêtre.
/// C'est le comportement de production actuel — celui dont on sait déjà qu'il
/// se pollue au recouvrement. Il sert d'étalon : une voie qui ne fait pas
/// mieux que lui n'apporte rien, et s'il ne se polluait PAS au banc, ce
/// serait le banc qu'il faudrait suspecter.
///
/// **Une seule duplication pour toutes les fenêtres.** DXGI n'accorde qu'un
/// nombre très limité de duplications concurrentes d'une même sortie ;
/// en ouvrir une par fenêtre échouerait dès la deuxième. C'est d'ailleurs
/// fidèle à la production : une duplication, N recadrages.
pub(super) struct VoieDuplication {
    capture: Rc<RefCell<DesktopCapture>>,
    region: Rect,
}

impl VoieDuplication {
    /// Crée la duplication partagée, une fois pour tout le banc.
    pub(super) fn partagee() -> Result<Rc<RefCell<DesktopCapture>>> {
        Ok(Rc::new(RefCell::new(DesktopCapture::new()?)))
    }

    pub(super) fn nouvelle(capture: Rc<RefCell<DesktopCapture>>) -> Self {
        Self { capture, region: Rect { x: 0, y: 0, width: 0, height: 0 } }
    }
}

impl VoieDeCapture for VoieDuplication {
    fn nom(&self) -> &'static str {
        "duplication"
    }

    fn ouvrir(&mut self, _hwnd: HWND, region: Rect) -> Result<()> {
        self.region = region;
        Ok(())
    }

    fn prochaine_image(&mut self) -> Result<Option<CapturedFrame>> {
        self.capture.borrow_mut().next_frame(self.region)
    }

    fn device(&self) -> ID3D11Device {
        // `ID3D11Device` est un pointeur COM à comptage de références : le
        // cloner ne duplique pas le périphérique, il incrémente un compteur.
        // Rendre une valeur plutôt qu'une référence évite au banc de tenir un
        // emprunt sur la voie pendant qu'il l'appelle.
        self.capture.borrow().device().clone()
    }
}

/// Voie 4 : `PrintWindow(PW_RENDERFULLCONTENT)`, derrière le trait.
///
/// Verdict du temps 1 (`replis.rs`) : CONDITIONNELLE — l'image rendue sous
/// recouvrement est juste (`verdict=Juste`, pixel exact), mais le chemin est
/// CPU, pas GPU. Câblée ici pour chiffrer CE coût à N fenêtres plutôt que de
/// le laisser théorique : c'est l'information la plus utile qui restait à
/// produire pour le chantier suivant.
///
/// Contrairement à `VoieDuplication`, chaque instance ne se dispute aucune
/// ressource limitée en nombre : `PrintWindow` s'adresse directement à un
/// HWND, sans le plafond de duplications concurrentes de DXGI. Le
/// périphérique D3D11 est néanmoins créé une seule fois pour tout le banc et
/// cloné dans chaque voie (un clone COM n'incrémente qu'un compteur de
/// références) : huit périphériques indépendants n'apporteraient rien et
/// compliqueraient la cohérence avec l'encodeur, qui doit tourner sur LE
/// périphérique de sa voie.
pub(super) struct VoiePrintWindow {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    hwnd: HWND,
    region: Rect,
    /// Texture de téléversement, allouée à l'ouverture une fois la taille
    /// connue, puis réutilisée à chaque image (`UpdateSubresource`) : c'est
    /// le rapatriement CPU→GPU que cette voie doit mesurer, pas une
    /// allocation à chaque trame.
    texture: Option<ID3D11Texture2D>,
}

impl VoiePrintWindow {
    /// Crée le périphérique D3D11 partagé, une fois pour tout le banc.
    ///
    /// Sans cible d'adaptateur explicite : comme pour les mires
    /// (`capture::DesktopCapture::new`), l'adaptateur par défaut est celui
    /// qui porte le GPU réel de la VM, seul capable d'héberger ensuite
    /// l'encodeur matériel.
    pub(super) fn partagee() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
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

    pub(super) fn nouvelle(device: ID3D11Device, context: ID3D11DeviceContext) -> Self {
        Self {
            device,
            context,
            hwnd: HWND::default(),
            region: Rect { x: 0, y: 0, width: 0, height: 0 },
            texture: None,
        }
    }
}

impl VoieDeCapture for VoiePrintWindow {
    fn nom(&self) -> &'static str {
        "printwindow"
    }

    fn ouvrir(&mut self, hwnd: HWND, region: Rect) -> Result<()> {
        self.hwnd = hwnd;
        self.region = region;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: region.width,
            Height: region.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut texture)) }
            .context("allocation de la texture de téléversement printwindow")?;
        self.texture = texture;
        Ok(())
    }

    fn prochaine_image(&mut self) -> Result<Option<CapturedFrame>> {
        let (largeur, hauteur) = (self.region.width, self.region.height);

        // Capture GDI dans un DC mémoire, comme au temps 1 (`replis.rs`) :
        // c'est précisément ce rapatriement en mémoire centrale que cette
        // voie doit chiffrer, pas contourner.
        let ecran = unsafe { GetDC(None) };
        let memoire = unsafe { CreateCompatibleDC(Some(ecran)) };
        let bitmap = unsafe { CreateCompatibleBitmap(ecran, largeur as i32, hauteur as i32) };
        let ancien = unsafe { SelectObject(memoire, bitmap.into()) };

        let rendu =
            unsafe { PrintWindow(self.hwnd, memoire, super::replis::PW_RENDERFULLCONTENT) }
                .as_bool();

        let mut tampon = vec![0u8; (largeur as usize) * (hauteur as usize) * 4];
        let mut entete = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: largeur as i32,
                // Négatif : bitmap top-down, même ordre de lignes qu'une
                // texture D3D11 — sans quoi l'image téléversée serait
                // retournée verticalement.
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

        // `PrintWindow` en échec ou `GetDIBits` n'ayant copié aucune ligne :
        // aucune image disponible, comme une capture DXGI qui n'a rien de
        // neuf — pas une panne du banc.
        if !rendu || lignes_lues == 0 {
            return Ok(None);
        }

        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("texture printwindow non ouverte (ouvrir() jamais appelée)"))?;
        // Le coût mesuré par cette voie : téléverser le bitmap CPU vers la
        // texture GPU que l'encodeur consommera.
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

        Ok(Some(CapturedFrame { texture: texture.clone(), width: largeur, height: hauteur }))
    }

    fn device(&self) -> ID3D11Device {
        self.device.clone()
    }
}
