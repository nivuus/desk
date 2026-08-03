//! L'asservissement au réseau vu depuis la boucle : péremption de
//! l'estimation de bande passante, et application d'une décision du
//! contrôleur de congestion.
//!
//! La branche correspondante de la liste de priorités (a0ter, voir `tick`) ne
//! mute JAMAIS `Rtc` : elle ne touche que la source vidéo, l'encodeur audio
//! et la file de contrôle. Elle rend quand même la main aussitôt, comme les
//! branches qui, elles, mutent réellement `Rtc` — garder une seule action par
//! tour est ce qui rend la liste lisible.
//!
//! Le redimensionnement demandé par l'utilisateur, qui reconfigure lui aussi
//! l'encodage mais pour une tout autre raison, vit dans
//! `redimensionnement`. L'application d'une part de budget accordée par le
//! capteur (sous-bloc D6), extraite de ce fichier pour rester sous le plafond
//! de 500 lignes, vit dans `part`.

use std::time::{Duration, Instant};

use proto::control::AgentControl;

use super::Session;
use crate::congestion;

/// Estimation de bande passante de départ, avant toute rétroaction du pair.
///
/// Compromis mesuré à la tâche 12 : trop bas, le démarrage sur LAN met du
/// temps à rejoindre le plafond et la recette perd des images par seconde ;
/// trop haut, le premier instant d'une session sur lien étroit sature avant
/// la première correction. 2,5 Mb/s est le point de départ, à confirmer.
pub(super) const ESTIMATION_INITIALE_BPS: u32 = 2_500_000;

/// Durée au-delà de laquelle une estimation de bande passante non renouvelée
/// est traitée comme absente (I4, revue finale de branche).
///
/// `Event::EgressBitrateEstimate` et `Event::MediaEgressStats` n'arrivent pas
/// ensemble (voir le commentaire du champ `derniere_estimation_bps`) : sans
/// cette borne, une estimation reçue une seule fois puis plus jamais (TWCC qui
/// se tarit alors que la session survit) resterait utilisée indéfiniment par
/// le contrôleur — potentiellement la dernière valeur haute avant l'incident,
/// ce qui annoncerait « Bonne » sur un lien mort. `MediaEgressStats` arrive
/// environ une fois par seconde (`set_stats_interval`) : 5 s laisse plusieurs
/// occasions manquées avant de conclure à l'absence, sans laisser une
/// estimation figée vivre des dizaines de secondes.
const EXPIRATION_ESTIMATION: Duration = Duration::from_secs(5);

impl Session {
    /// Décision d'adaptation actuellement retenue. Alimente le message d'état
    /// du lien envoyé au navigateur (tâche 10).
    ///
    /// `encode_size` et `video_bitrate_bps` viennent de ce que le transport a
    /// RÉELLEMENT réussi à appliquer (`encode_size_appliquee`,
    /// `bitrate_applique`), pas de ce que le contrôleur a décidé : celui-ci
    /// reste optimiste par construction (voir `congestion::Controleur`), et
    /// seul le transport sait si l'encodeur a accepté le dernier réglage.
    /// Annoncer au navigateur une taille ou un débit que la piste n'émet pas
    /// serait un mensonge de la même famille que celui déjà corrigé sur
    /// `qualite` à la tâche 5. Les autres champs (`qualite`, `adaptation`,
    /// `opus_loss_perc`) restent ceux du contrôleur : aucun mécanisme de
    /// refus équivalent n'existe pour eux ici.
    pub fn decision_courante(&self) -> congestion::Decision {
        congestion::Decision {
            encode_size: self.encode_size_appliquee,
            video_bitrate_bps: self.bitrate_applique,
            ..self.congestion.courant()
        }
    }

    /// Dernière estimation de bande passante, si elle est encore fraîche à
    /// `now` (voir `EXPIRATION_ESTIMATION`). `None` signifie « pas
    /// d'estimation utilisable », qu'aucune ne soit jamais arrivée ou que la
    /// dernière ait vieilli — les deux cas conduisent le contrôleur à
    /// `Adaptation::Indisponible`.
    ///
    /// Extraite de `handle_event` pour être vérifiable directement :
    /// l'événement `MediaEgressStats` qui la consomme exige une session
    /// négociée, la péremption non.
    pub(super) fn estimation_fraiche(&self, now: Instant) -> Option<u32> {
        self.derniere_estimation_bps.and_then(|(bps, at)| {
            (now.saturating_duration_since(at) <= EXPIRATION_ESTIMATION).then_some(bps)
        })
    }

