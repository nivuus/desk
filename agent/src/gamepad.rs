//! Manette virtuelle : trois fichiers de nature différente.
//!
//! - `gamepad.rs` (ici) porte les logiques pures d'ordonnancement des états
//!   reçus (`plus_recent`) et de limitation du débit des vibrations
//!   (`LimiteurVibration`). Rien n'y est spécifique à Windows : elles vivent
//!   hors de tout `#[cfg(windows)]` et se testent sur cette machine Linux.
//! - `gamepad/win.rs` porte le module ViGEmBus définitif (`win`, sous
//!   `#[cfg(windows)]`, tâche 10) : `VirtualPad` branche une manette Xbox
//!   360 virtuelle et lui applique les états reçus, `spawn_rumble` relaie
//!   ses notifications de vibration vers le client via le canal de
//!   contrôle.
//! - `gamepad/probe.rs` porte la sonde du chantier B (`probe`) : ViGEmBus
//!   est-il utilisable, et son rappel de vibration restitue-t-il les
//!   magnitudes ? **Gardée volontairement** malgré l'arrivée du module
//!   définitif : elle reste appelée par `agent/src/diagnostics.rs`
//!   (`VIGEM_PROBE`), et la tâche 16 (recette) prévoit explicitement de la
//!   réutiliser pour relire l'état de la manette par `XInputGetState`.

use std::time::{Duration, Instant};

/// Débit maximal des messages de vibration vers le client. Le canal de
/// contrôle est FIABLE : l'inonder lui ferait accumuler du retard exactement
/// quand le jeu produit le plus de vibrations.
pub const PERIODE_MIN: Duration = Duration::from_millis(20);

/// Vrai si `nouveau` succède à `courant` dans l'espace des séquences.
///
/// La soustraction en `u16` puis la relecture en `i16` traite le bouclage
/// sans cas particulier : 0 succède bien à 65535.
pub fn plus_recent(nouveau: u16, courant: u16) -> bool {
    (nouveau.wrapping_sub(courant)) as i16 > 0
}

/// Limite le débit des vibrations sans jamais perdre l'état courant.
///
/// `observer` rend l'état à émettre immédiatement, ou `None` s'il est
/// mémorisé. `echu`, appelée périodiquement, rend l'état mémorisé une fois le
/// délai écoulé. Un état mémorisé écrase le précédent : seul le dernier
/// décrit ce que le jeu demande.
pub struct LimiteurVibration {
    dernier_emis: Option<(u8, u8)>,
    dernier_envoi: Option<Instant>,
    en_attente: Option<(u8, u8)>,
}

impl Default for LimiteurVibration {
    fn default() -> Self {
        Self::new()
    }
}

impl LimiteurVibration {
    pub fn new() -> Self {
        Self {
            dernier_emis: None,
            dernier_envoi: None,
            en_attente: None,
        }
    }

    pub fn observer(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        if self.dernier_emis == Some(etat) && self.en_attente.is_none() {
            return None;
        }
        let assez_tot = self
            .dernier_envoi
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if assez_tot {
            self.emettre(maintenant, etat)
        } else {
            self.en_attente = Some(etat);
            None
        }
    }

    pub fn echu(&mut self, maintenant: Instant) -> Option<(u8, u8)> {
        let etat = self.en_attente?;
        let assez_tot = self
            .dernier_envoi
            .is_none_or(|precedent| maintenant.duration_since(precedent) >= PERIODE_MIN);
        if !assez_tot {
            return None;
        }
        self.en_attente = None;
        if self.dernier_emis == Some(etat) {
            return None;
        }
        self.emettre(maintenant, etat)
    }

    fn emettre(&mut self, maintenant: Instant, etat: (u8, u8)) -> Option<(u8, u8)> {
        self.dernier_emis = Some(etat);
        self.dernier_envoi = Some(maintenant);
        self.en_attente = None;
        Some(etat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_etat_plus_recent_est_accepte() {
        assert!(plus_recent(11, 10));
        assert!(plus_recent(1000, 1));
    }

    #[test]
    fn un_etat_plus_ancien_est_rejete() {
        assert!(!plus_recent(9, 10));
        assert!(!plus_recent(1, 1000));
    }

    #[test]
    fn un_etat_identique_est_rejete() {
        assert!(!plus_recent(10, 10));
    }

    #[test]
    fn le_bouclage_de_la_sequence_est_franchi_correctement() {
        // Le point du champ `seq` : à 250 Hz, l'u16 boucle toutes les
        // 4 minutes. Une comparaison naïve `nouveau > courant` rejetterait
        // alors tous les états pendant une demi-boucle — soit deux minutes
        // de manette figée.
        assert!(plus_recent(0, 65535));
        assert!(plus_recent(3, 65533));
        assert!(!plus_recent(65535, 0));
    }

    #[test]
    fn la_premiere_vibration_passe_immediatement() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        assert_eq!(limiteur.observer(t0, (200, 100)), Some((200, 100)));
    }

    #[test]
    fn un_etat_identique_n_est_pas_reemis() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (200, 100));
        assert_eq!(limiteur.observer(t0 + PERIODE_MIN * 2, (200, 100)), None);
    }

    #[test]
    fn un_changement_trop_rapproche_est_differe_puis_emis() {
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        assert_eq!(limiteur.observer(t0 + Duration::from_millis(5), (20, 0)), None);
        assert_eq!(limiteur.echu(t0 + Duration::from_millis(10)), None);
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((20, 0)));
    }

    #[test]
    fn seul_le_dernier_etat_differe_est_emis() {
        // Une rafale de vibrations pendant la fenêtre de limitation ne doit
        // pas produire une file d'états périmés : c'est le DERNIER qui décrit
        // ce que le jeu demande maintenant.
        let t0 = Instant::now();
        let mut limiteur = LimiteurVibration::new();
        limiteur.observer(t0, (10, 0));
        limiteur.observer(t0 + Duration::from_millis(2), (20, 0));
        limiteur.observer(t0 + Duration::from_millis(4), (30, 0));
        limiteur.observer(t0 + Duration::from_millis(6), (40, 0));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN), Some((40, 0)));
        assert_eq!(limiteur.echu(t0 + PERIODE_MIN * 3), None);
    }
}

#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use probe::probe;
#[cfg(windows)]
pub use win::{spawn_connect, spawn_rumble, VirtualPad};
