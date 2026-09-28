//! Measurement ① of the spec: how many simultaneous virtual outputs does an indirect
//! display driver accept, and is a window placed on one captured
//! correctly.
//!
//! This module contains ONLY pure logic: the trait a driver
//! must fulfil, the guard that destroys what was created, and coordinate
//! conversions. The Windows glue lives in the submodules `pilote`,
//! `sudovda`, `peripherique` and `purge`, promoted from
//! `diagnostics/multifenetre/` in sub-block D1.
//!
//! It is NOT under `#[cfg(windows)]`, deliberately: a virtual output
//! outlives the process, so the guard below is the only bulwark against
//! a VM left with ghost monitors — it is exactly the kind of
//! code that must have tests, and they would not run under
//! `#[cfg(windows)]`.

// Windows glue of the SudoVDA driver, promoted from `diagnostics/multifenetre/`
// in sub-block D1: it is no longer measurement tooling, it is the path
// through which the product makes its outputs appear. The parent module stays
// outside `#[cfg(windows)]` — it is what lets its `Sorties` guard have
// tests, and that reason has not changed.
#[cfg(windows)]
pub mod guid;
#[cfg(windows)]
pub mod peripherique;
#[cfg(windows)]
pub mod pilote;
#[cfg(windows)]
pub mod purge;
#[cfg(windows)]
pub mod sudovda;

// Outside `#[cfg(windows)]`, like the parent module and for the same reason:
// the allocation of GUID numbers decides whether an orphaned virtual output
// stays recoverable, and this kind of code must have tests. See its
// header comment (fix I1).
pub mod numeros;

// Outside `#[cfg(windows)]` for the same reason again: the RULE that exchanges a
// target identifier for a GDI name decides the pairing of EVERY
// window, and it must have tests. Its Win32 half is in its internal `mod
// win` — the pattern of `superviseur/placement.rs`.
pub mod config_affichage;

// Outside `#[cfg(windows)]` for the same reason: the VERDICT of a purge is a
// pure rule, and an `ERROR` that wrongly cries at each start-up is an `ERROR`
// no one reads anymore. See its header comment.
pub mod verdict_purge;

use anyhow::{Context, Result};

use crate::geometry::Rect;

/// Identifier of a virtual output, as the driver returns it.
pub type IdSortie = u32;

/// The adapter on which the driver created an output: a Win32 `LUID`,
/// written in two halves — exactly as `sudovda::SortieAjoutee` already
/// writes it, and for the same reason (the assumed layout must be readable where
/// it is at stake).
///
/// 🔴 **The driver returns THREE numbers, and the product only kept ONE.**
/// `SortieAjoutee` carries `(adaptateur_bas, adaptateur_haut, identifiant_cible)`
/// ; until batch 32 only the third survived `create`, the other two
/// only being logged. Yet it is the PAIR that designates a display
/// target unambiguously: a target identifier is only unique PER
/// adapter, and this VM has more than one (SudoVDA, plus QEMU's VGA when
/// it is present).
pub type Adaptateur = (u32, i32);

/// What this block expects from a virtual display driver, whichever it is.
///
/// The indirection exists for two reasons. Spec §6.4 records a fallback —
/// changing driver if the VM's resists — and this fallback must require rewriting
/// neither the scale-up in N nor the guard. And the guard below must
/// be testable without Windows.
pub trait PiloteAffichageVirtuel {
    fn create(&self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie>;
    fn detruire(&self, id: IdSortie) -> Result<()>;
}

/// Destroys the created outputs whatever happens, including if the thread panics.
///
/// Without it, a probe that crashes at the fifth creation leaves five
/// monitors behind it, and the state outlives the process.
pub struct Sorties<'p> {
    pilote: &'p dyn PiloteAffichageVirtuel,
    creees: Vec<IdSortie>,
}

impl<'p> Sorties<'p> {
    pub fn nouvelles(pilote: &'p dyn PiloteAffichageVirtuel) -> Self {
        Self {
            pilote,
            creees: Vec::new(),
        }
    }

    /// A driver refusal comes out as is and does not count as a creation:
    /// destroying an identifier the driver never returned would at best add
    /// one more error to the log, at worst destroy someone else's output.
    pub fn create(&mut self, largeur: u32, hauteur: u32, hertz: u32) -> Result<IdSortie> {
        let id = self.pilote.create(largeur, hauteur, hertz)?;
        self.creees.push(id);
        Ok(id)
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.creees.len()
    }

