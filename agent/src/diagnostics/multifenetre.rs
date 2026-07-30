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

pub(super) mod mires;

use anyhow::Result;

/// Renvoie `true` si une sonde de ce chantier a tourné.
pub(super) fn aiguiller() -> Result<bool> {
    Ok(false)
}
