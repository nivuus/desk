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
        let en_amorcage = o.at.duration_since(
            self.premiere_estimation_a
                .expect("vient d'être posé si absent"),
        ) < DELAI_AMORCAGE;

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
        let vise = if en_amorcage && vise > barreau_courant {
            barreau_courant
        } else {
            vise
        };
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

        let debit_change =
            ecart_relatif(self.courant.video_bitrate_bps, disponible) >= ECART_MINIMAL_DEBIT;
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
mod tests;
