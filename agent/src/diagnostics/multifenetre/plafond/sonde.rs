//! La sonde minimale : D duplications DXGI, tenues, et rien d'autre.
//!
//! **Rien d'autre est le point.** Ni périphérique D3D11 d'encodage, ni
//! encodeur, ni fenêtre, ni WebRTC. Si le refus de la 5ᵉ duplication observé
//! au sous-bloc D2 ne se reproduit pas ici, c'est que le plafond ne porte pas
//! sur la duplication mais sur ce qui l'accompagnait dans l'enfant (hypothèse
//! H3 de la spec) — et c'est un résultat, pas une panne de la sonde.
//!
//! Le rang de la sonde vient de `MULTIFENETRE_PLAFOND_RANG` ; chaque ligne le
//! porte, car `agent.log` mêle le porteur et toutes ses sondes par héritage de
//! `stdout` et rien d'autre ne distinguerait l'émetteur (piège relevé en D1).
//!
//! `DesktopCapture::sur_sortie` retente une ouverture refusée pour
//! indisponibilité passagère pendant `capture_reprise::DUREE_FENETRE_OUVERTURE`
//! (3 s). Un refus rapporté ici est donc **déjà un refus durable**, pas un
//! transitoire — ce n'est pas plus fort que cela.

use anyhow::{Context, Result};

/// Fichier témoin du porteur : sa présence ordonne la sortie.
pub(super) fn chemin_arret() -> std::path::PathBuf {
    std::env::temp_dir().join("plafond-arret")
}

/// Fichier de verdict d'une sonde.
pub(super) fn chemin_verdict(rang: u8) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("plafond-sonde-{rang}.verdict"))
}

// `pub(in super::super)` et non `pub(crate)` : c'est la visibilité la plus
// étroite qui satisfait encore le réexport `pub(super) use sonde::sonder;` de
// `plafond.rs` — `super::super` désigne `multifenetre` depuis `sonde`, ce qui
// correspond exactement à la portée `pub(in multifenetre)` que ce réexport
// déclare. Un `pub(super)` ici (portée `plafond` seul) serait trop étroit et
// ferait échouer ce réexport en E0364 (« sonder is private, and cannot be
// re-exported ») — vérifié à la Task 8. `pub(crate)` compilerait aussi mais
// ouvrirait `sonder` à tout le crate sans raison : rien en dehors de
// `multifenetre` n'en a besoin.
pub(in super::super) fn sonder(sorties: &[String]) -> Result<()> {
    let rang: u8 = std::env::var("MULTIFENETRE_PLAFOND_RANG")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .context("MULTIFENETRE_PLAFOND_RANG doit être un entier")?;

    tracing::info!(sonde = rang, sorties = ?sorties, "sonde démarrée");

    // Les duplications sont TENUES dans ce vecteur : les relâcher libérerait
    // la place et la mesure ne mesurerait plus rien.
    let mut tenues = Vec::new();
    let mut verdict = String::from("OK");
    for (rang_local, nom) in sorties.iter().enumerate() {
        match crate::capture::DesktopCapture::sur_sortie(nom) {
            Ok(duplication) => {
                tracing::info!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    "duplication ouverte"
                );
                tenues.push(duplication);
            }
            Err(erreur) => {
                // Le HRESULT EXACT, et le rang : c'est la seule donnée que la
                // matrice exploite.
                tracing::error!(
                    sonde = rang,
                    duplication = rang_local + 1,
                    %nom,
                    %erreur,
                    "duplication REFUSÉE"
                );
                verdict = format!("KO {erreur} {nom}");
                break;
            }
        }
    }

    std::fs::write(chemin_verdict(rang), &verdict)
        .with_context(|| format!("écriture du verdict de la sonde {rang}"))?;
    tracing::info!(sonde = rang, ouvertes = tenues.len(), %verdict, "verdict déposé");

    // Tenir jusqu'au signal du porteur. Les duplications restent ouvertes tant
    // que `tenues` est vivant.
    while !chemin_arret().exists() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    tracing::info!(sonde = rang, "arrêt demandé, relâchement des duplications");
    Ok(())
}
