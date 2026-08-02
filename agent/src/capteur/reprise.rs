//! Borner la reprise après une rupture du canal média.
//!
//! **Pur à dessein**, sur le modèle exact de `capture/reprise.rs` : c'est la
//! pièce qui décide si une session meurt, et elle doit se tester sur l'hôte.

use std::time::{Duration, Instant};

/// Durée pendant laquelle une rupture du canal est tolérée avant d'être
/// déclarée définitive.
///
/// **Majorante et non calibrée, et il faut le dire.** Elle doit couvrir la
/// détection de la mort du capteur par le superviseur (un tour de boucle), le
/// relancement du processus, l'ouverture de son serveur de tube, et la
/// reconnexion de l'enfant. Aucun de ces quatre délais n'est mesuré à ce jour ;
/// le critère 2 de la recette en donnera un premier ordre de grandeur.
///
/// Ce qui borne le coût d'une valeur trop grande : pendant la fenêtre, la
/// session reste ouverte sur une image figée. Trop petite, elle tue les
/// sessions que la relance devait sauver — le risque est franchement
/// asymétrique, d'où le choix d'une valeur large.
pub const DUREE_FENETRE_CANAL: Duration = Duration::from_secs(15);

/// Fenêtre ouverte à la première rupture et **refermée par le premier
/// succès**. La durée court donc depuis la DERNIÈRE rupture constatée après un
/// succès, jamais depuis la première de la session.
#[derive(Debug, Default)]
pub struct FenetreCanal {
    ouverte_depuis: Option<Instant>,
}

impl FenetreCanal {
    pub fn nouvelle() -> Self {
        Self::default()
    }

    /// À appeler à chaque constat de rupture. Rend **vrai** quand la fenêtre
    /// est expirée, c'est-à-dire quand l'épuisement est acquis.
    pub fn rupture(&mut self, maintenant: Instant) -> bool {
        match self.ouverte_depuis {
            None => {
                self.ouverte_depuis = Some(maintenant);
                false
            }
            Some(debut) => maintenant.duration_since(debut) > DUREE_FENETRE_CANAL,
        }
    }

    /// À appeler dès qu'une lecture aboutit : la fenêtre se referme et le
    /// budget repart entier pour une rupture ultérieure.
    pub fn succes(&mut self) {
        self.ouverte_depuis = None;
    }

    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: Duration) {
        if let Some(debut) = self.ouverte_depuis {
            self.ouverte_depuis = Some(debut - ecart);
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
        assert!(!fenetre.rupture(t0), "la première rupture ouvre la fenêtre, elle ne conclut pas");
        assert!(!fenetre.rupture(t0 + Duration::from_secs(1)));
    }

    #[test]
    fn une_rupture_ininterrompue_au_dela_de_la_fenetre_epuise() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.rupture(t0 + DUREE_FENETRE_CANAL + Duration::from_millis(1)));
    }

    /// Le raccrochage referme la fenêtre : une SECONDE relance du capteur,
    /// plus tard, doit retrouver son budget entier.
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
}
