//! L'état que le fil d'installation et la boucle de découverte se partagent.
//!
//! 🔴 CE MODULE N'A AUCUN `cfg`, ET C'EST LE POINT. Le fil d'installation est
//! Windows ; ce qu'il partage avec la découverte ne l'est pas, et le ranger
//! derrière le `cfg` obligerait `apps::brancher` — qui a une variante hôte — à
//! porter deux signatures. C'est aussi ce qui rend ces deux mécanismes
//! observables sur l'hôte.
//!
//! 🔴 IL N'Y A QUE DEUX POINTS DE CONTACT ENTRE `apps` ET `installation`, et
//! les voici tous les deux. `apps::boucle` n'a rien d'autre à connaître des
//! installations — c'est ce qui permet à la décision D11 de tenir : deux
//! familles, deux consommateurs, chacune avec un seul.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::fenetre::Fenetres;

/// Ce que le fil partage avec la boucle de découverte.
#[derive(Clone)]
pub struct Partage {
    /// Les fenêtres ouvertes. La boucle y verse `diff.apparues.len()` à chaque
    /// réconciliation ; le fil les ouvre et les ferme.
    pub fenetres: Arc<Mutex<Fenetres>>,
    /// 🔴 « RÉCONCILIE MAINTENANT ». À la sortie de l'installeur, l'agent
    /// **force** une réconciliation plutôt que d'attendre les trente secondes
    /// suivantes : sans cela, le verdict d'une installation de dix secondes
    /// arriverait une demi-minute plus tard.
    ///
    /// ⚠️ **LE FUTUR EST PASSÉ** (sous-bloc G4) : cette phrase disait « c'est ce
    /// que G4 RENDRA immédiat », et `apps::surveillance` existe. **G3 n'en
    /// dépendait pas** — sa fenêtre fonctionne à `PERIODE_RECONCILIATION` —, et
    /// c'est exactement la garantie que l'ordre des sous-blocs existait pour
    /// préserver.
    ///
    /// 🔴 **CE QUE G4 A TROUVÉ ICI, ET QUI N'ÉTAIT PAS UNE ACCÉLÉRATION** : la
    /// poignée de main `reconcilier` / `reconciliee` portait une COURSE. Le
    /// drapeau était lu et baissé **après** la réconciliation, si bien qu'une
    /// réconciliation périodique DÉJÀ EN COURS quand l'installeur sortait
    /// déclarait `reconciliee` pour un tour commencé AVANT que l'installeur
    /// n'ait fini d'écrire — un `sans-effet` FAUX. La fenêtre valait ≈ 0,2 % des
    /// sorties d'installeur, et **G4 l'aurait élargie d'un ordre de grandeur**
    /// puisque tout son objet est de rendre les réconciliations plus fréquentes
    /// pendant une installation. Corrigée dans `apps/boucle.rs`, qui porte le
    /// détail.
    ///
    /// ⚠️ **LA CORRECTION N'A POUR PREUVE QU'UN ARGUMENT DE FLOT DE CONTRÔLE** :
    /// `boucle.rs` est `#[cfg(windows)]`, et la recette de G4 ne lance aucune
    /// installation. Mesure LÉGUÉE, et déclarée manquante.
    pub reconcilier: Arc<AtomicBool>,
    /// Posé par le fil quand une réconciliation forcée a eu lieu.
    pub reconciliee: Arc<AtomicBool>,
}

impl Partage {
    pub fn neuf() -> Self {
        Self {
            fenetres: Arc::new(Mutex::new(Fenetres::nouvelles())),
            reconcilier: Arc::new(AtomicBool::new(false)),
            reconciliee: Arc::new(AtomicBool::new(false)),
        }
    }
}
