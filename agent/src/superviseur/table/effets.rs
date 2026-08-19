//! Les effets que `Table` rend, et que le superviseur exécute.
//!
//! Extrait de `table.rs` (tâche 9 du sous-bloc D10, à la revue, 6 août 2026) :
//! le fichier parent était à 499 lignes, marge 1, une fois posé le champ
//! `taille` de `LancerEnfant` — la contrainte du plan (« extraction d'abord,
//! sans exception », pas « extraction une fois 500 franchi ») s'applique dès
//! cette marge-là, avant même de la dépasser. Purement déclaratif : aucune
//! logique ici, seulement l'énumération et sa documentation, déjà lourde —
//! même motif que `attribution.rs`, voisin dans ce même répertoire.

use super::{IdFenetre, IdSession};

/// Ce que la table demande au monde extérieur de faire. Le superviseur les
/// exécute dans l'ordre rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    AnnoncerOuverture { session: IdSession, titre: String },
    /// `titre` accompagne la demande parce que le refus qui peut en découler
    /// s'affiche à un humain. Sans lui, l'appelant n'a que l'identifiant de
    /// session sous la main et la page-shell annonce « *« w-3 » n'a pas pu
    /// s'ouvrir* » — un message qui ne désigne rien pour l'utilisateur.
    CreerSortie { session: IdSession, titre: String, largeur: u32, hauteur: u32 },
    LancerEnfant {
        session: IdSession,
        fenetre: IdFenetre,
        /// Nom DXGI de la sortie (`\\.\DISPLAYn`), **et non un couple
        /// d'index** : ceux-ci sont positionnels, l'enfant les résout à son
        /// démarrage — donc plus tard — et une sortie apparue ou disparue
        /// entre-temps le fait capturer autre chose, ou échouer.
        nom_sortie: String,
        /// La taille RETENUE (`placement::taille_retenue`), pas celle de la
        /// sortie : la sortie peut être bien plus grande (registre pollué,
        /// voir `creation_sortie::creer_sortie`). C'est cette taille que le
        /// superviseur pose sur l'enfant (`TAILLE_FENETRE`), pour qu'il la
        /// redise au capteur à l'attache (tâche 9 du sous-bloc D10) — le
        /// capteur en a besoin pour recadrer (tâche 8).
        taille: (u32, u32),
    },
    TuerEnfant { session: IdSession },
    /// `sortie_pilote` est **l'identifiant du PILOTE**, pas le nom DXGI : le
    /// pilote ne sait retirer une sortie que par ce qu'il a lui-même rendu à
    /// la création ; lui présenter un nom DXGI ne détruirait rien, ou
    /// détruirait la sortie d'autrui. Les deux identifiants désignent la même
    /// sortie et n'ont aucune relation calculable — d'où les deux champs.
    ///
    /// `nom_sortie` accompagne la destruction parce que l'entrée a déjà
    /// quitté la table quand cet effet est rendu : sans lui, l'appelant ne
    /// pourrait plus savoir quelle place DXGI redevient libre.
    DetruireSortie { sortie_pilote: u32, nom_sortie: String },
    AnnoncerFermeture { session: IdSession },
    AnnoncerRefus { titre: String, motif: String },
}
