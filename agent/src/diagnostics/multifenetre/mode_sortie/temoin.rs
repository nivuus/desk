//! Le TÉMOIN de l'étape 3 et la troisième inconnue annexe de D8 (« la sortie
//! garde-t-elle son nom `\\.\DISPLAYn` ? »).
//!
//! Extrait de `mode_sortie.rs` à la tâche 1 du sous-bloc D9, pour le plafond
//! de 500 lignes (`CLAUDE.md`). Aucune de ces deux fonctions ne touche à un
//! champ privé de `mode_sortie` : l'une rejoue un geste déjà exposé
//! (`combinaisons::appliquer_combo`), l'autre relit une topologie déjà
//! exposée (`montee::relever_topologie`).

use std::collections::HashSet;

use anyhow::Result;
use windows::Win32::Graphics::Gdi::DISP_CHANGE_SUCCESSFUL;

use super::combinaisons::{appliquer_combo, Combo};
use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// Le TÉMOIN (étape 3, brief D9) : rejoue `combo` sur `nom_sortie` — une
/// sortie NEUVE, sans duplication ouverte — et relit son propre mouvement.
///
/// Sans lui, un refus au tour ÉLIMINATOIRE s'imputerait à la duplication
/// tenue pendant ce tour, alors qu'il pourrait tout aussi bien venir du mode
/// choisi lui-même : le témoin rejoue le MÊME geste, la seule variable qui
/// change étant la duplication.
pub(super) fn rejouer_temoin(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    avant: (u32, u32),
    cible: (u32, u32),
    combo: &Combo,
) -> Result<()> {
    let dernier_code = appliquer_combo(nom_sortie, cible.0, cible.1, combo);
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let releve = relever_topologie(&format!("après tentative TÉMOIN « {} »", combo.etiquette()))?;
    let derniere_taille = releve
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_sortie)
        .map(|sortie| (sortie.rect.width, sortie.rect.height))
        .unwrap_or((0, 0));
    let mouvement = derniere_taille != avant;
    tracing::info!(
        etiquette = combo.etiquette(),
        code_brut = dernier_code,
        api_annonce_succes = (dernier_code == DISP_CHANGE_SUCCESSFUL.0),
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        largeur_relue = derniere_taille.0,
        hauteur_relue = derniere_taille.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        mouvement,
        verdict = if mouvement { "TEMOIN RECU" } else { "TEMOIN REFUSE" },
        "verdict TEMOIN : le meme geste, SANS duplication ouverte, sur une sortie neuve -- \
         departage si un refus de l'eliminatoire vient de la duplication tenue ou du mode choisi"
    );
    Ok(())
}

/// Le nom sous lequel la sortie testée se retrouve après le tour éliminatoire
/// — l'inconnue annexe n°3 de D8 (« la sortie garde-t-elle son nom
/// `\\.\DISPLAYn` ? »).
///
/// Cherche d'abord le nom INCHANGÉ. À défaut, cherche un successeur parmi les
/// noms apparus depuis le tout début de la sonde et qui ne sont ni la sortie
/// testée elle-même ni l'une des deux voisines : une seule candidate tranche,
/// plusieurs ou aucune laissent la question ouverte (`<disparue>`, journalisé
/// à part plutôt que deviné).
pub(super) fn nom_apres_tour(
    nom_sortie: &str,
    connues_avant_tout: &HashSet<String>,
    autres_noms_a_nous: &HashSet<String>,
) -> Result<String> {
    let releve = relever_topologie("après le tour (inconnues annexes)")?;
    if releve.iter().any(|sortie| sortie.nom_sortie == nom_sortie) {
        return Ok(nom_sortie.to_string());
    }
    let candidats: Vec<&str> = releve
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !connues_avant_tout.contains(*nom) && !autres_noms_a_nous.contains(*nom))
        .collect();
    if let [seul] = candidats.as_slice() {
        return Ok(seul.to_string());
    }
    tracing::warn!(
        nom_sortie,
        ?candidats,
        "la sortie testée n'apparaît plus sous son nom d'origine, et aucun successeur univoque \
         ne se dégage -- nom_apres = <disparue>"
    );
    Ok("<disparue>".to_string())
}
