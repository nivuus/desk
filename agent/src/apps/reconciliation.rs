//! Le diff d'une réconciliation à l'autre.
//!
//! 🔴 CE MODULE EST PUR, et il est le cœur du sous-bloc : c'est lui qui fait
//! que le catalogue voyage en DELTA et non en 154 lignes toutes les
//! 30 secondes.
//!
//! ⚠️ IL NE TIENT AUCUNE MÉMOIRE DES DISPARITIONS. Une application qui revient
//! après avoir disparu ressort en `apparues`, parce que « une disparition
//! n'est pas une suppression » et que c'est la PLATEFORME qui sait qu'elle la
//! connaît déjà — elle porte `disparue_a`, l'agent non. Lui donner cette
//! mémoire dupliquerait un état qui vit déjà ailleurs, et les deux copies
//! divergeraient au premier redémarrage d'agent.

use std::collections::BTreeMap;

use proto::plateforme::Application;

/// Ce qui a changé entre deux lectures du disque.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    pub apparues: Vec<Application>,
    pub modifiees: Vec<Application>,
    /// Des CLÉS, jamais des objets : la plateforme n'a besoin que de
    /// l'identité pour marquer une disparition, et l'objet ferait grossir le
    /// message sans rien porter d'utile.
    pub disparues: Vec<String>,
}

impl Diff {
    /// Rien n'a bougé — l'état nominal, tour après tour, sur un disque au
    /// repos. C'est lui qui décide s'il faut émettre quoi que ce soit.
    pub fn est_vide(&self) -> bool {
        self.apparues.is_empty() && self.modifiees.is_empty() && self.disparues.is_empty()
    }
}

/// Compare deux catalogues PAR CLÉ, et rend un résultat TRIÉ.
///
/// 🔴 PAR CLÉ, JAMAIS PAR ORDRE : le parcours d'un répertoire ne garantit
/// aucun ordre, et comparer positionnellement produirait un diff plein à
/// chaque tour pour un disque qui n'a pas bougé.
///
/// 🔴 TRIÉ, ET C'EST UNE PROPRIÉTÉ DU PROTOCOLE, PAS UNE COMMODITÉ : sans
/// elle, deux réconciliations successives émettraient des messages différents
/// pour un état identique. Le journal montrerait un catalogue qui bouge sans
/// cause, et toute recette qui compare deux tours serait indécidable. La
/// `BTreeMap` la donne par construction — c'est pourquoi ce n'est pas une
/// `HashMap` suivie d'un `sort`.
///
/// ⚠️ UN DOUBLON DE CLÉ DANS `aujourdhui` NE COMPTE QU'UNE FOIS : deux `.lnk`
/// au même triplet sont une seule application, et c'est le cas mesuré — 167
/// raccourcis retenus rendent 154 clés sur la VM. Le dernier lu gagne ; les
/// champs qui les distinguent (nom, chemin du `.lnk`) ne participent pas à
/// l'identité, donc aucun des deux n'est « le bon ».
pub fn diff(hier: &[Application], aujourdhui: &[Application]) -> Diff {
    let anciennes: BTreeMap<&str, &Application> =
        hier.iter().map(|a| (a.cle.as_str(), a)).collect();
    let nouvelles: BTreeMap<&str, &Application> =
        aujourdhui.iter().map(|a| (a.cle.as_str(), a)).collect();

    let mut sortie = Diff::default();
    for (cle, neuve) in &nouvelles {
        match anciennes.get(cle) {
            None => sortie.apparues.push((*neuve).clone()),
            // L'égalité porte sur TOUS les champs, pas seulement la clé : le
            // nom et le chemin du `.lnk` n'ont aucune part dans l'identité,
            // mais ils doivent suivre — et c'est ce chemin-là que le
            // lancement emploie.
            Some(ancienne) if ancienne != neuve => sortie.modifiees.push((*neuve).clone()),
            Some(_) => {}
        }
    }
    for cle in anciennes.keys() {
        if !nouvelles.contains_key(cle) {
            sortie.disparues.push((*cle).to_string());
        }
    }
    sortie
}

#[cfg(test)]
#[path = "reconciliation/tests.rs"]
mod tests;
