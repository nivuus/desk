//! Resolving a DXGI output by target, and duplicating that output.
//!
//! Extracted from `capture.rs` in task 3 of sub-block D2: adding
//! `EchecAcquisition` and the resumption in `next_frame` took the parent
//! file above the 500-line cap (`CLAUDE.md`). These two
//! functions touch no private field of `DesktopCapture` — they
//! take the device and the target as parameters — so no access reason
//! required keeping them in the parent file.
//!
//! `dupliquer_avec_reprise` joined them in task 11 bis, for the same
//! cap reason and with no more private access than them.

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
};

use super::CibleCapture;

/// Dimensions of a DXGI output designated by its name, **without opening its
/// duplication**.
///
/// It is the only way to know a window's size before having
/// decided it deserved an encoder: DXGI only allows one open duplication
/// per output, and opening one here would take the output's mutex —
/// hence take it away from the session that may already be capturing it. This
/// function opens nothing, duplicates nothing, and disturbs no neighbour.
///
/// **The size is read on `DesktopCoordinates`, never from WMI nor from what
/// the caller asked of the driver.** WMI returns a field seen stale for 68 s
/// (multi-window probe reading), and the virtual output driver
/// QUANTISES the requested resolution — 1280×632 requested yields a
/// 1280×720 output (sub-block D2). Only the value returned by DXGI is true.
///
/// ⚠️ **It may nonetheless differ from the size of the TEXTURE** that
/// acquisition will return later (`duplication.GetDesc().ModeDesc`): on a
/// scaled output, the ratio is the DPI factor — 3413×960 announced
/// for 5120×1440 real, read at 150 % by the probe, hence
/// `moniteurs_virtuels::facteur_echelle`. On the product path, where the
/// virtual outputs are created at the viewport size and without
/// scaling, the two coincide; a caller that cannot guarantee it must
/// treat this value as an ANNOUNCEMENT, and check against the real size
/// once the source is built.
pub fn taille_de_sortie(nom: &str) -> Result<(u32, u32)> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;
    // `ouvrir_sortie` already does exactly the resolution by name, with its
    // trace: redoing it here would be a second truth to maintain.
    let (_adaptateur, sortie) = ouvrir_sortie(&factory, &CibleCapture::Sortie(nom.to_string()))?;
    let desc = unsafe { sortie.GetDesc() }.with_context(|| format!("description de {nom}"))?;
    let rect = desc.DesktopCoordinates;
    let largeur = (rect.right - rect.left).max(0) as u32;
    let hauteur = (rect.bottom - rect.top).max(0) as u32;
    // The same threshold as `region_de_sortie`: below it, the even alignment
    // NV12 requires leaves nothing to encode.
    if largeur < 2 || hauteur < 2 {
        bail!("sortie {nom} de dimensions inexploitables ({largeur}x{hauteur})");
    }
    Ok((largeur, hauteur))
}

/// Opens an output designated by its name, or — for `CibleCapture::Bureau` —
/// finds and opens the output that composes the desktop.
pub(super) fn ouvrir_sortie(
    factory: &IDXGIFactory1,
    cible: &CibleCapture,
) -> Result<(IDXGIAdapter1, IDXGIOutput1)> {
    let mut index_adaptateur = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index_adaptateur) } {
        let mut index_sortie = 0u32;
        while let Ok(output) = unsafe { adapter.EnumOutputs(index_sortie) } {
            let desc = match unsafe { output.GetDesc() } {
                Ok(desc) => desc,
                Err(_) => {
                    index_sortie += 1;
                    continue;
                }
            };
            let nom = String::from_utf16_lossy(&desc.DeviceName)
                .trim_end_matches('\0')
                .to_string();
            let retenue = match cible {
                CibleCapture::Sortie(vise) => &nom == vise,
                CibleCapture::Bureau => desc.AttachedToDesktop.as_bool(),
            };
            if retenue {
                let name = match unsafe { adapter.GetDesc1() } {
                    Ok(adapter_desc) => String::from_utf16_lossy(&adapter_desc.Description)
                        .trim_end_matches('\0')
                        .to_string(),
                    Err(_) => "<inconnu>".to_string(),
                };
                tracing::info!(
                    adaptateur = %name, index_adaptateur, index_sortie, nom_sortie = %nom,
                    attachee = desc.AttachedToDesktop.as_bool(),
                    "sortie retenue pour la duplication"
                );
                return Ok((adapter.clone(), output.cast()?));
            }
            index_sortie += 1;
        }
        index_adaptateur += 1;
    }
    match cible {
        CibleCapture::Sortie(nom) => bail!("aucune sortie DXGI nommée {nom}"),
        CibleCapture::Bureau => {
            bail!("aucune sortie attachée au bureau : la session est-elle interactive ?")
        }
    }
}

