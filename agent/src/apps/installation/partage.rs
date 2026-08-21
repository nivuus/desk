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
    /// ⚠️ C'EST CE QUE G4 RENDRA IMMÉDIAT par `ReadDirectoryChangesW`. **G3 n'en
    /// dépend pas** : sa fenêtre fonctionne à `PERIODE_RECONCILIATION`, et
    /// c'est exactement la garantie que l'ordre des sous-blocs existe pour
    /// préserver.
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
