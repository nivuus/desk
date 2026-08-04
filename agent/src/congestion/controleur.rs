//! Le contrôleur : convertit les observations du pair en décisions de débit
//! et de résolution, en passant par l'échelle et l'hystérésis.

use std::time::Instant;

use super::echelle::Echelle;
use super::hysteresis::{Hysteresis, DELAI_AMORCAGE};
use super::{Adaptation, Config, Decision, Observation, Qualite};
use super::{ECART_MINIMAL_DEBIT, MARGE, PERTE_MAX_OPUS};

pub struct Controleur {
    /// `pub(super)` : `reconfiguration::changer_source`, dans un module
    /// frère, lit et écrit ces quatre champs — voir la doc de tête de
    /// `congestion.rs`.
    pub(super) config: Config,
    pub(super) echelle: Echelle,
    pub(super) hysteresis: Hysteresis,
    pub(super) courant: Decision,
    /// Instant de la toute première estimation reçue, `None` tant qu'aucune
    /// n'est arrivée.
    ///
    /// **Repurposé en revue finale de branche (I3+I2).** Ce champ existait
    /// déjà comme simple booléen (`estimation_vue`), écrit à la première
    /// estimation mais jamais relu — fossile d'une intention perdue, signalé
    /// par la revue. Il devient utile en portant l'INSTANT de cette première
    /// estimation plutôt qu'un simple drapeau : c'est ce qui permet de borner
    /// `DELAI_AMORCAGE` (voir sa doc), la fenêtre pendant laquelle la rampe du
    /// BWE ne doit pas être prise pour une dégradation.
    ///
    /// **`pub(super)` depuis le sous-bloc D6 (tâche 7, fix round 2)** :
    /// `reconfiguration::changer_plafond` en a besoin pour distinguer « aucune
    /// estimation jamais reçue » de « estimation reçue puis périmée ». C'est
    /// le seul champ qui porte cette distinction : contrairement à
    /// `courant.adaptation` (dérivé, réversible — il retombe à `Indisponible`
    /// aussi bien avant la première estimation qu'après la péremption d'une
    /// estimation ancienne), celui-ci est un fait MONOTONE, posé une fois et
    /// jamais effacé.
    pub(super) premiere_estimation_a: Option<Instant>,
}

impl Controleur {
    pub fn new(config: Config, now: Instant) -> Self {
        let echelle = Echelle::depuis(config.source, config.fps);
        let courant = Decision {
            video_bitrate_bps: config.plafond_bps,
            encode_size: echelle.barreaux()[0].taille,
            opus_loss_perc: 0,
            qualite: Qualite::Bonne,
            adaptation: Adaptation::Indisponible,
        };
        Self {
            config,
            echelle,
            hysteresis: Hysteresis::new(0, now),
            courant,
            premiere_estimation_a: None,
        }
    }

    /// Décision actuellement appliquée. Sert au démarrage, avant toute
    /// observation, et à alimenter le message d'état du lien.
    pub fn courant(&self) -> Decision {
        self.courant
    }

    /// Change le budget réservé à la piste audio.
    ///
    /// **Zéro quand la session ne porte pas le son** (sous-bloc D7). Avant lui,
    /// `Config::audio_bps` valait inconditionnellement `opus::BITRATE_BPS`, et
    /// une fenêtre sans aucune piste audio amputait quand même son budget vidéo
    /// de 128 kb/s.
    ///
    /// **Ne recalcule rien de lui-même**, et c'est délibéré : la valeur ne mord
    /// qu'au prochain `observer`, qui est le seul endroit où le budget vidéo se
    /// dérive de l'estimation. Recalculer ici demanderait une estimation qui
    /// peut n'avoir jamais existé.
    pub fn changer_audio_bps(&mut self, bps: u32) {
        self.config.audio_bps = bps;
    }

