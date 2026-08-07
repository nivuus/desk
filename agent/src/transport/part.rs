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

use std::sync::OnceLock;

use str0m::bwe::Bitrate;

use super::Session;

/// L'objectif de sondage est-il armé ?
///
/// **Variable de BANC, pas de produit** : elle n'existe que pour l'A/B
/// différentiel de la consignation n°4 du sous-bloc D6, jamais joué à ce jour.
/// `PART_SONDAGE=0` neutralise `set_desired_bitrate` ; toute autre valeur, et
/// l'absence de variable, l'arment. Même convention que `AUDIO` et
/// `PLEIN_ECRAN` : on désarme sur `=0` ce qui est livré.
///
/// ⚠️ **Ce que l'A/B établira, et rien de plus** : que l'appel a un effet
/// observable sur le trafic émis. Il n'établira PAS qu'il est nécessaire — la
/// prémisse qui le disait « le plus important » a été réfutée par D6 elle-même,
/// le pont portant ≥ 1,44 Gb/s pour `packetsLost = 0`.
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
    /// Applique une part du budget de session accordée par le capteur
    /// (sous-bloc D6).
    ///
    /// **Deux applications, et elles n'ont pas la même portée.**
    /// `changer_plafond` borne ce que le contrôleur décidera d'encoder — c'est
    /// par lui que la part agit réellement, en faisant descendre l'échelle
    /// d'un barreau, donc la RÉSOLUTION. `set_desired_bitrate` borne ce que le
    /// sous-système BWE **sonde** : sans elle, N fenêtres viseraient chacune le
    /// lien entier en injectant du trafic de sondage, ce qui est excessif en
    /// principe même si aucune n'encodait au-delà de sa part.
    ///
    /// ⚠️ **Ne pas relire cette seconde application comme le remède au défaut
    /// que D6 corrige.** La recette de la branche a RÉFUTÉ la prémisse dont
    /// elle était tirée : le pont porte ≥ 1,44 Gb/s et `packetsLost` vaut 0 aux
    /// onze exécutions — **le lien n'a jamais été le goulot**, donc le sondage
    /// cumulé ne saturait rien et n'était lu comme de la congestion par
    /// personne. Ce qui sature est le **décodeur du navigateur**, et le seul
    /// levier mesuré efficace contre lui est le nombre de pixels
    /// (`docs/superpowers/plans/2026-08-03-multifenetres-partage-capacite-resultats.md`,
    /// §1 et §3.6). Le mécanisme reste juste, sa justification a changé.
    ///
    /// **Une part d'ENDORMIE ne va pas au contrôleur**, et c'est la seule
    /// asymétrie de cette fonction. Le plancher `PART_DORMANTE_BPS`
    /// (256 kb/s) est très en dessous du barreau le plus bas de l'échelle
    /// (691 200 bps à 1280×720/60) : le passer à `changer_plafond` poserait
    /// `video_bitrate_bps = 256_000` par son `min`, et **rien ne le
    /// remonterait au réveil** — le plafond remonterait, pas le débit, car la
    /// seule réparation est `Controleur::observer`, appelé depuis le bras
    /// `MediaEgressStats`, que str0m n'émet jamais pour un flux qui n'a rien
    /// envoyé (`send_stats.rs`, `if self.bytes == 0 { return; }`). Une endormie
    /// n'envoie rien, par définition. Et le remède serait de toute façon sans
    /// objet : une endormie a déjà relâché son encodeur (D5), lui imposer un
    /// plafond d'ENCODAGE ne borne rien qui existe. Seul le sondage, lui, a
    /// encore un sens — sa `PeerConnection` vit.
    ///
    /// L'état de sommeil est LU (`VideoSource::est_endormie`), jamais deviné :
    /// le déduire d'une comparaison de la part à `PART_DORMANTE_BPS`
    /// couplerait deux processus par une valeur — un couplage qui se romprait
    /// en silence le jour où l'un des deux changerait de constante.
    pub(super) fn appliquer_part(&mut self, bps: u32) {
        let endormie = self.source.est_endormie();
        if !endormie {
            self.pending_decision = Some(self.congestion.changer_plafond(bps));
        }
        if sondage_arme() {
            self.rtc.bwe().set_desired_bitrate(Bitrate::bps(bps as u64));
        }
        // `session` : sans ce champ la trace n'est PAS attribuable. Tous les
        // enfants héritent le même `agent.log` (stdout partagé depuis D4), et
        // la somme des parts accordées — le critère ③ de la recette — se
        // calculerait alors sur un multiensemble de nombres anonymes. Même
        // motif et même champ que `cadence de la piste vidéo (côté enfant)`.
        // `endormie` : sans lui, une part appliquée au seul sondage se lit
        // dans le journal exactement comme une part appliquée aux deux.
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

    /// Source factice dont l'état de sommeil se pilote depuis le test —
    /// exactement ce que `SourceDistante` expose au vu des `Sommeil` poussés
    /// par le capteur, sans dépendre du capteur.
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

    /// Le défaut de fond relevé par la revue finale de branche (I1), et le
    /// seul test qui le voie : **le plafond remonte au réveil, pas le débit.**
    ///
    /// La chaîne, entièrement dans le code : `PART_DORMANTE_BPS` vaut 256 kb/s,
    /// `changer_plafond` fait `video_bitrate_bps.min(plafond)` dès qu'une
    /// estimation a existé, et rien ne défait ce `min` — la seule réparation
    /// serait `Controleur::observer`, appelé depuis le bras `MediaEgressStats`
    /// que str0m n'émet pas pour un flux qui n'a rien envoyé. Une endormie
    /// n'envoie rien. Ce n'est pas un cas limite : c'est l'état de CHAQUE
    /// réveil.
    ///
    /// Le régime importe : il faut une estimation RÉELLE avant la part
    /// dormante, sinon `changer_plafond` fait suivre le débit au plafond dans
    /// les deux sens (régime « jamais aucune estimation ») et le défaut ne se
    /// manifeste pas — le test serait vert par construction. Même précaution
    /// que `une_part_qui_remonte_ne_releve_pas_le_debit_au_dela_de_l_estimation`.
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
        session.handle_event(Event::MediaEgressStats(stats_video(mid)), &mut |_| {}, &mut |_| {});
        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Active,
            "précondition : une estimation réelle a bien été observée, donc `changer_plafond` BORNE"
        );
        let part_eveillee = 1_333_333; // 12 Mb/s partagés à huit fenêtres.
        assert!(
            session.congestion.courant().video_bitrate_bps > part_eveillee,
            "précondition : l'estimation laisse de la place au-dessus de la part d'éveillée, \
             sans quoi l'assertion finale ne prouverait rien"
        );

        // L'observation ci-dessus a elle-même posé une décision en attente :
        // la vider ici est ce qui rend l'assertion suivante lisible — sans
        // cela, elle constaterait un `Some` hérité et non celui qu'on cherche.
        session.pending_decision = None;

        // La fenêtre s'endort : le capteur pousse le plancher, très en dessous
        // du barreau le plus bas de l'échelle.
        endormie.store(true, Ordering::SeqCst);
        session.appliquer_part(PART_DORMANTE_BPS);
        assert!(
            session.pending_decision.is_none(),
            "une part d'endormie ne pose aucune décision : il n'y a plus d'encodeur à régler"
        );

        // Elle se réveille, et reçoit sa part d'éveillée.
        endormie.store(false, Ordering::SeqCst);
        session.appliquer_part(part_eveillee);

        assert_eq!(
            session.congestion.courant().video_bitrate_bps,
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
