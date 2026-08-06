//! Le contrôle de PERSISTANCE (tâche 2bis, sous-bloc D9) : le changement de
//! mode qui a fait bouger la sortie SOUS TEST survit-il à la création d'une
//! sortie virtuelle supplémentaire — l'événement le plus banal de la
//! production, ouvrir une fenêtre de plus ?
//!
//! **Ce que la tâche 2 avait déjà sous les yeux sans le nommer.** Son verdict
//! « ACCEPTE » ne portait que sur le DÉCLENCHEMENT du mouvement (un `WARN`
//! plus tard corrigé) ; le retour de `\\.\DISPLAY5` à sa taille de création,
//! entre la ligne « après le tour » et la ligne « après création (témoin) »,
//! n'était lisible qu'en recoupant deux blocs de topologie à dix lignes
//! d'écart — c'est ce recoupement manuel qui l'a fait manquer au premier
//! rapport. La tâche 2 portait UNE SEULE combinaison exercée
//! (`CDS_TYPE(0)`, dynamique et non persistée par construction) : cette
//! tâche répond pour `CDS_UPDATEREGISTRY`, celle qui persiste par
//! construction, en restreignant le tour à ce seul bras
//! (`combinaisons::combos_du_tour`).
//!
//! Extrait à part plutôt que versé dans `mode_sortie.rs` : ce fichier est à
//! 496 lignes pour un plafond de 500 (`CLAUDE.md`), marge de 4 — toute
//! addition y appelle une extraction, pas une compression.

use crate::sortie_dxgi::SortieDxgi;

/// Restreint le tour à UNE combinaison, désignée par son étiquette exacte
/// (`MULTIFENETRE_MODE_SORTIE_DRAPEAUX`). Sans elle, le comportement actuel
/// est inchangé : les quatre combinaisons, dans l'ordre.
///
/// **Pourquoi** : `CDS_TYPE(0)` réussit au premier essai, ce qui laisse les
/// trois combinaisons suivantes non sollicitées — dont `CDS_UPDATEREGISTRY`,
/// la seule qui persiste par construction. Sans ce sélecteur, la question
/// « le changement PERSISTANT survit-il, lui, à la création d'une sortie ? »
/// n'est pas atteignable.
pub(super) fn combinaison_imposee() -> Option<String> {
    std::env::var("MULTIFENETRE_MODE_SORTIE_DRAPEAUX").ok()
}

/// Journalise le verdict de PERSISTANCE comme un événement nommé, plutôt que
/// de le laisser recomposable seulement en recoupant deux relevés de
/// topologie à dix lignes d'écart (voir le commentaire de tête du module).
///
/// `avant_creation` : la taille de la sortie sous test relevée par
/// `temoin::nom_apres_tour`, APRÈS le tour, duplication encore tenue — pas
/// une supposition sur ce que le tour vient de fixer.
///
/// `apres_creation_releve` : la topologie relue APRÈS que la sortie témoin
/// (une sortie virtuelle NEUVE) a été créée ; `nom_cible` y est recherchée
/// nommément, jamais par position -- doctrine constante de ce module.
///
/// `survit` compare deux tailles obtenues par la MÊME relecture DXGI
/// (`GetDesc`/`DesktopCoordinates`) que le reste de la sonde : jamais le code
/// de retour de `ChangeDisplaySettingsExW`, mesuré ailleurs rendant `0` sur
/// une sortie qui n'a pas bougé d'un pixel.
pub(super) fn journaliser_verdict(
    combinaison: Option<&str>,
    nom_cible: &str,
    avant_creation: (u32, u32),
    apres_creation_releve: &[SortieDxgi],
) {
    let apres_creation = apres_creation_releve
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_cible)
        .map(|sortie| (sortie.rect.width, sortie.rect.height))
        .unwrap_or((0, 0));
    tracing::info!(
        combinaison = ?combinaison,
        avant_creation_l = avant_creation.0,
        avant_creation_h = avant_creation.1,
        apres_creation_l = apres_creation.0,
        apres_creation_h = apres_creation.1,
        survit = avant_creation == apres_creation,
        "PERSISTANCE : le changement de mode survit-il à la création d'une sortie ?"
    );
}
