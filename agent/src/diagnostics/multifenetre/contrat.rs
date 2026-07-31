//! Sonde `MULTIFENETRE_CONTRAT` : le contrat lu en amont correspond-il au
//! pilote d'affichage virtuel réellement installé sur cette VM ?
//!
//! **Pourquoi cette sonde existe, alors que le plan ne la prévoyait pas.** Le
//! plan s'arrêtait à une vérification de compilation : rien, dans son
//! périmètre, n'appelait jamais le pilote. Or seuls le GUID d'interface et deux
//! des six codes IOCTL sont confirmés octet pour octet dans la DLL installée ;
//! la disposition de toutes les structures vient d'un en-tête amont antérieur
//! de onze mois au pilote (canal-de-controle.md §5.2, réserve). S'en tenir là
//! ferait découvrir un contrat faux à la tâche suivante **en même temps**
//! qu'elle prend sa mesure — et les deux échecs seraient indiscernables.
//!
//! Elle ne crée AUCUN moniteur, et n'appelle que les deux IOCTL sans effet de
//! bord aux tampons les plus simples : 4 puis 8 octets de sortie, aucune
//! entrée. C'est la piste que recommande la fin du §5.3 du même document.

use anyhow::Result;

use super::moniteurs::{ouvrir_pilote, Veille, VersionProtocole};

/// Ce qu'un succès établit : que le GUID d'interface ouvre bien un périphérique
/// vivant, que la formule `CTL_CODE` employée pour les quatre codes NON
/// confirmés par octets est la bonne (`IOCTL_LIRE_VERSION_PROTOCOLE` en fait
/// partie), et que le pilote rend exactement le nombre d'octets que suppose la
/// traduction `#[repr(C)]` de ces deux structures.
///
/// Ce qu'un succès n'établit PAS : rien sur `VIRTUAL_DISPLAY_ADD_PARAMS`, dont
/// les 56 octets d'entrée restent une lecture amont non confirmée. Un tampon
/// d'entrée mal formé n'est d'ailleurs pas du même ordre de risque qu'un tampon
/// de sortie mal dimensionné : c'est le pilote qui le lira.
pub(super) fn valider_contrat() -> Result<()> {
    let pilote = ouvrir_pilote()?;
    tracing::info!("périphérique SudoVDA ouvert — le GUID d'interface est le bon");

    let (version, rendus_version) = pilote.version_protocole()?;
    tracing::info!(
        majeure = version.majeure,
        mineure = version.mineure,
        increment = version.increment,
        version_de_test = version.version_de_test,
        octets_rendus = rendus_version,
        attendus = std::mem::size_of::<VersionProtocole>(),
        "version de protocole annoncée par le pilote"
    );

    let (veille, rendus_veille) = pilote.veille()?;
    tracing::info!(
        delai = veille.delai,
        decompte = veille.decompte,
        octets_rendus = rendus_veille,
        attendus = std::mem::size_of::<Veille>(),
        "watchdog du pilote (unité non documentée en amont — nombres bruts)"
    );

    // Le verdict est énoncé ici plutôt que laissé à la lecture du journal : ce
    // qui compte n'est pas que les appels aient réussi, mais que les comptes
    // d'octets rendus correspondent aux tailles supposées. Un pilote qui aurait
    // gagné un champ depuis l'en-tête amont réussirait l'appel tout en rendant
    // un compte différent.
    let conforme = rendus_version as usize == std::mem::size_of::<VersionProtocole>()
        && rendus_veille as usize == std::mem::size_of::<Veille>();
    tracing::info!(
        conforme,
        "verdict : les deux tampons de sortie simples {} la disposition lue en amont",
        if conforme { "confirment" } else { "CONTREDISENT" }
    );
    Ok(())
}
