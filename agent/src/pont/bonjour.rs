//! La règle de la poignée de main `Bonjour` : **pousser les écritures dues, ou
//! les RETENIR**. **PUR** — aucun `cfg`, aucune E/S, entièrement testé sur
//! l'hôte.
//!
//! # Le danger que ce module existe pour empêcher
//!
//! Le journal des dues (F2) survit à l'arrêt du pont. À la reprise, il porte
//! des chemins **relatifs à une racine** — et rien, dans le journal, ne dit
//! LAQUELLE. Si l'utilisateur revient en choisissant un **autre** répertoire,
//! rejouer aveuglément écrirait *les fichiers d'une session dans le dossier
//! d'une autre* (spec §6.4 cas 2).
//!
//! ⚠️ **Et le journal N'EST PAS VIDÉ quand on retient** : jeter perdrait la
//! donnée, pousser la mettrait au mauvais endroit. **On ne fait ni l'un ni
//! l'autre : on NOMME**, et le navigateur affiche « Reprendre
//! l'enregistrement ».
//!
//! # 🔴 CE QUE CE MODULE NE PEUT PAS FAIRE, ET IL FAUT LE LIRE AINSI
//!
//! Il compare un **NOM**. `isSameEntry()` compare deux poignées **vivantes**,
//! jamais une poignée à un souvenir (spec §6.4 cas 2) : il n'existe aucun moyen
//! de reconnaître un répertoire d'une visite à l'autre. **Le nom est un indice,
//! pas une preuve** — deux répertoires homonymes sur deux disques différents
//! mettraient cette règle en défaut, et rien ici ne le dirait.
//!
//! ⚠️ **Et le MODÈLE DE PERMISSION n'est éprouvé par rien** :
//! `showDirectoryPicker()`, `queryPermission`, `requestPermission` et
//! l'activation utilisateur transitoire ne sont appelés nulle part dans ce
//! dépôt — legs de F1, reconduit par F2, F3 et F4. Le montage OPFS de la
//! recette crée deux répertoires de noms différents et monte l'un puis l'autre :
//! il éprouve **la règle**, jamais **qu'elle suffise**.

/// Ce que le fil doit faire de ses écritures dues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Pousser, et mémoriser le nom annoncé.
    Pousser,
    /// Ne rien pousser, et l'annoncer au navigateur par `Dues.retenues`.
    ///
    /// ⚠️ **Le nom annoncé n'est PAS mémorisé** : le mémoriser ferait qu'un
    /// second `Bonjour` sur ce même répertoire pousserait, alors que
    /// l'utilisateur n'a rien confirmé. **Seule une confirmation explicite
    /// (`forcer`) change le nom mémorisé.**
    Retenir,
}

/// Décide, à partir du nom mémorisé et de ce que le navigateur annonce.
///
/// | Nom mémorisé | Nom annoncé | `forcer` | Décision |
/// | --- | --- | --- | --- |
/// | absent | quelconque | — | **Pousser**, et mémoriser |
/// | `X` | `X` | — | **Pousser** |
/// | `X` | `Y ≠ X` | `false` | **Retenir**, et ne rien mémoriser |
/// | `X` | `Y ≠ X` | `true` | **Pousser**, et mémoriser `Y` |
///
/// ⚠️ **Le premier montage POUSSE, et ce n'est pas un trou** : rien ne peut y
/// être mal placé, le journal étant vide ou né de ce même montage. Retenir au
/// premier montage rendrait toute reprise impossible sans un clic, y compris
/// après un simple redémarrage sur le même répertoire.
pub fn decider(memorise: Option<&str>, annonce: &str, forcer: bool) -> Decision {
    match memorise {
        None => Decision::Pousser,
        Some(x) if x == annonce => Decision::Pousser,
        Some(_) if forcer => Decision::Pousser,
        Some(_) => Decision::Retenir,
    }
}

/// Le nom à mémoriser après cette décision, s'il faut en mémoriser un.
///
/// 🔴 **`None` sur `Retenir`, et c'est la moitié qui compte.** Mémoriser le nom
/// qu'on vient de refuser ferait que le `Bonjour` suivant — un simple
/// rechargement de page — le trouverait « connu » et pousserait. *La retenue ne
/// durerait qu'une visite, et le second essai ferait le dommage que le premier
/// a évité.*
pub fn a_memoriser(decision: Decision, annonce: &str) -> Option<&str> {
    match decision {
        Decision::Pousser => Some(annonce),
        Decision::Retenir => None,
    }
}

#[cfg(test)]
mod tests;
