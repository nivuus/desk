//! How many hardware H.264 encoders does this RTX 4070 accept in
//! parallel?
//!
//! The accepted order of magnitude (8 sessions on Ada) is a specification
//! rumour, not a measurement on this card and this driver. Work stream D
//! depends on it directly: if the ceiling is low, suspending the encoding of
//! hidden windows stops being "desirable in itself" and becomes a
//! viability condition.
//!
//! The multi-window capture probe measured 8 instances on a SINGLE, shared D3D11
//! device, the 9th failing at the binding of the input type
//! (`MF_E_UNSUPPORTED_D3D_TYPE`). It could not tell whether the constraint was
//! the hardware encoder itself or the sharing of the device: that is
//! exactly what the `separe` mode settles here, the `partage` mode remaining
//! available as a control so that its measurement stays reproducible.

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};

/// Beyond that, we stop searching: the result would already be largely
/// sufficient for work stream D.
const SEARCH_CEILING: usize = 16;

/// A new D3D11 device, unrelated to capture.
///
/// The adapter is left to the system's choice (`D3D_DRIVER_TYPE_HARDWARE`):
/// on this VM there is only one real GPU, and it is the one carrying NVENC.
pub(super) fn peripherique_autonome() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut contexte: Option<ID3D11DeviceContext> = None;
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
            Some(&mut contexte),
        )
        .context("création d'un périphérique D3D11 autonome")?;
    }
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 autonome absent"))?;
    let contexte = contexte.ok_or_else(|| anyhow!("contexte D3D11 autonome absent"))?;

    // Same protection as `DesktopCapture::ouvrir`: this device is
    // handed to Media Foundation, whose colour converter and
    // encoder enter it from their own worker threads. Without it,
    // two threads enter the driver together and one may never
    // come out — the block measured at milestone 1.
    let multithread: ID3D11Multithread = contexte
        .cast()
        .context("obtention de ID3D11Multithread sur le périphérique autonome")?;
    let protection_precedente = unsafe { multithread.SetMultithreadProtected(true) };
    tracing::debug!(
        protection_precedente = protection_precedente.as_bool(),
        "protection multi-fils activée sur un périphérique autonome"
    );

    Ok((device, contexte))
}

/// `partage` reproduces the probe's measurement: ONE D3D11 device for
/// all encoders. `separe` answers the question it left
/// open: did the refusal of the 9th instance come from the hardware encoder, or
/// from the sharing of the device?
pub(super) fn plafond(mode: &str) -> Result<()> {
    let partage = match mode {
        "partage" => Some(crate::capture::DesktopCapture::new()?),
        "separe" => None,
        autre => {
            anyhow::bail!("MULTIFENETRE_NVENC vaut « partage » ou « separe », pas « {autre} »")
        }
    };
    tracing::info!(mode, "plafond d'encodeurs : mode retenu");

    // The standalone devices MUST stay alive as long as
    // the encoders that rely on them: releasing them at the next round would
    // measure something other than what we think.
    let mut peripheriques = Vec::new();
    let mut encodeurs = Vec::new();

    let issue = search(mode, partage.as_ref(), &mut peripheriques, &mut encodeurs);

    // The bench's encoding pass killed the process AT THE EXIT of its loop,
    // hence at the DESTRUCTION of the encoders, not while feeding them. ✅ Defect
    // diagnosed and fixed on 31 July 2026 (`encode::arret`: the NVIDIA MFT
    // kept a work item in flight at release) — 0 recurrence
    // over 20 runs of the comparable case, which is not a proof
    // of absence. Here nothing is encoded, but the destruction does take place: these
    // two traces bracket the release so that a crash at this place
    // reads as such, and is never confused with a ceiling — "the process
    // dies" and "the creation is refused" are two distinct failure
    // modes.
    tracing::info!(
        encodeurs = encodeurs.len(),
        peripheriques = peripheriques.len(),
        "relâchement des encodeurs et des périphériques"
    );
    drop(encodeurs);
    drop(peripheriques);
    drop(partage);
    tracing::info!("relâchement terminé — le processus a survécu à la destruction");

    issue
}

/// Creates encoders until refusal, or until `SEARCH_CEILING`.
///
/// Separate from `plafond` so that the release of resources stays under the
/// control of the caller, whatever the exit path.
fn search(
    mode: &str,
    partage: Option<&crate::capture::DesktopCapture>,
    peripheriques: &mut Vec<(ID3D11Device, ID3D11DeviceContext)>,
    encodeurs: &mut Vec<crate::encode::H264Encoder>,
) -> Result<()> {
    for rang in 1..=SEARCH_CEILING {
        let device = match partage {
            Some(capture) => capture.device().clone(),
            None => {
                let (device, contexte) = peripherique_autonome()?;
                peripheriques.push((device.clone(), contexte));
                device
            }
        };
        match crate::encode::H264Encoder::new(&device, (1280, 720), (1280, 720), 60, 8_000_000) {
            Ok(encodeur) => {
                encodeurs.push(encodeur);
                tracing::info!(rang, mode, "encodeur créé");
            }
            Err(error) => {
                tracing::info!(
                    plafond = rang - 1,
                    mode,
                    causes = %super::causes(error),
                    "plafond d'encodeurs atteint — création du suivant refusée"
                );
                return Ok(());
            }
        }
    }
    tracing::info!(
        search_ceiling = SEARCH_CEILING,
        mode,
        "aucun plafond atteint sous {SEARCH_CEILING} encodeurs"
    );
    Ok(())
}