    pub fn observer(&mut self, o: Observation) -> Option<Decision> {
        let Some(estimate) = o.estimate_bps else {
            // Sans estimation, rien à asservir sur le débit ni la résolution.
            // L'INDISPONIBILITÉ, elle, doit être reflétée immédiatement :
            // sans cette ligne, `self.courant.adaptation` resterait figé à
            // `Active` après une première estimation suivie d'un silence
            // prolongé (TWCC qui se tarit, voir I4 côté transport), et
            // `courant()` mentirait sur l'état réel du lien à quiconque
            // l'interroge pendant ce silence — précisément le trou que la
            // revue finale de branche a nommé (I2). Seul ce champ bouge ici :
            // qualité, débit et taille restent ceux de la dernière décision
            // réelle, il n'y a rien de neuf à en tirer sans estimation.
            self.courant.adaptation = Adaptation::Indisponible;
            return None;
        };
        if self.premiere_estimation_a.is_none() {
            self.premiere_estimation_a = Some(o.at);
        }
        // Fenêtre d'amorçage : le sous-système BWE part bas et sonde à la
        // hausse (voir `DELAI_AMORCAGE`) — pendant cette rampe, un débit
        // disponible bas ne signifie pas un lien dégradé.
        let en_amorcage = o.at.duration_since(self.premiere_estimation_a.expect(
            "vient d'être posé si absent",
        )) < DELAI_AMORCAGE;

        // Part vidéo : marge de sécurité, moins le budget audio, borné au
        // plafond. `saturating_sub` : une estimation plus basse que le seul
        // budget audio ne doit pas déborder.
        let disponible = ((estimate as f32 * MARGE) as u32)
            .saturating_sub(self.config.audio_bps)
            .min(self.config.plafond_bps);

        let vise = self.echelle.barreau_finance(disponible);
        let taille_avant = self.courant.encode_size;
        let barreau_courant = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == taille_avant)
            .unwrap_or(0);
        // Pendant l'amorçage, ne jamais VISER un barreau pire que celui déjà
        // en place : on nourrit l'hystérésis avec le barreau courant plutôt
        // qu'avec la cible calculée, pour qu'aucune descente ne s'accumule
        // sur la rampe du BWE (voir `DELAI_AMORCAGE`). Une cible MEILLEURE
        // (remontée) reste autorisée sans restriction.
        let vise = if en_amorcage && vise > barreau_courant { barreau_courant } else { vise };
        if let Some(nouveau) = self.hysteresis.observer(vise, o.at) {
            self.courant.encode_size = self.echelle.barreaux()[nouveau].taille;
        }
        // Signal correct d'un changement de résolution, capturé avant/après
        // l'appel à l'hystérésis : depuis que `qualite` peut basculer dès que
        // le débit s'effondre (avant même que l'hystérésis n'ait bougé la
        // résolution), le changement de résolution qui arrive *plus tard* ne
        // fait souvent plus varier ni `qualite` ni `video_bitrate_bps` — sans
        // ce signal, ce changement de résolution ne remonterait jamais à
        // l'appelant. Ne pas confondre avec la clause plus bas comparant
        // `encode_size` au barreau appliqué : elle est toujours fausse par
        // construction (point relevé en revue, laissé pour la revue finale
        // de branche) — ce nouveau signal la complète sans la remplacer.
        let resolution_changee = self.courant.encode_size != taille_avant;

