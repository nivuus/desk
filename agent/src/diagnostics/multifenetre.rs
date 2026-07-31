//! Sonde préalable au chantier D : quelle voie de capture rend une image
//! correcte par fenêtre quand les fenêtres se recouvrent, et à quel coût.
//!
//! Spec : `docs/superpowers/specs/2026-07-30-sonde-capture-multifenetre-design.md`.
//!
//! Chaque voie s'active par SA PROPRE variable d'environnement, et chaque
//! exécution du binaire n'en éprouve qu'une : `captureservice.dll` plantait
//! en `0xc0000005` au jalon 1, et un plantage de ce genre emporte le
//! processus entier. Les éprouver ensemble ferait perdre les autres avec la
//! première.

pub(super) mod banc;
pub(super) mod contrat;
pub(super) mod disponibilite;
pub(super) mod mires;
pub(super) mod moniteurs;
pub(super) mod nvenc;
pub(super) mod replis;
pub(super) mod sudovda;
pub(super) mod voies;
pub(super) mod wgc;

use anyhow::{Context, Result};

/// Renvoie `true` si une sonde de ce chantier a tourné.
pub(super) fn aiguiller() -> Result<bool> {
    // Relevé DXGI : quelles sorties existent, laquelle porte le bureau.
    if std::env::var("MULTIFENETRE_DXGI").is_ok() {
        disponibilite::relever_dxgi()?;
        return Ok(true);
    }
    // Voie 1 : Windows.Graphics.Capture, re-test honnête (abandonnée au
    // jalon 1 — voir le commentaire de tête de `wgc.rs`).
    if std::env::var("MULTIFENETRE_WGC").is_ok() {
        wgc::eprouver()?;
        return Ok(true);
    }
    // Voie 4 : les replis par fenêtre, sondés en dernier — la moins
    // prometteuse (voir le commentaire de tête de `replis.rs`).
    if std::env::var("MULTIFENETRE_REPLIS").is_ok() {
        replis::eprouver()?;
        return Ok(true);
    }
    // Temps 2 : le banc, sur la voie et le nombre de fenêtres demandés.
    if let Ok(voie) = std::env::var("MULTIFENETRE_BANC") {
        let nombre: u8 = std::env::var("MULTIFENETRE_N")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .context("MULTIFENETRE_N doit être un entier")?;
        banc::executer(&voie, nombre)?;
        return Ok(true);
    }
    // Mesure ① — validation du contrat du pilote d'affichage virtuel, avant
    // toute création de sortie. Le GUID d'interface et 2 des 6 codes IOCTL
    // sont confirmés par octets dans la DLL installée ; la disposition des
    // tampons, elle, est une lecture amont d'un en-tête antérieur de onze mois
    // au pilote. Sans cette sonde, la mesure suivante découvrirait un contrat
    // faux EN MÊME TEMPS qu'elle prend sa mesure, et les deux échecs seraient
    // indiscernables. Elle ne crée aucun moniteur.
    if std::env::var("MULTIFENETRE_CONTRAT").is_ok() {
        contrat::valider_contrat()?;
        return Ok(true);
    }
    // Task 10 : le plafond d'encodeurs H.264 matériels simultanés.
    if std::env::var("MULTIFENETRE_NVENC").is_ok() {
        nvenc::plafond()?;
        return Ok(true);
    }
    Ok(false)
}
