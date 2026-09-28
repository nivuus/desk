use str0m::format::Codec;
use str0m::{Event, Output};

use super::*;
use crate::audio::{AudioPacket, AudioSource};
use crate::transport::fixtures;

/// Ferme la réserve ouverte par la tâche 8 : le test du brief ne couvre
/// que la MÉMORISATION (`dispatch_controle_de_test` → `pending_visibility`
/// dans `evenements.rs`). Ce test-ci couvre l'APPLICATION, symétrique à
/// `a_resize_recalibrates_the_controller_on_the_obtained_size`
/// dans `redimensionnement.rs` pour `pending_resize`.
#[test]
fn a_pending_visibility_is_applied_to_the_source_then_released() {
    let inner = fixtures::video_test_source();
    let awake_recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let source = Box::new(SourceWithSleep {
        inner,
        awake_recus: awake_recus.clone(),
        sommeil_prepare: None,
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session.pending_visibility = Some((false, true));
    session
        .act_on_timeout(Instant::now())
        .expect("applying a visibility must never make the session fail");

    assert_eq!(
        *awake_recus.lock().unwrap(),
        vec![(false, true)],
        "set_awake must have received exactly the memorised visibility"
    );
    assert_eq!(
        session.pending_visibility, None,
        "the applied request must not stay pending forever"
    );
}

/// Un changement de sommeil rendu par la source doit être traduit en
/// `AgentControl::Asleep` mis en file pour le navigateur — et une seule
/// fois, puisque `sommeil_a_annoncer` consomme (voir son commentaire sur
/// le trait `VideoSource`).
#[test]
fn a_source_sleep_change_is_translated_into_an_asleep_message() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithSleep {
        inner,
        awake_recus: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        sommeil_prepare: Some((true, "masquee".to_string())),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("announcing sleep must never make the session fail");

    assert!(
        session.pending_control.iter().any(|message| matches!(
            message,
            proto::control::AgentControl::Asleep { asleep: true, reason, .. }
                if reason == "masquee"
        )),
        "the expected Asleep message is not queued: {:?}",
        session.pending_control
    );

    // `sommeil_prepare` était un `Option` pris par `take()` : la source
    // elle-même n'a donc plus rien à rendre à un second appel — c'est ce
    // qui, en production, empêche la réémission (couvert au niveau de
    // `SourceDistante` par `capteur/distante/tests.rs`, dont le contrat
    // de consommation est identique). Un second appel à `act_on_timeout`
    // n'est pas rejoué ici : il ferait retomber la liste de priorités sur
    // des branches sans rapport (vidéo, attente socket) qui n'ont rien à
    // voir avec ce que ce test vérifie.
}

/// Ferme la réserve relevée en revue de la tâche 8 (sous-bloc D6) : les
/// tests de `transport/part.rs` appellent `Session::appliquer_part`
/// directement, si bien que supprimer tout le bloc a1quater d'
/// `act_on_timeout` les laissait verts — seule la disparition d'un
/// avertissement `dead_code` aurait trahi l'absence de câblage. Ce test-ci
/// pilote `act_on_timeout` à travers une source factice, comme
/// `a_pending_visibility_is_applied_to_the_source_then_released` et
/// `a_source_sleep_change_is_translated_into_an_asleep_message`
/// ci-dessus le font pour leurs branches respectives.
#[test]
fn a_pending_share_is_applied_by_act_on_timeout() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithShare {
        inner,
        part_preparee: Some(3_000_000),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("applying a share must never make the session fail");

    assert_eq!(
        session.congestion.current().video_bitrate_bps, 3_000_000,
        "the share returned by the source must have been applied to the controller by branch a1quater"
    );
    assert!(
        session.pending_decision.is_some(),
        "the decision stemming from the share must be memorised so that a0ter applies it at the next round"
    );
}

/// Preuve d'intégration pour C1 (cadence) et C2 (drainage) : les tests
/// ci-dessus valident les fonctions pures, mais la revue demandait une
/// mesure réelle de cadence. Sans navigateur disponible, on simule le
/// pair offrant avec un second `Rtc` str0m en loopback UDP — exactement
/// la forme que le brief attribue au navigateur (piste vidéo recvonly et
/// deux canaux de données). `Session::run` tourne sur un thread dédié,
/// comme en production, pendant que ce test pilote le pair et compte les
/// images vidéo reçues sur une fenêtre fixe après connexion.
///
/// Avant le correctif de C1, l'attente entre deux images valait jusqu'à
/// l'échéance que réclame `Rtc` (jusqu'à 1 s, imposée par les timers
/// RTCP/statistiques) au lieu d'être bornée par `next_frame_at` : ce test
/// aurait alors mesuré environ 1 image/s au lieu de ~60.
#[test]
fn reaches_the_target_video_cadence_with_a_local_peer() {
    use std::thread;
    use str0m::change::SdpAnswer;
    use str0m::media::{Direction, MediaKind};

    /// Source audio de test : rend un paquet toutes les 10 ms au plus
    /// tôt, avec un `pts_48k` qui avance de 480 (une trame de 10 ms) à
    /// chaque paquet rendu — comme le ferait `WindowsAudioSource`
    /// (`PacketRing` alimenté par un fil de capture cadencé, jamais
    /// disponible en continu). La charge utile n'a pas besoin d'être un
    /// Opus valide : ce test vérifie que la `Session` ACHEMINE les
    /// paquets jusqu'au pair, pas ce qu'un décodeur en ferait.
    ///
    /// **Constaté pendant l'écriture de ce test (ronde de correction
    /// 1)** : une première version rendait un paquet à CHAQUE appel, sans
    /// pacage. La branche `a3` passant avant la branche `b` (par
    /// construction, voir plus haut), un flux audio en continu
    /// affamait totalement la vidéo — `video_count` retombait à 0 sur
    /// toute la fenêtre de mesure. Ce n'est pas un défaut de la source
    /// réelle (`PacketRing`, bornée à 10 paquets et alimentée par un fil
    /// séparé au rythme de la capture, ne peut pas rendre en continu),
    /// mais un artefact d'une source de test irréaliste. Le pacage à
    /// 10 ms ci-dessous restaure un comportement fidèle à
    /// `WindowsAudioSource` : la plupart des appels à `next_packet`
    /// rendent `None`, exactement comme en production.
    struct DummyAudioSource {
        next_pts_48k: u64,
        next_due: Instant,
    }

    impl AudioSource for DummyAudioSource {
        fn next_packet(&mut self) -> Option<AudioPacket> {
            let now = Instant::now();
            if now < self.next_due {
                return None;
            }
            self.next_due += Duration::from_millis(10);
            let pts_48k = self.next_pts_48k;
            self.next_pts_48k += 480;
            Some(AudioPacket {
                data: vec![0xF8, 0xFF, 0xFE],
                pts_48k,
                captured_at: now,
            })
        }
    }

    let local_ip = fixtures::local_ip();
    let source = Box::new(fixtures::video_test_source());

    let mut session = Session::new(source, local_ip, Instant::now(), 12_000_000).expect("session");

    // Pair « navigateur » minimal : un second `Rtc`, offrant, avec une
    // piste vidéo recvonly et les deux canaux de données.
    let (peer_socket, peer_addr, mut peer_rtc) = fixtures::local_peer(local_ip, true);

    let mut api = peer_rtc.sdp_api();
    api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
    // Non lié à une variable lue plus loin : le décompte plus bas
    // distingue audio et vidéo par codec, pas par `mid` (voir plus bas).
    api.add_media(MediaKind::Audio, Direction::RecvOnly, None, None, None);
    api.add_channel("control".to_string());
    api.add_channel("input".to_string());
    let (offer, pending) = api.apply().expect("non-empty offer");

    let answer_sdp = session
        .accept_offer(&offer.to_sdp_string())
        .expect("offer accepted");
    let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("valid SDP answer");
    peer_rtc
        .sdp_api()
        .accept_answer(pending, answer)
        .expect("answer accepted by the peer");

    // Ronde de correction 1 (revue) : la première version de ce test
    // faisait écrire le PAIR lui-même sur `audio_mid`, ce qui ne passait
    // jamais par `Session::write_audio` ni par la branche `a3` — la
    // suppression pure et simple de cette branche aurait laissé ce test
    // vert (constaté, voir le rapport de tâche). Ce qui doit réellement
    // être prouvé : une `Session` munie d'une source audio
    // (`set_audio_source`) ÉMET des paquets Opus que le pair reçoit. Le
    // décompte, plus bas, distingue les paquets audio des paquets vidéo
    // par leur codec (`Codec::Opus` vs `Codec::H264`), pas par leur
    // `mid` : `audio_mid` n'a donc plus besoin d'être lu après la
    // négociation SDP.
    session.set_audio_source(Box::new(DummyAudioSource {
        next_pts_48k: 0,
        next_due: Instant::now(),
    }));

    // La session tourne sur un thread dédié, comme en production (voir
    // `demarrage.rs` / `tokio::task::spawn_blocking`). Le thread n'est pas
    // rejoint : `Session::run` ne se termine qu'à la détection d'une
    // déconnexion ICE (délai de plusieurs secondes), ce qui ralentirait
    // ce test sans rien y ajouter. Le processus de test se termine de
    // toute façon en fin de suite ; le thread ne fuit pas au-delà.
    thread::spawn(move || {
        let mut on_input = |_| {};
        let mut on_control = |_| {};
        let _ = session.run(&mut on_input, &mut on_control);
    });

    // Boucle du pair : pilote son propre `Rtc` (STUN, ACKs DTLS...) et
    // compte les images vidéo ET les paquets audio reçus pendant une
    // fenêtre fixe démarrée à la connexion (pas avant : le temps de
    // poignée de main ICE/DTLS ne doit pas être compté contre la cadence
    // mesurée).
    //
    // Ronde de correction 1 (revue) : `media_count` comptait auparavant
    // tout `Event::MediaData` sous le nom d'« images vidéo ». Une fois la
    // source audio de test posée sur `Session` (ci-dessus), l'assertion
    // de cadence vidéo aurait aussi compté des paquets audio et serait
    // devenue fausse (silencieusement, sans jamais échouer pour la
    // mauvaise raison qu'un décompte trop haut). Les deux compteurs sont
    // désormais séparés par codec (`data.params.spec().codec`), pas par
    // `mid` — un paquet Opus reste un paquet Opus quel que soit le `mid`
    // qui le porte.
    let hard_deadline = Instant::now() + Duration::from_secs(10);
    let measure_window = Duration::from_secs(2);
    let mut connected_at: Option<Instant> = None;
    let mut video_count = 0usize;
    let mut audio_count = 0usize;

    loop {
        let now = Instant::now();
        if now >= hard_deadline {
            panic!("the local peer never connected within the allotted delay");
        }
        if let Some(connected_at) = connected_at {
            if now >= connected_at + measure_window {
                break;
            }
        }

        match peer_rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                let cap = match connected_at {
                    Some(c) => hard_deadline.min(c + measure_window),
                    None => hard_deadline,
                };
                let wait = t
                    .saturating_duration_since(now)
                    .min(cap.saturating_duration_since(now));
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
                    match data.params.spec().codec {
                        Codec::Opus => audio_count += 1,
                        Codec::H264 => video_count += 1,
                        _ => {}
                    }
                }
            }
            Output::Event(_) => {}
        }
    }

    let per_second = video_count as f64 / measure_window.as_secs_f64();
    eprintln!(
        "measured cadence: {video_count} video frames and {audio_count} audio packets received in {measure_window:?} ({per_second:.1} frames/s)"
    );

    // Preuve de C1 : au rythme voulu (~60 Hz), on attend nettement plus
    // de 10 images par seconde. Le bug de cadence corrigé n'en aurait
    // produit qu'environ une par seconde — le seuil ci-dessous exclut
    // sans ambiguïté ce rythme tout en restant robuste à une machine de
    // test lente ou une CI chargée.
    assert!(
        per_second > 10.0,
        "cadence too low: {per_second:.1} frames/s (expected far above 1/s, the mark of cadence bug C1)"
    );

    // Preuve de la tâche 8 (ronde de correction 1) : une `Session` munie
    // d'une source audio (`set_audio_source`, plus haut) doit
    // effectivement émettre des paquets Opus que le pair reçoit — pas
    // seulement négocier la piste. Sans la branche `a3` d'`act_on_timeout`
    // (celle qui appelle `write_audio`), ce compteur resterait à zéro :
    // constaté en la retirant temporairement (voir le rapport de tâche).
    assert!(
        audio_count > 0,
        "no audio packet received by the peer: the Session, equipped with an audio source, \
         emitted no Opus packet (is branch a3 of act_on_timeout really before b, \
         or does write_audio fail silently?)"
    );
}

