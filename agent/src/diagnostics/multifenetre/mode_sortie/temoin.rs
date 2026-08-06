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
use super::{choisir_cible, modes_annonces};
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// Le TÉMOIN (étape 3, brief D9) : rejoue `combo` sur `nom_sortie` — une
/// sortie NEUVE, sans duplication ouverte — et relit son propre mouvement.
///
/// Sans lui, un refus au tour ÉLIMINATOIRE s'imputerait à la duplication
/// tenue pendant ce tour, alors qu'il pourrait tout aussi bien venir du mode
/// choisi lui-même : le témoin rejoue le MÊME geste.
///
/// ⚠️ **Correction (revue de la tâche 2, point mineur)** : ce commentaire
/// affirmait « la seule variable qui change étant la duplication » — c'est
/// faux. La sortie témoin est une sortie NEUVE (pas la même que celle testée
/// par l'éliminatoire), et `drop(voisines)` relâche aussi les deux
/// duplications voisines avant que le témoin ne soit rejoué
/// (`mode_sortie.rs::executer`). Au moins trois choses diffèrent entre
/// l'éliminatoire et le témoin : la duplication SUT, les deux duplications
/// voisines, et l'identité de la sortie elle-même. Le témoin isole « aucune
/// duplication DXGI ouverte nulle part », pas « uniquement la duplication
/// SUT ».
///
/// `cible_eliminatoire` est la cible retenue pour l'ÉLIMINATOIRE, PAS
/// forcément celle appliquée ici : voir `cible_du_temoin`, qui la substitue
/// dans le cas dégénéré où la sortie témoin naît déjà à cette valeur
/// (persistance registre, doctrine D8) -- exactement le défaut F1 que
/// `choisir_cible` corrige déjà pour l'éliminatoire, et qui frapperait le
/// témoin à l'identique sans cette parade (Critique 2 de la revue de la
/// tâche 1).
pub(super) fn rejouer_temoin(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    avant: (u32, u32),
    cible_eliminatoire: (u32, u32),
    combo: &Combo,
) -> Result<()> {
    let Some(cible) = cible_du_temoin(nom_sortie, avant, cible_eliminatoire) else {
        tracing::error!(
            verdict = "TEMOIN NON MESURABLE",
            raison = "la sortie temoin nait deja a la cible de l'eliminatoire, et aucun mode \
                      annonce ne differe de sa taille courante",
            largeur_avant_tentative = avant.0,
            hauteur_avant_tentative = avant.1,
            largeur_cible_eliminatoire = cible_eliminatoire.0,
            hauteur_cible_eliminatoire = cible_eliminatoire.1,
            "verdict TEMOIN : mesure impossible, aucune tentative effectuee"
        );
        return Ok(());
    };
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

/// Choisit la cible RÉELLEMENT appliquée par le témoin.
///
/// En général la MÊME que l'éliminatoire (`cible_eliminatoire`) — c'est le
/// sens même du mot « témoin » : rejouer le geste à l'identique. Mais si
/// cette sortie NEUVE naît déjà à `cible_eliminatoire`, appliquer CE combo
/// sur CETTE cible ne pourrait JAMAIS observer de mouvement, quel que soit le
/// verdict réel du pilote : un pilote qui accepte et un pilote qui refuse
/// rendraient tous deux `derniere_taille == avant`. C'est le défaut F1
/// rejoué — le même que `choisir_cible` corrige pour l'éliminatoire.
///
/// **Ce cas n'est pas un accident de tirage.** La doctrine D8 est qu'une
/// sortie naît à la DERNIÈRE taille laissée au registre par un
/// `CDS_UPDATEREGISTRY` antérieur, et `CLAUDE.md` rapporte que
/// « `CDS_UPDATEREGISTRY` seul a toujours suffi quand quelque chose
/// bougeait » : SI le bras gagnant de l'éliminatoire écrit le registre (3 des
/// 4 bras de `combinaisons::combos` le font), la sortie témoin, créée
/// juste APRÈS, naît alors précisément à `cible_eliminatoire`. C'est donc le
/// cas ATTENDU sur un bras gagnant persistant, pas une exception rare.
///
/// La parade est la MÊME que pour l'éliminatoire : substituer une cible
/// mesurable, choisie parmi ce que CETTE sortie annonce, en excluant sa
/// taille courante. `None` si aucun mode annoncé n'en diffère — cas
/// dégénéré, voir `choisir_cible`.
fn cible_du_temoin(
    nom_sortie: &str,
    avant: (u32, u32),
    cible_eliminatoire: (u32, u32),
) -> Option<(u32, u32)> {
    if avant != cible_eliminatoire {
        return Some(cible_eliminatoire);
    }
    let annonces = modes_annonces(nom_sortie);
    let substituee = choisir_cible(avant, cible_eliminatoire, &annonces);
    if let Some(cible) = substituee {
        tracing::warn!(
            largeur_avant_tentative = avant.0,
            hauteur_avant_tentative = avant.1,
            largeur_cible_eliminatoire = cible_eliminatoire.0,
            hauteur_cible_eliminatoire = cible_eliminatoire.1,
            largeur_cible_temoin = cible.0,
            hauteur_cible_temoin = cible.1,
            "la sortie temoin nait deja a la cible de l'eliminatoire (persistance registre \
             probable) -- cible substituee pour rester mesurable, meme parade que choisir_cible \
             pour l'eliminatoire (defaut F1)"
        );
    }
    substituee
}

/// Le nom sous lequel la sortie testée se retrouve après le tour éliminatoire
/// — l'inconnue annexe n°3 de D8 (« la sortie garde-t-elle son nom
/// `\\.\DISPLAYn` ? »), et sa taille à ce même instant — le point
/// `avant_creation` du contrôle de PERSISTANCE (`persistance::journaliser_verdict`,
/// tâche 2bis de D9), pour ne pas relire une seconde fois une topologie déjà
/// en main.
///
/// Cherche d'abord le nom INCHANGÉ. À défaut, cherche un successeur parmi les
/// noms apparus depuis le tout début de la sonde et qui ne sont ni la sortie
/// testée elle-même ni l'une des deux voisines : une seule candidate tranche,
/// plusieurs ou aucune laissent la question ouverte (`<disparue>`, journalisé
/// à part plutôt que deviné).
///
/// ⚠️ **Correction (revue de la tâche 2bis, I4)** : la taille rend désormais
/// `Option<(u32, u32)>`, PAS `(0, 0)` en repli sur le cas `<disparue>`. Un
/// repli `(0, 0)` faisait dire à `journaliser_verdict` qu'une sortie
/// disparue avait « survécu » dès lors que l'autre bout du calcul valait
/// aussi `(0, 0)` — `None` rend cette confusion impossible par construction.
pub(super) fn nom_apres_tour(
    nom_sortie: &str,
    connues_avant_tout: &HashSet<String>,
    autres_noms_a_nous: &HashSet<String>,
) -> Result<(String, Option<(u32, u32)>)> {
    let releve = relever_topologie("après le tour (inconnues annexes)")?;
    if let Some(sortie) = releve.iter().find(|sortie| sortie.nom_sortie == nom_sortie) {
        return Ok((nom_sortie.to_string(), Some((sortie.rect.width, sortie.rect.height))));
    }
    let candidats: Vec<&str> = releve
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !connues_avant_tout.contains(*nom) && !autres_noms_a_nous.contains(*nom))
        .collect();
    if let [seul] = candidats.as_slice() {
        let taille = releve
            .iter()
            .find(|sortie| sortie.nom_sortie == *seul)
            .map(|sortie| (sortie.rect.width, sortie.rect.height));
        return Ok((seul.to_string(), taille));
    }
    tracing::warn!(
        nom_sortie,
        ?candidats,
        "la sortie testée n'apparaît plus sous son nom d'origine, et aucun successeur univoque \
         ne se dégage -- nom_apres = <disparue>"
    );
    Ok(("<disparue>".to_string(), None))
}
