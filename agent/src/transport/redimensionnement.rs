//! Le redimensionnement de la fenêtre capturée, demandé par le navigateur.
//!
//! Séparé d'`adaptation` bien que les deux reconfigurent l'encodage : là-bas
//! c'est le RÉSEAU qui commande et seul le flux transporté maigrit ; ici
//! c'est l'UTILISATEUR, et c'est la vraie fenêtre Windows qui change de
//! taille. Les deux se rejoignent en un point, décrit plus bas : un
//! redimensionnement oblige à recalibrer le contrôleur de congestion, dont
//! les seuils sont dérivés de la taille de capture.
//!
//! Comme la branche a0ter, la branche a1 ne mute JAMAIS `Rtc` — elle ne
//! touche que la source vidéo et la file de contrôle.

use std::time::Instant;

use proto::control::AgentControl;

use super::Session;

impl Session {
    /// Branche `a1` de la liste de priorités (voir `tick`) : applique le
    /// dernier redimensionnement demandé par le navigateur.
    ///
    /// Ne rend rien : la branche conclut toujours le tour, réussite ou échec
    /// du redimensionnement, et c'est `tick` qui le dit.
    pub(super) fn appliquer_redimensionnement(&mut self, width: u32, height: u32) {
        match self.source.resize(width, height) {
            Ok(()) => {
                // La fenêtre peut refuser la taille demandée (bornes
                // minimales, alignement pair...) : le navigateur doit
                // connaître les dimensions RÉELLEMENT obtenues, pas
                // celles demandées.
                let (actual_width, actual_height) = self.source.dimensions();
                self.dimensions = (actual_width, actual_height);

                // C1 (revue finale de branche). `WindowsSource::resize`
                // reconstruit désormais TOUJOURS l'encodeur à la taille
                // pleine de la nouvelle capture (voir son commentaire) :
                // la taille réellement appliquée vient donc de changer
                // par ce seul fait, sans être jamais passée par
                // `set_encode_size`. On l'enregistre directement — il n'y
                // a rien à « appliquer » ici, c'est déjà fait — plutôt
                // que de la laisser transiter par `pending_decision`
                // comme le ferait une décision normale du contrôleur.
                self.encode_size_appliquee = (actual_width, actual_height);
                // Une cible refusée avant ce redimensionnement n'a plus
                // cours : la taille encodée vient de changer sous elle.
                self.taille_refus_signalee = None;

                // Le contrôleur doit être reconstruit pour la nouvelle
                // taille de source : ses seuils (`min_bps` par barreau)
                // sont dérivés de la taille de capture, qui vient de
                // changer. Sans cela, l'échelle resterait calibrée pour
                // une source qui n'existe plus — et pourrait viser une
                // taille d'encodage supérieure à la nouvelle capture.
                // `changer_source` conserve le barreau (le NIVEAU de
                // réduction), pas la taille absolue ; la décision qui en
                // résulte est mémorisée pour que la branche a0ter,
                // au tour SUIVANT, la compare à `encode_size_appliquee`
                // (celle ci-dessus, la taille pleine) et rappelle
                // `set_encode_size` si le barreau conservé exige encore
                // une réduction.
                let decision = self
                    .congestion
                    .changer_source((actual_width, actual_height), Instant::now());
                self.pending_decision = Some(decision);

                let mic = self.micro_disponible();
                self.queue_control(AgentControl::ready(actual_width, actual_height, mic));
            }
            Err(e) => {
                // Un échec de redimensionnement ne doit pas terminer la
                // session : on journalise et la session continue avec
                // les dimensions précédentes.
                tracing::warn!(
                    erreur = %e,
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

    /// Réserve consignée par `CLAUDE.md` depuis le chantier C : « le câblage
    /// de `resize` côté `transport.rs` (branche a1, qui appelle
    /// `Controleur::changer_source`) n'a aucun verrou automatisé :
    /// `VideoSource::resize` est un no-op par défaut dans toutes les sources
    /// factices, et rien ne positionne `pending_resize` dans les tests ». Ce
    /// test ferme les deux moitiés de cette réserve.
    #[test]
    fn un_redimensionnement_recalibre_le_controleur_sur_la_taille_obtenue() {
        /// Source dont `resize` réussit mais impose un alignement pair, comme
        /// le fait une vraie fenêtre Windows : c'est ce qui rend observable
        /// la distinction entre taille DEMANDÉE et taille OBTENUE, sur
        /// laquelle repose toute la branche.
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

        assert_eq!(session.encode_size_appliquee, (1280, 720), "précondition du test");
        assert_eq!(
            session.congestion.courant().encode_size,
            (1280, 720),
            "précondition : l'échelle est calibrée sur la source d'origine"
        );
        // Trace d'un refus antérieur, qui n'a plus cours dès que la taille
        // encodée change sous elle.
        session.taille_refus_signalee = Some((960, 540));

        // Le navigateur demande une taille impaire ; la fenêtre en rendra une
        // paire. `pending_resize` est ce que `dispatch_channel_data` pose à
        // la réception d'un `ClientControl::Resize`.
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
            session.taille_refus_signalee, None,
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
                AgentControl::Ready { width: 640, height: 480, .. }
            )),
            "le navigateur doit être informé des dimensions réellement obtenues : file = {:?}",
            session.pending_control
        );
    }
}
