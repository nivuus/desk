//! La piste vidéo : négociation du payload type H.264, ancrage de l'instant
//! de capture sur l'origine d'horloge de la session, et écriture des unités
//! d'accès vers str0m.

use std::time::{Duration, Instant};

use str0m::format::Codec;
use str0m::media::{MediaTime, Mid, Pt};

use super::tick::Tick;
use super::Session;
use crate::clock::instant_from_pts;
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};

/// Cadence d'interrogation de la source vidéo : une toutes les 10 ms (100 Hz).
///
/// Ce n'est PAS la cadence d'émission : `VideoSource::next_frame` ne rend une
/// unité d'accès que s'il y en a une de prête, et rend `None` sinon (cas
/// courant et normal, voir `windows_source`). La cadence d'émission réelle est
/// donc celle de la source, bornée par celle-ci.
///
/// **Pourquoi 100 Hz et non 60 (28/07).** À 60 Hz, la capture ne récupérait
/// que 46 images/s d'un bureau qui, lui, se met à jour à 68,5 Hz — mesuré
/// directement par `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`
/// (`desktop_updates_hz` dans la trace `SOURCE_TRACE`). Chaque tour ne peut
/// remonter qu'une image, quel que soit le nombre de mises à jour que DXGI a
/// fusionnées entre-temps : interroger une source à 68,5 Hz seulement 60 fois
/// par seconde en perd mécaniquement une partie. Interroger plus souvent que
/// la source ne produit lève cette borne sans rien coûter quand il n'y a rien
/// à prendre — `AcquireNextFrame` est appelée avec un délai NUL, donc un tour
/// à vide se résume à un aller-retour DXGI immédiat.
///
/// Le plafond de 60 im/s visé par le jalon reste, lui, celui du contenu : rien
/// ici ne fabrique d'images qui n'existent pas.
///
/// **Vérifié le 28/07** : sonder 5× plus vite (2 ms) ne change rien au débit
/// — `produced_hz` reste à 47,5 pour un bureau à 68,5 Hz. La cadence de
/// sondage n'était donc pas le facteur limitant ; c'était le
/// `MF_MT_FRAME_RATE` annoncé aux MFT (voir `demarrage.rs`).
pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(10);

/// Vue minimale d'un profil de charge utile négocié, indépendante de str0m
/// pour rester testable sans session RTC réelle : les champs de
/// `str0m::format::PayloadParams` (dont `pt`) sont `pub(crate)` côté str0m,
/// donc impossibles à construire depuis ce crate pour un test.
#[derive(Debug, Clone, Copy)]
struct CandidatePt {
    codec: Codec,
    packetization_mode: Option<u8>,
    pt: Pt,
}

/// Sélectionne le type de charge utile à utiliser pour envoyer du H.264.
///
/// `enable_h264(true)` négocie sept profils (modes de paquetisation 0 et 1,
/// quatre profils de compatibilité) : prendre le premier de la liste, comme
/// le faisait la version initiale, ne garantit rien sur ce que produira
/// l'encodeur. On filtre explicitement sur le mode de paquetisation 1
/// (non-interleaved, RFC 6184 §6.2) — le seul que les tâches suivantes
/// produiront. Le profil exact (constrained-baseline, etc.) n'est pas
/// discriminé plus finement ici : ce n'est vérifiable qu'avec un navigateur
/// réel et un encodeur réel, pas avant les tâches 8/11.
fn select_h264_pt(candidates: impl Iterator<Item = CandidatePt>) -> Option<Pt> {
    candidates
        .filter(|p| p.codec == Codec::H264 && p.packetization_mode == Some(1))
        .map(|p| p.pt)
        .next()
}

/// Échéance de la prochaine image, calculée à partir de l'échéance
/// *précédente* plutôt que de l'instant courant, pour ne pas accumuler de
/// dérive : un léger retard sur une image ne retarde pas systématiquement
/// toutes les suivantes. Borné à un intervalle de rattrapage : au-delà, on
/// abandonne le calcul fondé sur `previous` (qui produirait une rafale
/// d'images pour rattraper tout le retard d'un coup) et on repart d'un
/// intervalle après `now`.
pub(super) fn next_frame_deadline(previous: Instant, now: Instant, interval: Duration) -> Instant {
    let candidate = previous + interval;
    if now.saturating_duration_since(candidate) > interval {
        now + interval
    } else {
        candidate
    }
}

