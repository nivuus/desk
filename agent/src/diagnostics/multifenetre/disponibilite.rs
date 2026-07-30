//! Temps 1 de la sonde : la viabilité de chaque voie, une par exécution.

use anyhow::Result;

/// Relève toutes les sorties DXGI de la machine.
///
/// Premier acte de la sonde, parce que deux documents du dépôt se
/// contredisent sur l'adaptateur qui pilote le bureau
/// (`plans/fix-debit-socket-report.md:163` contre le commit `4493b24`) et que
/// tout le dimensionnement des voies 2 et 3 en dépend.
pub(super) fn relever_dxgi() -> Result<()> {
    let sorties = crate::capture::enumerer_sorties()?;
    tracing::info!(nombre = sorties.len(), "sorties DXGI relevées");
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
        "verdict : {} sortie(s) attachée(s) au bureau — la voie « un moniteur \
         par fenêtre » exige d'en obtenir 8",
        attachees
    );
    Ok(())
}
