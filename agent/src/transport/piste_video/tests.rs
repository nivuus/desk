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
    let now = start + Duration::from_micros(100); // slight send delay
    let next = next_frame_deadline(previous, now, interval);
    // Based on `previous`, not on `now`: the delay does not accumulate.
    assert_eq!(next, previous + interval);
}

#[test]
fn rattrapage_borne_apres_un_long_blocage() {
    let start = Instant::now();
    let interval = Duration::from_micros(16_667);
    let previous = start;
    let now = start + Duration::from_millis(500); // blocked well over one interval
    let next = next_frame_deadline(previous, now, interval);
    // No catch-up burst: we restart one interval after
    // now rather than trying to resend all the missed
    // frames at once.
    assert_eq!(next, now + interval);
}

/// Non-regression on the A/V sync fix: `write_frame`
/// must announce the CAPTURE instant, not the write one. An
/// origin placed in the PAST makes the two impossible to confuse:
/// if the method read the current clock, the result would be
/// later than `avant`, not earlier.
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

    // A frame captured 2 s after the origin carries PTS 180,000.
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

/// Non-regression net on THE fix of this workstream (in `write_frame`):
/// if `write_frame` went back to `Instant::now()` instead of
/// `self.capture_instant(unit.pts_90k)`, no existing test would
/// detect it — `la_session_ancre_l_instant_de_capture_sur_son_origine`
/// only exercises `capture_instant` in isolation, never its use at the call
/// site, which requires a negotiated session.
///
/// The `wallclock` passed to `writer.write()` is NOT observable on the peer side
/// through `Event::MediaData::network_time`: that field is documented (str0m
/// 0.21, `media/event.rs`) as the instant of local RECEPTION of the first
/// packet — unrelated to the `wallclock` emitted by the agent. The
/// field that really reflects the `wallclock` is
/// `MediaData::last_sender_info`, fed by the most recent RTCP Sender Report
/// (SR) received for this stream
/// (`str0m::streams::receive::ReceiverStream::set_sender_info`).
///
/// `str0m::streams::send::SendStream::sender_info` builds the SR's
/// (ntp_time, rtp_time) pair by extrapolation from the LAST
/// `write()`:
/// `rtp_time = pts_of_last_write + (sr_instant -
/// wallclock_of_last_write)`.
/// By choosing a `clock_origin` shifted 10 s into the past, the
/// two behaviours become numerically unmistakable once
/// converted to seconds:
///   - correct (`capture_instant`): `wallclock = clock_origin +
///     pts/90000`, so the `pts` term cancels algebraically and
///     `rtp_time_secondes == sr_instant - clock_origin` — a gap
///     of about 10 s from the time elapsed since the start of the test;
///   - regression (`Instant::now()` at write time): `wallclock` is
///     close to the real write instant (not the shifted origin),
///     so `rtp_time_secondes ≈ sr_instant - test_instant_before`
///     — no 10 s shift.
///
/// The assertion's threshold (3 s) is far from both real values (~10 s
/// vs ~0 s): a wide margin for test noise (loopback latency,
/// granularity of the polling loop), without ever being able to confuse
/// the two behaviours.
///
/// Demonstration of the net's effectiveness (final review, see the task
/// report for the full output): by temporarily replacing
/// `self.capture_instant(unit.pts_90k)` with `Instant::now()` in `write_frame`,
/// this test fails with a measured gap close to 0 s instead of ~10 s.
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

    // RR_INTERVAL_VIDEO (str0m) is 1 s and the first SR is eligible
    // from the first frame written: 15 s of margin is largely
    // enough even on a loaded CI.
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
                            // > 0 excludes the degenerate SR (`MediaTime::ZERO`)
                            // that `sender_info` can emit before any
                            // write — never happens in practice here,
                            // kept out of caution.
                            if rtp_time_secondes > 0.0 {
                                // `Instant::now()` here is later than or
                                // equal to the real instant the SR was
                                // built: a safe upper bound on
                                // `sr_instant - avant`, which can only
                                // REDUCE the gap measured below, never
                                // inflate it artificially.
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
