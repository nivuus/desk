//! L'application, côté enfant, d'une part de budget de session accordée par
//! le capteur (sous-bloc D6) : voir `capteur/repartiteur.rs` pour la règle qui
//! calcule cette part, et `capteur/distante.rs` pour son transport jusqu'ici.
//!
//! Extrait d'`adaptation`, qui a franchi 500 lignes en accueillant ce câblage
//! (tâche 8) : les deux fichiers reconfigurent l'encodage, mais pour des
//! raisons différentes — `adaptation` réagit à ce que le RÉSEAU observe,
//! celui-ci à ce que le CAPTEUR arbitre entre plusieurs fenêtres.
//!
//! **Contrairement à la branche a0ter, celle-ci MUTE bien un champ interne de
//! `Rtc`** : `rtc.bwe().set_desired_bitrate` écrit l'objectif de sondage du
//! sous-système BWE et, si une estimation existe déjà, reconfigure le pacer
//! de str0m (`configure_pacer`, appelé en interne). Elle ne met en revanche
//! AUCUN paquet en file d'attente : l'effet différé qu'elle programme côté
//! contrôleur de sondage (`ProbeControl` de str0m — qui peut avancer
//! l'échéance de la prochaine sonde et faire émettre du bourrage) n'est
//! évalué qu'au PROCHAIN traitement de `Input::Timeout`, jamais pendant cet
//! appel. C'est cette absence de mise en file — et non une absence de
//! mutation de `Rtc`, qui serait fausse — qui préserve l'invariant de
//! drainage documenté en tête de `tick.rs`.

use str0m::bwe::Bitrate;

use super::Session;

impl Session {
    /// Applique une part du budget de session accordée par le capteur
    /// (sous-bloc D6).
    ///
    /// **Deux applications, et la seconde n'est pas la moins importante.**
    /// `changer_plafond` borne ce que le contrôleur décidera d'encoder ;
    /// `set_desired_bitrate` borne ce que le sous-système BWE **sonde**. Sans
    /// la seconde, N fenêtres continueraient de viser chacune le lien entier
    /// en injectant du trafic de sondage — la cause même du défaut que D6
    /// corrige — même si aucune n'encodait au-delà de sa part.
    pub(super) fn appliquer_part(&mut self, bps: u32) {
        let decision = self.congestion.changer_plafond(bps);
        self.rtc.bwe().set_desired_bitrate(Bitrate::bps(bps as u64));
        // `session` : sans ce champ la trace n'est PAS attribuable. Tous les
        // enfants héritent le même `agent.log` (stdout partagé depuis D4), et
        // la somme des parts accordées — le critère ③ de la recette — se
        // calculerait alors sur un multiensemble de nombres anonymes. Même
        // motif et même champ que `cadence de la piste vidéo (côté enfant)`.
        tracing::info!(session = %self.session_id, part_bps = bps, "part de budget appliquee");
        self.pending_decision = Some(decision);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use str0m::bwe::BweKind;
    use str0m::media::Mid;
    use str0m::Event;

    use super::*;
    use crate::congestion;
    use crate::transport::fixtures;
    use crate::transport::fixtures::stats_video;

    #[test]
    fn une_part_recue_borne_le_plafond_du_controleur() {
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        // Avant la part, le plafond est celui de `Session::new`.
        assert_eq!(session.decision_courante().video_bitrate_bps, 12_000_000);

        session.appliquer_part(3_000_000);

        // `assert_eq!`, pas `<=` : dans le régime « jamais aucune estimation »
        // (voir le test suivant), `changer_plafond` fait suivre le débit
        // EXACTEMENT au plafond — une borne large (`<=`) laisserait passer
        // 0 ou 1 tout aussi bien qu'une vraie borne.
        assert_eq!(
            session.congestion.courant().video_bitrate_bps, 3_000_000,
            "la décision du contrôleur doit être bornée par la part"
        );
        assert!(
            session.pending_decision.is_some(),
            "la part doit poser une décision que la branche a0ter appliquera"
        );
    }

    /// Une part qui remonte ne doit pas faire dépasser ce que le lien porte :
    /// elle lève une borne, elle n'en crée pas une nouvelle vers le haut.
    ///
    /// ⚠️ **Écart signalé au brief** : sa version de ce test appelait
    /// `appliquer_part` sans qu'aucune estimation n'ait jamais été observée.
    /// Or `Session::new` ne fait jamais elle-même d'observation
    /// (`premiere_estimation_a` reste `None`), et `changer_plafond` documente
    /// justement que dans ce régime précis (« jamais aucune estimation ») le
    /// débit est une pure valeur de repli qui SUIT le plafond, à la hausse
    /// comme à la baisse (voir `congestion/reconfiguration.rs`, régime 1, et
    /// son test `un_plafond_qui_monte_est_suivi_tant_qu_aucune_estimation_n_est_jamais_arrivee`).
    /// Tel quel, l'appel `appliquer_part(50_000_000)` remonte bien à
    /// 50 000 000 — ce n'est pas un bug de l'implémentation, c'est le test qui
    /// n'exerçait pas le régime qu'il prétend couvrir. Corrigé en injectant
    /// une véritable observation avant la baisse, comme le fait déjà
    /// `une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce`
    /// (`transport/adaptation.rs`).
    #[test]
    fn une_part_qui_remonte_ne_releve_pas_le_debit_au_dela_de_l_estimation() {
        let mid = Mid::from("0");
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        session.video_mid = Some(mid);
        // Une observation réelle : sans elle, `premiere_estimation_a` reste
        // `None` et `changer_plafond` fait suivre le débit au plafond dans
        // les deux sens (régime « jamais aucune estimation »), ce qui rend
        // l'assertion ci-dessous vraie par construction plutôt que par la
        // borne qu'elle prétend vérifier.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(8_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        session.handle_event(Event::MediaEgressStats(stats_video(mid)), &mut |_| {}, &mut |_| {});
        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Active,
            "précondition : une estimation réelle a bien été observée"
        );

        session.appliquer_part(2_000_000);
        let apres_baisse = session.congestion.courant().video_bitrate_bps;
        assert!(apres_baisse <= 2_000_000, "précondition : la baisse a bien été appliquée");

        session.appliquer_part(50_000_000);

        assert!(
            session.congestion.courant().video_bitrate_bps <= apres_baisse,
            "sans observation neuve, une part plus large ne remonte pas le débit d'elle-même"
        );
    }
}