/// Creates the D3D11 device on the adapter carrying the chosen output, and
/// makes it shareable with Media Foundation.
///
/// Extracted from `ouvrir` in task 11 bis: the opening retry added a
/// line to `capture.rs`, already at exactly 500 lines (`CLAUDE.md`), and the
/// instruction is to extract, never to compress. **Pure move: no
/// call, no order, no value changed.**
pub(super) fn creer_peripherique(
    adapter: &IDXGIAdapter1,
) -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            adapter,
            // Un adaptateur explicite impose le type « inconnu ».
            windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .context("création du périphérique D3D11")?;
    }
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?;
    let context = context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?;

    // The D3D11 immediate context is NOT safe for concurrent access by
    // default: the driver assumes a single thread and sets no lock. Yet this
    // device does not stay private — it is handed to Media Foundation
    // through an `IMFDXGIDeviceManager` (see `encode::share_device`), and the
    // BGRA→NV12 converter as well as the hardware H.264 encoder use it
    // from their own internal worker threads, while our main
    // thread calls `CopySubresourceRegion` in `crop` and the output
    // duplication — built on this same device — serves
    // `AcquireNextFrame`.
    //
    // Without this protection, two threads enter the driver at the same
    // time and one of them may never come out. It is the block
    // measured here: four runs out of four frozen in
    // `AcquireNextFrame`, yet called with a ZERO timeout,
    // hence supposed never to block — the wait did not come from DXGI but
    // from the driver's internal lock. No error is raised, the
    // function simply never returns.
    //
    // `SetMultithreadProtected(TRUE)` makes the driver take its internal
    // lock around each command: it is the documented condition
    // for sharing a D3D11 device with Media Foundation, and it
    // must be set BEFORE `DuplicateOutput`, the duplication inheriting the
    // device as it is at that instant.
    let multithread: ID3D11Multithread = context
        .cast()
        .context("obtention de ID3D11Multithread depuis le contexte immédiat")?;
    let was_protected = unsafe { multithread.SetMultithreadProtected(true) };
    tracing::info!(
        protection_precedente = was_protected.as_bool(),
        "protection multi-fils activée sur le contexte immédiat D3D11"
    );

    Ok((device, context))
}

/// Duplicates the output and reads its dimensions. The only piece of `ouvrir` that
/// `rouvrir` redoes.
///
/// windows-rs 0.62 API gap: `GetDesc` no longer takes an output
/// parameter; it returns the structure directly (by value for
/// `IDXGIOutputDuplication`, in a `Result` for `IDXGIOutput1` and
/// `IDXGIAdapter1`, the last two being fallible).
pub(super) fn dupliquer(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    let duplication =
        unsafe { output.DuplicateOutput(device) }.context("duplication de la sortie écran")?;
    let desc = unsafe { duplication.GetDesc() };
    Ok((duplication, desc.ModeDesc.Width, desc.ModeDesc.Height))
}

/// Duplicates the output, **retrying as long as DXGI says "not now"**,
/// and at most for `fenetre`.
///
/// Used only by the construction of `DesktopCapture` (`ouvrir`), and not
/// by `rouvrir`: that one is already called from the resumption window of
/// `next_frame`, which plays the same role through another set-up.
///
/// **Extracted here rather than written in `capture.rs`**: that parent file is
/// at exactly 500 lines, zero margin (`CLAUDE.md`), and this loop
/// only wraps `dupliquer`, which already lives here.
pub(super) fn dupliquer_avec_reprise(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
    cible: &CibleCapture,
    fenetre: std::time::Duration,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    // Retry ONLY the duplication, and on the spot.
    //
    // **Blocking is not legitimate everywhere, hence the `fenetre` received as an
    // argument rather than read here.** At the start-up of a supervisor child
    // it is full: nothing is running yet — no keyframe to serve, no
    // network adaptation, no pending resize. But `ouvrir` is
    // ALSO called again mid-session by `WindowsSource::resize`
    // (`DesktopCapture::new`, two factories in `rebuild_or_recover`), on
    // the blocking thread of `Session::run`: that one passes a ZERO duration, and
    // therefore behaves exactly as before this retry. It is the same
    // trade-off as `next_frame`, which already refuses to sleep for this reason.
    //
    // Without this retry, a first `DuplicateOutput` landing while
    // Windows reconfigures its topology killed the child, which the supervisor
    // restarted by DESTROYING then RECREATING its output — thereby abandoning
    // the mutex of all the duplications already open. The
    // compensation cost more than the failure: 32 of the 44 reopenings of the
    // reading of 1 August 2026 came from this single step.
    let debut = std::time::Instant::now();
    loop {
        match dupliquer(device, output) {
            Ok(rendu) => break Ok(rendu),
            Err(erreur) => {
                // `dupliquer` returns an `anyhow::Error` built through `.context()`
                // on a `windows::core::Error`: anyhow keeps the underlying
                // cause and `downcast_ref` finds it again. If it ever
                // could not be read, `retentable` would be `false` and
                // the opening would fail without retry — safe degradation,
                // never a loop.
                let code = erreur
                    .downcast_ref::<windows::core::Error>()
                    .map(|e| e.code().0);
                let retentable = code.is_some_and(crate::capture_reprise::est_ouverture_retentable);
                if !retentable || debut.elapsed() >= fenetre {
                    // Loud on purpose: it is here that a cap on
                    // concurrent duplications shows, indistinguishable from a
                    // reconfiguration by the HRESULT alone.
                    tracing::error!(
                        hresult = code.map(|c| format!("{c:#010x}")),
                        attendu_ms = debut.elapsed().as_millis() as u64,
                        cible = ?cible,
                        "ouverture de la duplication abandonnée"
                    );
                    break Err(erreur);
                }
                tracing::info!(
                    hresult = code.map(|c| format!("{c:#010x}")),
                    cible = ?cible,
                    "duplication indisponible à l'ouverture, nouvel essai"
                );
                std::thread::sleep(crate::capture_reprise::PAS_REPRISE);
            }
        }
    }
}