/// Branche a1septies (sous-bloc P1) : un presse-papier rendu par la source
/// doit être traduit en `AgentControl::Clipboard` mis en file pour le
/// navigateur. Même patron que
/// `a_source_sleep_change_is_translated_into_an_asleep_message` et
/// `a_pending_share_is_applied_by_act_on_timeout` ci-dessus : la
/// source factice consomme son annonce, exactement comme `SourceDistante`.
#[test]
fn a_source_clipboard_is_translated_into_a_clipboard_message() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithClipboard {
        inner,
        presse_papier_prepare: Some((Some("bonjour".to_string()), 7)),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("announcing a clipboard must never make the session fail");

    assert!(
        session.pending_control.iter().any(|message| matches!(
            message,
            proto::control::AgentControl::Clipboard { text: Some(texte), bytes: 7, .. }
                if texte == "bonjour"
        )),
        "the expected Clipboard message is not queued: {:?}",
        session.pending_control
    );
}

/// Le REFUS de taille (D-P1-1) doit traverser la même branche : `text` à
/// `None` et `bytes` portant la taille refusée. Sans cela, le bandeau du
/// navigateur ne saurait jamais qu'une copie a été refusée — et un refus
/// silencieux est exactement ce que la spécification interdit.
#[test]
fn a_size_refusal_is_translated_into_a_clipboard_message_without_text() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithClipboard {
        inner,
        presse_papier_prepare: Some((None, 100_000)),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("announcing a refusal must never make the session fail");

    assert!(
        session.pending_control.iter().any(|message| matches!(
            message,
            proto::control::AgentControl::Clipboard {
                text: None,
                bytes: 100_000,
                ..
            }
        )),
        "the expected refusal is not queued: {:?}",
        session.pending_control
    );
}

