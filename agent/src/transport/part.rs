//! Child-side application of a session budget share granted by
//! the sensor (sub-block D6): see `capteur/repartiteur.rs` for the rule that
//! computes this share, and `capteur/distante.rs` for its transport to here.
//!
//! Extracted from `adaptation`, which crossed 500 lines by taking in this wiring
//! (task 8): both files reconfigure encoding, but for
//! different reasons — `adaptation` reacts to what the NETWORK observes,
//! this one to what the SENSOR arbitrates between several windows.
//!
//! **Unlike branch a0ter, this one DOES mutate an internal field of
//! `Rtc`**: `rtc.bwe().set_desired_bitrate` writes the BWE subsystem's probing
//! target and, if an estimate already exists, reconfigures str0m's
//! pacer (`configure_pacer`, called internally). It does, however, queue
//! NO packet: the deferred effect it schedules on the probing
//! controller side (str0m's `ProbeControl` — which can bring forward
//! the next probe's deadline and cause padding to be emitted) is only
//! evaluated at the NEXT handling of `Input::Timeout`, never during this
//! call. It is this absence of queueing — and not an absence of
//! mutation of `Rtc`, which would be false — that preserves the drain
//! invariant documented at the head of `tick.rs`.

use std::sync::OnceLock;

use str0m::bwe::Bitrate;

use super::Session;

/// Is the probing target armed?
///
/// **BENCH variable, not a product one**: it only exists for the differential
/// A/B of sub-block D6's recorded item no. 4, never played to date.
/// `PART_SONDAGE=0` neutralises `set_desired_bitrate`; any other value, and
/// the variable's absence, arm it. Same convention as `AUDIO` and
/// `PLEIN_ECRAN`: what is shipped is disarmed with `=0`.
///
/// ⚠️ **What the A/B will establish, and nothing more**: that the call has an
/// observable effect on emitted traffic. It will NOT establish that it is necessary — the
/// premise that called it "the most important" was refuted by D6 itself,
/// the bridge carrying ≥ 1.44 Gb/s for `packetsLost = 0`.
fn sondage_arme() -> bool {
    static ARME: OnceLock<bool> = OnceLock::new();
    *ARME.get_or_init(|| {
        let arme = std::env::var("PART_SONDAGE").as_deref() != Ok("0");
        if !arme {
            tracing::warn!("objectif de sondage DESARME (PART_SONDAGE=0) : bras A/B, jamais une configuration livrée");
        }
        arme
    })
}

