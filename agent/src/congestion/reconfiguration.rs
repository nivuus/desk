//! Hot reconfiguration: what happens when the source changes
//! size during a session. The ladder is rebuilt, and the current
//! rung carried over to the new one — bounded if it is shorter.

use std::time::Instant;

use super::controleur::Controleur;
use super::echelle::Echelle;
use super::hysteresis::Hysteresis;
use super::Decision;

impl Controleur {
    /// Rebuilds the ladder for a new source size, keeping the
    /// current rung.
    ///
    /// Called when the user resizes their window: the source changes,
    /// so the ladder's thresholds do too. Without it, the ladder would stay
    /// calibrated for a source that no longer exists — and could request an
    /// encoding size larger than the capture.
    ///
    /// The rung is kept and not the absolute size: it is the LEVEL of
    /// reduction that makes sense, not the number of pixels. It is bounded by the
    /// length of the new ladder, which may be shorter (see
    /// the invariant of `Echelle`).
    pub fn changer_source(&mut self, source: (u32, u32), now: Instant) -> Decision {
        // Index of the rung currently applied, on the OLD ladder —
        // it is the encoded size in place that carries this information, there
        // is no dedicated field. `unwrap_or(0)`: if `encode_size` does not
        // match any rung (should not happen), starting again from the
        // top is the safest choice, never the one that would lack
        // bitrate.
        let index_before = self
            .echelle
            .barreaux()
            .iter()
            .position(|b| b.size == self.current.encode_size)
            .unwrap_or(0);

        self.echelle = Echelle::depuis(source, self.config.fps);
        self.config.source = source;

        // Bounding: the new ladder may have fewer rungs than
        // the old one (tiny source after an extreme shrink, see
        // the invariant of `Echelle`).
        let indice = index_before.min(self.echelle.barreaux().len() - 1);
        self.hysteresis = Hysteresis::new(indice, now);
        self.current.encode_size = self.echelle.barreaux()[indice].size;

        self.current
    }

    /// Changes the bitrate upper bound, without touching the ladder.
    ///
    /// Called when the sensor grants a new share of the session
    /// budget (sub-block D6). **Rebuilds nothing**: `Echelle::depuis` only
    /// depends on the source size and the frame rate, never on the ceiling —
    /// unlike `changer_source` just above, which must carry the
    /// rung over to a new ladder.
    ///
    /// Three possible regimes, and only TWO behaviours — the witness
    /// that distinguishes them is `premiere_estimation_a`
    /// (`Option<Instant>`), not `current.adaptation`: the latter is a
    /// DERIVED and REVERSIBLE state (it falls back to `Indisponible` both before the
    /// very first estimate and after an old estimate goes stale
    /// — see `observer`, branch `o.estimate_bps == None`), whereas
    /// `premiere_estimation_a` is a MONOTONIC fact: set once at the
    /// first estimate, never erased. A first version of this
    /// method relied on `adaptation` and confused the last two
    /// regimes — fixed in review.
    ///
    /// - **Never any estimate** (`premiere_estimation_a.is_none()`,
    ///   initial state of `Controleur::new`, permanent if TWCC is never
    ///   negotiated): `video_bitrate_bps` is only a fallback value equal to the
    ///   ceiling (see the doc of `Config::plafond_bps`), nothing has ever
    ///   controlled it. The bitrate **directly follows** the new ceiling — without this
    ///   case, a ceiling rise would stay frozen on the old value
    ///   forever, nothing else ever raising it.
    /// - **An estimate exists** (`premiere_estimation_a.is_some()`), whether
    ///   `adaptation` is `Active` (it drives the decision right now)
    ///   or `Indisponible` through staleness (the link has just gone silent,
    ///   `observer` then deliberately keeps the last operating
    ///   point rather than controlling nothing): in BOTH cases,
    ///   `video_bitrate_bps` carries real information about what the link
    ///   recently carried, and the bitrate stays **bounded** by it — a
    ///   ceiling going back up never raises it automatically, at the risk of
    ///   saturating a link that has precisely just gone silent. The rise
    ///   only comes at the next usable observation, going through
    ///   `observer` and its hysteresis.
    ///
    /// ⚠️ **This `min` is a ratchet, and it only makes sense if the caller
    /// keeps EMITTING** (I1, final branch review of sub-block D6). The
    /// only thing that loosens it is `observer`, fed by str0m's
    /// `MediaEgressStats` arm, which str0m does not emit for a stream that has
    /// sent nothing (`send_stats.rs`, `if self.bytes == 0 { return; }`). Passing
    /// here the floor of a SLEEPING window — which emits nothing and no longer has
    /// an encoder — would therefore freeze `video_bitrate_bps` for the rest of the
    /// session: the ceiling would go back up on wake-up, not the bitrate.
    /// `Session::appliquer_part` guards against it, and it is there that the guard
    /// must stay: this controller has no business knowing the notion of sleep.
    pub fn changer_plafond(&mut self, plafond_bps: u32) -> Decision {
        self.config.plafond_bps = plafond_bps;
        self.current.video_bitrate_bps = if self.premiere_estimation_a.is_none() {
            plafond_bps
        } else {
            self.current.video_bitrate_bps.min(plafond_bps)
        };
        self.current
    }
}

