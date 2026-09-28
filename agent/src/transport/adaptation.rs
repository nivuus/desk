//! Network feedback control seen from the loop: staleness of the
//! bandwidth estimate, and application of a decision of the
//! congestion controller.
//!
//! The matching branch of the priority list (a0ter, see `tick`) NEVER
//! mutates `Rtc`: it only touches the video source, the audio encoder
//! and the control queue. It still gives control back at once, like the
//! branches that really do mutate `Rtc` — keeping a single action per
//! round is what makes the list readable.
//!
//! The resize requested by the user, which also reconfigures
//! the encoding but for an entirely different reason, lives in
//! `redimensionnement`. Applying a budget share granted by the
//! sensor (sub-block D6), extracted from this file to stay under the
//! 500-line ceiling, lives in `part`.

use std::time::{Duration, Instant};

use proto::control::AgentControl;

use super::Session;
use crate::congestion;

/// Starting bandwidth estimate, before any feedback from the peer.
///
/// Compromise measured in task 12: too low, a LAN startup takes
/// time to reach the ceiling and acceptance loses frames per second;
/// too high, the first instant of a session on a narrow link saturates before
/// the first correction. 2.5 Mb/s is the starting point, to be confirmed.
pub(super) const ESTIMATION_INITIALE_BPS: u32 = 2_500_000;

/// Duration beyond which a bandwidth estimate that is not renewed
/// is treated as absent (I4, final branch review).
///
/// `Event::EgressBitrateEstimate` and `Event::MediaEgressStats` do not arrive
/// together (see the comment of the `derniere_estimation_bps` field): without
/// this bound, an estimate received once then never again (TWCC
/// drying up while the session survives) would stay in use indefinitely by
/// the controller — potentially the last high value before the incident,
/// which would announce "Good" on a dead link. `MediaEgressStats` arrives
/// about once per second (`set_stats_interval`): 5 s leaves several
/// missed opportunities before concluding absence, without letting a
/// frozen estimate live for tens of seconds.
const EXPIRATION_ESTIMATION: Duration = Duration::from_secs(5);

impl Session {
    /// Adaptation decision currently retained. Feeds the link state
    /// message sent to the browser (task 10).
    ///
    /// `encode_size` and `video_bitrate_bps` come from what the transport
    /// ACTUALLY managed to apply (`encode_size_appliquee`,
    /// `bitrate_applique`), not from what the controller decided: the latter
    /// stays optimistic by construction (see `congestion::Controleur`), and
    /// only the transport knows whether the encoder accepted the last setting.
    /// Announcing to the browser a size or bitrate the track does not emit
    /// would be a lie of the same family as the one already fixed on
    /// `qualite` in task 5. The other fields (`qualite`, `adaptation`,
    /// `opus_loss_perc`) stay the controller's: no equivalent refusal
    /// mechanism exists for them here.
    pub fn decision_courante(&self) -> congestion::Decision {
        congestion::Decision {
            encode_size: self.encode_size_appliquee,
            video_bitrate_bps: self.bitrate_applique,
            ..self.congestion.courant()
        }
    }

    /// Last bandwidth estimate, if it is still fresh at
    /// `now` (see `EXPIRATION_ESTIMATION`). `None` means "no
    /// usable estimate", whether none ever arrived or the
    /// last one has aged — both cases lead the controller to
    /// `Adaptation::Indisponible`.
    ///
    /// Extracted from `handle_event` to be checkable directly:
    /// the `MediaEgressStats` event that consumes it requires a negotiated
    /// session, staleness does not.
    pub(super) fn estimation_fraiche(&self, now: Instant) -> Option<u32> {
        self.derniere_estimation_bps.and_then(|(bps, at)| {
            (now.saturating_duration_since(at) <= EXPIRATION_ESTIMATION).then_some(bps)
        })
    }

