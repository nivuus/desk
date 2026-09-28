//! The tests of `evenements.rs`, moved into their own file under
//! the 500-line rule: the module crossed the ceiling by gaining the
//! non-regression net for the TWO audio m-lines (workstream E, block E1,
//! task 7), and the repository's doctrine requires EXTRACTING, never compressing
//! a comment to get back under the line.
//!
//! Declared at the parent through `#[path]` — the use explicitly OUTSIDE
//! `CLAUDE.md`'s "Child module convention", which only targets modules
//! moved out of a `#[cfg(windows)]` parent to compile them on the host. Here
//! the only reason is size, and the precedent is `superviseur/table.rs`.

use std::time::Duration;

use anyhow::Result;
use str0m::Output;

use super::*;
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::transport::fixtures;
use str0m::media::{Direction, MediaKind, Mid};

/// The tests of `memoriser_controle`, extracted BEFORE this file
/// crossed 500 (sub-block P2) — see their header comment.
///
/// ⚠️ **The `#[path]` is MANDATORY here, and it is not a style choice**:
/// this module is itself declared through `#[path]` from `evenements.rs`, and
/// rustc then resolves its children in the PARENT file's directory
/// (`evenements/`), not in a directory bearing its name. A bare `mod
/// memorisation;` would look for `evenements/memorisation.rs`. That is what
/// distinguishes this case from `tick/tests.rs`, whose parent uses an ordinary
/// `mod` and whose children therefore do land in `tick/tests/`.
#[path = "tests/memorisation.rs"]
mod memorisation;

/// Integration proof that `Event::KeyframeRequest` (emitted by str0m
/// when the peer sends an RTCP PLI/FIR — which a browser does after
/// a packet loss detected by its decoder) is indeed relayed down to
/// `VideoSource::request_keyframe`, without going through a mock of the
/// `Event` trait: the local peer here is a real second str0m `Rtc`, as in
/// `atteint_la_cadence_video_visee_avec_un_pair_local`.
///
/// Does NOT exercise the real `WindowsSource`/`H264Encoder::request_keyframe`
/// path (`#![cfg(windows)]`, unavailable on the Linux build
/// machine): only the `handle_event` → `Session::source` relay is proven
/// here. The `WindowsSource::request_keyframe` →
/// `H264Encoder::request_keyframe` wiring (`SetValue` on
/// `CODECAPI_AVEncVideoForceKeyFrame`) stays checked by reading and by
/// Windows cross-compilation, not by an automated test.
#[test]
fn relaie_une_demande_d_image_cle_du_pair_vers_la_source() {
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;
    use std::thread;
    use str0m::change::SdpAnswer;
    use str0m::media::{Direction, KeyframeRequestKind, MediaKind};

    /// Wraps `FileSource` counting the calls to
    /// `request_keyframe`, the only way to observe from this test that the
    /// relay did happen (the counter is shared through `Arc` before
    /// the source is moved into `Session`, which then owns it
    /// from `Session::run`'s dedicated thread).
    struct CountingSource {
        inner: crate::source::FileSource,
        keyframe_requests: Arc<AtomicUsize>,
    }

    impl VideoSource for CountingSource {
        fn next_frame(&mut self) -> Option<AccessUnit> {
            self.inner.next_frame()
        }
        fn dimensions(&self) -> (u32, u32) {
            self.inner.dimensions()
        }
        fn request_keyframe(&mut self) -> Result<()> {
            self.keyframe_requests.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }
    }

    let local_ip = fixtures::local_ip();
    let keyframe_requests = Arc::new(AtomicUsize::new(0));
    let source = Box::new(CountingSource {
        inner: fixtures::video_test_source(),
        keyframe_requests: keyframe_requests.clone(),
    });

    let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

    let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

    let mut api = peer_rtc.sdp_api();
    // Recvonly on the peer side == the video track the browser actually receives
    // from the agent; it is on this `mid` that `writer(...)` will emit
    // the PLI below (str0m names this access "writer" regardless of the
    // media's direction — it is the API through which RTCP feedback goes out).
    let video_mid = api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
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

    let hard_deadline = Instant::now() + Duration::from_secs(10);
    let mut keyframe_requested_at_peer = false;

    loop {
        let now = Instant::now();
        if keyframe_requests.load(AtomicOrdering::SeqCst) > 0 {
            break; // Preuve faite : le relais a atteint la source.
        }
        if now >= hard_deadline {
            panic!(
                "délai dépassé : le pair local ne s'est jamais connecté, ou \
                 Event::KeyframeRequest n'a jamais atteint VideoSource::request_keyframe \
                 (compteur toujours à 0)"
            );
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
                if !keyframe_requested_at_peer {
                    keyframe_requested_at_peer = true;
                    // Exactly what a browser does after a
                    // packet loss detected by its decoder: request
                    // a keyframe through an RTCP PLI. `fb_pli` is true by
                    // default for a video codec in str0m (see
                    // `format::payload_params::PayloadParams::new`), so
                    // this negotiation has nothing special to enable on the
                    // SDP offer/answer side.
                    let mut writer = peer_rtc.writer(video_mid).expect("writer vidéo");
                    writer
                        .request_keyframe(None, KeyframeRequestKind::Pli)
                        .expect("PLI négocié par défaut sur un codec vidéo (fb_pli)");
                }
            }
            Output::Event(_) => {}
        }
    }

    assert!(
        keyframe_requests.load(AtomicOrdering::SeqCst) > 0,
        "Event::KeyframeRequest du pair n'a jamais atteint VideoSource::request_keyframe"
    );
}