impl Session {
    /// Branche `b` de la liste de priorités (voir `tick`) : émet une image
    /// vidéo si son échéance est atteinte et la piste négociée.
    ///
    /// Rend `Some(Tick::Continue)` quand l'échéance était atteinte — le tour
    /// est alors conclu, qu'une image ait été écrite ou non : la tentative
    /// elle-même est l'action du tour, et une écriture réussie est une
    /// mutation de `Rtc` qui doit être suivie du drainage différé de la
    /// branche `a0`. Rend `None` quand l'échéance n'est pas atteinte ou que
    /// la piste n'est pas négociée, sans avoir rien muté.
    pub(super) fn brancher_video(&mut self) -> Option<Tick> {
        let mid = self.video_mid?;
        let now = Instant::now();
        if now < self.next_frame_at {
            return None;
        }
        self.next_frame_at = next_frame_deadline(self.next_frame_at, now, FRAME_INTERVAL);
        match self.source.next_frame() {
            Some(unit) => {
                // `writer.write()` ne fait qu'empiler l'image dans la file
                // interne `to_payload` de str0m — c'est
                // `Rtc::handle_input(Input::Timeout(..))` qui la dépile
                // réellement en paquets RTP (`do_payload`), jamais
                // `poll_output()` seul (voir `session.rs` de str0m).
                // L'appeler ICI serait une seconde mutation dans le même
                // appel à `act_on_timeout`, sans `poll_output` entre les
                // deux — exactement la violation que ce mécanisme doit
                // éviter (ronde de correction 1). On pose donc un drapeau :
                // la PROCHAINE invocation d'`act_on_timeout` le traite en
                // priorité absolue (branche `a0`). La file de charge non vide
                // fait renvoyer une échéance immédiate par `poll_output()`,
                // donc `run()` rappelle aussitôt.
                if self.write_frame(mid, unit) {
                    self.video_write_pending_drain = true;
                }
            }
            None => {
                // Ronde de correction 1 : l'absence de nouvelle image est le
                // cas courant et normal d'une capture en direct (bureau
                // immobile) — pas une fin de session. Seule une source
                // réellement épuisée (fenêtre fermée, erreur non
                // récupérable) le justifie, via `VideoSource::is_exhausted`.
                // `FileSource` ne renvoie jamais `None` et n'atteint donc
                // jamais ce chemin.
                if self.source.is_exhausted() {
                    self.begin_ending("source vidéo épuisée");
                }
            }
        }
        // Compteur de cadence côté enfant, le pendant de celui du capteur —
        // voir `cadence_video.rs` (extrait de ce fichier, tâche 8 : l'ajout
        // dépassait le plafond de 500 lignes de ce fichier).
        self.compter_la_cadence_video();
        Some(Tick::Continue)
    }

    /// Sélectionne le type de charge utile H.264 négocié pour `mid`, s'il y
    /// en a un. Appel séparé de `write_frame` pour que l'emprunt sur `self`
    /// via `Rtc::writer` se termine avant tout appel `&mut self` ultérieur
    /// (le journal d'avertissement, notamment).
    fn select_negotiated_h264_pt(&mut self, mid: Mid) -> Option<Pt> {
        let writer = self.rtc.writer(mid)?;
        select_h264_pt(writer.payload_params().map(|p| CandidatePt {
            codec: p.spec().codec,
            packetization_mode: p.spec().format.packetization_mode,
            pt: p.pt(),
        }))
    }

    /// Instant réel auquel l'image d'horodatage `pts_90k` a été capturée.
    ///
    /// C'est cette valeur que `write_frame` annonce à str0m comme `wallclock`.
    /// Extraite en méthode pour être vérifiable directement : l'écriture
    /// elle-même exige une session négociée, la conversion non.
    fn capture_instant(&self, pts_90k: u64) -> Instant {
        instant_from_pts(self.clock_origin, pts_90k, CLOCK_RATE_HZ as u32)
    }