/// Branche a1nonies (sous-bloc A1) : une couleur d'accent rendue par la source
/// doit être traduite en `AgentControl::Accent` mis en file pour le navigateur.
///
/// 🔴 **ROUGE sur l'arbre d'avant la branche** : le maillon 6 du plan est
/// « NON gardé par le compilateur — un `if let` oublié compile ». Sans ces deux
/// tests, retirer tout le bloc a1nonies laisserait `cargo test` VERT.
#[test]
fn a_source_accent_is_translated_into_an_accent_message() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithAccent {
        inner,
        accent_prepare: Some("#7aa2f7".to_string()),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("announcing an accent must never make the session fail");

    assert!(
        session.pending_control.iter().any(|message| matches!(
            message,
            proto::control::AgentControl::Accent { couleur, .. } if couleur == "#7aa2f7"
        )),
        "the expected Accent message is not queued: {:?}",
        session.pending_control
    );
}

/// L'annonce est CONSOMMÉE : un second tour ne la remet pas en file.
///
/// 🔴 **ROUGE si `accent_a_annoncer` LISAIT sans consommer** : la branche
/// émettrait alors un message PAR TOUR, soit ~100 Hz sur le canal de contrôle,
/// et le critère ④ de la recette — « exactement une ligne par session sur un
/// palier de 60 s » — deviendrait indémontrable. C'est le régime que ce fichier
/// documente déjà pour a1ter-bis et a1septies.
#[test]
fn the_announced_accent_is_consumed_and_not_resent_next_round() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceWithAccent {
        inner,
        accent_prepare: Some("#fa8c16".to_string()),
    });
    let mut session =
        Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000).expect("session");

    session.act_on_timeout(Instant::now()).expect("first round");
    let apres_le_premier = session
        .pending_control
        .iter()
        .filter(|message| matches!(message, proto::control::AgentControl::Accent { .. }))
        .count();
    assert_eq!(
        apres_le_premier, 1,
        "the first round must queue EXACTLY one announcement"
    );

    // Dix tours de plus : la source n'a plus rien à annoncer.
    for _ in 0..10 {
        session.act_on_timeout(Instant::now()).expect("next round");
    }
    let total = session
        .pending_control
        .iter()
        .filter(|message| matches!(message, proto::control::AgentControl::Accent { .. }))
        .count();
    assert_eq!(
        total, 1,
        "eleven rounds produced only ONE announcement: the accent is consumed, never read again"
    );
}