    /// Branche `a0ter` de la liste de priorités (voir `tick`) : applique une
    /// décision du contrôleur de congestion à l'encodeur vidéo et à
    /// l'encodeur audio, puis annonce au navigateur ce qui a RÉELLEMENT été
    /// appliqué.
    ///
    /// Ne rend rien : la branche conclut toujours le tour, et c'est `tick`
    /// qui le dit.
    pub(super) fn appliquer_decision(&mut self, decision: congestion::Decision) {
        match self.source.set_bitrate(decision.video_bitrate_bps) {
            Ok(()) => self.bitrate_applique = decision.video_bitrate_bps,
            Err(e) => {
                // L'encodeur refuse le débit à chaud : on garde le débit
                // courant et on continue d'adapter par la résolution. Une
                // seule ligne, pas une par seconde.
                if !self.refus_debit_signale {
                    self.refus_debit_signale = true;
                    tracing::warn!(erreur = %e, "l'encodeur refuse le réglage du débit à chaud");
                }
            }
        }
        // **`taille_refus_signalee` est une GARDE, pas un simple témoin de
        // journal** (I1, revue finale de branche du sous-bloc D4).
        //
        // Après un refus, `encode_size_appliquee` n'avance pas : la condition
        // `decision.encode_size != encode_size_appliquee` reste donc vraie et
        // la MÊME cible serait resoumise à la source à chaque décision du
        // contrôleur — une par seconde, potentiellement des heures sous
        // congestion soutenue. Or `WindowsSource::set_encode_size` construit
        // un `H264Encoder` NEUF avant de relâcher l'ancien : à huit fenêtres
        // c'est la construction d'un neuvième encodeur, refusée par
        // construction (plafond de 8, mesuré 18/18 à la recette du sous-bloc
        // D4), donc jusqu'à huit instanciations par seconde de la MFT
        // matérielle dans le processus qui tient les huit duplications. Le
        // `warn!` était bien dédupliqué ; le TRAVAIL ne l'était pas.
        //
        // Les deux chemins qui effacent cette mémoire — un `set_encode_size`
        // réussi ci-dessous, et un redimensionnement de fenêtre
        // (`redimensionnement.rs`) — sont ce qui rend une cible de nouveau
        // soumissible : dans les deux cas la taille encodée a changé sous
        // elle, et le refus précédent ne préjuge plus de rien.
        let deja_refusee = self.taille_refus_signalee == Some(decision.encode_size);
        if decision.encode_size != self.encode_size_appliquee && !deja_refusee {
            match self.source.set_encode_size(decision.encode_size.0, decision.encode_size.1) {
                Ok(()) => {
                    tracing::info!(
                        largeur = decision.encode_size.0,
                        hauteur = decision.encode_size.1,
                        "taille d'encodage changée"
                    );
                    self.encode_size_appliquee = decision.encode_size;
                    // Un refus ultérieur de cette même taille (ou d'une
                    // autre) redeviendra une information neuve.
                    self.taille_refus_signalee = None;
                }
                Err(e) => {
                    // On reste au barreau courant. La session vit. La garde
                    // ci-dessus assure qu'on n'atteint ce point que pour une
                    // cible qui n'a pas DÉJÀ été refusée : une seule ligne de
                    // journal par cible, et un seul essai par cible.
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
        // On annonce `decision_courante()`, pas `decision` : `bitrate` et
        // `encode_size` doivent refléter ce que l'encodeur a RÉELLEMENT
        // accepté ci-dessus (`self.bitrate_applique`,
        // `self.encode_size_appliquee`), pas la cible visée par le
        // contrôleur — un refus d'encodeur laisserait sinon passer au
        // navigateur exactement le mensonge que `decision_courante()`
        // existe pour éviter (voir sa documentation et le test
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

    /// Réserve consignée par `CLAUDE.md` depuis le chantier C : « les
    /// transitions d'`Adaptation` (`Active` → `Indisponible`) et l'expiration
    /// de l'estimation BWE à 5 s n'ont pas de test : vérifiées par lecture de
    /// code ». Ce test les ferme.
    ///
    /// Il pilote `handle_event` avec des événements str0m synthétiques (leurs
    /// types sont entièrement publics) plutôt qu'avec un pair réel en boucle
    /// locale : ce qui est en jeu est un enchaînement d'états internes sur
    /// une échelle de temps de plusieurs secondes, pas un acheminement réseau
    /// — et aucun pair ne sait faire *cesser* l'émission de TWCC à la demande.
    #[test]
    fn une_estimation_perimee_bascule_l_adaptation_en_indisponible_et_l_annonce() {
        let mid = Mid::from("0");
        let source = Box::new(fixtures::video_test_source());
        let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
            .expect("session");
        // `MediaEgressStats` d'une autre piste est ignoré : sans `video_mid`,
        // aucune observation n'atteindrait le contrôleur. La négociation
        // elle-même est prouvée ailleurs (`piste_video`, `evenements`).
        session.video_mid = Some(mid);

        // 1) Estimation fraîche, puis statistiques : l'adaptation est active
        //    et rien n'est signalé.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(4_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        assert!(
            session.estimation_fraiche(Instant::now()).is_some(),
            "une estimation qui vient d'arriver est fraîche"
        );
        session.handle_event(Event::MediaEgressStats(stats_video(mid)), &mut |_| {}, &mut |_| {});
        assert_eq!(session.congestion.courant().adaptation, congestion::Adaptation::Active);
        assert!(!session.absence_bwe_signalee);
        assert!(!session.indisponibilite_annoncee);

        // 2) La même estimation, vieillie au-delà d'`EXPIRATION_ESTIMATION` :
        //    TWCC s'est tari alors que la session survit. Elle doit être
        //    traitée comme ABSENTE, et non resservir indéfiniment — c'est
        //    exactement la dernière valeur haute avant l'incident qui
        //    annoncerait « Bonne » sur un lien mort.
        let (bps, _) = session.derniere_estimation_bps.expect("posée en 1");
        let perimee_a = Instant::now() - EXPIRATION_ESTIMATION - Duration::from_millis(1);
        session.derniere_estimation_bps = Some((bps, perimee_a));
        assert_eq!(
            session.estimation_fraiche(Instant::now()),
            None,
            "au-delà d'EXPIRATION_ESTIMATION, une estimation ne doit plus être utilisable"
        );

        session.pending_decision = None;
        session.handle_event(Event::MediaEgressStats(stats_video(mid)), &mut |_| {}, &mut |_| {});

        assert_eq!(
            session.congestion.courant().adaptation,
            congestion::Adaptation::Indisponible,
            "l'expiration de l'estimation doit faire basculer l'adaptation, pas la laisser à Active"
        );
        assert!(session.absence_bwe_signalee, "l'absence doit être journalisée une fois");
        assert!(session.indisponibilite_annoncee);
        let decision = session.pending_decision.expect(
            "l'indisponibilité doit être RELAYÉE au navigateur (I2), pas seulement journalisée : \
             `Controleur::observer` ne produit aucune décision sans estimation",
        );
        assert_eq!(decision.adaptation, congestion::Adaptation::Indisponible);

        // 3) La branche a0ter transforme cette décision mémorisée en message
        //    `Link` pour le navigateur.
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

        // 4) Une estimation fraîche qui revient réarme l'annonce : une
        //    indisponibilité ultérieure est une information neuve.
        session.handle_event(
            Event::EgressBitrateEstimate(BweKind::Twcc(Bitrate::bps(4_000_000))),
            &mut |_| {},
            &mut |_| {},
        );
        session.handle_event(Event::MediaEgressStats(stats_video(mid)), &mut |_| {}, &mut |_| {});
        assert_eq!(session.congestion.courant().adaptation, congestion::Adaptation::Active);
        assert!(
            !session.indisponibilite_annoncee,
            "une coupure ultérieure de TWCC doit être annoncée de nouveau"
        );
    }

    /// Tailles d'encodage réellement SOUMISES à la source, dans l'ordre.
    type TaillesSoumises = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>;

    /// Une session dont la source refuse TOUTE taille d'encodage, et retient
    /// celles qui lui ont été soumises.
    ///
    /// Retenir les soumissions et pas seulement leur nombre : c'est ce qui
    /// permet de distinguer « une cible refusée n'est plus resoumise » d'« une
    /// cible différente est toujours essayée ».
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
        let session =
            Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");
        (session, soumises)
    }

    /// Applique `decision` par la branche `a0ter`, comme le ferait un tour de
    /// la boucle de transport.
    fn appliquer(session: &mut Session, decision: congestion::Decision) {
        session.pending_decision = Some(decision);
        session
            .act_on_timeout(Instant::now())
            .expect("un refus de l'encodeur ne doit jamais faire échouer la session");
    }

    /// Une décision de congestion visant `encode_size`, le reste étant sans
    /// effet sur ce que ces tests observent.
    fn decision_vers(encode_size: (u32, u32)) -> congestion::Decision {
        congestion::Decision {
            video_bitrate_bps: 5_000_000,
            encode_size,
            opus_loss_perc: 0,
            qualite: congestion::Qualite::Degradee,
            adaptation: congestion::Adaptation::Active,
        }
    }

    /// **I1 de la revue finale de branche du sous-bloc D4.** Une cible déjà
    /// refusée ne doit plus être RESOUMISE à la source — pas seulement ne plus
    /// être rejournalisée.
    ///
    /// Ce que ce test protège n'est pas cosmétique : en capture mutualisée,
    /// `WindowsSource::set_encode_size` construit un `H264Encoder` neuf avant
    /// de relâcher l'ancien, et à huit fenêtres cette construction est refusée
    /// par construction. Sans la garde, le contrôleur relancerait ce travail à
    /// chaque décision — une par seconde et par fenêtre, indéfiniment.
    #[test]
    fn une_cible_deja_refusee_n_est_plus_soumise_a_la_source() {
        let (mut session, soumises) = session_refusant_les_tailles();
        let taille_originale = session.encode_size_appliquee;
        let refusee = (640, 360);
        assert_ne!(refusee, taille_originale, "précondition du test");

        // Cinq décisions identiques, comme le contrôleur en produirait cinq
        // secondes durant sous congestion soutenue.
        for _ in 0..5 {
            appliquer(&mut session, decision_vers(refusee));
        }
        assert_eq!(
            *soumises.lock().unwrap(),
            vec![refusee],
            "la cible refusée ne doit être soumise qu'UNE fois, pas à chaque décision"
        );

        // Une cible DIFFÉRENTE reste une information neuve : la garde ne doit
        // pas figer l'adaptation, seulement supprimer la répétition.
        let autre = (960, 540);
        appliquer(&mut session, decision_vers(autre));
        assert_eq!(
            *soumises.lock().unwrap(),
            vec![refusee, autre],
            "une cible jamais essayée doit l'être, même après un refus précédent"
        );

        // Et la nouvelle cible refusée devient à son tour la cible gardée :
        // la mémoire suit la dernière, elle ne s'accumule pas.
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
        // Ronde de correction (revue post-tâche 9) : `Controleur::observer`
        // reste optimiste par construction — il met à jour `courant.encode_size`
        // que l'encodeur accepte ou non le changement. Sans la distinction
        // que ce test vérifie, `decision_courante()` annoncerait au
        // navigateur (message d'état du lien, tâche 10) une taille que la
        // piste vidéo n'émet jamais.
        //
        // Ce test couvre aussi la déduplication du journal côté refus
        // (`taille_refus_signalee`) : trois décisions identiques de suite,
        // comme le ferait le contrôleur une fois par seconde sous
        // congestion soutenue, ne doivent faire grandir ni changer cette
        // mémoire au-delà de sa première écriture — compter les lignes de
        // journal elles-mêmes n'est pas praticable dans ce harnais (aucune
        // capture de `tracing` n'existe dans ce module).
        let (mut session, _soumises) = session_refusant_les_tailles();
        let taille_originale = session.encode_size_appliquee;
        let taille_visee = (640, 360);
        assert_ne!(taille_visee, taille_originale, "précondition du test");

        // Trois décisions successives, comme le ferait le contrôleur une
        // fois par seconde sous congestion soutenue : la même taille
        // refusée à chaque tour.
        for _ in 0..3 {
            appliquer(&mut session, decision_vers(taille_visee));
        }

        // Trouvaille 2 : `decision_courante()` doit continuer à rapporter
        // l'ANCIENNE taille, celle réellement émise — pas celle refusée.
        assert_eq!(
            session.decision_courante().encode_size,
            taille_originale,
            "un refus de l'encodeur ne doit jamais se refléter dans la décision annoncée"
        );

        // Trouvaille 1 : la mémoire de dédoublonnage retient la cible
        // refusée, stable sur les trois tours identiques — c'est elle qui
        // empêche la répétition du journal à chaque décision.
        assert_eq!(
            session.taille_refus_signalee,
            Some(taille_visee),
            "la cible refusée doit être mémorisée pour éviter de rejournaliser à chaque tour"
        );
    }
}
