//! Bounding the resumption after a media channel break.
//!
//! **Pure on purpose**, on the exact model of `capture/reprise.rs`: it is the
//! piece that decides whether a session dies, and it must be tested on the host.

use std::time::{Duration, Instant};

/// Duration during which a channel break is tolerated before being
/// declared definitive.
///
/// **An upper bound, not calibrated, and it must be said.** It must cover the
/// supervisor's detection of the sensor's death (one loop round), the
/// process restart, the opening of its pipe server, and the
/// child's reconnection. None of these four delays is measured to date;
/// criterion 2 of the acceptance run will give a first order of magnitude.
///
/// What bounds the cost of a value too large: during the window, the
/// session stays open on a frozen frame. Too small, it kills the
/// sessions the restart was meant to save — the risk is frankly
/// asymmetric, hence the choice of a wide value.
pub const DUREE_FENETRE_CANAL: Duration = Duration::from_secs(15);

/// Intervalle minimal entre deux tentatives de rattachement.
///
/// `next_frame` is called ~100 times per second (`FRAME_INTERVAL` is
/// 10 ms): without this step, a break would cause about a hundred pipe
/// opening attempts per second and per window. 250 ms give the
/// supervisor time to restart the sensor without the resumption dragging —
/// at worst 250 ms of delay on a possible re-attachment, against 15 s of
/// total budget.
pub const PAS_RATTACHEMENT: Duration = Duration::from_millis(250);

/// Window opened at the first break and **closed by the first
/// success**. The duration therefore runs from the LAST break observed after a
/// success, never from the first of the session.
#[derive(Debug, Default)]
pub struct FenetreCanal {
    ouverte_depuis: Option<Instant>,
    /// Last re-attachment attempt. `None` = none since the last
    /// success, so the next one is immediate.
    dernier_essai: Option<Instant>,
}

impl FenetreCanal {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// To be called at every observed break. Returns **true** when the window
    /// has expired, that is when exhaustion is settled.
    pub fn rupture(&mut self, maintenant: Instant) -> bool {
        match self.ouverte_depuis {
            None => {
                self.ouverte_depuis = Some(maintenant);
                false
            }
            Some(debut) => maintenant.duration_since(debut) > DUREE_FENETRE_CANAL,
        }
    }

    /// True if a re-attachment attempt is due. To be called only after a
    /// non-expired `rupture`.
    pub fn peut_reessayer(&mut self, maintenant: Instant) -> bool {
        let du = match self.dernier_essai {
            None => true,
            Some(precedent) => maintenant.duration_since(precedent) > PAS_RATTACHEMENT,
        };
        if du {
            self.dernier_essai = Some(maintenant);
        }
        du
    }

    /// To be called as soon as a read succeeds: the window closes and the
    /// budget starts whole again for a later break.
    pub fn succes(&mut self) {
        self.ouverte_depuis = None;
        self.dernier_essai = None;
    }

    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: Duration) {
        if let Some(debut) = self.ouverte_depuis {
            self.ouverte_depuis = Some(debut - ecart);
        }
        if let Some(dernier) = self.dernier_essai {
            self.dernier_essai = Some(dernier - ecart);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_rupture_n_epuise_pas_immediatement() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(
            !fenetre.rupture(t0),
            "la première rupture ouvre la fenêtre, elle ne conclut pas"
        );
        assert!(!fenetre.rupture(t0 + Duration::from_secs(1)));
    }

    #[test]
    fn une_rupture_ininterrompue_au_dela_de_la_fenetre_epuise() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.rupture(t0 + DUREE_FENETRE_CANAL + Duration::from_millis(1)));
    }

    /// Reconnecting closes the window: a SECOND restart of the sensor,
    /// later, must find its whole budget again.
    #[test]
    fn un_succes_referme_la_fenetre_et_rend_le_budget_entier() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        fenetre.succes();
        let t1 = t0 + DUREE_FENETRE_CANAL * 3;
        assert!(!fenetre.rupture(t1), "la fenêtre doit repartir de zéro");
        assert!(!fenetre.rupture(t1 + DUREE_FENETRE_CANAL / 2));
        assert!(fenetre.rupture(t1 + DUREE_FENETRE_CANAL + Duration::from_millis(1)));
    }

    /// The spacing step exists because `next_frame` is called ~100
    /// times per second: without it, a break would trigger 100 reconnection
    /// attempts per second and per window.
    #[test]
    fn les_essais_de_rattachement_sont_espaces() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0), "le premier essai est immédiat");
        assert!(
            !fenetre.peut_reessayer(t0),
            "deux essais dans le même instant"
        );
        assert!(!fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT / 2));
        assert!(fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT + Duration::from_millis(1)));
    }

    /// A success must restore the whole attempt budget, not only the
    /// expiry one: a second break, later, must be able to retry
    /// right away.
    #[test]
    fn un_succes_rend_aussi_le_droit_de_reessayer_immediatement() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0));
        fenetre.succes();
        let t1 = t0 + Duration::from_millis(1);
        assert!(!fenetre.rupture(t1));
        assert!(
            fenetre.peut_reessayer(t1),
            "après un succès, le premier essai est immédiat"
        );
    }
}