    /// Écrit une unité d'accès sur la piste vidéo. Mutation émise depuis
    /// l'intérieur de la boucle de `run()` (voir `act_on_timeout`), donc
    /// suivie d'un retour immédiat à `poll_output` — conforme à la règle de
    /// drainage de str0m.
    ///
    /// Renvoie `true` si `writer.write()` a réellement été appelée et a
    /// réussi (donc qu'une entrée a bien été empilée dans `to_payload` et
    /// nécessite le drainage différé — voir `video_write_pending_drain`),
    /// `false` si l'écriture n'a pas eu lieu (négociation incomplète,
    /// piste indisponible) ou a échoué : dans ces deux cas, aucune entrée
    /// n'a été ajoutée à `to_payload`, poser le drapeau de drainage serait
    /// à tort et provoquerait un `handle_input(Timeout)` inutile.
    pub(super) fn write_frame(&mut self, mid: Mid, unit: AccessUnit) -> bool {
        let Some(pt) = self.select_negotiated_h264_pt(mid) else {
            // I4 : négociation incomplète (aucun profil H.264 en mode de
            // paquetisation 1) — sans ce journal, l'image est jetée
            // silencieusement, produisant un écran noir muet indéfiniment
            // sans le moindre indice dans les journaux.
            self.warn_negotiation_once(
                "aucun type de charge utile H.264 négocié (mode de paquetisation 1) : images jetées",
            );
            return false;
        };
        // Le `wallclock` de str0m est « the real world time that corresponds
        // to the MediaTime » — l'instant de CAPTURE, pas celui de l'écriture.
        // Passer `Instant::now()` ici encapsulait tout le délai de capture et
        // d'encodage matériel dans la correspondance annoncée, ce qui restait
        // invisible tant que la vidéo était seule. Avec une piste audio, dont
        // le chemin est bien plus court, l'audio devancerait la vidéo de tout
        // ce délai et la synchro labiale serait fausse par construction.
        //
        // L'horodatage fait l'aller-retour par Media Foundation sans perte
        // (`encode.rs`), donc l'instant de capture se reconstruit exactement
        // depuis l'origine partagée. Calculé avant l'emprunt de `writer` :
        // celui-ci retient `&mut self.rtc`, incompatible avec l'emprunt
        // immuable de `self.clock_origin` qu'exige `capture_instant`.
        let capture_at = self.capture_instant(unit.pts_90k);
        let Some(writer) = self.rtc.writer(mid) else {
            self.warn_negotiation_once("piste vidéo plus accessible en écriture : images jetées");
            return false;
        };
        match writer.write(pt, capture_at, MediaTime::from_90khz(unit.pts_90k), unit.data) {
            Ok(()) => {
                // C'est ICI, et seulement ici, que l'écriture a réellement
                // eu lieu — voir `compter_la_cadence_video`, qui journalise
                // ce compte, jamais un tour de boucle ni un `next_frame` à
                // vide.
                self.unites_video_ecrites += 1;
                true
            }
            Err(e) => {
                // Échec d'écriture applicatif (ex. RID inconnu) : on clôt la
                // session plutôt que de faire remonter l'erreur jusqu'au
                // processus. Seules `Session::new` et `accept_offer` — avant
                // qu'une session n'existe vraiment — justifient de tuer le
                // processus entier.
                tracing::warn!(erreur = %e, "échec d'écriture de l'image, fin de session");
                self.begin_ending("échec d'écriture vidéo");
                false
            }
        }
    }

