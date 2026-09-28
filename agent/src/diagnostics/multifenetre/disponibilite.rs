//! Phase 1 of the probe: the viability of each path, one per run.

use anyhow::Result;

/// Surveys all the machine's DXGI outputs.
///
/// First act of the probe, because two documents of the repository
/// contradict each other on the adapter that drives the desktop
/// (`plans/fix-debit-socket-report.md:163` versus commit `4493b24`) and
/// the whole sizing of paths 2 and 3 depends on it.
pub(super) fn relever_dxgi() -> Result<()> {
    let sorties = crate::capture::enumerer_sorties()?;
    tracing::info!(count = sorties.len(), "DXGI outputs recorded");
    for sortie in &sorties {
        tracing::info!(
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            nom = %sortie.nom_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur = sortie.rect.width,
            hauteur = sortie.rect.height,
            "sortie"
        );
    }
    let attachees = sorties.iter().filter(|s| s.attachee_au_bureau).count();
    tracing::info!(
        attachees,
        "verdict: {} output(s) attached to the desktop — the \"one monitor \
         per window\" path requires getting 8 of them",
        attachees
    );
    Ok(())
}
