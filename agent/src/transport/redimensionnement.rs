//! Resizing the captured window, requested by the browser.
//!
//! Separate from `adaptation` although both reconfigure encoding: there
//! it is the NETWORK that commands and only the transported stream slims down; here
//! it is the USER, and it is the real Windows window that changes
//! size. The two meet at one point, described below: a
//! resize forces recalibrating the congestion controller, whose
//! thresholds are derived from the capture size.
//!
//! Like branch a0ter, branch a1 NEVER mutates `Rtc` — it only
//! touches the video source and the control queue.

use std::time::Instant;

use proto::control::AgentControl;

use super::Session;

impl Session {
    /// Branch `a1` of the priority list (see `tick`): applies the
    /// last resize requested by the browser.
    ///
    /// Returns nothing: the branch always concludes the round, resize success or
    /// failure, and it is `tick` that says so.
    pub(super) fn appliquer_redimensionnement(&mut self, width: u32, height: u32) {
        match self.source.resize(width, height) {
            Ok(()) => {
                // The window can refuse the requested size (minimum
                // bounds, even alignment...): the browser must
                // know the dimensions ACTUALLY obtained, not
                // the requested ones.
                let (actual_width, actual_height) = self.source.dimensions();
                self.dimensions = (actual_width, actual_height);

                // C1 (final branch review). `WindowsSource::resize`
                // now ALWAYS rebuilds the encoder at the full size
                // of the new capture (see its comment): the size
                // actually applied has therefore just changed
                // by that fact alone, without ever going through
                // `set_encode_size`. We record it directly — there
                // is nothing to "apply" here, it is already done — rather
                // than letting it transit through `pending_decision`
                // as a normal controller decision would.
                self.encode_size_appliquee = (actual_width, actual_height);
                // A target refused before this resize no longer
                // applies: the encoded size has just changed under it.
                self.reported_refused_size = None;

                // The controller must be rebuilt for the new
                // source size: its thresholds (`min_bps` per rung)
                // are derived from the capture size, which has just
                // changed. Without this, the ladder would stay calibrated for
                // a source that no longer exists — and could aim at an
                // encoding size larger than the new capture.
                // `changer_source` keeps the rung (the reduction LEVEL),
                // not the absolute size; the decision that
                // results is stored so that branch a0ter,
                // at the NEXT round, compares it to `encode_size_appliquee`
                // (the one above, the full size) and calls
                // `set_encode_size` again if the kept rung still requires
                // a reduction.
                let decision = self
                    .congestion
                    .changer_source((actual_width, actual_height), Instant::now());
                self.pending_decision = Some(decision);

                let mic = self.micro_disponible();
                self.queue_control(AgentControl::ready(actual_width, actual_height, mic));
            }
            Err(e) => {
                // A resize failure must not end the
                // session: we log and the session continues with
                // the previous dimensions.
                tracing::warn!(
                    error = %e,
                    width,
                    height,
                    "échec du redimensionnement, ignoré"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::h264::AccessUnit;
    use crate::source::VideoSource;
    use crate::transport::fixtures;

    /// Reservation recorded by `CLAUDE.md` since workstream C: "the wiring
    /// of `resize` on the `transport.rs` side (branch a1, which calls
    /// `Controleur::changer_source`) has no automated lock:
    /// `VideoSource::resize` is a no-op by default in all fake
    /// sources, and nothing sets `pending_resize` in the tests". This
    /// test closes both halves of that reservation.
    #[test]
    fn a_resize_recalibrates_the_controller_on_the_obtained_size() {
        /// A source whose `resize` succeeds but imposes even alignment, as
        /// a real Windows window does: it is what makes observable
        /// the distinction between REQUESTED size and OBTAINED size, on
        /// which the whole branch rests.
        struct SourceRedimensionnable {
            inner: crate::source::FileSource,
            dimensions: (u32, u32),
        }

        impl VideoSource for SourceRedimensionnable {
            fn next_frame(&mut self) -> Option<AccessUnit> {
                self.inner.next_frame()
            }
            fn dimensions(&self) -> (u32, u32) {
                self.dimensions
            }
            fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
                self.dimensions = (width & !1, height & !1);
                Ok(())
            }
        }

        let inner = fixtures::video_test_source();
        let dimensions = inner.dimensions();
        let source = Box::new(SourceRedimensionnable { inner, dimensions });
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");

        assert_eq!(
            session.encode_size_appliquee,
            (1280, 720),
            "précondition du test"
        );
        assert_eq!(
            session.congestion.current().encode_size,
            (1280, 720),
            "précondition : l'échelle est calibrée sur la source d'origine"
        );
        // Trace of an earlier refusal, which no longer applies as soon as the encoded
        // size changes under it.
        session.reported_refused_size = Some((960, 540));

        // The browser requests an odd size; the window will return an even
        // one. `pending_resize` is what `dispatch_channel_data` sets on
        // receiving a `ClientControl::Resize`.
        session.pending_resize = Some((641, 481));
        session
            .act_on_timeout(Instant::now())
            .expect("un redimensionnement ne doit jamais faire échouer la session");

        assert_eq!(
            session.dimensions,
            (640, 480),
            "la session doit retenir les dimensions RÉELLEMENT obtenues, pas celles demandées"
        );
        assert_eq!(
            session.encode_size_appliquee,
            (640, 480),
            "C1 : `resize` reconstruit l'encodeur à la taille pleine de la nouvelle capture, \
             sans passer par `set_encode_size` — la taille appliquée doit être enregistrée ici"
        );
        assert_eq!(
            session.reported_refused_size, None,
            "une cible refusée avant ce redimensionnement n'a plus cours"
        );

        let decision = session.pending_decision.expect(
            "`changer_source` doit avoir produit une décision, mémorisée pour que a0ter la \
             confronte à `encode_size_appliquee` au tour suivant",
        );
        assert!(
            decision.encode_size.0 <= 640 && decision.encode_size.1 <= 480,
            "le contrôleur doit être recalibré sur la NOUVELLE source : sans `changer_source`, \
             l'échelle viserait encore une taille d'encodage plus grande que la capture ({:?})",
            decision.encode_size
        );

        assert!(
            session.pending_control.iter().any(|message| matches!(
                message,
                AgentControl::Ready {
                    width: 640,
                    height: 480,
                    ..
                }
            )),
            "le navigateur doit être informé des dimensions réellement obtenues : file = {:?}",
            session.pending_control
        );
    }
}
