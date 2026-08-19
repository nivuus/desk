//! Contrôle de placement : remet une fenêtre sur sa sortie DXGI si elle en
//! est partie.
//!
//! Extrait de `boucle.rs` (tâche 7 du sous-bloc D3) pour rester sous le
//! plafond de 500 lignes du projet — pas pour une raison de conception : ces
//! deux fonctions font partie de la boucle comme les autres, dans le même
//! module logique, juste dans un fichier voisin. Même schéma que
//! `superviseur/table/attribution.rs`.

use super::*;
use crate::geometry::Rect;

/// Remet sur sa sortie toute fenêtre qui en est partie.
///
/// **`&Table`, et non `&mut Table`.** Jusqu'au sous-bloc D10, cette fonction
/// rafraîchissait aussi `taille_sortie` depuis la taille DXGI brute de la
/// sortie (héritage d'IMPORTANT 5, revue de la tâche 9 de D8, qui tenait ce
/// champ à jour d'un changement de mode fait hors de cette table par
/// `WindowsSource::changer_mode_de_sortie`). Ce chemin a été retiré au
/// sous-bloc D9 ; le rafraîchissement, lui, avait survécu par précaution,
/// alors qu'il ne pouvait déjà plus rien faire dériver.
///
/// **D10 le rend carrément FAUX, et c'est pourquoi il a disparu plutôt que
/// d'être conservé.** Depuis `sortie_pour_viewport` (une sortie peut être
/// bien plus grande que le viewport, registre pollué oblige),
/// `Table::taille_sortie_de` porte la taille RETENUE — celle à laquelle la
/// fenêtre est posée et que la capture recadre —, qui n'a plus aucune raison
/// d'égaler `GetDesc`/`DesktopCoordinates` de la sortie DXGI. Rafraîchir
/// depuis cette dernière aurait donc écrasé la taille retenue par la taille
/// PLEINE de la sortie à chaque tour — reposant la fenêtre en grand une
/// seconde après que `creer_sortie` l'a posée à sa taille recadrée. La table
/// est désormais la seule source de vérité de cette taille, posée une fois à
/// la création (tâche 6) et à la réutilisation (tâche 7) : ce contrôle
/// périodique la relit, il ne la recalcule plus.
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
///
/// **La position vient de la sortie DXGI, la taille de la table** (sous-bloc
/// D10) : `sortie.rect` donne l'origine dans le bureau virtuel, mais
/// `Table::taille_sortie_de` donne la taille RETENUE — celle, éventuellement
/// bien plus petite que la sortie, à laquelle la fenêtre a été posée et que la
/// capture recadre. Le **TROISIÈME** `let Some` — celui de
/// `Table::taille_sortie_de` — ne peut, en pratique, jamais échouer une fois
/// le PREMIER passé (`nom_sortie_de`) : `sortie_creee` pose `nom_sortie` et
/// `taille_sortie` ensemble, jamais l'un sans l'autre (`table.rs`) — ce n'est
/// donc pas `Etat::Vivante` qui gouverne ici, mais cet invariant-là. Gardé
/// tel quel plutôt que supposé, pour ne rien devoir à un fichier voisin.
///
/// ❌ **Cette phrase disait « le second `let Some` », et elle désignait le
/// TROISIÈME** (constat de la revue de la tâche 6, différé puis repris à la
/// revue finale de branche). ⚠️ **L'erreur n'était pas seulement de comptage** :
/// le vrai second — la recherche DXGI par nom, `toutes.iter().find(...)` —
/// **PEUT** échouer après le premier, une sortie pouvant avoir disparu de la
/// topologie entre deux tours. Un lecteur qui comptait les `let Some`
/// attribuait donc la clause « ne peut jamais échouer » au **mauvais garde**,
/// celui pour lequel elle est fausse.
pub(super) fn replacer_si_besoin(table: &Table, session: &IdSession, toutes: &[SortieDxgi]) {
    let Some(nom) = table.nom_sortie_de(session) else {
        return;
    };
    let Some(sortie) = toutes.iter().find(|s| s.nom_sortie == nom) else {
        return;
    };
    let Some((largeur, hauteur)) = table.taille_sortie_de(session) else {
        return;
    };
    let cible = Rect { x: sortie.rect.x, y: sortie.rect.y, width: largeur, height: hauteur };
    let Some(fenetre) = table.fenetre_de(session) else { return };
    let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
    let Ok(actuel) = placement::rectangle_de(hwnd) else { return };
    if placement::doit_etre_replacee(&actuel, &cible) {
        tracing::info!(
            session = %session.0,
            de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
            vers = format!("{}x{}+{}+{}", cible.width, cible.height, cible.x, cible.y),
            "fenêtre sortie de sa sortie, replacement"
        );
        if let Err(erreur) = placement::poser(hwnd, &cible) {
            tracing::warn!(session = %session.0, %erreur, "replacement échoué");
        }
    }
}
