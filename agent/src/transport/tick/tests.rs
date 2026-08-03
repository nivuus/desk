//! Tests de `act_on_timeout` — fichier voisin plutôt que module en ligne :
//! `tick.rs` a franchi 500 lignes en ajoutant la couverture des branches
//! a1bis/a1ter (tâche 8, sous-bloc D5). Même schéma d'extraction que
//! `capteur/distante.rs` → `capteur/distante/tests.rs`.

use str0m::format::Codec;
use str0m::{Event, Output};

use super::*;
use crate::audio::{AudioPacket, AudioSource};
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::transport::fixtures;

/// Source factice qui enregistre les appels à `set_awake` et rend une
/// annonce de sommeil préparée une seule fois — comme le fait réellement
/// `SourceDistante` (`Option` consommé par `take()`), sans dépendre du
/// capteur : ce test vérifie le câblage des branches a1bis/a1ter
/// d'`act_on_timeout`, pas la logique de `SourceDistante` elle-même
/// (couverte par `capteur/distante/tests.rs`).
struct SourceAvecSommeil {
    inner: crate::source::FileSource,
    awake_recus: std::sync::Arc<std::sync::Mutex<Vec<(bool, bool)>>>,
    sommeil_prepare: Option<(bool, String)>,
}

impl VideoSource for SourceAvecSommeil {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn set_awake(&mut self, visible: bool, focalisee: bool) -> anyhow::Result<()> {
        self.awake_recus.lock().unwrap().push((visible, focalisee));
        Ok(())
    }
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil_prepare.take()
    }
}

/// Ferme la réserve ouverte par la tâche 8 : le test du brief ne couvre
/// que la MÉMORISATION (`dispatch_controle_de_test` → `pending_visibility`
/// dans `evenements.rs`). Ce test-ci couvre l'APPLICATION, symétrique à
/// `un_redimensionnement_recalibre_le_controleur_sur_la_taille_obtenue`
/// dans `redimensionnement.rs` pour `pending_resize`.
#[test]
fn une_visibilite_en_attente_est_appliquee_a_la_source_puis_relachee() {
    let inner = fixtures::video_test_source();
    let awake_recus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let source = Box::new(SourceAvecSommeil {
        inner,
        awake_recus: awake_recus.clone(),
        sommeil_prepare: None,
    });
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.pending_visibility = Some((false, true));
    session
        .act_on_timeout(Instant::now())
        .expect("appliquer une visibilité ne doit jamais faire échouer la session");

    assert_eq!(
        *awake_recus.lock().unwrap(),
        vec![(false, true)],
        "set_awake doit avoir reçu exactement la visibilité mémorisée"
    );
    assert_eq!(
        session.pending_visibility, None,
        "la demande appliquée ne doit pas rester en attente indéfiniment"
    );
}

/// Un changement de sommeil rendu par la source doit être traduit en
/// `AgentControl::Asleep` mis en file pour le navigateur — et une seule
/// fois, puisque `sommeil_a_annoncer` consomme (voir son commentaire sur
/// le trait `VideoSource`).
#[test]
fn un_changement_de_sommeil_de_la_source_est_traduit_en_message_asleep() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceAvecSommeil {
        inner,
        awake_recus: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        sommeil_prepare: Some((true, "masquee".to_string())),
    });
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("annoncer un sommeil ne doit jamais faire échouer la session");

    assert!(
        session.pending_control.iter().any(|message| matches!(
            message,
            proto::control::AgentControl::Asleep { asleep: true, reason, .. }
                if reason == "masquee"
        )),
        "le message Asleep attendu n'est pas en file : {:?}",
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

/// Source factice qui rend une part de budget préparée une seule fois —
/// comme le fait réellement `SourceDistante` (`Option` consommé par
/// `take()`), sans dépendre du capteur : ce test vérifie le CÂBLAGE de la
/// branche a1quater d'`act_on_timeout`, pas la logique de `SourceDistante`
/// elle-même (couverte par `capteur/distante/tests.rs`) ni celle de
/// `Session::appliquer_part` (couverte par `transport/part.rs`).
struct SourceAvecPart {
    inner: crate::source::FileSource,
    part_preparee: Option<u32>,
}

impl VideoSource for SourceAvecPart {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part_preparee.take()
    }
}

/// Ferme la réserve relevée en revue de la tâche 8 (sous-bloc D6) : les
/// tests de `transport/part.rs` appellent `Session::appliquer_part`
/// directement, si bien que supprimer tout le bloc a1quater d'
/// `act_on_timeout` les laissait verts — seule la disparition d'un
/// avertissement `dead_code` aurait trahi l'absence de câblage. Ce test-ci
/// pilote `act_on_timeout` à travers une source factice, comme
/// `une_visibilite_en_attente_est_appliquee_a_la_source_puis_relachee` et
/// `un_changement_de_sommeil_de_la_source_est_traduit_en_message_asleep`
/// ci-dessus le font pour leurs branches respectives.
#[test]
fn une_part_en_attente_est_appliquee_par_act_on_timeout() {
    let inner = fixtures::video_test_source();
    let source = Box::new(SourceAvecPart { inner, part_preparee: Some(3_000_000) });
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session
        .act_on_timeout(Instant::now())
        .expect("appliquer une part ne doit jamais faire échouer la session");

    assert_eq!(
        session.congestion.courant().video_bitrate_bps, 3_000_000,
        "la part rendue par la source doit avoir été appliquée au contrôleur par la branche a1quater"
    );
    assert!(
        session.pending_decision.is_some(),
        "la décision issue de la part doit être mémorisée pour que a0ter l'applique au tour suivant"
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
fn atteint_la_cadence_video_visee_avec_un_pair_local() {
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
    let (offer, pending) = api.apply().expect("offre non vide");

    let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
    let answer = SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP valide");
    peer_rtc
        .sdp_api()
        .accept_answer(pending, answer)
        .expect("réponse acceptée par le pair");

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
            panic!("le pair local ne s'est jamais connecté dans le délai imparti");
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
                let wait = t.saturating_duration_since(now).min(cap.saturating_duration_since(now));
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
        "cadence mesurée : {video_count} images vidéo et {audio_count} paquets audio reçus en {measure_window:?} ({per_second:.1} images/s)"
    );

    // Preuve de C1 : au rythme voulu (~60 Hz), on attend nettement plus
    // de 10 images par seconde. Le bug de cadence corrigé n'en aurait
    // produit qu'environ une par seconde — le seuil ci-dessous exclut
    // sans ambiguïté ce rythme tout en restant robuste à une machine de
    // test lente ou une CI chargée.
    assert!(
        per_second > 10.0,
        "cadence trop basse : {per_second:.1} images/s (attendu très supérieur à 1/s, la marque du bug de cadence C1)"
    );

    // Preuve de la tâche 8 (ronde de correction 1) : une `Session` munie
    // d'une source audio (`set_audio_source`, plus haut) doit
    // effectivement émettre des paquets Opus que le pair reçoit — pas
    // seulement négocier la piste. Sans la branche `a3` d'`act_on_timeout`
    // (celle qui appelle `write_audio`), ce compteur resterait à zéro :
    // constaté en la retirant temporairement (voir le rapport de tâche).
    assert!(
        audio_count > 0,
        "aucun paquet audio reçu par le pair : la Session, munie d'une source audio, \
         n'a émis aucun paquet Opus (la branche a3 d'act_on_timeout est-elle bien avant b, \
         ou write_audio échoue-t-il silencieusement ?)"
    );
}
