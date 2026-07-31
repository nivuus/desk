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
    D3D11_BIND_RENDER_TARGET, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
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
    ///
    /// `tour` identifie le tick courant du banc (voir `Mires::trame`, dont
    /// le banc lit la valeur après chaque `peindre()`). Les voies qui
    /// partagent une source unique entre plusieurs fenêtres (voir
    /// `SourceDuplication`) s'en servent pour n'acquérir cette source
    /// qu'UNE FOIS par tour, quel que soit le nombre de voies qui la
    /// recadrent ensuite — sans quoi la première voie interrogée dans un
    /// tour consomme le seul changement que la source signale, et les
    /// suivantes ne récoltent plus rien (voir le commentaire de
    /// `SourceDuplication`, qui documente cette famine telle que mesurée
    /// avant correction). Les voies sans source partagée (`VoiePrintWindow`)
    /// l'ignorent : chaque fenêtre s'y capture indépendamment.
    ///
    /// **Contrat de durée de vie, non garanti au-delà d'un appel.** La
    /// texture portée par le `CapturedFrame` rendu est la texture de
    /// recadrage PROPRE à cette voie (voir `creer_texture_recadrage`) :
    /// l'appel suivant à `prochaine_image` sur la MÊME voie l'écrase (par
    /// `CopySubresourceRegion` ou `UpdateSubresource` selon
    /// l'implémentation). Elle n'est valide que jusqu'à cet appel suivant.
    /// Sans effet dans ce banc, synchrone (chaque image est lue ou encodée
    /// avant l'appel suivant) — mais tout consommateur asynchrone du
    /// chantier D verrait son image réécrite silencieusement s'il en
    /// conservait une référence au-delà d'un tour.
    fn prochaine_image(&mut self, tour: u64) -> Result<Option<CapturedFrame>>;
    /// Périphérique D3D11 propriétaire des textures rendues par cette voie.
    /// Le banc en a besoin pour lire un pixel et pour créer l'encodeur : une
    /// texture ne se lit pas depuis un autre périphérique que le sien.
    fn device(&self) -> ID3D11Device;
}

/// Alloue une texture D3D11 de destination pour un recadrage : format et
/// usage attendus par l'encodeur (`encode.rs::feed_converter`, qui enveloppe
/// la texture via `MFCreateDXGISurfaceBuffer` en BGRA non-sRGB).
///
/// Chaque voie possède la SIENNE, jamais une texture partagée avec une autre
/// voie ni la texture interne à `DesktopCapture` (`next_frame` réutilise un
/// seul emplacement, quel que soit l'appelant : le confier tel quel à N
/// voies de même taille — le cas courant ici, `disposition::tuiles` produit
/// des places uniformes — ferait que le recadrage de la voie suivante
/// écrase celui de la précédente avant qu'elle l'ait consommé, silencieusement,
/// puisque `ID3D11Texture2D::clone()` ne copie pas le contenu, seulement la
/// référence COM).
fn creer_texture_recadrage(device: &ID3D11Device, region: Rect) -> Result<ID3D11Texture2D> {
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
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("allocation d'une texture de recadrage")?;
    texture.ok_or_else(|| anyhow!("texture de recadrage absente"))
}

