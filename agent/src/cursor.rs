//! Observation du curseur Windows : décision de mode (absolu / relatif) et
//! forme à afficher.
//!
//! La décision de mode est stabilisée ici, hors de tout appel système, pour
//! être testable sous Linux — même raison que `geometry.rs` pour le mapping
//! de coordonnées.

/// Nombre d'observations consécutives cohérentes avant qu'un changement de
/// mode soit retenu.
///
/// Les transitions d'écran font clignoter le curseur : sans ce filtre, le
/// pointeur se verrouillerait et se déverrouillerait pendant les chargements.
/// À 50 ms par sondage, trois observations coûtent ~150 ms de latence de
/// bascule — imperceptible, puisqu'elle accompagne un changement de scène.
pub const SEUIL: u8 = 3;

/// Filtre de stabilité sur un état booléen observé périodiquement.
///
/// Rend `Some(nouvel_état)` au moment précis où un changement est retenu, et
/// `None` sinon — y compris pour toutes les observations qui suivent le
/// changement. L'appelant n'a donc rien à mémoriser : il émet un message
/// chaque fois qu'on lui rend `Some`.
pub struct Hysteresis {
    courant: bool,
    compte_contraire: u8,
}

impl Hysteresis {
    pub fn new(initial: bool) -> Self {
        Self { courant: initial, compte_contraire: 0 }
    }

    pub fn observer(&mut self, observe: bool) -> Option<bool> {
        if observe == self.courant {
            self.compte_contraire = 0;
            return None;
        }
        self.compte_contraire += 1;
        if self.compte_contraire < SEUIL {
            return None;
        }
        self.courant = observe;
        self.compte_contraire = 0;
        Some(observe)
    }

    /// État actuellement retenu. Utile au fil de sondage pour renseigner le
    /// drapeau partagé avec l'injecteur d'entrées.
    pub fn courant(&self) -> bool {
        self.courant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_etat_stable_ne_produit_aucun_changement() {
        let mut h = Hysteresis::new(true);
        for _ in 0..10 {
            assert_eq!(h.observer(true), None);
        }
    }

    #[test]
    fn trois_observations_contraires_retiennent_le_changement() {
        let mut h = Hysteresis::new(true);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), Some(false));
    }

    #[test]
    fn le_changement_n_est_annonce_qu_une_seule_fois() {
        let mut h = Hysteresis::new(true);
        for _ in 0..SEUIL - 1 {
            assert_eq!(h.observer(false), None);
        }
        assert_eq!(h.observer(false), Some(false));
        assert_eq!(h.observer(false), None);
    }

    #[test]
    fn une_oscillation_ne_bascule_jamais() {
        let mut h = Hysteresis::new(true);
        for _ in 0..20 {
            assert_eq!(h.observer(false), None);
            assert_eq!(h.observer(true), None);
        }
    }

    #[test]
    fn un_retour_a_l_etat_courant_remet_le_compteur_a_zero() {
        let mut h = Hysteresis::new(true);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(true), None); // le compteur repart de zéro
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), None);
        assert_eq!(h.observer(false), Some(false));
    }

    #[test]
    fn bascule_dans_les_deux_sens() {
        let mut h = Hysteresis::new(true);
        for _ in 0..SEUIL - 1 {
            h.observer(false);
        }
        assert_eq!(h.observer(false), Some(false));
        for _ in 0..SEUIL - 1 {
            assert_eq!(h.observer(true), None);
        }
        assert_eq!(h.observer(true), Some(true));
    }
}
