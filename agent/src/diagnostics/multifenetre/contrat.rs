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

use super::moniteurs::ouvrir_pilote;
use super::sudovda::{Veille, VersionProtocole};

/// `VDAProtocolVersion = { 0, 2, 1, true }`, la constante de l'en-tête amont de
/// septembre 2024.
///
/// Elle est confrontée au relevé, et non seulement journalisée à côté : sans
/// cette comparaison, `conforme` ne porterait que des TAILLES, et une taille ne
/// dit rien du contenu. C'est le seul élément du relevé qui corrobore autre
/// chose qu'un dimensionnement.
const VERSION_AMONT: VersionProtocole =
    VersionProtocole { majeure: 0, mineure: 2, increment: 1, version_de_test: 1 };

/// Ce qu'un succès établit : que le GUID d'interface ouvre bien un périphérique
/// vivant, que la formule `CTL_CODE` employée pour les quatre codes NON
/// confirmés par octets est la bonne (`IOCTL_LIRE_VERSION_PROTOCOLE` en fait
/// partie), que le pilote rend exactement le nombre d'octets que suppose la
/// traduction `#[repr(C)]` de ces deux structures, et que les quatre octets de
/// version coïncident avec la constante amont.
///
/// Ce qu'un succès n'établit PAS. D'abord, rien sur
/// `VIRTUAL_DISPLAY_ADD_PARAMS`, dont les 56 octets d'entrée restent une
/// lecture amont non confirmée. Ensuite, l'ORDRE des champs n'est éprouvé que
/// PARTIELLEMENT, et inégalement selon la structure :
///
/// - une taille rendue, elle, ne dit jamais rien des offsets ;
/// - `VersionProtocole` est en revanche bien contrainte par la comparaison des
///   quatre octets : sur les 24 permutations de `{0, 2, 1, 1}`, 22 donnent un
///   quadruplet différent et seraient donc détectées. Seule celle qui échange
///   les deux derniers champs — `increment` et `version_de_test`, tous deux à
///   `1` — passerait inaperçue ;
/// - `Veille` est la seule à être hors d'atteinte d'un tel test : elle rend
///   `delai` = `decompte`, deux valeurs identiques, donc l'ordre de ses deux
///   champs est structurellement indiscernable.
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
    // qui compte n'est pas que les appels aient réussi, mais que ce qu'ils
    // rendent corresponde à ce qu'on suppose. Un pilote ayant gagné un champ
    // depuis l'en-tête amont réussirait l'appel tout en rendant un compte
    // différent.
    //
    // Les deux critères sont énoncés SÉPARÉMENT, et le message dit exactement
    // ce que chacun teste — ni plus. Faire porter à un seul booléen le mot
    // « disposition » alors qu'il ne compare que des tailles serait affirmer
    // au-delà du relevé.
    let tailles_conformes = rendus_version as usize == std::mem::size_of::<VersionProtocole>()
        && rendus_veille as usize == std::mem::size_of::<Veille>();
    let version_conforme = version == VERSION_AMONT;
    tracing::info!(
        tailles_conformes,
        version_conforme,
        conforme = tailles_conformes && version_conforme,
        // Les tailles chiffrées ici sont les SUPPOSÉES, pas les rendues : ce
        // message est constant, il sera émis à l'identique quand
        // `tailles_conformes` vaut `false`. Y annoncer « les tailles rendues
        // (4 et 8) » affirmerait alors exactement ce que le verdict dément.
        "verdict : les tailles rendues sont confrontées aux tailles supposées \
         (4 et 8), et les quatre octets de version à la constante amont \
         {{0, 2, 1, true}} — l'ordre des champs, lui, n'est testé que \
         partiellement"
    );
    Ok(())
}