/// Builds a bare session and hands it synthetic `MediaAdded` events.
///
/// `MediaAdded` carries fields that are all public (`str0m::media::MediaAdded`),
/// which lets us exercise the discrimination directly, without a full SDP
/// negotiation. The fact that str0m indeed returns the LOCAL direction is
/// established by MEASUREMENT: probe 1 (`transport::sonde_montante`) sees the
/// receiver announce `RecvOnly` for a track offered as `SendOnly`.
fn session_avec_pistes(pistes: &[(MediaKind, Direction)]) -> Session {
    let source = Box::new(fixtures::video_test_source());
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");
    for (i, &(kind, direction)) in pistes.iter().enumerate() {
        let mid: Mid = format!("m{i}").as_str().into();
        session.handle_event(
            Event::MediaAdded(str0m::media::MediaAdded {
                mid,
                kind,
                direction,
                simulcast: None,
            }),
            &mut |_| {},
            &mut |_| {},
        );
    }
    session
}

/// `evenements.rs` set `audio_mid` for ANY audio track, whatever
/// its direction. With two audio m-lines — workstream A's
/// (agent → browser) and the microphone's (browser → agent) —, the
/// second OVERWROTE the first, and the downstream sound went out on a
/// `recvonly` track, that is, nowhere. **SILENT, without a single `WARN`.**
///
/// This test is a non-regression net on a defect that ALREADY existed,
/// and it was seen red on the code from before.
#[test]
fn une_piste_audio_recvonly_ne_devient_jamais_la_piste_de_sortie() {
    let s = session_avec_pistes(&[
        (MediaKind::Video, Direction::RecvOnly),
        (MediaKind::Audio, Direction::SendOnly),
        (MediaKind::Audio, Direction::RecvOnly),
    ]);
    assert_eq!(
        s.audio_mid,
        Some("m1".into()),
        "la piste audio RECVONLY (le micro) a écrasé la piste de sortie du chantier A"
    );
    assert_eq!(
        s.mic_mid,
        Some("m2".into()),
        "la piste du micro n'a pas été retenue"
    );
}

/// The same fact taken in reverse order: the microphone negotiated BEFORE the
/// downstream sound. The order of m-lines is not under our control — it is
/// the browser that offers.
#[test]
fn une_piste_audio_sendonly_reste_la_piste_de_sortie_meme_apres_le_micro() {
    let s = session_avec_pistes(&[
        (MediaKind::Audio, Direction::RecvOnly),
        (MediaKind::Audio, Direction::SendOnly),
    ]);
    assert_eq!(s.audio_mid, Some("m1".into()));
    assert_eq!(s.mic_mid, Some("m0".into()));
}

/// `SendRecv` stays an EMISSION track for us: it is what a peer
/// that does not distinguish the two directions would negotiate. Filing it on
/// the microphone side would cut the downstream sound.
#[test]
fn une_piste_audio_sendrecv_est_une_piste_de_sortie() {
    let s = session_avec_pistes(&[(MediaKind::Audio, Direction::SendRecv)]);
    assert_eq!(s.audio_mid, Some("m0".into()));
    assert_eq!(s.mic_mid, None);
}

