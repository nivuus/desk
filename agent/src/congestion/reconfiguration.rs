//! Reconfiguration à chaud : ce qui se passe quand la source change de
//! taille en cours de session. L'échelle est reconstruite, et le barreau
//! courant reporté sur la nouvelle — borné si elle est plus courte.

use std::time::Instant;

use super::controleur::Controleur;
use super::echelle::Echelle;
use super::hysteresis::Hysteresis;
use super::{Adaptation, Decision};

impl Controleur {
    /// Rebâtit l'échelle pour une nouvelle taille de source, en conservant le
    /// barreau courant.
    ///
    /// Appelée quand l'utilisateur redimensionne sa fenêtre : la source change,
    /// donc les seuils de l'échelle aussi. Sans cela, l'échelle resterait
    /// calibrée pour une source qui n'existe plus — et pourrait demander une
    /// taille d'encodage supérieure à la capture.
    ///
    /// Le barreau est conservé et non la taille absolue : c'est le NIVEAU de
    /// réduction qui a du sens, pas le nombre de pixels. Il est borné à la
    /// longueur de la nouvelle échelle, qui peut être plus courte (voir
    /// l'invariant d'`Echelle`).
    pub fn changer_source(&mut self, source: (u32, u32), now: Instant) -> Decision {
        // Indice du barreau actuellement appliqué, sur l'ANCIENNE échelle —
        // c'est la taille encodée en place qui porte cette information, il
        // n'existe pas de champ dédié. `unwrap_or(0)` : si `encode_size` ne
        // correspond à aucun barreau (ne devrait pas arriver), repartir du
        // sommet est le choix le plus sûr, jamais celui qui manquerait de
        // débit.
        let indice_avant = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        self.echelle = Echelle::depuis(source, self.config.fps);
        self.config.source = source;

        // Bornage : la nouvelle échelle peut compter moins de barreaux que
        // l'ancienne (source minuscule après un rétrécissement extrême, voir
        // l'invariant d'`Echelle`).
        let indice = indice_avant.min(self.echelle.barreaux().len() - 1);
        self.hysteresis = Hysteresis::new(indice, now);
        self.courant.encode_size = self.echelle.barreaux()[indice].taille;

        self.courant
    }

    /// Change la borne haute de débit, sans toucher à l'échelle.
    ///
    /// Appelée quand le capteur accorde une nouvelle part du budget de
    /// session (sous-bloc D6). **Ne reconstruit rien** : `Echelle::depuis` ne
    /// dépend que de la taille source et de la cadence, jamais du plafond —
    /// contrairement à `changer_source` juste au-dessus, qui doit reporter le
    /// barreau sur une échelle neuve.
    ///
    /// Deux régimes, distingués par `courant.adaptation` — déjà tenu par
    /// `observer`, aucun second témoin introduit ici :
    ///
    /// - **`Indisponible`** (état initial de `Controleur::new`, permanent si
    ///   TWCC n'est jamais négocié) : aucune estimation n'a jamais borné la
    ///   décision, `video_bitrate_bps` n'est qu'une valeur de repli égale au
    ///   plafond (voir la doc de `Config::plafond_bps`). Le débit **suit
    ///   directement** le nouveau plafond — sans ce cas, une hausse de
    ///   plafond resterait figée sur l'ancienne valeur pour toujours, rien
    ///   d'autre ne la relevant jamais.
    /// - **`Active`** : une estimation réelle commande déjà la décision. Le
    ///   débit reste borné par elle — un plafond qui remonte ne le relève
    ///   jamais d'office ; la remontée n'arrive qu'à la prochaine
    ///   observation, en passant par `observer` et son hystérésis.
    pub fn changer_plafond(&mut self, plafond_bps: u32) -> Decision {
        self.config.plafond_bps = plafond_bps;
        self.courant.video_bitrate_bps = match self.courant.adaptation {
            Adaptation::Indisponible => plafond_bps,
            Adaptation::Active => self.courant.video_bitrate_bps.min(plafond_bps),
        };
        self.courant
    }
}

/// Réglages de test partagés avec `controleur::tests`.
///
/// `pub(super)` : les tests de `changer_source` (ici) et ceux de
/// `controleur::observer` (module frère) en ont tous deux besoin. Un helper
/// niché dans un `mod tests` privé n'aurait été visible que de son propre
/// sous-arbre — voir la doc de tête de `congestion.rs`.
#[cfg(test)]
pub(super) fn config() -> super::Config {
    super::Config {
        plafond_bps: 12_000_000,
        audio_bps: 128_000,
        source: (1920, 1080),
        fps: 60,
    }
}

#[cfg(test)]
mod tests {
    use super::super::hysteresis::t0;
    use super::*;
    use std::time::Duration;

