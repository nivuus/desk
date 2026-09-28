//! Test windows of the multi-window probe: N borderless windows,
//! painted by D3D11 with a colour that encodes their identity and their frame
//! number (`crate::mire`).
//!
//! Rendering goes through a D3D11 swapchain and not through GDI. `PrintWindow`
//! fails precisely on D3D content: a test pattern painted in GDI would validate
//! the fallback path, which would then collapse in front of a real game.
//!
//! The animation is not decorative — Desktop Duplication only emits an image
//! when the desktop changes. Without colour alternation, the bench measures
//! a capture that receives nothing (a trap already paid for at milestone 1).

use anyhow::{anyhow, Context, Result};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
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
    /// Opens one window per slot, painted and visible, without ever taking
    /// focus (`WS_EX_NOACTIVATE`): a test pattern that stole the foreground
    /// would change the covering the bench stages.
    pub(super) fn ouvrir(device: &ID3D11Device, places: &[Rect]) -> Result<Self> {
        anyhow::ensure!(
            places.len() <= mire::MIRES_MAX as usize,
            "au plus {} mires, {} demandées",
            mire::MIRES_MAX,
            places.len()
        );
        enregistrer_classe()?;

        let dxgi: IDXGIDevice = device
            .cast()
            .context("IDXGIDevice depuis le périphérique D3D11")?;
        let adaptateur = unsafe { dxgi.GetAdapter() }.context("adaptateur DXGI")?;
        let fabrique: IDXGIFactory2 =
            unsafe { adaptateur.GetParent() }.context("fabrique DXGI depuis l'adaptateur")?;
        let contexte = unsafe { device.GetImmediateContext() }.context("contexte immédiat")?;

        // Each iteration may fail after having already created a Win32
        // window (swapchain, render view). Without explicit cleanup here, a
        // partial failure would leave the already-open windows orphaned:
        // `Fenetre` has no `Drop` of its own (only `Mires` has one), and
        // the normal unwinding of `?` would drop the local `Vec<Fenetre>`
        // without ever calling `DestroyWindow`. An orphaned test pattern would skew
        // the next measurement, since the bench is relaunched several times in a row.
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

        Ok(Self {
            fenetres,
            contexte,
            trame: 0,
        })
    }

    /// Paints one frame on all the test patterns and presents it.
    pub(super) fn peindre(&mut self) -> Result<()> {
        for fenetre in &self.fenetres {
            let (r, g, b) = mire::couleur_mire(fenetre.id, self.trame);
            // Non-sRGB UNORM format: the float value is written as
            // is into the byte, so `r/255` returns exactly `r` on
            // reading. An `_SRGB` format would impose a conversion and the
            // pixel check would fail on a test pattern that is actually right.
            let couleur = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
            unsafe {
                self.contexte
                    .ClearRenderTargetView(&fenetre.cible, &couleur)
            };
            unsafe { fenetre.swapchain.Present(0, Default::default()) }
                .ok()
                .context("présentation d'une mire")?;
        }
        self.trame += 1;
        Ok(())
    }

    /// Empties the windows' message queue. Without it Windows considers them
    /// frozen and stops composing them.
    pub(super) fn pomper(&self) {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            let _ = unsafe { TranslateMessage(&message) };
            unsafe { DispatchMessageW(&message) };
        }
    }

    /// Puts test pattern `dessus` on top of test pattern `dessous`, moving it onto
    /// its slot and bringing it to the foreground. It is the staging of
    /// the elimination gate: the covered test pattern must remain capturable.
    ///
    /// Also updates `Fenetre.place` on the moved side: it is the only
    /// exposed way (`place()`) to know a test pattern's position, and
    /// a stale value after covering would make the elimination gate
    /// uninterpretable for the tasks that rely on it (6, 8, 9).
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

        let fenetre = self
            .fenetres
            .iter_mut()
            .find(|f| f.id == dessus)
            .ok_or_else(|| anyhow!("aucune mire n°{dessus}"))?;
        fenetre.place = cible;
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
        // An orphaned test pattern would skew the next measurement, and the bench is
        // launched several times in a row.
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
        // `RegisterClassW` returns 0 on failure. A second registration of
        // the same class would fail too — hence the `Once`.
        if unsafe { RegisterClassW(&classe) } == 0 {
            // windows-rs 0.62 API gap relative to the brief:
            // `windows::core::Error::from_win32()` (which read `GetLastError`
            // and converted it into an HRESULT) no longer exists — replaced by
            // `Error::from_thread()`, which reads the same current-thread error.
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

/// Creates a test pattern window, its swapchain and its render view.
///
/// Destroys the Win32 window itself if a failure occurs after its
/// creation (swapchain, back buffer, render view): it is the only point
/// that still knows the HWND at that instant, the caller only receiving an
/// error.
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
            Ok(Fenetre {
                id,
                hwnd,
                place,
                swapchain,
                cible,
            })
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
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
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