    /// Branch `a0ter` of the priority list (see `tick`): applies a
    /// decision of the congestion controller to the video encoder and to
    /// the audio encoder, then announces to the browser what was ACTUALLY
    /// applied.
    ///
    /// Returns nothing: the branch always concludes the round, and it is `tick`
    /// that says so.
    pub(super) fn appliquer_decision(&mut self, decision: congestion::Decision) {
        match self.source.set_bitrate(decision.video_bitrate_bps) {
            Ok(()) => self.bitrate_applique = decision.video_bitrate_bps,
            Err(e) => {
                // The encoder refuses the hot bitrate change: we keep the current
                // bitrate and keep adapting through resolution. A
                // single line, not one per second.
                if !self.refus_debit_signale {
                    self.refus_debit_signale = true;
                    tracing::warn!(erreur = %e, "l'encodeur refuse le réglage du débit à chaud");
                }
            }
        }
        // **`taille_refus_signalee` is a GUARD, not a mere log
        // witness** (I1, final branch review of sub-block D4).
        //
        // After a refusal, `encode_size_appliquee` does not advance: the condition
        // `decision.encode_size != encode_size_appliquee` therefore stays true and
        // the SAME target would be resubmitted to the source at each decision of the
        // controller — one per second, potentially for hours under
        // sustained congestion. Yet `WindowsSource::set_encode_size` builds
        // a NEW `H264Encoder` before releasing the old one: at eight windows
        // that is building a ninth encoder, refused by
        // construction (ceiling of 8, measured 18/18 during sub-block
        // D4's acceptance), hence up to eight instantiations per second of the hardware
        // MFT in the process holding the eight duplications. The
        // `warn!` was indeed deduplicated; the WORK was not.
        //
        // The two paths that clear this memory — a successful `set_encode_size`
        // below, and a window resize
        // (`redimensionnement.rs`) — are what make a target
        // submittable again: in both cases the encoded size changed under
        // it, and the previous refusal no longer prejudges anything.
        let deja_refusee = self.taille_refus_signalee == Some(decision.encode_size);
        if decision.encode_size != self.encode_size_appliquee && !deja_refusee {
            match self
                .source
                .set_encode_size(decision.encode_size.0, decision.encode_size.1)
            {
                Ok(()) => {
                    // `session`: without this field the trace is NOT
                    // attributable. All children share the same
                    // `agent.log` since D4, and this line was therefore
                    // anonymous — D6's acceptance run (task 10) could not DATE
                    // a named window's ladder promotion, and had to
                    // fall back on sampling `getStats()` on the
                    // browser side, which made it wrongly blame the product
                    // for a delay that was only an observation window that was too
                    // short. Same field and same reason as the
                    // budget-share-applied and video-track-cadence
                    // (child side) log lines.
                    tracing::info!(
                        session = %self.session_id,
                        largeur = decision.encode_size.0,
                        hauteur = decision.encode_size.1,
                        "taille d'encodage changée"
                    );
                    self.encode_size_appliquee = decision.encode_size;
                    // A later refusal of this same size (or of
                    // another) will become new information again.
                    self.taille_refus_signalee = None;
                }
                Err(e) => {
                    // We stay at the current rung. The session lives. The guard
                    // above ensures this point is only reached for a
                    // target that has NOT ALREADY been refused: a single log
                    // line per target, and a single attempt per target.
                    self.taille_refus_signalee = Some(decision.encode_size);
                    tracing::warn!(
                        erreur = %e,
                        largeur = decision.encode_size.0,
                        hauteur = decision.encode_size.1,
                        "changement de taille d'encodage refusé, barreau conservé"
                    );
                }
            }
        }
        if let Some(audio) = self.audio_source.as_mut() {
            if let Err(e) = audio.set_packet_loss_perc(decision.opus_loss_perc) {
                tracing::warn!(erreur = %e, "réglage du taux de perte Opus refusé");
            }
        }
        // We announce `decision_courante()`, not `decision`: `bitrate` and
        // `encode_size` must reflect what the encoder ACTUALLY
        // accepted above (`self.bitrate_applique`,
        // `self.encode_size_appliquee`), not the target aimed at by the
        // controller — an encoder refusal would otherwise pass to the
        // browser exactly the lie `decision_courante()`
        // exists to avoid (see its documentation and the test
        // `un_refus_repete_de_set_encode_size_ne_remonte_pas_dans_decision_courante`).
        let etat_lien = self.decision_courante();
        self.queue_control(AgentControl::link(
            etat_lien.video_bitrate_bps,
            etat_lien.encode_size,
            match etat_lien.qualite {
                congestion::Qualite::Bonne => proto::control::LinkQuality::Bonne,
                congestion::Qualite::Degradee => proto::control::LinkQuality::Degradee,
                congestion::Qualite::Insuffisante => proto::control::LinkQuality::Insuffisante,
            },
            match etat_lien.adaptation {
                congestion::Adaptation::Active => proto::control::LinkAdaptation::Active,
                congestion::Adaptation::Indisponible => {
                    proto::control::LinkAdaptation::Indisponible
                }
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use anyhow::anyhow;
    use str0m::bwe::{Bitrate, BweKind};
    use str0m::media::Mid;
    use str0m::Event;

    use super::*;
    use crate::h264::AccessUnit;
    use crate::source::VideoSource;
    use crate::transport::fixtures;
    use crate::transport::fixtures::stats_video;

    /// Reservation recorded by `CLAUDE.md` since workstream C: "the
    /// `Adaptation` transitions (`Active` → `Indisponible`) and the expiry
    /// of the BWE estimate at 5 s have no test: checked by reading
    /// code". This test closes them.
    ///
    /// It drives `handle_event` with synthetic str0m events (their
    /// types are entirely public) rather than with a real peer in local
    /// loopback: what is at stake is a sequence of internal states over
    /// a time scale of several seconds, not network routing
    /// — and no peer knows how to make TWCC emission *stop* on demand.
    #[test]
    fn une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce() {
        let mid = Mid::from("0");
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        // `MediaEgressStats` of another track is ignored: without `video_mid`,
        // no observation would reach the controller. The negotiation
        // itself is proven elsewhere (`piste_video`, `evenements`).
        session.video_mid = Some(mid);

        // 1) Fresh estimate, then statistics: adaptation is active
        //    and nothing is reported.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(4_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        assert!(
            session.estimation_fraiche(Instant::now()).is_some(),
            "une estimation qui vient d'arriver est fraîche"
        );
        session.handle_event(
            Event::MediaEgressStats(stats_video(mid)),
            &mut |_| {},
            &mut |_| {},
        );
        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Active
        );
        assert!(!session.absence_bwe_signalee);
        assert!(!session.indisponibilite_annoncee);

        // 2) The same estimate, aged beyond `EXPIRATION_ESTIMATION`:
        //    TWCC dried up while the session survives. It must be
        //    treated as ABSENT, and not serve again indefinitely — it is
        //    exactly the last high value before the incident that
        //    would announce "Good" on a dead link.
        let (bps, _) = session.derniere_estimation_bps.expect("posée en 1");
        let perimee_a = Instant::now() - EXPIRATION_ESTIMATION - Duration::from_millis(1);
        session.derniere_estimation_bps = Some((bps, perimee_a));
        assert_eq!(
            session.estimation_fraiche(Instant::now()),
            None,
            "au-delà d'EXPIRATION_ESTIMATION, une estimation ne doit plus être utilisable"
        );

        session.pending_decision = None;
        session.handle_event(
            Event::MediaEgressStats(stats_video(mid)),
            &mut |_| {},
            &mut |_| {},
        );

        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Indisponible,
            "l'expiration de l'estimation doit faire basculer l'adaptation, pas la laisser à Active"
        );
        assert!(
            session.absence_bwe_signalee,
            "l'absence doit être journalisée une fois"
        );
        assert!(session.indisponibilite_annoncee);
        let decision = session.pending_decision.expect(
            "l'indisponibilité doit être RELAYÉE au navigateur (I2), pas seulement journalisée : \
             `Controleur::observer` ne produit aucune décision sans estimation",
        );
        assert_eq!(decision.adaptation, congestion::Adaptation::Indisponible);

        // 3) Branch a0ter turns that stored decision into a
        //    `Link` message for the browser.
        session
            .act_on_timeout(Instant::now())
            .expect("appliquer une décision ne doit jamais faire échouer la session");
        assert!(
            session.pending_control.iter().any(|message| matches!(
                message,
                AgentControl::Link {
                    adaptation: proto::control::LinkAdaptation::Indisponible,
                    ..
                }
            )),
            "le navigateur doit recevoir Indisponible — surtout pas un silence qui ressemble à \
             « tout va bien » : file = {:?}",
            session.pending_control
        );

        // 4) A fresh estimate coming back re-arms the announcement: a
        //    later unavailability is new information.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(4_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        session.handle_event(
            Event::MediaEgressStats(stats_video(mid)),
            &mut |_| {},
            &mut |_| {},
        );
        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Active
        );
        assert!(
            !session.indisponibilite_annoncee,
            "une coupure ultérieure de TWCC doit être annoncée de nouveau"
        );
    }

    /// Encoding sizes actually SUBMITTED to the source, in order.
    type TaillesSoumises = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>;

    /// A session whose source refuses ANY encoding size, and records
    /// those that were submitted to it.
    ///
    /// Recording the submissions and not only their number: it is what
    /// lets us distinguish "a refused target is no longer resubmitted" from "a
    /// different target is always tried".
    fn session_refusant_les_tailles() -> (Session, TaillesSoumises) {
        struct SourceRefusant {
            inner: crate::source::FileSource,
            soumises: TaillesSoumises,
        }

        impl VideoSource for SourceRefusant {
            fn next_frame(&mut self) -> Option<AccessUnit> {
                self.inner.next_frame()
            }
            fn dimensions(&self) -> (u32, u32) {
                self.inner.dimensions()
            }
            fn set_encode_size(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
                self.soumises.lock().unwrap().push((width, height));
                Err(anyhow!("pilote imaginaire : refuse toujours"))
            }
        }

        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let source_path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let soumises: TaillesSoumises = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let source = Box::new(SourceRefusant {
            inner: crate::source::FileSource::from_path(source_path, 1280, 720, 60)
                .expect("chargement du flux de test"),
            soumises: soumises.clone(),
        });
        let session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");
        (session, soumises)
    }

    /// Applies `decision` through branch `a0ter`, as a round of
    /// the transport loop would.
    fn appliquer(session: &mut Session, decision: congestion::Decision) {
        session.pending_decision = Some(decision);
        session
            .act_on_timeout(Instant::now())
            .expect("un refus de l'encodeur ne doit jamais faire échouer la session");
    }

    /// A congestion decision targeting `encode_size`, the rest having no
    /// effect on what these tests observe.
    fn decision_vers(encode_size: (u32, u32)) -> congestion::Decision {
        congestion::Decision {
            video_bitrate_bps: 5_000_000,
            encode_size,
            opus_loss_perc: 0,
            qualite: congestion::Qualite::Degradee,
            adaptation: congestion::Adaptation::Active,
        }
    }

    /// **I1 of the final branch review of sub-block D4.** An already
    /// refused target must no longer be RESUBMITTED to the source — not only no longer
    /// be logged again.
    ///
    /// What this test protects is not cosmetic: in shared capture,
    /// `WindowsSource::set_encode_size` builds a new `H264Encoder` before
    /// releasing the old one, and at eight windows that construction is refused
    /// by construction. Without the guard, the controller would relaunch that work at
    /// each decision — one per second and per window, indefinitely.
    #[test]
    fn une_cible_deja_refusee_n_est_plus_soumise_a_la_source() {
        let (mut session, soumises) = session_refusant_les_tailles();
        let taille_originale = session.encode_size_appliquee;
        let refusee = (640, 360);
        assert_ne!(refusee, taille_originale, "précondition du test");

        // Five identical decisions, as the controller would produce for five
        // seconds under sustained congestion.
        for _ in 0..5 {
            appliquer(&mut session, decision_vers(refusee));
        }
        assert_eq!(
            *soumises.lock().unwrap(),
            vec![refusee],
            "la cible refusée ne doit être soumise qu'UNE fois, pas à chaque décision"
        );

        // A DIFFERENT target stays new information: the guard must
        // not freeze adaptation, only remove repetition.
        let autre = (960, 540);
        appliquer(&mut session, decision_vers(autre));
        assert_eq!(
            *soumises.lock().unwrap(),
            vec![refusee, autre],
            "une cible jamais essayée doit l'être, même après un refus précédent"
        );

        // And the new refused target becomes in turn the guarded target:
        // the memory follows the last one, it does not accumulate.
        appliquer(&mut session, decision_vers(autre));
        assert_eq!(*soumises.lock().unwrap(), vec![refusee, autre]);
        assert_eq!(session.taille_refus_signalee, Some(autre));
        assert_eq!(
            session.decision_courante().encode_size,
            taille_originale,
            "aucun de ces refus ne doit se refléter dans la décision annoncée"
        );
    }

    #[test]
    fn un_refus_repete_de_set_encode_size_ne_remonte_pas_dans_decision_courante() {
        // Fix round (post-task 9 review): `Controleur::observer`
        // stays optimistic by construction — it updates `courant.encode_size`
        // whether or not the encoder accepts the change. Without the distinction
        // this test checks, `decision_courante()` would announce to the
        // browser (link state message, task 10) a size the
        // video track never emits.
        //
        // This test also covers deduplication of the log on the refusal side
        // (`taille_refus_signalee`): three identical decisions in a row,
        // as the controller would do once per second under
        // sustained congestion, must neither grow nor change this
        // memory beyond its first write — counting the log lines
        // themselves is not practicable in this harness (no
        // `tracing` capture exists in this module).
        let (mut session, _soumises) = session_refusant_les_tailles();
        let taille_originale = session.encode_size_appliquee;
        let taille_visee = (640, 360);
        assert_ne!(taille_visee, taille_originale, "précondition du test");

        // Three successive decisions, as the controller would do once
        // per second under sustained congestion: the same size
        // refused at every round.
        for _ in 0..3 {
            appliquer(&mut session, decision_vers(taille_visee));
        }

        // Finding 2: `decision_courante()` must keep reporting
        // the OLD size, the one actually emitted — not the refused one.
        assert_eq!(
            session.decision_courante().encode_size,
            taille_originale,
            "un refus de l'encodeur ne doit jamais se refléter dans la décision annoncée"
        );

        // Finding 1: the deduplication memory retains the refused
        // target, stable over the three identical rounds — it is what
        // prevents repeating the log at each decision.
        assert_eq!(
            session.taille_refus_signalee,
            Some(taille_visee),
            "la cible refusée doit être mémorisée pour éviter de rejournaliser à chaque tour"
        );
    }
}