    #[test]
    fn changer_source_conserve_le_barreau_courant_sur_un_agrandissement() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Simule un barreau 2 déjà appliqué (comme après une dégradation
        // réseau), sans passer par le délai réel de l'hystérésis : ce test
        // porte sur `changer_source`, pas sur la façon d'atteindre ce
        // barreau.
        c.hysteresis = Hysteresis::new(2, base);
        c.courant.encode_size = c.echelle.barreaux()[2].taille;

        // Agrandissement de la source (1920×1080 -> 2560×1440).
        let nouvelle_source = (2560, 1440);
        let decision = c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let nouvelle_echelle = Echelle::depuis(nouvelle_source, config().fps);
        assert_eq!(
            decision.encode_size,
            nouvelle_echelle.barreaux()[2].taille,
            "le barreau 2 doit être conservé, à la taille de la NOUVELLE échelle"
        );
        assert_eq!(c.config.source, nouvelle_source, "la source mémorisée doit suivre");
    }

    #[test]
    fn changer_source_borne_le_barreau_quand_la_nouvelle_echelle_est_plus_courte() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Barreau 3 (le plus bas de l'échelle nominale 1920×1080) déjà
        // appliqué.
        c.hysteresis = Hysteresis::new(3, base);
        c.courant.encode_size = c.echelle.barreaux()[3].taille;
        assert_eq!(c.echelle.barreaux().len(), 4, "précondition : 4 barreaux sur la source nominale");

        // Rétrécissement vers une source minuscule dont l'échelle ne compte
        // qu'un seul barreau (voir `echelle_minuscule_sans_doublons`) :
        // l'indice 3 n'existe plus, il doit être borné à 0, le seul barreau
        // disponible — pas paniquer sur un accès hors bornes.
        let nouvelle_source = (2, 2);
        let decision = c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let nouvelle_echelle = Echelle::depuis(nouvelle_source, config().fps);
        assert_eq!(nouvelle_echelle.barreaux().len(), 1);
        assert_eq!(decision.encode_size, nouvelle_echelle.barreaux()[0].taille);
        assert_eq!(decision.encode_size, (2, 2));
    }

    #[test]
    fn changer_source_recalcule_les_seuils_min_bps_pour_la_nouvelle_taille() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        let nouvelle_source = (1280, 720);
        c.changer_source(nouvelle_source, base + Duration::from_secs(1));

        let echelle_attendue = Echelle::depuis(nouvelle_source, config().fps);
        let echelle_1080p = Echelle::depuis((1920, 1080), config().fps);
        // Précondition : les deux échelles ont bien des seuils différents,
        // sans quoi ce test ne prouverait rien.
        assert_ne!(echelle_attendue.barreaux()[0].min_bps, echelle_1080p.barreaux()[0].min_bps);

        assert_eq!(
            c.echelle.barreaux()[0].min_bps,
            echelle_attendue.barreaux()[0].min_bps,
            "les seuils min_bps doivent suivre la nouvelle taille de source, pas rester ceux de 1920×1080"
        );
    }

    #[test]
    fn changer_plafond_borne_la_decision_sans_toucher_l_echelle() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Identité, pas seulement valeur : un pointeur vers l'allocation du
        // `Vec` interne. Une reconstruction avec la même source et le même
        // `fps` produirait des valeurs identiques mais une allocation
        // DIFFÉRENTE — c'est cette distinction qu'une simple comparaison de
        // valeurs ne peut pas faire.
        let ptr_avant = c.echelle.barreaux().as_ptr();

        let decision = c.changer_plafond(3_000_000);
        assert_eq!(decision.video_bitrate_bps, 3_000_000, "le débit suit le nouveau plafond");
        assert_eq!(
            c.echelle.barreaux().as_ptr(),
            ptr_avant,
            "l'échelle ne doit pas être reconstruite : même allocation avant et après"
        );
    }

    #[test]
    fn un_plafond_qui_remonte_ne_depasse_pas_l_estimation_courante() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Une estimation modeste, puis un plafond très haut : c'est
        // l'estimation qui doit continuer de commander.
        c.observer(super::super::Observation {
            estimate_bps: Some(2_000_000),
            rtt: None,
            loss: None,
            at: base + Duration::from_secs(1),
        });
        let decision = c.changer_plafond(50_000_000);
        assert!(
            decision.video_bitrate_bps <= 2_000_000,
            "le plafond ne doit jamais faire dépasser l'estimation : {}",
            decision.video_bitrate_bps
        );
    }

    #[test]
    fn un_plafond_qui_monte_est_suivi_tant_qu_aucune_estimation_n_est_jamais_arrivee() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Précondition : aucune observation n'a eu lieu, l'adaptation est
        // encore celle posée par `Controleur::new`.
        assert_eq!(c.courant().adaptation, Adaptation::Indisponible);

        // L'ancien plafond (12 000 000, voir `config()`) est bien inférieur
        // au nouveau : sans le remède, le `min` figerait le débit sur
        // l'ancienne valeur pour toujours, puisqu'aucune observation ne
        // viendra jamais le relever.
        let decision = c.changer_plafond(50_000_000);
        assert_eq!(
            decision.video_bitrate_bps, 50_000_000,
            "sans estimation, le débit est une pure valeur de repli : il doit suivre le plafond"
        );
    }
}
