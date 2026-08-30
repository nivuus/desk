//! Ce que le pilote d'affichage virtuel retient entre deux appels.
//!
//! Extrait de `pilote.rs` (lot 32, tâche 1) pour tenir sous le plafond de
//! 500 lignes du projet — **avant** l'addition qui l'aurait fait franchir, et
//! non après. Même geste et même raison que le module voisin `controle.rs`,
//! et que `superviseur/boucle/creation_sortie.rs` en D10 : ce dépôt a douze
//! fois franchi ce plafond, ne l'a rattrapé par une compression que deux
//! fois, et `CLAUDE.md` interdit désormais nommément la seconde forme.
//!
//! **Aucune décision ici, aucun comportement** : la structure et sa
//! documentation sont déplacées telles quelles. Le seul changement est de
//! visibilité — les champs deviennent `pub(super)`, faute de quoi le parent
//! ne pourrait plus les lire. C'est le piège nommé par `CLAUDE.md` (« une
//! extraction n'est jamais rigoureusement verbatim […] déplace les
//! visibilités »), traité ici plutôt que découvert au compilateur.

use windows::core::GUID;

use crate::moniteurs_virtuels::numeros::Numeros;
use crate::moniteurs_virtuels::IdSortie;

/// Tout ce que le pilote doit retenir entre deux appels, sous un verrou
/// unique — le distributeur de numéros et les deux listes ne servent qu'un
/// seul invariant (« toute sortie créée a un GUID connu tant qu'elle n'est pas
/// retirée »), et deux verrous pour un invariant seraient un piège gratuit.
///
/// **Deux listes en revanche, et non une**, parce que ce sont deux rôles et
/// deux durées de vie.
///
/// Une entrée d'`apparies` sert à **traduire un identifiant en GUID**. Une
/// entrée d'`a_purger` sert à **se souvenir d'un retrait dû**. Confondre les
/// deux fait qu'une sortie dont le retrait a échoué reste indexée par un
/// identifiant que le pilote peut réattribuer : un `detruire` ultérieur
/// apparierait alors l'entrée périmée, enverrait le mauvais GUID, et la sortie
/// vivante ne serait jamais détruite. Un identifiant qu'on ne peut plus honorer
/// doit donc quitter `apparies`, sans que le GUID soit perdu pour autant.
#[derive(Default)]
pub(super) struct EtatSorties {
    /// Sorties vivantes dont l'identifiant est FIABLE — c'est-à-dire rendu par
    /// un tampon de sortie de la bonne taille. Le trait rend un `IdSortie`
    /// (`u32`) alors que le pilote retire par GUID : c'est ici que se fait la
    /// traduction, et rien d'autre n'a le droit d'y figurer.
    pub(super) apparies: Vec<(IdSortie, GUID)>,
    /// GUID de sorties dont la création a réussi et dont le retrait est DÛ,
    /// sans qu'aucun identifiant fiable ne permette de les redemander. Jamais
    /// consultée par `detruire` : elle ne sert qu'à ne pas perdre la trace de
    /// ce qui doit être purgé — voir la tâche 7.
    pub(super) a_purger: Vec<GUID>,
    /// Distributeur des numéros de GUID, avec recyclage à la destruction
    /// réussie — voir `numeros` pour le défaut que ce recyclage corrige (I1).
    pub(super) numeros: Numeros,
}
