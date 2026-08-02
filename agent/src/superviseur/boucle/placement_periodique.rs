//! Contrôle de placement : remet une fenêtre sur sa sortie DXGI si elle en
//! est partie.
//!
//! Extrait de `boucle.rs` (tâche 7 du sous-bloc D3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : ces
//! deux fonctions font partie de la boucle comme les autres, dans le même
//! module logique, juste dans un fichier voisin. Même schéma que
//! `superviseur/table/attribution.rs`.

use super::*;

/// Remet sur sa sortie toute fenêtre qui en est partie.
pub(super) fn controler_le_placement(table: &Table) {
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        replacer_si_besoin(table, &session, &toutes);
    }
}

/// Remet une fenêtre sur sa sortie si elle en est partie.
///
/// Appelée par le contrôle périodique, **et par le bras `LancerEnfant`** : sur
/// le chemin de réutilisation d'une sortie retenue (§7.1 du sous-bloc D3),
/// `creer_sortie` n'est pas appelée, donc `placement::poser` non plus. Entre
/// la mort de l'enfant et sa relance, l'application a pu déplacer ou retailler
/// sa fenêtre ; sans cet appel, l'enfant capturerait une fenêtre mal posée
/// jusqu'au prochain contrôle périodique — jusqu'à `PERIODE_PLACEMENT` plus
/// tard.
///
/// Idempotente : `doit_etre_replacee` garde l'appel, donc le chemin de
/// création — où la fenêtre vient d'être posée — n'émet **normalement** aucun
/// second `SetWindowPos`. « Normalement » et non « jamais » : si Windows a
/// clampé la taille demandée (taille minimale de la fenêtre, contrainte du DPI),
/// le rectangle obtenu diffère de la cible, `doit_etre_replacee` est vrai, et un
/// second `SetWindowPos` **est** émis — sans plus d'effet que le premier.
pub(super) fn replacer_si_besoin(table: &Table, session: &IdSession, toutes: &[SortieDxgi]) {
    let Some(nom) = table.nom_sortie_de(session) else {
        return;
    };
    let Some(cible) = toutes.iter().find(|s| s.nom_sortie == nom) else {
        return;
    };
    let Some(fenetre) = table.fenetre_de(session) else { return };
    let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
    let Ok(actuel) = placement::rectangle_de(hwnd) else { return };
    if placement::doit_etre_replacee(&actuel, &cible.rect) {
        tracing::info!(
            session = %session.0,
            de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
            vers = format!(
                "{}x{}+{}+{}",
                cible.rect.width, cible.rect.height, cible.rect.x, cible.rect.y
            ),
            "fenêtre sortie de sa sortie, replacement"
        );
        if let Err(erreur) = placement::poser(hwnd, &cible.rect) {
            tracing::warn!(session = %session.0, %erreur, "replacement échoué");
        }
    }
}
