//! The controller: converts the peer's observations into bitrate and
//! resolution decisions, going through the ladder and the hysteresis.

use std::time::Instant;

use super::echelle::Echelle;
use super::hysteresis::{Hysteresis, DELAI_AMORCAGE};
use super::{Adaptation, Config, Decision, Observation, Qualite};
use super::{ECART_MINIMAL_DEBIT, MARGE, PERTE_MAX_OPUS};

pub struct Controleur {
    /// `pub(super)`: `reconfiguration::changer_source`, in a sibling
    /// module, reads and writes these four fields — see the head doc of
    /// `congestion.rs`.
    pub(super) config: Config,
    pub(super) echelle: Echelle,
    pub(super) hysteresis: Hysteresis,
    pub(super) courant: Decision,
    /// Instant of the very first estimate received, `None` as long as none
    /// has arrived.
    ///
    /// **Repurposed in final branch review (I3+I2).** This field already
    /// existed as a mere boolean (`estimation_vue`), written at the first
    /// estimate but never re-read — a fossil of a lost intent, flagged
    /// by the review. It becomes useful by carrying the INSTANT of that first
    /// estimate rather than a mere flag: that is what makes it possible to bound
    /// `DELAI_AMORCAGE` (see its doc), the window during which the BWE ramp
    /// must not be taken for a degradation.
    ///
    /// **`pub(super)` since sub-block D6 (task 7, fix round 2)**:
    /// `reconfiguration::changer_plafond` needs it to distinguish "no
    /// estimate ever received" from "estimate received then stale". It is
    /// the only field carrying that distinction: unlike
    /// `courant.adaptation` (derived, reversible — it falls back to `Indisponible`
    /// both before the first estimate and after an old estimate
    /// goes stale), this one is a MONOTONIC fact, set once and
    /// never erased.
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

    /// Decision currently applied. Serves at start-up, before any
    /// observation, and to feed the link state message.
    pub fn courant(&self) -> Decision {
        self.courant
    }

    /// Changes the budget reserved for the audio track.
    ///
    /// **Zero when the session does not carry sound** (sub-block D7). Before it,
    /// `Config::audio_bps` was unconditionally `opus::BITRATE_BPS`, and
    /// a window with no audio track at all still cut its video budget
    /// by 128 kb/s.
    ///
    /// **Recomputes nothing by itself**, and it is deliberate: the value only bites
    /// at the next `observer`, which is the only place where the video budget is
    /// derived from the estimate. Recomputing here would require an estimate that
    /// may never have existed.
    pub fn changer_audio_bps(&mut self, bps: u32) {
        self.config.audio_bps = bps;
    }

    pub fn observer(&mut self, o: Observation) -> Option<Decision> {
        let Some(estimate) = o.estimate_bps else {
            // Without an estimate, nothing to control on bitrate or resolution.
            // UNAVAILABILITY, on the other hand, must be reflected immediately:
            // without this line, `self.courant.adaptation` would stay frozen at
            // `Active` after a first estimate followed by a prolonged
            // silence (TWCC drying up, see I4 on the transport side), and
            // `courant()` would lie about the real link state to whoever
            // queries it during that silence — precisely the gap the
            // final branch review named (I2). Only this field moves here:
            // quality, bitrate and size stay those of the last real
            // decision, there is nothing new to draw from them without an estimate.
            self.courant.adaptation = Adaptation::Indisponible;
            return None;
        };
        if self.premiere_estimation_a.is_none() {
            self.premiere_estimation_a = Some(o.at);
        }
        // Bootstrap window: the BWE subsystem starts low and probes
        // upwards (see `DELAI_AMORCAGE`) — during this ramp, a low available
        // bitrate does not mean a degraded link.
        let en_amorcage = o.at.duration_since(
            self.premiere_estimation_a
                .expect("vient d'être posé si absent"),
        ) < DELAI_AMORCAGE;

        // Video share: safety margin, minus the audio budget, bounded by the
        // ceiling. `saturating_sub`: an estimate lower than the audio budget
        // alone must not overflow.
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
        // During bootstrap, never AIM at a rung worse than the one already
        // in place: we feed the hysteresis with the current rung rather than
        // with the computed target, so that no descent accumulates
        // on the BWE ramp (see `DELAI_AMORCAGE`). A BETTER target
        // (going back up) remains allowed without restriction.
        let vise = if en_amorcage && vise > barreau_courant {
            barreau_courant
        } else {
            vise
        };
        if let Some(nouveau) = self.hysteresis.observer(vise, o.at) {
            self.courant.encode_size = self.echelle.barreaux()[nouveau].taille;
        }
        // Correct signal of a resolution change, captured before/after
        // the call to the hysteresis: since `qualite` can switch as soon as
        // the bitrate collapses (even before the hysteresis has moved the
        // resolution), the resolution change that arrives *later* often no
        // longer changes either `qualite` or `video_bitrate_bps` — without
        // this signal, that resolution change would never reach
        // the caller. Not to be confused with the clause further down comparing
        // `encode_size` to the applied rung: it is always false by
        // construction (point raised in review, left for the final branch
        // review) — this new signal complements it without replacing it.
        let resolution_changee = self.courant.encode_size != taille_avant;

        let dernier = self.echelle.barreaux().len() - 1;
        let barreau_applique = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.taille == self.courant.encode_size)
            .unwrap_or(0);

        // `video_bitrate_bps` switches immediately (no hysteresis on it),
        // whereas `encode_size` only moves after the descent delay.
        // Comparing only with the minimum of the *applied* rung would therefore
        // show `Bonne` during this window, while the bitrate
        // already arriving no longer finances the resolution still in place. We
        // also compare `disponible` with the minimum of the applied rung, not
        // only with its index.
        //
        // During bootstrap (`en_amorcage`), this last comparison is
        // disabled: it is what, on a ≥1080p source, made
        // "Image réduite par le réseau" show as a persistent banner from
        // the first observation of the BWE ramp (I3, final branch
        // review) — the available bitrate is low there by construction, without
        // the link being so. What stays active during bootstrap: the
        // floor (`Insuffisante`, above) if the bitrate REALLY falls
        // below the last rung, and degradation by an already
        // applied rung (`barreau_applique > 0`) if a reduction really took
        // place before the bootstrap.
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

/// Relative gap between two bitrates, relative to the larger of the two to
/// stay symmetric — otherwise a division by a zero `avant` would blow up, and
/// a rise from 1 to 2 would not weigh like a drop from 2 to 1.
fn ecart_relatif(avant: u32, apres: u32) -> f32 {
    let max = avant.max(apres);
    if max == 0 {
        return 0.0;
    }
    (avant as f32 - apres as f32).abs() / max as f32
}

#[cfg(test)]
mod tests;
