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
use crate::survie_verdict::verdict_persistance;

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
/// `combinaison_imposee` : la valeur BRUTE de `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`,
/// que l'opérateur a demandée -- PAS forcément celle qui a gagné (`combinaison_gagnante`).
/// ⚠️ **Correction (revue de la tâche 2bis, mineurs)** : la première version
/// ne journalisait QUE le bras gagnant, si bien qu'un tour resté VIDE parce
/// que l'étiquette imposée ne correspondait à rien (mojibake, voir le
/// commentaire de tête du module) affichait `combinaison=None` -- identique
/// à l'absence de toute contrainte. Les deux champs distincts permettent de
/// relire directement CE que l'opérateur a demandé et CE qui s'est
/// réellement passé.
///
/// `avant_creation` : la taille de la sortie sous test relevée par
/// `temoin::nom_apres_tour`, APRÈS le tour, duplication encore tenue — pas
/// une supposition sur ce que le tour vient de fixer. `None` si la sortie a
/// disparu à ce moment-là (`<disparue>`, voir `nom_apres_tour`).
///
/// `apres_creation_releve` : la topologie relue APRÈS que la sortie témoin
/// (une sortie virtuelle NEUVE) a été créée ; `nom_cible` y est recherchée
/// nommément, jamais par position -- doctrine constante de ce module.
///
/// ⚠️ **Correction (revue de la tâche 2bis, Important I4)** : `survit` ne
/// vaut PLUS `true` quand la sortie a disparu d'un côté ou de l'autre. La
/// comparaison elle-même est déléguée à `survie_verdict::verdict_persistance`
/// depuis la correction n°14 de la seconde revue — un prédicat PUR, sorti de
/// cet arbre `#[cfg(windows)]` pour être testable sur l'hôte, voir son
/// commentaire de tête pour le défaut exact qu'il empêche de revenir.
///
/// La comparaison elle-même, quand les deux relevés existent, porte sur deux
/// tailles obtenues par la MÊME relecture DXGI (`GetDesc`/`DesktopCoordinates`)
/// que le reste de la sonde : jamais le code de retour de
/// `ChangeDisplaySettingsExW`, mesuré ailleurs rendant `0` sur une sortie qui
/// n'a pas bougé d'un pixel.
pub(super) fn journaliser_verdict(
    combinaison_imposee: Option<&str>,
    combinaison_gagnante: Option<&str>,
    nom_cible: &str,
    avant_creation: Option<(u32, u32)>,
    apres_creation_releve: &[SortieDxgi],
) {
    let apres_creation = apres_creation_releve
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_cible)
        .map(|sortie| (sortie.rect.width, sortie.rect.height));

    let (avant_mesurable, avant_l, avant_h) = match avant_creation {
        Some((l, h)) => (true, l, h),
        None => (false, 0, 0),
    };
    let (apres_mesurable, apres_l, apres_h) = match apres_creation {
        Some((l, h)) => (true, l, h),
        None => (false, 0, 0),
    };
    let survit = verdict_persistance(avant_creation, apres_creation);

    tracing::info!(
        combinaison_imposee = ?combinaison_imposee,
        combinaison_gagnante = ?combinaison_gagnante,
        avant_creation_mesurable = avant_mesurable,
        avant_creation_l = avant_l,
        avant_creation_h = avant_h,
        apres_creation_mesurable = apres_mesurable,
        apres_creation_l = apres_l,
        apres_creation_h = apres_h,
        survit,
        "PERSISTANCE : le changement de mode survit-il à la création d'une sortie ?"
    );
}
