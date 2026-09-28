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
        CandidatePt {
            codec: Codec::H264,
            packetization_mode: Some(0),
            pt: pt(96),
        },
        CandidatePt {
            codec: Codec::H264,
            packetization_mode: Some(1),
            pt: pt(98),
        },
        CandidatePt {
            codec: Codec::Opus,
            packetization_mode: None,
            pt: pt(111),
        },
    ];
    assert_eq!(select_h264_pt(candidates.into_iter()), Some(pt(98)));
}

#[test]
fn ignore_les_profils_sans_mode_1() {
    let candidates = vec![
        CandidatePt {
            codec: Codec::H264,
            packetization_mode: Some(0),
            pt: pt(96),
        },
        CandidatePt {
            codec: Codec::H264,
            packetization_mode: None,
            pt: pt(97),
        },
        CandidatePt {
            codec: Codec::Opus,
            packetization_mode: None,
            pt: pt(111),
        },
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
    assert_eq!(
        session.capture_instant(180_000),
        origine + Duration::from_secs(2)
    );
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
///
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

    let answer_sdp = session
        .accept_offer(&offer.to_sdp_string())
        .expect("offre acceptée");
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
                let wait = t
                    .saturating_duration_since(now)
                    .min(hard_deadline.saturating_duration_since(now));
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
                                let ecoule_depuis_avant = Instant::now()
                                    .saturating_duration_since(avant)
                                    .as_secs_f64();
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
