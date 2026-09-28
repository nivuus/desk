//! The tests of `piste_micro.rs`.
//!
//! Two families, and they answer different questions:
//! the unit tests exercise the exclusivity guard and the single
//! warnings on a bare `Session`; the integration test sends a
//! real Opus packet through a real str0m `Rtc` down to the sink.
//!
//! ⚠️ Probe 1 (`transport::sonde_montante`) answered on BARE str0m. The
//! integration test here answers on OUR path. Both coexist: the
//! first says what the library does, the second what the agent does.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use str0m::media::{Direction, Frequency, MediaKind, MediaTime};
use str0m::Output;

use super::*;
use crate::micro::{PuitsMicro, TrameMicro};
use crate::transport::fixtures;

/// A sink that records what it is given, and accepts or refuses at will.
struct PuitsEspion {
    recues: Arc<Mutex<Vec<TrameMicro>>>,
    accepte: bool,
}

impl PuitsMicro for PuitsEspion {
    fn deposer(&mut self, trame: TrameMicro) -> bool {
        self.recues.lock().unwrap().push(trame);
        self.accepte
    }
}

fn session_nue() -> Session {
    let source = Box::new(fixtures::video_test_source());
    Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session")
}

/// Spec §10: "upstream track not negotiated → no packet expected,
/// SINGLE warning" — modelled on `warn_audio_negotiation_once`.
///
/// The count is of lines ACTUALLY emitted (`journaux_micro` is only
/// incremented at the moment of `tracing::warn!`), not of the number of calls:
/// that is what makes the assertion able to fail.
#[test]
fn without_a_negotiated_mic_track_the_warning_comes_out_only_once() {
    let mut s = session_nue();
    assert!(!s.micro_disponible());
    for _ in 0..50 {
        s.avertir_micro_une_fois("trial");
    }
    assert_eq!(
        s.journaux_micro, 1,
        "the negotiation warning came out {} times",
        s.journaux_micro
    );
}

/// Spec §9: a second upstream stream is refused, logged ONCE, and its
/// track ignored. **The refusal comes from the SINK** (`deposer` returns `false`): the
/// transport knows no mutex, and that is the seam E2 will fill.
#[test]
fn a_refusing_sink_logs_only_once_and_kills_nothing() {
    let recues = Arc::new(Mutex::new(Vec::new()));
    let mut s = session_nue();
    s.set_puits_micro(Box::new(PuitsEspion {
        recues: recues.clone(),
        accepte: false,
    }));

    for i in 0..50u64 {
        s.deposer_trame_micro_de_test(TrameMicro {
            opus: vec![0x78, i as u8],
            rtp_48k: i * 960,
            echantillons: 960,
        });
    }

    assert_eq!(
        recues.lock().unwrap().len(),
        50,
        "the frames did not reach the sink"
    );
    assert_eq!(
        s.journaux_micro, 1,
        "the refusal was logged {} times instead of once",
        s.journaux_micro
    );
    // …and the session is not ending: a refused microphone
    // compromises nothing.
    assert!(!s.ending);
}

/// The microphone NEVER kills a working session (spec §10).
///
/// The property is partly STRUCTURAL — `deposer_micro` returns `()`, so
/// cannot propagate anything — and this test checks the part that
/// could change: the session's state is intact after a sink that refuses everything,
/// including the video track, which is what we protect.
#[test]
fn a_refusing_mic_does_not_compromise_the_video() {
    let mut s = session_nue();
    s.video_mid = Some("v0".into());
    s.set_puits_micro(Box::new(PuitsEspion {
        recues: Arc::new(Mutex::new(Vec::new())),
        accepte: false,
    }));
    for i in 0..200u64 {
        s.deposer_trame_micro_de_test(TrameMicro {
            opus: vec![0x78],
            rtp_48k: i * 960,
            echantillons: 960,
        });
    }
    assert_eq!(s.video_mid, Some("v0".into()), "the video track was lost");
    assert!(!s.ending, "the session ended because of the mic");
}