/// `Inactive` is NEITHER one NOR the other. Without this arm, a track switched off
/// by the peer would take the place of a live track.
#[test]
fn une_piste_audio_inactive_n_est_retenue_nulle_part() {
    let s = session_avec_pistes(&[
        (MediaKind::Audio, Direction::SendOnly),
        (MediaKind::Audio, Direction::Inactive),
    ]);
    assert_eq!(s.audio_mid, Some("m0".into()));
    assert_eq!(s.mic_mid, None);
}

/* ═══════════════════════════════════════════════════════════════════════════
DATA CHANNEL ROUTING — sub-block F1, task 17.

🔴 THE DEFECT FIXED, AND WHY IT IS NOT LEFT AS LEGACY. `dispatch_channel_data`
routed on the `data.binary` flag ALONE: any binary frame, whatever
its channel, went into `InputMessage::decode`. The label was nevertheless
available in `Event::ChannelOpen(id, label)` and simply unused —
only `"control"` was recognised there, to store its `ChannelId`.

The file bridge's dedicated `PeerConnection` (decision D4) makes this defect
moot FOR F1: the `fichiers` channel lives in another `PeerConnection`, in
another process. But it does not CLOSE it — it remains whole in
each child's `PeerConnection`, dormant because today only `input`
is binary there, and it is the exact trap the first person who
judges the "third channel" route cheap enough will fall into. Its only symptom
would be an "invalid input message" `WARN` per frame.

It is therefore fixed, for three reasons of which the third decides: it is
TESTABLE ON THE HOST, so the check can be seen red; it costs about
ten lines; and this repository paid FOUR times for the catch-all arm of
`capteur/pont_media.rs` (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
`PleinEcran`) to learn that a router that does not name its cases
pays for it at each new message.
═══════════════════════════════════════════════════════════════════════════ */

/// The routing decision, exercised with a STAND-IN identifier.
///
/// 🔴 IT IS THIS TEST THAT COVERS "a frame arrived BEFORE its channel's
/// `ChannelOpen`", a case no local-peer setup can produce: str0m always
/// emits `ChannelOpen` before the first `ChannelData`. It is nevertheless
/// the INITIAL state of every session, where both fields are `None` — and it
/// is only testable because `destination` is generic over its
/// identifier, `str0m::channel::ChannelId` being deliberately unconstructible
/// outside str0m's crate.
#[test]
fn une_trame_arrivee_avant_tout_channel_open_est_refusee() {
    // L'état initial : aucun canal n'est encore nommé.
    assert_eq!(destination(1u8, None, None), Destination::Ignoree);
    // Le transitoire réel : `control` s'ouvre, `input` pas encore. Une trame
    // d'`input` reçue ici est refusée, PAS prise pour du contrôle.
    assert_eq!(destination(1u8, None, Some(2u8)), Destination::Ignoree);
}

#[test]
fn l_aiguillage_nomme_ses_deux_canaux_et_refuse_les_autres() {
    assert_eq!(destination(1u8, Some(1), Some(2)), Destination::Entree);
    assert_eq!(destination(2u8, Some(1), Some(2)), Destination::Controle);
    // 🔴 LE CANAL TIERS : hier décodé comme une entrée souris dès qu'il était
    // binaire, aujourd'hui refusé.
    assert_eq!(destination(3u8, Some(1), Some(2)), Destination::Ignoree);
}

