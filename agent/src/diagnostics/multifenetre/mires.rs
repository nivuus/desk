//! Fenêtres de test de la sonde multi-fenêtres : N fenêtres sans bordure,
//! peintes par D3D11 d'une couleur qui encode leur identité et leur numéro
//! de trame (`crate::mire`).
//!
//! Le rendu passe par une swapchain D3D11 et non par GDI. `PrintWindow`
//! échoue précisément sur le contenu D3D : une mire peinte en GDI validerait
//! la voie des replis, qui s'effondrerait ensuite devant un vrai jeu.
//!
//! L'animation n'est pas décorative — Desktop Duplication n'émet une image
//! que lorsque le bureau change. Sans alternance de couleur, le banc mesure
//! une capture qui ne reçoit rien (piège déjà payé au jalon 1).

use anyhow::{anyhow, Context, Result};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW,
    SetWindowPos, ShowWindow, TranslateMessage, HWND_TOP, MSG, PM_REMOVE, SWP_NOACTIVATE,
    SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_NOACTIVATE, WS_POPUP, WS_VISIBLE,
};

use crate::geometry::Rect;
use crate::mire;

const CLASSE: PCWSTR = w!("SondeMultifenetreMire");

struct Fenetre {
    id: u8,
    hwnd: HWND,
    place: Rect,
    swapchain: IDXGISwapChain1,
    cible: ID3D11RenderTargetView,
}

pub(super) struct Mires {
    fenetres: Vec<Fenetre>,
    contexte: ID3D11DeviceContext,
    trame: u64,
}

impl Mires {
    /// Ouvre une fenêtre par place, peinte et visible, sans jamais prendre le
    /// focus (`WS_EX_NOACTIVATE`) : une mire qui volerait le premier plan
    /// changerait le recouvrement que le banc met en scène.
    pub(super) fn ouvrir(device: &ID3D11Device, places: &[Rect]) -> Result<Self> {
        anyhow::ensure!(
            places.len() <= mire::MIRES_MAX as usize,
            "au plus {} mires, {} demandées",
            mire::MIRES_MAX,
            places.len()
        );
        enregistrer_classe()?;

        let dxgi: IDXGIDevice = device.cast().context("IDXGIDevice depuis le périphérique D3D11")?;
        let adaptateur = unsafe { dxgi.GetAdapter() }.context("adaptateur DXGI")?;
        let fabrique: IDXGIFactory2 =
            unsafe { adaptateur.GetParent() }.context("fabrique DXGI depuis l'adaptateur")?;
        let contexte = unsafe { device.GetImmediateContext() }.context("contexte immédiat")?;

        // Chaque itération peut échouer après avoir déjà créé une fenêtre
        // Win32 (swapchain, vue de rendu). Sans nettoyage explicite ici, un
        // échec partiel laisserait les fenêtres déjà ouvertes orphelines :
        // `Fenetre` ne porte pas de `Drop` propre (seul `Mires` en a un), et
        // le déroulement normal de `?` abandonnerait le `Vec<Fenetre>` local
        // sans jamais appeler `DestroyWindow`. Une mire orpheline fausserait
        // la mesure suivante, le banc étant relancé plusieurs fois de suite.
        let mut fenetres = Vec::with_capacity(places.len());
        for (index, place) in places.iter().enumerate() {
            match creer_fenetre(device, &fabrique, index as u8, *place) {
                Ok(fenetre) => fenetres.push(fenetre),
                Err(e) => {
                    for fenetre in &fenetres {
                        let _ = unsafe { DestroyWindow(fenetre.hwnd) };
                    }
                    return Err(e);
                }
            }
        }

        Ok(Self { fenetres, contexte, trame: 0 })
    }

    /// Peint une trame sur toutes les mires et la présente.
    pub(super) fn peindre(&mut self) -> Result<()> {
        for fenetre in &self.fenetres {
            let (r, g, b) = mire::couleur_mire(fenetre.id, self.trame);
            // Format UNORM non-sRGB : la valeur flottante est écrite telle
            // quelle dans l'octet, donc `r/255` rend exactement `r` à la
            // lecture. Un format `_SRGB` imposerait une conversion et la
            // vérification par pixels échouerait sur une mire pourtant juste.
            let couleur = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            unsafe { self.contexte.ClearRenderTargetView(&fenetre.cible, &couleur) };
            unsafe { fenetre.swapchain.Present(0, Default::default()) }
                .ok()
                .context("présentation d'une mire")?;
        }
        self.trame += 1;
        Ok(())
    }