/// Probe 1 answered on bare str0m. This one answers on OUR path: an
/// Opus packet written by the peer is found in the session's sink,
/// with the duration READ from the packet and the peer's RTP timestamp (spec §11).
#[test]
fn an_upstream_opus_packet_reaches_the_session_sink() {
    use crate::opus::OpusEncoder;

    // A real 10 ms Opus frame: it is what gives meaning to
    // `echantillons`, which an arbitrary payload would make unreadable.
    let mut enc = OpusEncoder::new().expect("encodeur");
    let pcm: Vec<i16> = (0..crate::opus::FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();
    let charge = enc.encode(&pcm).expect("encoding");

    let recues = Arc::new(Mutex::new(Vec::new()));
    let local_ip = fixtures::local_ip();
    let mut session = session_nue();
    session.set_puits_micro(Box::new(PuitsEspion {
        recues: recues.clone(),
        accepte: true,
    }));

    let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, true);
    let mut api = peer_rtc.sdp_api();
    api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
    // The microphone: the BROWSER emits, so the agent receives.
    let mid_micro = api.add_media(MediaKind::Audio, Direction::SendOnly, None, None, None);
    let (offer, pending) = api.apply().expect("non-empty offer");

    let answer_sdp = session
        .accept_offer(&offer.to_sdp_string())
        .expect("offer accepted");
    let answer = str0m::change::SdpAnswer::from_sdp_string(&answer_sdp).expect("SDP answer");
    peer_rtc
        .sdp_api()
        .accept_answer(pending, answer)
        .expect("answer accepted");

    std::thread::spawn(move || {
        let mut on_input = |_| {};
        let mut on_control = |_| {};
        let _ = session.run(&mut on_input, &mut on_control);
    });

    let echeance = Instant::now() + Duration::from_secs(15);
    let mut horodatage = 0u64;
    while recues.lock().unwrap().is_empty() {
        let maintenant = Instant::now();
        assert!(
            maintenant < echeance,
            "no mic frame reached the sink within 15 s"
        );
        match peer_rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                // Write at EACH deadline: the first write may precede
                // SRTP establishment, and a single lost packet would make
                // a test whose answer is "yes" fail.
                if let Some(writer) = peer_rtc.writer(mid_micro) {
                    let pt = writer
                        .payload_params()
                        .find(|p| p.spec().codec == str0m::format::Codec::Opus)
                        .map(|p| p.pt());
                    if let Some(pt) = pt {
                        let temps = MediaTime::new(horodatage, Frequency::FORTY_EIGHT_KHZ);
                        if peer_rtc
                            .writer(mid_micro)
                            .unwrap()
                            .write(pt, Instant::now(), temps, charge.clone())
                            .is_ok()
                        {
                            horodatage += 480;
                        }
                    }
                }
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(Duration::from_millis(5));
                fixtures::poll_peer_socket(
                    &mut peer_rtc,
                    &peer_socket,
                    peer_addr,
                    maintenant,
                    attente,
                );
            }
            Output::Transmit(t) => {
                let _ = peer_socket.send_to(&t.contents, t.destination);
            }
            Output::Event(_) => {}
        }
    }

    let recues = recues.lock().unwrap();
    let trame = &recues[0];
    eprintln!(
        "mic track: {} bytes, rtp_48k={}, samples={}",
        trame.opus.len(),
        trame.rtp_48k,
        trame.echantillons
    );
    assert_eq!(
        trame.opus, charge,
        "the payload did not pass through byte for byte"
    );
    assert_eq!(
        trame.echantillons,
        crate::opus::FRAME_SAMPLES,
        "the duration was not READ from the packet"
    );
}

// ── Block E3: the exclusivity refusal is TOLD to the browser ───────────────
//
// 🔴 All these tests go through `deposer_trame_micro_de_test`, which **DELEGATES**
// to the production path. The header of `piste_micro.rs` documents why:
// a `#[cfg(test)]` entry point that COPIED the logic would turn
// green mutations that must turn red — it happened in E1, task 8.

/// The exclusivity verdicts actually queued, in order.
fn verdicts_annonces(s: &Session) -> Vec<bool> {
    s.pending_control
        .iter()
        .filter_map(|m| match m {
            proto::control::AgentControl::MicState { granted, .. } => Some(*granted),
            _ => None,
        })
        .collect()
}

fn trame_muette() -> TrameMicro {
    TrameMicro {
        opus: vec![0xF8, 0x00],
        rtp_48k: 0,
        echantillons: 960,
    }
}

/// A sink whose answer is driven from outside, to play a RESUMPTION.
struct PuitsPilotable {
    accepte: Arc<Mutex<bool>>,
}

impl PuitsMicro for PuitsPilotable {
    fn deposer(&mut self, _trame: TrameMicro) -> bool {
        *self.accepte.lock().unwrap()
    }
}