/// État partagé entre toutes les instances `VoieDuplication` d'un même banc :
/// LA duplication DXGI (une seule par sortie — mesuré, pas supposé : la
/// deuxième `DuplicateOutput` échoue en `0x80070057`, voir
/// `banc::executer`), et le cache de l'image de bureau du tour courant.
///
/// **Ronde de correction 1.** La première version de ce fichier faisait
/// appeler `next_frame(region)` — acquisition + recadrage + relâchement en
/// un seul appel — une fois PAR VOIE et par tour. Mesuré : à N≥2, une seule
/// fenêtre restait nourrie (~100 i/s) et toutes les autres tombaient sous
/// 1,3 i/s, quel que soit N. Cause : `AcquireNextFrame` ne signale un
/// changement de bureau qu'UNE fois ; la première voie de la boucle du banc
/// à l'appeler après un changement consomme ce signal et relâche aussitôt
/// l'image ; les voies suivantes, appelées à la microseconde suivante dans
/// le MÊME tour, ne trouvent plus rien de neuf. Ce n'était pas Desktop
/// Duplication qui se dégradait avec N, mais l'architecture de mesure : la
/// disputer voie par voie plutôt que l'amorcer une fois pour toutes.
///
/// Le correctif : `amorcer` n'appelle `next_frame` qu'une fois par valeur de
/// `tour`, sur le bureau ENTIER ; chaque voie fait ensuite son propre
/// sous-recadrage GPU (`CopySubresourceRegion`, bon marché) depuis cette
/// image commune vers SA texture. Toutes les voies d'un même tour reçoivent
/// ainsi la même fraîcheur — soit toutes une image neuve, soit toutes rien,
/// jamais une seule sur N.
pub(super) struct SourceDuplication {
    capture: DesktopCapture,
    bureau: Rect,
    contexte: ID3D11DeviceContext,
    /// Numéro de tour pour lequel `dernier_bureau` a été amorcé. `None`
    /// avant le premier appel.
    dernier_tour: Option<u64>,
    /// Image de bureau entière amorcée pour `dernier_tour`. `None` si le
    /// bureau n'avait rien de neuf à cet instant — le cas courant, pas une
    /// erreur : `DesktopCapture::next_frame` ne rend une image que lorsque
    /// le bureau a changé depuis le dernier appel.
    dernier_bureau: Option<CapturedFrame>,
}

impl SourceDuplication {
    /// Dimensions de la TEXTURE que rend l'acquisition de cette sortie — pas
    /// celles annoncées par DXGI, dont elles peuvent différer d'un facteur DPI.
    pub(super) fn dimensions_bureau(&self) -> (u32, u32) {
        (self.bureau.width, self.bureau.height)
    }

    /// Amorce le bureau entier pour `tour`, une seule fois par valeur de
    /// `tour` quel que soit le nombre de voies qui appellent cette méthode.
    fn amorcer(&mut self, tour: u64) -> Result<()> {
        if self.dernier_tour == Some(tour) {
            return Ok(());
        }
        self.dernier_bureau = self.capture.next_frame(self.bureau)?;
        self.dernier_tour = Some(tour);
        Ok(())
    }
}

/// Voie de référence : Desktop Duplication du bureau, recadrée sur la
/// fenêtre. C'est le comportement de production actuel — celui dont on sait
/// déjà qu'il se pollue au recouvrement. Il sert d'étalon : une voie qui ne
/// fait pas mieux que lui n'apporte rien, et s'il ne se polluait PAS au
/// banc, ce serait le banc qu'il faudrait suspecter.
///
/// N'est PAS fidèle à la production au sens strict : la production
/// aujourd'hui ne capture qu'UNE fenêtre par session (voir `CLAUDE.md`,
/// « Known Constraints »). Le partage d'une seule duplication entre N
/// recadrages, mesuré et corrigé ici (voir `SourceDuplication`), est un
/// comportement que le chantier D devra construire, pas un qui existe déjà.
pub(super) struct VoieDuplication {
    source: Rc<RefCell<SourceDuplication>>,
    region: Rect,
    /// Texture propre à cette voie (voir `creer_texture_recadrage`), allouée
    /// à `ouvrir()`.
    texture: Option<ID3D11Texture2D>,
}

impl VoieDuplication {
    /// Crée la duplication partagée, une fois pour tout le banc, sur la sortie
    /// DXGI désignée — ou sur celle du bureau si aucune ne l'est.
    ///
    /// Le bureau retourné par `desktop_size()` est celui de CETTE sortie, dans
    /// ses dimensions de MODE — c'est-à-dire les dimensions physiques, là où
    /// `DXGI_OUTPUT_DESC::DesktopCoordinates` donne les dimensions mises à
    /// l'échelle par le DPI. Les régions passées à `ouvrir()` doivent donc être
    /// exprimées dans le repère de la texture, pas dans celui des fenêtres :
    /// voir `moniteurs_virtuels::vers_texture`, dont `banc::executer` se sert
    /// pour convertir.
    pub(super) fn partagee_sur(
        sortie: Option<(u32, u32)>,
    ) -> Result<Rc<RefCell<SourceDuplication>>> {
        let capture = match sortie {
            Some((adaptateur, index)) => DesktopCapture::sur_sortie(adaptateur, index)?,
            None => DesktopCapture::new()?,
        };
        let (largeur, hauteur) = capture.desktop_size();
        let contexte = unsafe { capture.device().GetImmediateContext() }
            .context("contexte immédiat pour les sous-recadrages partagés")?;
        Ok(Rc::new(RefCell::new(SourceDuplication {
            capture,
            bureau: Rect { x: 0, y: 0, width: largeur, height: hauteur },
            contexte,
            dernier_tour: None,
            dernier_bureau: None,
        })))
    }