    /// Returns an output to the driver **during** execution, and stops
    /// holding it.
    ///
    /// Without this method, an output is only returned at the destruction of the
    /// guard, that is when the supervisor stops: the driver's pool
    /// (ten outputs, measured) would then be consumed at each window OPENING
    /// and not per simultaneous window, and about ten
    /// openings-closings would be enough to block any new window.
    ///
    /// On a driver refusal, the output **stays held**: it is still due,
    /// and the guard will retry it at destruction. Forgetting it here would make it
    /// unrecoverable — the driver only removes through a GUID of which only it and
    /// `PiloteParIoctl` keep track.
    pub fn detruire(&mut self, id: IdSortie) -> Result<()> {
        let rang = self
            .creees
            .iter()
            .position(|connu| *connu == id)
            .with_context(|| format!("sortie {id} non tenue par cette garde — rien à rendre"))?;
        self.pilote.detruire(id)?;
        self.creees.remove(rang);
        Ok(())
    }
}

impl Drop for Sorties<'_> {
    fn drop(&mut self) {
        // In reverse order of creation: if the driver has an order-dependent state,
        // undoing it in the order it was built is the only safe choice.
        // `Drop` also runs during the unwinding of a panic — it is
        // precisely the case the guard exists to cover.
        for id in self.creees.drain(..).rev() {
            if let Err(error) = self.pilote.detruire(id) {
                tracing::error!(
                    id,
                    %error,
                    "sortie virtuelle NON détruite — purge manuelle requise"
                );
            }
        }
    }
}

/// Ratio between the dimensions ANNOUNCED by the output
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`) and those of the texture
/// ACTUALLY returned by the acquisition.
///
/// The probe noted a virtual output announced as 3413×960 by DXGI while
/// WMI said 5120×1440 — ratio 1.5006, DPI scaling at 150 %.
/// If a crop is computed on the announced rectangle while the texture
/// is at physical dimensions, it is shifted by as much. Returns `None` if
/// the announcement is degenerate: a ratio would make no sense there.
pub fn facteur_echelle(annonce: (u32, u32), texture: (u32, u32)) -> Option<(f64, f64)> {
    if annonce.0 == 0 || annonce.1 == 0 {
        return None;
    }
    Some((
        texture.0 as f64 / annonce.0 as f64,
        texture.1 as f64 / annonce.1 as f64,
    ))
}

/// Converts a rectangle expressed in virtual desktop coordinates — those
/// where windows live — into the coordinates of the texture returned by
/// the acquisition of `sortie`.
///
/// Two corrections in one: the offset of the output's origin in the
/// virtual desktop, and the scale factor of `facteur_echelle`.
pub fn vers_texture(region: Rect, sortie: Rect, facteur: (f64, f64)) -> Rect {
    let x = (region.x - sortie.x) as f64 * facteur.0;
    let y = (region.y - sortie.y) as f64 * facteur.1;
    Rect {
        x: x.round() as i32,
        y: y.round() as i32,
        width: (region.width as f64 * facteur.0).round() as u32,
        height: (region.height as f64 * facteur.1).round() as u32,
    }
}

/// The full-frame slot of each output, expressed in the frame of reference of ITS
/// texture.
///
/// Work stream D's "one window per output" set-up places a window that
/// covers its whole output; the region to crop is therefore the whole texture.
/// The computation is not trivial for all that: each output carries its own
/// DPI scale factor, and applying the first one's to all would silently shift
/// the crops of the others. The single-output bench only had one
/// factor to know; this one has N.
///
/// `sorties` carries the rectangles announced by DXGI
/// (`DXGI_OUTPUT_DESC::DesktopCoordinates`), `textures` the dimensions
/// actually returned by the acquisition of each, in the same order.
pub fn places_texture_par_sortie(sorties: &[Rect], textures: &[(u32, u32)]) -> Result<Vec<Rect>> {
    anyhow::ensure!(
        sorties.len() == textures.len(),
        "{} sorties pour {} textures : l'appariement serait arbitraire",
        sorties.len(),
        textures.len()
    );
    sorties
        .iter()
        .zip(textures)
        .enumerate()
        .map(|(index, (sortie, texture))| {
            let facteur =
                facteur_echelle((sortie.width, sortie.height), *texture).with_context(|| {
                    format!(
                        "sortie {index} annoncée {}x{} : dimension nulle, aucun facteur \
                         d'échelle n'a de sens",
                        sortie.width, sortie.height
                    )
                })?;
            Ok(vers_texture(*sortie, *sortie, facteur))
        })
        .collect()
}

#[cfg(test)]
mod tests;