impl Session {
    /// Applies a share of the session budget granted by the sensor
    /// (sub-block D6).
    ///
    /// **Two applications, and they do not have the same scope.**
    /// `changer_plafond` bounds what the controller will decide to encode — it is
    /// through it that the share really acts, by moving the ladder down
    /// one rung, hence the RESOLUTION. `set_desired_bitrate` bounds what the
    /// BWE subsystem **probes**: without it, N windows would each aim at the
    /// whole link by injecting probing traffic, which is excessive in
    /// principle even if none encoded beyond its share.
    ///
    /// ⚠️ **Do not reread this second application as the remedy for the defect
    /// D6 fixes.** The branch's acceptance run REFUTED the premise it
    /// was drawn from: the bridge carries ≥ 1.44 Gb/s and `packetsLost` is 0 in the
    /// eleven runs — **the link was never the bottleneck**, so the cumulative
    /// probing saturated nothing and was read as congestion by
    /// no one. What saturates is the **browser's decoder**, and the only
    /// lever measured effective against it is the number of pixels
    /// (`docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`,
    /// §1 and §3.6). The mechanism stays right, its justification has changed.
    ///
    /// **A SLEEPING window's share does not go to the controller**, and it is the only
    /// asymmetry of this function. The `PART_DORMANTE_BPS` floor
    /// (256 kb/s) is far below the ladder's lowest rung
    /// (691,200 bps at 1280×720/60): passing it to `changer_plafond` would set
    /// `video_bitrate_bps = 256_000` through its `min`, and **nothing would bring it
    /// back up on wake-up** — the ceiling would come back up, not the bitrate, because the
    /// only repair is `Controleur::observer`, called from the
    /// `MediaEgressStats` arm, which str0m never emits for a stream that has sent
    /// nothing (`send_stats.rs`, `if self.bytes == 0 { return; }`). A sleeping window
    /// sends nothing, by definition. And the remedy would be moot anyway:
    /// a sleeping window has already released its encoder (D5), imposing an
    /// ENCODING ceiling on it bounds nothing that exists. Only probing, for its part,
    /// still makes sense — its `PeerConnection` lives.
    ///
    /// The sleep state is READ (`VideoSource::est_endormie`), never guessed:
    /// deducing it from comparing the share with `PART_DORMANTE_BPS`
    /// would couple two processes through a value — a coupling that would break
    /// silently the day one of the two changed its constant.
    pub(super) fn appliquer_part(&mut self, bps: u32) {
        let endormie = self.source.est_endormie();
        if !endormie {
            self.pending_decision = Some(self.congestion.changer_plafond(bps));
        }
        if sondage_arme() {
            self.rtc.bwe().set_desired_bitrate(Bitrate::bps(bps as u64));
        }
        // `session`: without this field the trace is NOT attributable. All
        // children inherit the same `agent.log` (stdout shared since D4), and
        // the sum of granted shares — criterion ③ of the acceptance run — would
        // then be computed over a multiset of anonymous numbers. Same
        // reason and same field as the video-track-cadence (child side) line.
        // `endormie`: without it, a share applied to probing alone reads
        // in the log exactly like a share applied to both.
        tracing::info!(
            session = %self.session_id,
            part_bps = bps,
            endormie,
            "part de budget appliquee"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use str0m::bwe::BweKind;
    use str0m::media::Mid;
    use str0m::Event;

    use super::*;
    use crate::capteur::repartiteur::PART_DORMANTE_BPS;
    use crate::congestion;
    use crate::h264::AccessUnit;
    use crate::source::VideoSource;
    use crate::transport::fixtures;
    use crate::transport::fixtures::stats_video;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Fake source whose sleep state is driven from the test —
    /// exactly what `SourceDistante` exposes in view of the `Sommeil` messages pushed
    /// by the sensor, without depending on the sensor.
    struct SourceEndormable {
        inner: crate::source::FileSource,
        endormie: Arc<AtomicBool>,
    }

    impl VideoSource for SourceEndormable {
        fn next_frame(&mut self) -> Option<AccessUnit> {
            self.inner.next_frame()
        }
        fn dimensions(&self) -> (u32, u32) {
            self.inner.dimensions()
        }
        fn est_endormie(&self) -> bool {
            self.endormie.load(Ordering::SeqCst)
        }
    }

    /// The underlying defect found by the final branch review (I1), and the
    /// only test that sees it: **the ceiling comes back up on wake-up, not the bitrate.**
    ///
    /// The chain, entirely in the code: `PART_DORMANTE_BPS` is 256 kb/s,
    /// `changer_plafond` does `video_bitrate_bps.min(plafond)` as soon as an
    /// estimate has existed, and nothing undoes that `min` — the only repair
    /// would be `Controleur::observer`, called from the `MediaEgressStats` arm
    /// that str0m does not emit for a stream that has sent nothing. A sleeping window
    /// sends nothing. It is not an edge case: it is the state of EVERY
    /// wake-up.
    ///
    /// The regime matters: a REAL estimate is needed before the sleeping
    /// share, otherwise `changer_plafond` makes the bitrate follow the ceiling in
    /// both directions ("never any estimate" regime) and the defect does not
    /// show — the test would be green by construction. Same precaution
    /// as `une_part_qui_remonte_ne_releve_pas_le_debit_au_dela_de_l_estimation`.
    #[test]
    fn une_part_dormante_ne_borne_pas_le_controleur_et_le_reveil_est_suivi() {
        let mid = Mid::from("0");
        let endormie = Arc::new(AtomicBool::new(false));
        let source = Box::new(SourceEndormable {
            inner: fixtures::video_test_source(),
            endormie: endormie.clone(),
        });
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        session.video_mid = Some(mid);

        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(8_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        session.handle_event(
            Event::MediaEgressStats(stats_video(mid)),
            &mut |_| {},
            &mut |_| {},
        );
        assert_eq!(
            session.congestion.current().adaptation,
            congestion::Adaptation::Active,
            "précondition : une estimation réelle a bien été observée, donc `changer_plafond` BORNE"
        );
        let part_eveillee = 1_333_333; // 12 Mb/s shared between eight windows.
        assert!(
            session.congestion.current().video_bitrate_bps > part_eveillee,
            "précondition : l'estimation laisse de la place au-dessus de la part d'éveillée, \
             sans quoi l'assertion finale ne prouverait rien"
        );

        // The observation above itself set a pending decision:
        // emptying it here is what makes the next assertion readable — without
        // that, it would find an inherited `Some` and not the one we are looking for.
        session.pending_decision = None;

        // The window falls asleep: the sensor pushes the floor, far below
        // the ladder's lowest rung.
        endormie.store(true, Ordering::SeqCst);
        session.appliquer_part(PART_DORMANTE_BPS);
        assert!(
            session.pending_decision.is_none(),
            "une part d'endormie ne pose aucune décision : il n'y a plus d'encodeur à régler"
        );

        // It wakes up, and receives its awake share.
        endormie.store(false, Ordering::SeqCst);
        session.appliquer_part(part_eveillee);

        assert_eq!(
            session.congestion.current().video_bitrate_bps,
            part_eveillee,
            "le débit doit suivre la part de l'ÉVEILLÉE : avant le remède, le `min` du plancher \
             dormant le figeait à {PART_DORMANTE_BPS} bps pour toute la vie de la session"
        );
    }

    #[test]
    fn une_part_recue_borne_le_plafond_du_controleur() {
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        // Before the share, the ceiling is `Session::new`'s.
        assert_eq!(session.decision_courante().video_bitrate_bps, 12_000_000);

        session.appliquer_part(3_000_000);

        // `assert_eq!`, not `<=`: in the "never any estimate" regime
        // (see the next test), `changer_plafond` makes the bitrate follow
        // EXACTLY the ceiling — a loose bound (`<=`) would let through
        // 0 or 1 just as well as a real bound.
        assert_eq!(
            session.congestion.current().video_bitrate_bps,
            3_000_000,
            "la décision du contrôleur doit être bornée par la part"
        );
        assert!(
            session.pending_decision.is_some(),
            "la part doit poser une décision que la branche a0ter appliquera"
        );
    }

    /// A share going back up must not exceed what the link carries:
    /// it lifts a bound, it does not create a new upward one.
    ///
    /// ⚠️ **Deviation from the brief, reported**: its version of this test called
    /// `appliquer_part` without any estimate ever having been observed.
    /// Yet `Session::new` never makes an observation itself
    /// (`premiere_estimation_a` stays `None`), and `changer_plafond` documents
    /// precisely that in this exact regime ("never any estimate") the
    /// bitrate is a pure fallback value that FOLLOWS the ceiling, upwards
    /// as well as downwards (see `congestion/reconfiguration.rs`, regime 1, and
    /// its test `a_rising_ceiling_is_followed_while_no_estimate_has_ever_arrived`).
    /// As is, the call `appliquer_part(50_000_000)` does go back up to
    /// 50,000,000 — it is not an implementation bug, it is the test that
    /// did not exercise the regime it claims to cover. Fixed by injecting
    /// a real observation before the decrease, as
    /// `une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce`
    /// (`transport/adaptation.rs`) already does.
    #[test]
    fn une_part_qui_remonte_ne_releve_pas_le_debit_au_dela_de_l_estimation() {
        let mid = Mid::from("0");
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        session.video_mid = Some(mid);
        // A real observation: without it, `premiere_estimation_a` stays
        // `None` and `changer_plafond` makes the bitrate follow the ceiling in
        // both directions ("never any estimate" regime), which makes
        // the assertion below true by construction rather than by the
        // bound it claims to check.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(8_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        session.handle_event(
            Event::MediaEgressStats(stats_video(mid)),
            &mut |_| {},
            &mut |_| {},
        );
        assert_eq!(
            session.congestion.current().adaptation,
            congestion::Adaptation::Active,
            "précondition : une estimation réelle a bien été observée"
        );

        session.appliquer_part(2_000_000);
        let apres_baisse = session.congestion.current().video_bitrate_bps;
        assert!(
            apres_baisse <= 2_000_000,
            "précondition : la baisse a bien été appliquée"
        );

        session.appliquer_part(50_000_000);

        assert!(
            session.congestion.current().video_bitrate_bps <= apres_baisse,
            "sans observation neuve, une part plus large ne remonte pas le débit d'elle-même"
        );
    }
}