/// 🔴 LE TEST DE NON-RÉGRESSION, ET C'EST LE PLUS IMPORTANT DES TROIS : une
/// trame binaire du canal `input` doit TOUJOURS atteindre `on_input`. Un
/// aiguillage plus strict qui casserait l'entrée serait pire que le défaut
/// qu'il corrige.
///
/// 🔴 ET LE ROUGE DU SECOND : un canal binaire PARASITE est négocié à côté
/// d'`input`, et on écrit dessus. Sur le code d'avant cette tâche, `on_input`
/// est appelé DEUX fois. **Sans ce second canal, le test serait vacueux** — il
/// ne vérifierait que ce que le code faisait déjà.
#[test]
fn une_trame_binaire_du_canal_input_atteint_on_input_et_un_canal_inconnu_est_refuse() {
    use proto::input::InputMessage;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use str0m::change::SdpAnswer;

    // Deux messages DISTINGUABLES : lequel arrive compte autant que combien.
    let attendu = InputMessage::MouseMove { x: 1111, y: 2222 };
    let parasite = InputMessage::MouseMove { x: 9999, y: 8888 };

    let local_ip = fixtures::local_ip();
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

    let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, false);

    let mut api = peer_rtc.sdp_api();
    api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
    api.add_channel("control".to_string());
    let canal_input = api.add_channel("input".to_string());
    // 🔴 LE CANAL PARASITE. Il n'a pas besoin d'exister dans le produit : il
    // exhibe qu'un canal binaire QUELCONQUE était décodé comme une entrée.
    let canal_parasite = api.add_channel("parasite".to_string());
    let (offer, pending) = api.apply().expect("offre non vide");

    let answer_sdp = session
        .accept_offer(&offer.to_sdp_string())
        .expect("offre acceptée");
    let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
    peer_rtc
        .sdp_api()
        .accept_answer(pending, answer)
        .expect("réponse acceptée par le pair");

    let recus = Arc::new(Mutex::new(Vec::<InputMessage>::new()));
    let recus_fil = recus.clone();
    let fini = Arc::new(AtomicBool::new(false));
    let fini_fil = fini.clone();
    thread::spawn(move || {
        let mut on_input = |m: InputMessage| {
            recus_fil.lock().expect("verrou").push(m);
        };
        let mut on_control = |_| {};
        let _ = session.run(&mut on_input, &mut on_control);
        fini_fil.store(true, AtomicOrdering::SeqCst);
    });

    let butoir = Instant::now() + Duration::from_secs(10);
    let mut ecrit = false;
    // ⚠️ `Event::Connected` NE SUFFIT PAS : il marque la fin de la poignée de
    // main DTLS/ICE, alors que les canaux SCTP s'ouvrent APRÈS. Écrire à ce
    // moment-là fait rendre `None` à `peer_rtc.channel(...)` — ce qui, dans un
    // test, se lit comme une panne du produit alors que c'est le protocole du
    // test qui est en avance. On attend donc les `ChannelOpen` des DEUX canaux.
    let mut ouverts: Vec<str0m::channel::ChannelId> = Vec::new();
    // Une fois le message attendu reçu, on continue de pomper un peu : le
    // parasite voyage sur un AUTRE flux SCTP, donc son ordre d'arrivée n'est
    // pas garanti par celui de l'écriture. Sans ce répit, « pas encore arrivé »
    // se lirait comme « n'arrivera jamais ».
    let mut repit: Option<Instant> = None;

    loop {
        let maintenant = Instant::now();
        if let Some(fin) = repit {
            if maintenant >= fin {
                break;
            }
        }
        if maintenant >= butoir {
            panic!(
                "délai dépassé : le pair local ne s'est jamais connecté, ou la trame du \
                 canal `input` n'a jamais atteint on_input (reçus : {:?})",
                recus.lock().expect("verrou")
            );
        }
        if repit.is_none() && recus.lock().expect("verrou").contains(&attendu) {
            repit = Some(maintenant + Duration::from_millis(400));
        }

        match peer_rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(Duration::from_millis(50));
                if fixtures::poll_peer_socket(
                    &mut peer_rtc,
                    &peer_socket,
                    peer_addr,
                    maintenant,
                    attente,
                ) {
                    continue;
                }
            }
            Output::Transmit(t) => {
                let _ = peer_socket.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::ChannelOpen(id, _)) => {
                if !ouverts.contains(&id) {
                    ouverts.push(id);
                }
                if !ecrit && ouverts.contains(&canal_input) && ouverts.contains(&canal_parasite) {
                    ecrit = true;
                    // Le parasite EN PREMIER : s'il devait passer, il aurait
                    // toute l'avance.
                    peer_rtc
                        .channel(canal_parasite)
                        .expect("canal parasite ouvert")
                        .write(true, &parasite.encode())
                        .expect("écriture sur le canal parasite");
                    peer_rtc
                        .channel(canal_input)
                        .expect("canal input ouvert")
                        .write(true, &attendu.encode())
                        .expect("écriture sur le canal input");
                }
            }
            Output::Event(_) => {}
        }
    }

    let recus = recus.lock().expect("verrou").clone();
    assert!(
        recus.contains(&attendu),
        "la trame du canal `input` n'a pas atteint on_input : {recus:?}"
    );
    assert!(
        !recus.contains(&parasite),
        "🔴 une trame binaire d'un canal INCONNU a été décodée comme une entrée : {recus:?}"
    );
    assert_eq!(
        recus.len(),
        1,
        "une seule entrée attendue, reçues : {recus:?}"
    );
}