        let dernier = self.echelle.barreaux().len() - 1;
        let barreau_applique = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        // `video_bitrate_bps` bascule immédiatement (pas d'hystérésis dessus),
        // alors que `encode_size` ne bouge qu'après le délai de descente.
        // Comparer seulement au minimum du barreau *appliqué* laisserait donc
        // afficher `Bonne` pendant cette fenêtre, alors que le débit qui
        // arrive déjà ne finance plus la résolution encore en place. On
        // compare aussi `disponible` au minimum du barreau appliqué, pas
        // seulement à son indice.
        //
        // Pendant l'amorçage (`en_amorcage`), cette dernière comparaison est
        // désactivée : c'est elle qui, sur une source ≥1080p, faisait
        // afficher « Image réduite par le réseau » en bandeau persistant dès
        // la première observation de la rampe du BWE (I3, revue finale de
        // branche) — le débit disponible y est bas par construction, sans
        // que le lien le soit. Ce qui reste actif pendant l'amorçage : le
        // plancher (`Insuffisante`, ci-dessus) si le débit tombe RÉELLEMENT
        // sous le dernier barreau, et la dégradation par barreau déjà
        // appliqué (`barreau_applique > 0`) si une réduction a réellement eu
        // lieu avant l'amorçage.
        let qualite = if disponible < self.echelle.barreaux()[dernier].min_bps {
            Qualite::Insuffisante
        } else if barreau_applique > 0
            || (!en_amorcage && disponible < self.echelle.barreaux()[barreau_applique].min_bps)
        {
            Qualite::Degradee
        } else {
            Qualite::Bonne
        };

        let perte = o
            .loss
            .map(|l| ((l * 100.0).round() as i32).clamp(0, PERTE_MAX_OPUS))
            .unwrap_or(self.courant.opus_loss_perc);

        let debit_change = ecart_relatif(self.courant.video_bitrate_bps, disponible)
            >= ECART_MINIMAL_DEBIT;
        let change = debit_change
            || resolution_changee
            || qualite != self.courant.qualite
            || perte != self.courant.opus_loss_perc
            || self.courant.adaptation != Adaptation::Active
            || self.courant.encode_size != self.echelle.barreaux()[barreau_applique].taille;

        if debit_change {
            self.courant.video_bitrate_bps = disponible;
        }
        self.courant.qualite = qualite;
        self.courant.opus_loss_perc = perte;
        self.courant.adaptation = Adaptation::Active;

        change.then_some(self.courant)
    }
}

/// Écart relatif entre deux débits, rapporté au plus grand des deux pour
/// rester symétrique — sinon une division par un `avant` nul exploserait, et
/// une hausse de 1 à 2 ne pèserait pas comme une baisse de 2 à 1.
fn ecart_relatif(avant: u32, apres: u32) -> f32 {
    let max = avant.max(apres);
    if max == 0 {
        return 0.0;
    }
    (avant as f32 - apres as f32).abs() / max as f32
}

#[cfg(test)]
mod tests {
    use super::super::hysteresis::t0;
    use super::super::reconfiguration::config;
    use super::*;
    use std::time::Duration;

    fn obs(estimate_bps: Option<u32>, loss: Option<f32>, at: Instant) -> Observation {
        Observation { estimate_bps, rtt: None, loss, at }
    }

    #[test]
    fn sans_estimation_le_debit_reste_au_plafond_et_l_adaptation_est_indisponible() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c.courant();