/// Test settings shared with `controleur::tests`.
///
/// `pub(super)`: the tests of `changer_source` (here) and those of
/// `controleur::observer` (sibling module) both need it. A helper
/// nested in a private `mod tests` would only have been visible to its own
/// subtree — see the head doc of `congestion.rs`.
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
    use super::super::Adaptation;
    use super::*;
    use std::time::Duration;

    #[test]
    fn change_source_keeps_the_current_rung_on_an_enlargement() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Simulates an already applied rung 2 (as after a network
        // degradation), without going through the real hysteresis delay: this test
        // is about `changer_source`, not about how to reach that
        // rung.
        c.hysteresis = Hysteresis::new(2, base);
        c.current.encode_size = c.echelle.barreaux()[2].size;

        // Source enlargement (1920×1080 -> 2560×1440).
        let new_source = (2560, 1440);
        let decision = c.changer_source(new_source, base + Duration::from_secs(1));

        let new_ladder = Echelle::depuis(new_source, config().fps);
        assert_eq!(
            decision.encode_size,
            new_ladder.barreaux()[2].size,
            "le barreau 2 doit être conservé, à la taille de la NOUVELLE échelle"
        );
        assert_eq!(
            c.config.source, new_source,
            "la source mémorisée doit suivre"
        );
    }

    #[test]
    fn change_source_clamps_the_rung_when_the_new_ladder_is_shorter() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Rung 3 (the lowest of the nominal 1920×1080 ladder) already
        // applied.
        c.hysteresis = Hysteresis::new(3, base);
        c.current.encode_size = c.echelle.barreaux()[3].size;
        assert_eq!(
            c.echelle.barreaux().len(),
            4,
            "précondition : 4 barreaux sur la source nominale"
        );

        // Shrink to a tiny source whose ladder has
        // only one rung (see `echelle_minuscule_sans_doublons`):
        // index 3 no longer exists, it must be bounded to 0, the only
        // available rung — not panic on an out-of-bounds access.
        let new_source = (2, 2);
        let decision = c.changer_source(new_source, base + Duration::from_secs(1));

        let new_ladder = Echelle::depuis(new_source, config().fps);
        assert_eq!(new_ladder.barreaux().len(), 1);
        assert_eq!(decision.encode_size, new_ladder.barreaux()[0].size);
        assert_eq!(decision.encode_size, (2, 2));
    }

    #[test]
    fn change_source_recomputes_the_min_bps_thresholds_for_the_new_size() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        let new_source = (1280, 720);
        c.changer_source(new_source, base + Duration::from_secs(1));

        let echelle_attendue = Echelle::depuis(new_source, config().fps);
        let echelle_1080p = Echelle::depuis((1920, 1080), config().fps);
        // Precondition: the two ladders do have different thresholds,
        // otherwise this test would prove nothing.
        assert_ne!(
            echelle_attendue.barreaux()[0].min_bps,
            echelle_1080p.barreaux()[0].min_bps
        );

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
        // Identity, not only value: a pointer to the allocation of the
        // inner `Vec`. A rebuild with the same source and the same
        // `fps` would produce identical values but a DIFFERENT
        // allocation — it is this distinction a mere comparison of
        // values cannot make.
        let ptr_before = c.echelle.barreaux().as_ptr();

        let decision = c.changer_plafond(3_000_000);
        assert_eq!(
            decision.video_bitrate_bps, 3_000_000,
            "le débit suit le nouveau plafond"
        );
        assert_eq!(
            c.echelle.barreaux().as_ptr(),
            ptr_before,
            "l'échelle ne doit pas être reconstruite : même allocation avant et après"
        );
    }

    #[test]
    fn un_plafond_qui_remonte_ne_depasse_pas_l_estimation_courante() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // A modest estimate, then a very high ceiling: it is the
        // estimate that must keep driving.
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
    fn a_rising_ceiling_is_followed_while_no_estimate_has_ever_arrived() {
        let base = t0();
        let mut c = Controleur::new(config(), base);
        // Precondition: no observation has taken place, the adaptation is
        // still the one set by `Controleur::new`.
        assert_eq!(c.current().adaptation, Adaptation::Indisponible);

        // The old ceiling (12,000,000, see `config()`) is indeed lower
        // than the new one: without the remedy, the `min` would freeze the bitrate on
        // the old value forever, since no observation
        // will ever come to raise it.
        let decision = c.changer_plafond(50_000_000);
        assert_eq!(
            decision.video_bitrate_bps, 50_000_000,
            "sans estimation, le débit est une pure valeur de repli : il doit suivre le plafond"
        );
    }

    #[test]
    fn un_plafond_qui_monte_apres_une_estimation_perimee_ne_saute_pas_au_plafond() {
        let base = t0();
        let mut c = Controleur::new(config(), base);

        // A first real estimate: it sets an operating
        // point below the ceiling.
        let d = c
            .observer(super::super::Observation {
                estimate_bps: Some(2_000_000),
                rtt: None,
                loss: None,
                at: base + Duration::from_secs(1),
            })
            .expect("la première estimation doit produire une décision");
        assert_eq!(
            d.adaptation,
            Adaptation::Active,
            "précondition : l'estimation est active"
        );

        // The link goes silent: no more estimates arrive (that is how
        // staleness, handled at the transport level by `EXPIRATION_ESTIMATION`
        // in `transport/adaptation.rs`, translates for the controller — it
        // knows no delay, only absence). Long after the
        // bootstrap window so as not to mix with it.
        c.observer(super::super::Observation {
            estimate_bps: None,
            rtt: None,
            loss: None,
            at: base + Duration::from_secs(30),
        });
        assert_eq!(
            c.current().adaptation,
            Adaptation::Indisponible,
            "précondition : le lien est déclaré indisponible, MAIS une estimation a déjà eu lieu"
        );

        // The current bitrate, inherited from the last real estimate — NOT the
        // ceiling's fallback value, unlike the "never received" case.
        let bitrate_before = c.current().video_bitrate_bps;
        assert!(
            bitrate_before < 50_000_000,
            "précondition : bien en dessous du plafond visé"
        );

        let decision = c.changer_plafond(50_000_000);
        assert_eq!(
            decision.video_bitrate_bps, bitrate_before,
            "une estimation périmée n'est pas « jamais reçue » : le débit ne doit pas sauter au plafond plein sur un lien qui vient de se taire"
        );
    }
}
