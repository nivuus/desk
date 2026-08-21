//! Les types de charge utile de l'INSTALLATION d'un logiciel téléversé.
//!
//! Ils vivent dans un module frère de `apps`, pour la même raison et par le
//! même mécanisme : la règle des 500 lignes de `CLAUDE.md`, et non la
//! « Convention de module enfant », qui vise les modules extraits d'un parent
//! `#[cfg(windows)]`.

use serde::{Deserialize, Serialize};

/// Où en est une installation.
///
/// ⚠️ **LA PHASE `empreinte` N'EST PAS ICI, ET CE N'EST PAS UN OUBLI.** Elle se
/// déroule dans le NAVIGATEUR, avant que la plateforme n'ait la moindre ligne à
/// écrire : elle ne traverse jamais ce canal, qui va de l'agent à la
/// plateforme. Le hub la connaît parce qu'il la vit.
///
/// 🔴 **`Execution` NE PORTE AUCUN POURCENTAGE**, et c'est une décision, pas une
/// lacune : un installeur Windows n'en publie pas. En inventer un serait
/// mentir à l'utilisateur sur une progression que personne ne mesure. Elle
/// porte le TEMPS ÉCOULÉ, et l'interface affiche un état indéterminé.
///
/// ⚠️ **AUCUNE VARIANTE DE CET ENUM N'A DEUX MOTS, DONC SON `rename_all` EST
/// INOBSERVABLE**, et il faut le dire plutôt que de laisser croire le
/// contraire. Le sous-bloc G1 a MESURÉ la lacune : passer `kebab-case` à
/// `snake_case` sur `IssueLancement` — quatre variantes d'un seul mot — laisse
/// `cargo test -p proto` entièrement vert. **Le plan de G3 prescrivait de jouer
/// cette rouge SUR `Phase`, en citant `sans-effet` : c'est une contradiction de
/// son propre texte** — `sans-effet` appartient à [`Issue`], et `Phase` n'a
/// aucune variante de deux mots. La rouge est donc jouée sur [`Issue`], qui en
/// a deux, et **on n'a PAS inventé une quatrième phase pour rendre une mutation
/// observable** : ce serait ajouter au produit un état qu'il n'a pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Les octets descendent de la plateforme vers la VM.
    Transfert,
    /// L'installeur tourne. Aucun pourcentage — voir ci-dessus.
    Execution,
    /// L'installeur est sorti ; on attend la réconciliation qui ferme la
    /// fenêtre de comptage.
    Reconciliation,
}

/// Ce qu'une installation a produit.
///
/// 🔴 **LE CODE DE SORTIE N'ENTRE PAS DANS CETTE DÉCISION**, et c'est la règle
/// qui gouverne tout le sous-bloc : `msiexec` rend **3010** pour un succès qui
/// demande un redémarrage, et beaucoup d'installeurs rendent **0** après une
/// annulation. Un produit qui jugerait sur le code se tromperait dans les deux
/// sens. Le code est RAPPORTÉ, à côté de l'issue ; il ne la décide pas.
///
/// ⚠️ **`Refusee` EST UNE ADDITION À LA SPÉCIFICATION**, qui n'en nomme que
/// trois. Les quatre cas qu'elle couvre — empreinte fausse, élévation requise,
/// extension refusée, processus assigné à un job object — ne sont ni un succès,
/// ni un « sans effet », ni une ignorance : **ce sont des refus, et ils portent
/// leur motif**. Les fondre dans `IssueInconnue` ferait lire « on ne sait pas »
/// là où l'on sait très bien.
///
/// ✅ **DEUX DE SES QUATRE VARIANTES ONT DEUX MOTS**, et c'est délibéré : c'est
/// ce qui rend le `rename_all` de cet enum OBSERVABLE, donc ce qui referme la
/// lacune que G1 a mesurée sans pouvoir la faire rougir (son leg n°9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Issue {
    /// La fenêtre de comptage a vu **au moins une** application apparaître.
    Reussie,
    /// La fenêtre s'est fermée à **zéro**. C'est le cas d'un installeur annulé.
    SansEffet,
    /// Le code de sortie n'a pas pu être recueilli : agent mort pendant
    /// l'exécution, ou expiration. **On ne sait pas**, et on le dit.
    IssueInconnue,
    /// L'installation n'a jamais démarré, et le champ `motif` dit pourquoi.
    Refusee,
}