        assert_eq!(d.video_bitrate_bps, 12_000_000);
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.adaptation, Adaptation::Indisponible);
        assert_eq!(d.qualite, Qualite::Bonne);

        // Cent observations sans estimation ne changent rien et ne
        // produisent aucune décision.
        for i in 0..100 {
            let at = base + Duration::from_millis(i * 100);
            assert_eq!(c.observer(obs(None, None, at)), None);
        }
        assert_eq!(c.courant().adaptation, Adaptation::Indisponible);
    }

    #[test]
    fn une_session_muette_ne_retranche_plus_le_budget_audio() {
        // Le défaut préexistant que D7 corrige : `Controleur::new` posait
        // `audio_bps` inconditionnellement, si bien que sept fenêtres sur huit
        // amputaient leur budget vidéo de 128 kb/s pour une piste qu'elles
        // n'avaient pas — environ 8,5 % d'une part de 1,5 Mb/s.
        let base = t0();
        let mut avec = Controleur::new(config(), base);
        let mut sans = Controleur::new(config(), base);
        sans.changer_audio_bps(0);

        let o = obs(Some(2_000_000), None, base + DELAI_AMORCAGE * 2);
        avec.observer(o);
        sans.observer(o);

        assert!(
            sans.courant().video_bitrate_bps > avec.courant().video_bitrate_bps,
            "sans piste audio, le budget video doit etre plus grand : {} vs {}",
            sans.courant().video_bitrate_bps,
            avec.courant().video_bitrate_bps
        );
        assert_eq!(
            sans.courant().video_bitrate_bps - avec.courant().video_bitrate_bps,
            crate::opus::BITRATE_BPS as u32,
            "l'ecart doit valoir exactement le budget audio"
        );
    }

    #[test]
    fn la_premiere_estimation_rend_l_adaptation_active() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c
            .observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
            .expect("la première estimation doit produire une décision");

        assert_eq!(d.adaptation, Adaptation::Active);
        // 9 Mb/s × 0,9 − 128 kb/s d'audio = 7,972 Mb/s.
        assert_eq!(d.video_bitrate_bps, 7_972_000);
        // 7,97 Mb/s finance encore le barreau 0 (minimum 6,2 Mb/s).
        assert_eq!(d.encode_size, (1920, 1080));
        assert_eq!(d.qualite, Qualite::Bonne);
    }

    #[test]
    fn le_debit_ne_bouge_pas_pour_moins_de_dix_pour_cent_d_ecart() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)))
            .expect("première décision");

        // +5 % : sous le seuil, aucune décision.
        assert_eq!(
            c.observer(obs(Some(9_450_000), None, base + Duration::from_secs(3))),
            None
        );
        // +20 % : au-delà du seuil, décision produite.
        assert!(c
            .observer(obs(Some(10_800_000), None, base + Duration::from_secs(5)))
            .is_some());
    }

    #[test]
    fn le_debit_ne_depasse_jamais_le_plafond() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        let d = c
            .observer(obs(Some(80_000_000), None, base + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.video_bitrate_bps, 12_000_000, "le plafond BITRATE doit borner");
    }

    #[test]
    fn une_contrainte_durable_fait_descendre_un_barreau_et_marque_la_degradation() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

        // 5 Mb/s : sous le minimum du barreau 0 (6,2 Mb/s). Il faut 2 s — mais
        // la toute première estimation date de 1 s ci-dessus, donc les 5 s
        // suivantes (jusqu'à 6 s) tombent dans `DELAI_AMORCAGE` (I3) : le
        // contrôleur n'y vise jamais un barreau pire que le courant, la
        // descente ne peut donc commencer à s'accumuler qu'à partir de 6 s,
        // pour aboutir à 8 s. La borne du balayage est repoussée en
        // conséquence (40 -> 80, soit 15,8 s) pour laisser cette marge, sans
        // quoi ce test daterait d'avant l'amorçage et échouerait à tort.
        let mut derniere = None;
        for i in 10..80 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(5_000_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_ne!(d.encode_size, (1920, 1080), "la résolution devait descendre");
        assert_eq!(d.qualite, Qualite::Degradee);
    }

    #[test]
    fn sous_le_plancher_la_qualite_est_declaree_insuffisante() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));

        let mut derniere = None;
        for i in 10..200 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(300_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber");
        assert_eq!(d.qualite, Qualite::Insuffisante);
        // On est descendu au dernier barreau, pas plus bas : la cadence n'est
        // jamais sacrifiée automatiquement.
        let echelle = Echelle::depuis((1920, 1080), 60);
        assert_eq!(d.encode_size, echelle.barreaux().last().unwrap().taille);
    }

    #[test]
    fn la_perte_est_convertie_en_pourcentage_et_plafonnee_a_vingt_cinq() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        let d = c
            .observer(obs(Some(9_000_000), Some(0.03), base + Duration::from_secs(1)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 3);

        // 60 % de perte : plafonné à 25, au-delà duquel la redondance coûte
        // plus qu'elle ne sauve.
        let d = c
            .observer(obs(Some(9_000_000), Some(0.60), base + Duration::from_secs(3)))
            .expect("décision");
        assert_eq!(d.opus_loss_perc, 25);
    }

    #[test]
    fn la_qualite_ne_ment_pas_pendant_la_fenetre_d_hysteresis() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Amorce le contrôleur puis laisse s'écouler `DELAI_AMORCAGE` (5 s,
        // voir I3) avant les deux observations qui font l'objet de ce test :
        // il vérifie un mensonge possible en RÉGIME ÉTABLI, pas pendant la
        // rampe de démarrage du BWE, que l'amorçage protège maintenant
        // délibérément (autre test dédié à ce cas : voir
        // `l_amorcage_ne_declenche_pas_de_fausse_alerte`).
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(1)));
        c.observer(obs(Some(9_000_000), None, base + Duration::from_secs(7)));

        // Effondrement à 5 Mb/s, observé une seule fois, moins de 2 s après
        // l'observation précédente : l'hystérésis de descente n'a pas eu le
        // temps de faire descendre la résolution, qui reste donc la taille
        // source. Le débit vidéo, lui, bascule immédiatement (aucune
        // hystérésis ne le protège).
        let d = c
            .observer(obs(Some(5_000_000), None, base + Duration::from_millis(7500)))
            .expect("le débit a assez bougé pour produire une décision");

        assert_eq!(
            d.encode_size,
            (1920, 1080),
            "l'hystérésis n'a pas eu le temps de faire descendre la résolution"
        );
        assert_ne!(
            d.qualite,
            Qualite::Bonne,
            "le débit ne finance plus la résolution encore appliquée : la qualité ne doit pas mentir"
        );
    }

    #[test]
    fn l_amorcage_ne_declenche_pas_de_fausse_alerte() {
        // I3 (revue finale de branche) : sur une source 1920×1080, le barreau
        // 0 exige 6,22 Mb/s, mais le BWE part volontairement bas
        // (`ESTIMATION_INITIALE_BPS` = 2,5 Mb/s côté transport) et sonde à la
        // hausse. Sans fenêtre d'amorçage, la toute première observation
        // annoncerait « Image réduite par le réseau » sur un lien par ailleurs
        // parfait.
        let base = t0();
        let mut c = Controleur::new(config(), base);

        // Première estimation, basse comme au vrai démarrage du BWE : 2,12 Mb/s
        // disponibles (2,5 M × 0,9 − 128 k), bien sous le minimum du barreau 0.
        let d = c
            .observer(obs(Some(2_500_000), None, base + Duration::from_secs(1)))
            .expect("le débit a assez bougé depuis le plafond pour produire une décision");

        assert_eq!(
            d.qualite,
            Qualite::Bonne,
            "la rampe de démarrage du BWE ne doit pas être prise pour une dégradation"
        );
        assert_eq!(
            d.encode_size,
            (1920, 1080),
            "aucune descente ne doit s'engager pendant l'amorçage"
        );

        // Une seconde estimation tout aussi basse, encore pendant la fenêtre
        // d'amorçage (moins de 5 s après la première) : toujours aucune
        // descente engagée, la qualité reste bonne.
        let d = c.observer(obs(Some(2_500_000), None, base + Duration::from_secs(3)));
        if let Some(d) = d {
            assert_eq!(d.qualite, Qualite::Bonne);
            assert_eq!(d.encode_size, (1920, 1080));
        }

        // Après la fenêtre d'amorçage (>= 5 s après la première estimation,
        // donc >= 6 s depuis `base`), une estimation toujours basse doit,
        // elle, produire la dégradation normale — l'amorçage ne doit protéger
        // que la rampe de démarrage, pas masquer un lien réellement mauvais.
        let mut derniere = None;
        for i in 30..80 {
            let at = base + Duration::from_millis(i * 200);
            if let Some(d) = c.observer(obs(Some(2_500_000), None, at)) {
                derniere = Some(d);
            }
        }
        let d = derniere.expect("une décision devait tomber une fois l'amorçage terminé");
        assert_eq!(
            d.qualite,
            Qualite::Degradee,
            "un débit durablement insuffisant hors amorçage doit dégrader normalement"
        );
        assert_ne!(d.encode_size, (1920, 1080), "la résolution devait finir par descendre");
    }
}
