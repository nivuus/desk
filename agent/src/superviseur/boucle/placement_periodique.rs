//! Contrôle de placement : remet une fenêtre sur sa sortie DXGI si elle en
//! est partie.
//!
//! Extrait de `boucle.rs` (tâche 7 du sous-bloc D3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : ces
//! deux fonctions font partie de la boucle comme les autres, dans le même
//! module logique, juste dans un fichier voisin. Même schéma que
//! `superviseur/table/attribution.rs`.

use super::*;

/// Remet sur sa sortie toute fenêtre qui en est partie, et rafraîchit la
/// taille de sortie que la table a retenue pour chacune.
///
/// **`&mut Table`, et non `&Table`** — hérité d'IMPORTANT 5 (revue de la
/// tâche 9), qui tenait `taille_sortie` à jour d'un changement de mode fait
/// par `WindowsSource::changer_mode_de_sortie` (D8), hors de cette table. Ce
/// chemin a été retiré au sous-bloc D9, mesure à l'appui (voir le constat en
/// tête de `capteur/plein_ecran.rs`) : la relecture DXGI que ce contrôle fait
/// déjà, chaque seconde, pour toutes les sessions vivantes, continue de
/// rafraîchir `taille_sortie` par précaution, mais rien ne peut plus la
/// faire dériver de la taille de création. Voir `Table::rafraichir_taille_sortie`.
pub(super) fn controler_le_placement(table: &mut Table) {
    let toutes = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        // Cloné : `nom_sortie_de` emprunte `table`, et `rafraichir_taille_sortie`
        // juste en dessous en a besoin `&mut`. Un `&str` emprunté ne
        // survivrait pas à cet appel.
        if let Some(nom) = table.nom_sortie_de(&session).map(str::to_string) {
            if let Some(sortie) = toutes.iter().find(|s| s.nom_sortie == nom) {
                table.rafraichir_taille_sortie(&session, (sortie.rect.width, sortie.rect.height));
            }
        }
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
