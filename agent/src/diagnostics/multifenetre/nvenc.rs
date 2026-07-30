//! Combien d'encodeurs H.264 matériels cette RTX 4070 accepte-t-elle en
//! parallèle ?
//!
//! L'ordre de grandeur admis (8 sessions sur Ada) est une rumeur de
//! spécification, pas une mesure sur cette carte et ce pilote. Le chantier D
//! en dépend directement : si le plafond est bas, suspendre l'encodage des
//! fenêtres masquées cesse d'être « souhaitable en soi » pour devenir une
//! condition de viabilité.

use anyhow::Result;

/// Au-delà, on cesse de chercher : le résultat serait déjà largement
/// suffisant pour le chantier D.
const PLAFOND_RECHERCHE: usize = 16;

pub(super) fn plafond() -> Result<()> {
    let capture = crate::capture::DesktopCapture::new()?;
    let mut encodeurs = Vec::new();
    for rang in 1..=PLAFOND_RECHERCHE {
        match crate::encode::H264Encoder::new(capture.device(), (1280, 720), (1280, 720), 60, 8_000_000)
        {
            Ok(encodeur) => {
                encodeurs.push(encodeur);
                tracing::info!(rang, "encodeur créé");
            }
            Err(erreur) => {
                tracing::info!(
                    plafond = rang - 1,
                    %erreur,
                    "plafond NVENC atteint — création du suivant refusée"
                );
                return Ok(());
            }
        }
    }
    tracing::info!(
        plafond_recherche = PLAFOND_RECHERCHE,
        "aucun plafond atteint sous {PLAFOND_RECHERCHE} encodeurs"
    );
    Ok(())
}