    /// Vide la file de messages des fenêtres. Sans cela Windows les tient
    /// pour figées et cesse de les composer.
    pub(super) fn pomper(&self) {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }
    }

    /// Met la mire `dessus` par-dessus la mire `dessous`, en la déplaçant sur
    /// sa place et en la portant au premier plan. C'est la mise en scène de
    /// la porte éliminatoire : la mire recouverte doit rester capturable.
    pub(super) fn recouvrir(&mut self, dessus: u8, dessous: u8) -> Result<()> {
        let cible = self.place(dessous)?;
        let hwnd = self.hwnd(dessus)?;
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                cible.x,
                cible.y,
                cible.width as i32,
                cible.height as i32,
                SWP_NOACTIVATE,
            )
        }
        .context("déplacement d'une mire par-dessus une autre")?;
        Ok(())
    }

    pub(super) fn hwnd(&self, id: u8) -> Result<HWND> {
        self.fenetres
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.hwnd)
            .ok_or_else(|| anyhow!("aucune mire n°{id}"))
    }

    pub(super) fn place(&self, id: u8) -> Result<Rect> {
        self.fenetres
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.place)
            .ok_or_else(|| anyhow!("aucune mire n°{id}"))
    }

    pub(super) fn trame(&self) -> u64 {
        self.trame
    }

    pub(super) fn nombre(&self) -> u8 {
        self.fenetres.len() as u8
    }
}

impl Drop for Mires {
    fn drop(&mut self) {
        // Une mire orpheline fausserait la mesure suivante, et le banc est
        // lancé plusieurs fois de suite.
        for fenetre in &self.fenetres {
            let _ = unsafe { DestroyWindow(fenetre.hwnd) };
        }
    }
}

fn enregistrer_classe() -> Result<()> {
    use std::sync::Once;
    static UNE_FOIS: Once = Once::new();
    let mut resultat = Ok(());
    UNE_FOIS.call_once(|| {
        let classe = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            lpszClassName: CLASSE,
            ..Default::default()
        };
        // `RegisterClassW` rend 0 en cas d'échec. Un second enregistrement de
        // la même classe échouerait aussi — d'où le `Once`.
        if unsafe { RegisterClassW(&classe) } == 0 {
            // écart d'API windows-rs 0.62 par rapport au brief :
            // `windows::core::Error::from_win32()` (qui lisait `GetLastError`
            // et le convertissait en HRESULT) n'existe plus — remplacé par
            // `Error::from_thread()`, qui lit la même erreur de fil courant.
            resultat = Err(anyhow!(
                "enregistrement de la classe de fenêtre : {}",
                windows::core::Error::from_thread()
            ));
        }
    });
    resultat
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// Crée une fenêtre de mire, sa swapchain et sa vue de rendu.
///
/// Détruit elle-même la fenêtre Win32 si un échec survient après sa
/// création (swapchain, tampon arrière, vue de rendu) : c'est le seul point
/// qui connaît encore le HWND à cet instant, l'appelant ne recevant qu'une
/// erreur.
fn creer_fenetre(
    device: &ID3D11Device,
    fabrique: &IDXGIFactory2,
    id: u8,
    place: Rect,
) -> Result<Fenetre> {
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE,
            CLASSE,
            PCWSTR::null(),
            WS_POPUP | WS_VISIBLE,
            place.x,
            place.y,
            place.width as i32,
            place.height as i32,
            None,
            None,
            None,
            None,
        )
    }
    .context("création d'une fenêtre de mire")?;

    match creer_swapchain_et_cible(device, fabrique, hwnd, place) {
        Ok((swapchain, cible)) => {
            let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
            Ok(Fenetre { id, hwnd, place, swapchain, cible })
        }
        Err(e) => {
            let _ = unsafe { DestroyWindow(hwnd) };
            Err(e)
        }
    }
}

fn creer_swapchain_et_cible(
    device: &ID3D11Device,
    fabrique: &IDXGIFactory2,
    hwnd: HWND,
    place: Rect,
) -> Result<(IDXGISwapChain1, ID3D11RenderTargetView)> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: place.width,
        Height: place.height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        ..Default::default()
    };
    let swapchain = unsafe { fabrique.CreateSwapChainForHwnd(device, hwnd, &desc, None, None) }
        .context("création de la swapchain d'une mire")?;

    let arriere: ID3D11Texture2D =
        unsafe { swapchain.GetBuffer(0) }.context("tampon arrière de la swapchain")?;
    let mut cible = None;
    unsafe { device.CreateRenderTargetView(&arriere, None, Some(&mut cible)) }
        .context("vue de rendu d'une mire")?;
    let cible = cible.ok_or_else(|| anyhow!("vue de rendu absente"))?;

    Ok((swapchain, cible))
}