/// The first deposit announces its verdict — `None` counts as a transition.
///
/// Without that, a window that loses the cable from its first packet
/// would NEVER learn anything: `Ready.mic` has already been emitted, and it says `true`.
#[test]
fn the_very_first_deposit_announces_its_verdict_to_the_browser() {
    for accepte in [true, false] {
        let mut s = session_nue();
        s.set_puits_micro(Box::new(PuitsEspion {
            recues: Arc::new(Mutex::new(Vec::new())),
            accepte,
        }));
        assert!(
            verdicts_annonces(&s).is_empty(),
            "nothing before the first deposit"
        );
        s.deposer_trame_micro_de_test(trame_muette());
        assert_eq!(verdicts_annonces(&s), vec![accepte]);
    }
}

/// 🔴 ON TRANSITION, never at each deposit. It is red R3.
///
/// The microphone deposits a frame every 20 ms. Announcing at each deposit
/// would put fifty messages per second into a queue bounded at 32
/// (`PLAFOND_CONTROLE_EN_FILE`), which would overflow in less than a second and
/// drown the cursor, rumble and clipboard.
///
/// **Fifty deposits, ONE message** — and the count is of messages
/// ACTUALLY queued, not of a call counter: that is what makes it
/// able to fail.
#[test]
fn fifty_deposits_of_the_same_verdict_make_only_one_announcement() {
    for accepte in [true, false] {
        let mut s = session_nue();
        s.set_puits_micro(Box::new(PuitsEspion {
            recues: Arc::new(Mutex::new(Vec::new())),
            accepte,
        }));
        for _ in 0..50 {
            s.deposer_trame_micro_de_test(trame_muette());
        }
        assert_eq!(
            verdicts_annonces(&s),
            vec![accepte],
            "fifty deposits of verdict {accepte} produced {} announcements",
            verdicts_annonces(&s).len()
        );
    }
}

/// 🔴 After a refusal is LIFTED, the client is INFORMED AGAIN. It is red R4.
///
/// ❌ This test would not exist if this module's original comment had
/// told the truth: it asserted that the refusal is "a PERMANENT condition — another
/// window holds the cable for the life of its process". **Decision 2
/// of block E2 refuted it** by making the acquisition attempt NON-STICKY:
/// a released cable is taken back at the next deposit. An announcement that only followed
/// the first transition would then leave the exclusivity banner displayed
/// forever on a window that has taken the microphone back.
#[test]
fn a_lifted_refusal_is_announced_again_to_the_browser() {
    let accepte = Arc::new(Mutex::new(false));
    let mut s = session_nue();
    s.set_puits_micro(Box::new(PuitsPilotable {
        accepte: accepte.clone(),
    }));

    for _ in 0..5 {
        s.deposer_trame_micro_de_test(trame_muette());
    }
    assert_eq!(
        verdicts_annonces(&s),
        vec![false],
        "the initial refusal, once"
    );

    // The other window dies, the cable is given back.
    *accepte.lock().unwrap() = true;
    for _ in 0..5 {
        s.deposer_trame_micro_de_test(trame_muette());
    }
    assert_eq!(
        verdicts_annonces(&s),
        vec![false, true],
        "the recovery must be announced, and only once"
    );

    // And the reverse direction too: the verdict follows transitions in BOTH
    // directions, which an "already announced" flag would not do.
    *accepte.lock().unwrap() = false;
    s.deposer_trame_micro_de_test(trame_muette());
    assert_eq!(verdicts_annonces(&s), vec![false, true, false]);
}

/// The LOG, for its part, stays single, and it is a distinct property.
///
/// Two flags, two roles: `refus_micro_signale` bounds the log to one
/// line for the whole session; `exclusivite_annoncee` follows transitions.
/// Confusing them would give either fifty log lines per second, or
/// a client banner that is never lifted.
#[test]
fn the_recovery_does_not_produce_a_second_log_line() {
    let accepte = Arc::new(Mutex::new(false));
    let mut s = session_nue();
    s.set_puits_micro(Box::new(PuitsPilotable {
        accepte: accepte.clone(),
    }));

    s.deposer_trame_micro_de_test(trame_muette());
    let apres_refus = s.journaux_micro;
    *accepte.lock().unwrap() = true;
    s.deposer_trame_micro_de_test(trame_muette());
    *accepte.lock().unwrap() = false;
    s.deposer_trame_micro_de_test(trame_muette());

    assert_eq!(apres_refus, 1, "the refusal is logged once");
    assert_eq!(s.journaux_micro, 1, "two more transitions add no log line");
}
