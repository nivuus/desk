//! Combien d'encodeurs H.264 matériels cette RTX 4070 accepte-t-elle en
//! parallèle ?
//!
//! L'ordre de grandeur admis (8 sessions sur Ada) est une rumeur de
//! spécification, pas une mesure sur cette carte et ce pilote. Le chantier D
//! en dépend directement : si le plafond est bas, suspendre l'encodage des
//! fenêtres masquées cesse d'être « souhaitable en soi » pour devenir une
//! condition de viabilité.
//!
//! La sonde de capture multi-fenêtres a mesuré 8 instances sur un périphérique
//! D3D11 UNIQUE et partagé, la 9ᵉ échouant à la liaison du type d'entrée
//! (`MF_E_UNSUPPORTED_D3D_TYPE`). Elle n'a pas su dire si la contrainte était
//! l'encodeur matériel lui-même ou le partage du périphérique : c'est
//! exactement ce que le mode `separe` tranche ici, le mode `partage` restant
//! disponible comme témoin pour que sa mesure reste reproductible.

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
};

/// Au-delà, on cesse de chercher : le résultat serait déjà largement
/// suffisant pour le chantier D.
const PLAFOND_RECHERCHE: usize = 16;

/// Un périphérique D3D11 neuf, sans lien avec la capture.
///
/// L'adaptateur est laissé au choix du système (`D3D_DRIVER_TYPE_HARDWARE`) :
/// sur cette VM il n'y a qu'un GPU réel, et c'est celui qui porte NVENC.
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

    // Même protection que `DesktopCapture::ouvrir` : ce périphérique est
    // confié à Media Foundation, dont le convertisseur de couleur et
    // l'encodeur y entrent depuis leurs propres fils de travail. Sans elle,
    // deux fils entrent ensemble dans le pilote et l'un peut ne pas
    // ressortir — le blocage mesuré au jalon 1.
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

/// `partage` reproduit la mesure de la sonde : UN périphérique D3D11 pour
/// tous les encodeurs. `separe` répond à la question qu'elle a laissée
/// ouverte : le refus de la 9ᵉ instance venait-il de l'encodeur matériel, ou
/// du partage du périphérique ?
pub(super) fn plafond(mode: &str) -> Result<()> {
    let partage = match mode {
        "partage" => Some(crate::capture::DesktopCapture::new()?),
        "separe" => None,
        autre => {
            anyhow::bail!("MULTIFENETRE_NVENC vaut « partage » ou « separe », pas « {autre} »")
        }
    };
    tracing::info!(mode, "plafond d'encodeurs : mode retenu");

    // Les périphériques autonomes DOIVENT rester vivants aussi longtemps que
    // les encodeurs qui s'y appuient : les relâcher au tour suivant ferait
    // mesurer autre chose que ce qu'on croit.
    let mut peripheriques = Vec::new();
    let mut encodeurs = Vec::new();

    let issue = chercher(mode, partage.as_ref(), &mut peripheriques, &mut encodeurs);

    // La passe d'encodage du banc tuait le processus À LA SORTIE de sa boucle,
    // donc à la DESTRUCTION des encodeurs, pas à leur alimentation. ✅ Défaut
    // diagnostiqué et corrigé le 31 juillet 2026 (`encode::arret` : la MFT
    // NVIDIA gardait un élément de travail en vol au relâchement) — 0 récidive
    // sur 20 exécutions du cas comparable, ce qui n'est pas une preuve
    // d'absence. Ici rien n'est encodé, mais la destruction a bien lieu : ces
    // deux traces encadrent le relâchement pour qu'un plantage à cet endroit se
    // lise comme tel, et ne se confonde jamais avec un plafond — « le processus
    // meurt » et « la création est refusée » sont deux modes d'échec
    // distincts.
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

/// Crée des encodeurs jusqu'au refus, ou jusqu'à `PLAFOND_RECHERCHE`.
///
/// Séparée de `plafond` pour que le relâchement des ressources reste sous le
/// contrôle de l'appelante, quel que soit le chemin de sortie.
fn chercher(
    mode: &str,
    partage: Option<&crate::capture::DesktopCapture>,
    peripheriques: &mut Vec<(ID3D11Device, ID3D11DeviceContext)>,
    encodeurs: &mut Vec<crate::encode::H264Encoder>,
) -> Result<()> {
    for rang in 1..=PLAFOND_RECHERCHE {
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
            Err(erreur) => {
                tracing::info!(
                    plafond = rang - 1,
                    mode,
                    causes = %super::causes(erreur),
                    "plafond d'encodeurs atteint — création du suivant refusée"
                );
                return Ok(());
            }
        }
    }
    tracing::info!(
        plafond_recherche = PLAFOND_RECHERCHE,
        mode,
        "aucun plafond atteint sous {PLAFOND_RECHERCHE} encodeurs"
    );
    Ok(())
}