    pub(super) fn nouvelle(source: Rc<RefCell<SourceDuplication>>) -> Self {
        Self { source, region: Rect { x: 0, y: 0, width: 0, height: 0 }, texture: None }
    }
}

impl VoieDeCapture for VoieDuplication {
    fn nom(&self) -> &'static str {
        "duplication"
    }

    fn ouvrir(&mut self, _hwnd: HWND, region: Rect) -> Result<()> {
        self.region = region;
        let device = self.source.borrow().capture.device().clone();
        self.texture = Some(creer_texture_recadrage(&device, region)?);
        Ok(())
    }

    fn prochaine_image(&mut self, tour: u64) -> Result<Option<CapturedFrame>> {
        let mut source = self.source.borrow_mut();
        source.amorcer(tour)?;
        let Some(bureau) = source.dernier_bureau.as_ref() else {
            return Ok(None);
        };

        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| anyhow!("texture de recadrage non ouverte (ouvrir() jamais appelée)"))?;
        // Sous-recadrage GPU, bon marché, depuis l'image de bureau commune
        // déjà amorcée par `SourceDuplication::amorcer` — pas un nouvel
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
            source.contexte.CopySubresourceRegion(texture, 0, 0, 0, 0, &bureau.texture, 0, Some(&box_));
        }

        Ok(Some(CapturedFrame {
            texture: texture.clone(),
            width: self.region.width,
            height: self.region.height,
        }))
    }

    fn device(&self) -> ID3D11Device {
        // `ID3D11Device` est un pointeur COM à comptage de références : le
        // cloner ne duplique pas le périphérique, il incrémente un compteur.
        // Rendre une valeur plutôt qu'une référence évite au banc de tenir un
        // emprunt sur la voie pendant qu'il l'appelle.
        self.source.borrow().capture.device().clone()
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
/// HWND, sans le plafond d'UNE seule duplication DXGI par sortie. Pas de
/// `SourceDuplication` équivalente ici, et `prochaine_image` ignore son
/// paramètre `tour` : chaque fenêtre se capture indépendamment, il n'y a
/// rien à amortir entre voies. Le périphérique D3D11 est néanmoins créé une
/// seule fois pour tout le banc et cloné dans chaque voie (un clone COM
/// n'incrémente qu'un compteur de références) : huit périphériques
/// indépendants n'apporteraient rien et compliqueraient la cohérence avec
/// l'encodeur, qui doit tourner sur LE périphérique de sa voie.
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

/// Crée un périphérique D3D11 matériel avec le support BGRA.
///
/// `pub(super)` parce que `paralleles.rs` en a besoin pour ses mires : il ne
/// peut pas emprunter celui d'une `DesktopCapture` provisoire, DXGI
/// n'autorisant qu'UNE duplication par sortie — la provisoire ferait échouer
/// la vraie en 0x80070057.
pub(super) fn creer_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
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
    /// Crée le périphérique D3D11 partagé, une fois pour tout le banc.
    ///
    /// Sans cible d'adaptateur explicite : comme pour les mires
    /// (`capture::DesktopCapture::new`), l'adaptateur par défaut est celui
    /// qui porte le GPU réel de la VM, seul capable d'héberger ensuite
    /// l'encodeur matériel.
    pub(super) fn partagee() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
        creer_device()
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
        self.texture = Some(creer_texture_recadrage(&self.device, region)?);
        Ok(())
    }

    fn prochaine_image(&mut self, _tour: u64) -> Result<Option<CapturedFrame>> {
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
