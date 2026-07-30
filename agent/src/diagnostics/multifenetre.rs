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

pub(super) mod disponibilite;
pub(super) mod mires;
pub(super) mod replis;
pub(super) mod wgc;

use anyhow::Result;

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
    Ok(false)
}
