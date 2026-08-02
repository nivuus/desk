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

/// Intervalle minimal entre deux tentatives de rattachement.
///
/// `next_frame` est appelée ~100 fois par seconde (`FRAME_INTERVAL` vaut
/// 10 ms) : sans ce pas, une rupture provoquerait une centaine de tentatives
/// d'ouverture de tube par seconde et par fenêtre. 250 ms laissent au
/// superviseur le temps de relancer le capteur sans que la reprise traîne —
/// au pire 250 ms de retard sur un rattachement possible, contre 15 s de
/// budget total.
pub const PAS_RATTACHEMENT: Duration = Duration::from_millis(250);

/// Fenêtre ouverte à la première rupture et **refermée par le premier
/// succès**. La durée court donc depuis la DERNIÈRE rupture constatée après un
/// succès, jamais depuis la première de la session.
#[derive(Debug, Default)]
pub struct FenetreCanal {
    ouverte_depuis: Option<Instant>,
    /// Dernier essai de rattachement. `None` = aucun depuis le dernier
    /// succès, donc le prochain est immédiat.
    dernier_essai: Option<Instant>,
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

    /// Vrai si un essai de rattachement est dû. À n'appeler qu'après une
    /// `rupture` non expirée.
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

    /// À appeler dès qu'une lecture aboutit : la fenêtre se referme et le
    /// budget repart entier pour une rupture ultérieure.
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

    /// Le pas d'espacement existe parce que `next_frame` est appelée ~100
    /// fois par seconde : sans lui, une rupture déclencherait 100 tentatives
    /// de reconnexion par seconde et par fenêtre.
    #[test]
    fn les_essais_de_rattachement_sont_espaces() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0), "le premier essai est immédiat");
        assert!(!fenetre.peut_reessayer(t0), "deux essais dans le même instant");
        assert!(!fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT / 2));
        assert!(fenetre.peut_reessayer(t0 + PAS_RATTACHEMENT + Duration::from_millis(1)));
    }

    /// Un succès doit rendre le budget d'essais entier, pas seulement celui
    /// d'expiration : une seconde rupture, plus tard, doit pouvoir réessayer
    /// tout de suite.
    #[test]
    fn un_succes_rend_aussi_le_droit_de_reessayer_immediatement() {
        let mut fenetre = FenetreCanal::nouvelle();
        let t0 = Instant::now();
        assert!(!fenetre.rupture(t0));
        assert!(fenetre.peut_reessayer(t0));
        fenetre.succes();
        let t1 = t0 + Duration::from_millis(1);
        assert!(!fenetre.rupture(t1));
        assert!(fenetre.peut_reessayer(t1), "après un succès, le premier essai est immédiat");
    }
}
