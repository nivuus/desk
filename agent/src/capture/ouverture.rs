//! Résolution d'une sortie DXGI par cible, et duplication de cette sortie.
//!
//! Extrait de `capture.rs` à la tâche 3 du sous-bloc D2 : l'ajout de
//! `EchecAcquisition` et de la reprise dans `next_frame` portait le fichier
//! parent au-dessus du plafond de 500 lignes (`CLAUDE.md`). Ces deux
//! fonctions ne touchent à aucun champ privé de `DesktopCapture` — elles
//! prennent le périphérique et la cible en paramètres — donc aucune raison
//! d'accès n'imposait de les garder dans le fichier parent.

use anyhow::{bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
};

use super::CibleCapture;

/// Ouvre une sortie désignée par son nom, ou — pour `CibleCapture::Bureau` —
/// trouve et ouvre la sortie qui compose le bureau.
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

/// Duplique la sortie et lit ses dimensions. Le seul morceau d'`ouvrir` que
/// `rouvrir` refait.
///
/// écart d'API windows-rs 0.62 : `GetDesc` ne prend plus de paramètre de
/// sortie ; elle renvoie directement la structure (par valeur pour
/// `IDXGIOutputDuplication`, dans un `Result` pour `IDXGIOutput1` et
/// `IDXGIAdapter1`, ces deux dernières pouvant échouer).
pub(super) fn dupliquer(
    device: &ID3D11Device,
    output: &IDXGIOutput1,
) -> Result<(IDXGIOutputDuplication, u32, u32)> {
    let duplication =
        unsafe { output.DuplicateOutput(device) }.context("duplication de la sortie écran")?;
    let desc = unsafe { duplication.GetDesc() };
    Ok((duplication, desc.ModeDesc.Width, desc.ModeDesc.Height))
}