    fn warn_negotiation_once(&mut self, message: &str) {
        if !self.warned_negotiation {
            self.warned_negotiation = true;
            tracing::warn!(message, "négociation vidéo incomplète (avertissement unique)");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use str0m::{Event, Output};

    use super::*;
    use crate::transport::fixtures;

    fn pt(v: u8) -> Pt {
        Pt::from(v)
    }

    #[test]
    fn selectionne_le_mode_de_paquetisation_1() {
        let candidates = vec![
            CandidatePt { codec: Codec::H264, packetization_mode: Some(0), pt: pt(96) },
            CandidatePt { codec: Codec::H264, packetization_mode: Some(1), pt: pt(98) },
            CandidatePt { codec: Codec::Opus, packetization_mode: None, pt: pt(111) },
        ];
        assert_eq!(select_h264_pt(candidates.into_iter()), Some(pt(98)));
    }

    #[test]
    fn ignore_les_profils_sans_mode_1() {
        let candidates = vec![
            CandidatePt { codec: Codec::H264, packetization_mode: Some(0), pt: pt(96) },
            CandidatePt { codec: Codec::H264, packetization_mode: None, pt: pt(97) },
            CandidatePt { codec: Codec::Opus, packetization_mode: None, pt: pt(111) },
        ];
        assert_eq!(select_h264_pt(candidates.into_iter()), None);
    }

    #[test]
    fn ignore_les_codecs_non_h264_meme_en_mode_1() {
        let candidates = vec![CandidatePt {
            codec: Codec::Vp8,
            packetization_mode: Some(1),
            pt: pt(100),
        }];
        assert_eq!(select_h264_pt(candidates.into_iter()), None);
    }

    #[test]
    fn cadence_normale_basee_sur_l_echeance_precedente_sans_derive() {
        let start = Instant::now();
        let interval = Duration::from_micros(16_667);
        let previous = start;
        let now = start + Duration::from_micros(100); // léger retard d'envoi
        let next = next_frame_deadline(previous, now, interval);
        // Basé sur `previous`, pas sur `now` : le retard ne s'accumule pas.
        assert_eq!(next, previous + interval);
    }

    #[test]
    fn rattrapage_borne_apres_un_long_blocage() {
        let start = Instant::now();
        let interval = Duration::from_micros(16_667);
        let previous = start;
        let now = start + Duration::from_millis(500); // bloqué bien plus d'un intervalle
        let next = next_frame_deadline(previous, now, interval);
        // Pas de rafale de rattrapage : on repart d'un intervalle après
        // maintenant plutôt que de tenter de renvoyer toutes les images
        // manquées d'un coup.
        assert_eq!(next, now + interval);
    }

    /// Non-régression sur la correction de la synchro A/V : `write_frame`
    /// doit annoncer l'instant de CAPTURE, pas celui de l'écriture. Une
    /// origine placée dans le PASSÉ rend les deux impossibles à confondre :
    /// si la méthode lisait l'horloge courante, le résultat serait
    /// postérieur à `avant`, pas antérieur.
    #[test]
    fn la_session_ancre_l_instant_de_capture_sur_son_origine() {
        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let source_path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let source = Box::new(
            crate::source::FileSource::from_path(source_path, 1280, 720, 60)
                .expect("chargement du flux de test"),
        );

        let avant = Instant::now();
        let origine = avant - Duration::from_secs(10);
        let session = Session::new(source, local_ip, origine, 12_000_000).expect("session");

        // Une image capturée 2 s après l'origine porte le PTS 180 000.
        assert_eq!(session.capture_instant(180_000), origine + Duration::from_secs(2));
        assert!(
            session.capture_instant(180_000) < avant,
            "l'instant doit être ancré sur l'origine (dans le passé), pas sur l'horloge courante"
        );
        assert_eq!(session.capture_instant(0), origine);
    }

    /// Filet de non-régression sur LA correction de ce chantier (dans `write_frame`) :
    /// si `write_frame` redevenait `Instant::now()` au lieu de
    /// `self.capture_instant(unit.pts_90k)`, aucun test existant ne le
    /// détecterait — `la_session_ancre_l_instant_de_capture_sur_son_origine`
    /// n'exerce que `capture_instant` isolément, jamais son usage au point
    /// d'appel, qui exige une session négociée.
    ///
    /// Le `wallclock` passé à `writer.write()` n'est PAS observable côté pair
    /// via `Event::MediaData::network_time` : ce champ est documenté (str0m
    /// 0.21, `media/event.rs`) comme l'instant de RÉCEPTION locale du premier
    /// paquet — sans aucun rapport avec le `wallclock` émis par l'agent. Le
    /// champ qui reflète réellement le `wallclock` est
    /// `MediaData::last_sender_info`, alimenté par le Sender Report RTCP
    /// (SR) le plus récent reçu pour ce flux
    /// (`str0m::streams::receive::ReceiverStream::set_sender_info`).
    ///
    /// `str0m::streams::send::SendStream::sender_info` construit la paire
    /// (ntp_time, rtp_time) du SR par extrapolation à partir du DERNIER
    /// `write()` :
    /// `rtp_time = pts_de_la_derniere_ecriture + (instant_du_SR -
    /// wallclock_de_la_derniere_ecriture)`.
    /// En choisissant une `clock_origin` décalée de 10 s dans le passé, les
    /// deux comportements deviennent numériquement inconfondables une fois
    /// convertis en secondes :
    ///   - correct (`capture_instant`) : `wallclock = clock_origin +
    ///     pts/90000`, donc le terme `pts` s'annule algébriquement et
    ///     `rtp_time_secondes == instant_du_SR - clock_origin` — un écart
    ///     d'environ 10 s avec le temps écoulé depuis le début du test ;
    ///   - régression (`Instant::now()` à l'écriture) : `wallclock` est
    ///     proche de l'instant réel d'écriture (pas de l'origine décalée),
    ///     donc `rtp_time_secondes ≈ instant_du_SR - instant_de_test_avant`
    ///     — aucun décalage de 10 s.
    /// Le seuil de l'assertion (3 s) est loin des deux valeurs réelles (~10 s
    /// vs ~0 s) : large marge pour le bruit de test (latence loopback,
    /// granularité de la boucle de sondage), sans jamais pouvoir confondre
    /// les deux comportements.
    ///
    /// Démonstration de l'efficacité du filet (revue finale, voir le rapport
    /// de tâche pour la sortie complète) : en remplaçant temporairement
    /// `self.capture_instant(unit.pts_90k)` par `Instant::now()` dans `write_frame`,
    /// ce test échoue avec un écart mesuré proche de 0 s au lieu de ~10 s.
    #[test]
    fn write_frame_annonce_l_instant_de_capture_au_pair_via_le_sender_report_rtcp() {
        use std::thread;
        use str0m::change::SdpAnswer;
        use str0m::media::{Direction, MediaKind};

        let local_ip = fixtures::local_ip();
        let source = Box::new(fixtures::video_test_source());

        let avant = Instant::now();
        let origine = avant - Duration::from_secs(10);
        let mut session = Session::new(source, local_ip, origine, 12_000_000).expect("session");

        let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

        let mut api = peer_rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        api.add_channel("control".to_string());
        api.add_channel("input".to_string());
        let (offer, pending) = api.apply().expect("offre non vide");

        let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
        let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
        peer_rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .expect("réponse acceptée par le pair");

        thread::spawn(move || {
            let mut on_input = |_| {};
            let mut on_control = |_| {};
            let _ = session.run(&mut on_input, &mut on_control);
        });

        // RR_INTERVAL_VIDEO (str0m) vaut 1 s et le premier SR est éligible
        // dès la première image écrite : 15 s de marge est largement
        // suffisant même sur une CI chargée.
        let hard_deadline = Instant::now() + Duration::from_secs(15);
        let mut connected_at: Option<Instant> = None;
        let mut mesure: Option<(f64, f64)> = None; // (rtp_time_secondes, ecoule_depuis_avant)

        loop {
            let now = Instant::now();
            if now >= hard_deadline {
                panic!(
                    "le pair local ne s'est jamais connecté, ou aucun Sender Report RTCP \
                     exploitable n'a été reçu dans le délai imparti"
                );
            }
            if mesure.is_some() {
                break;
            }

            match peer_rtc.poll_output().expect("poll_output du pair") {
                Output::Timeout(t) => {
                    let wait = t.saturating_duration_since(now).min(hard_deadline.saturating_duration_since(now));
                    if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, now, wait) {
                        continue;
                    }
                }
                Output::Transmit(t) => {
                    let _ = peer_socket.send_to(&t.contents, t.destination);
                }
                Output::Event(Event::Connected) => {
                    connected_at = Some(Instant::now());
                }
                Output::Event(Event::MediaData(data)) => {
                    if connected_at.is_some() {
                        if let Codec::H264 = data.params.spec().codec {
                            if let Some(info) = data.last_sender_info {
                                let rtp_time_secondes = info.rtp_time.as_seconds();
                                // > 0 exclut le SR dégénéré (`MediaTime::ZERO`)
                                // que `sender_info` peut émettre avant toute
                                // écriture — n'arrive jamais en pratique ici,
                                // gardé par prudence.
                                if rtp_time_secondes > 0.0 {
                                    // `Instant::now()` ici est postérieur ou
                                    // égal à l'instant réel de construction du
                                    // SR : une borne supérieure sûre de
                                    // `instant_du_SR - avant`, qui ne peut que
                                    // RÉDUIRE l'écart mesuré ci-dessous, jamais
                                    // le gonfler artificiellement.
                                    let ecoule_depuis_avant =
                                        Instant::now().saturating_duration_since(avant).as_secs_f64();
                                    mesure = Some((rtp_time_secondes, ecoule_depuis_avant));
                                }
                            }
                        }
                    }
                }
                Output::Event(_) => {}
            }
        }

        let (rtp_time_secondes, ecoule_depuis_avant) = mesure.expect("mesure du SR");
        let ecart = rtp_time_secondes - ecoule_depuis_avant;
        eprintln!(
            "wallclock RTCP : rtp_time={rtp_time_secondes:.3}s, écoulé depuis le début du test={ecoule_depuis_avant:.3}s, écart={ecart:.3}s (attendu ≈ 10 s si write_frame annonce bien l'instant de capture)"
        );
        assert!(
            ecart > 3.0,
            "écart de {ecart:.3} s trop faible (attendu ≈ 10 s) : write_frame semble annoncer \
             l'instant d'ÉCRITURE plutôt que l'instant de CAPTURE comme wallclock RTCP — \
             régression sur la correction centrale du chantier (dans `write_frame`)"
        );
    }
}
