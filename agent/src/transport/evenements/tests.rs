//! Les tests d'`evenements.rs`, sortis dans leur propre fichier au titre de
//! la règle des 500 lignes : le module a franchi le plafond en gagnant le
//! filet de non-régression des DEUX m-lines audio (chantier E, bloc E1,
//! tâche 7), et la doctrine du dépôt impose d'EXTRAIRE, jamais de compresser
//! un commentaire pour repasser sous la ligne.
//!
//! Déclaré chez le parent par `#[path]` — l'usage explicitement HORS de la
//! « Convention de module enfant » de `CLAUDE.md`, qui ne vise que les modules
//! qu'on sort d'un parent `#[cfg(windows)]` pour les compiler sur l'hôte. Ici
//! le seul motif est la taille, et le précédent est `superviseur/table.rs`.

use std::time::Duration;

use anyhow::Result;
use str0m::Output;

use super::*;
use crate::h264::AccessUnit;
use crate::source::VideoSource;
use crate::transport::fixtures;
use str0m::media::{Direction, MediaKind, Mid};

/// `ClientControl::Visibility` reçu doit être mémorisé dans
/// `pending_visibility`, pas appliqué sur-le-champ.
///
/// Même raison que pour `Resize` : ce code court pendant le drainage de
/// `poll_output`, et relâcher un encodeur y romprait l'invariant d'une
/// seule mutation de `Rtc` par appel. `dispatch_controle_de_test` est un
/// point d'entrée `#[cfg(test)]` qui court-circuite `ChannelData` (str0m
/// interdit délibérément sa construction hors du crate) tout en exerçant
/// exactement le même chemin de mémorisation que `dispatch_channel_data`.
#[test]
fn un_message_de_visibilite_est_memorise_et_non_applique_sur_le_champ() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    let json = r#"{"type":"visibility","v":3,"visible":false,"focused":false}"#;
    session.dispatch_controle_de_test(json);

    assert_eq!(session.pending_visibility, Some((false, false)));
}

/// `ClientControl::Clipboard` reçu doit être mémorisé dans
/// `pending_clipboard`, pas appliqué sur-le-champ — même raison que ses deux
/// voisins.
///
/// ROUGE si le bras posait le mauvais champ. Le bras ABSENT, lui, ne compile
/// pas : le `match` de `memoriser_controle` est exhaustif, et c'est le
/// compilateur qui l'a exigé au moment où la variante est née.
#[test]
fn un_message_de_collage_est_memorise_et_non_applique_sur_le_champ() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"bonjour"}"#);

    assert_eq!(session.pending_clipboard.as_deref(), Some("bonjour"));
    // Mémoriser n'injecte RIEN : l'écriture et l'injection vivent dans
    // `act_on_timeout`, hors du drainage de `poll_output`.
    assert!(!session.collage_a_injecter);
}

/// 🔴 **C'est la décision D-P2-3 rendue vérifiable, et c'est le seul endroit
/// où elle l'est.** Deux collages entre deux tours de boucle se réduisent au
/// SECOND ; le premier est perdu sans trace.
///
/// ROUGE si le champ accumulait (une file, un `Vec`) : le test lirait alors le
/// premier ou les deux. Ce test ne dit pas que l'écrasement est bon — il dit
/// que c'est bien ce que le produit fait, et la doc du champ en porte le coût.
#[test]
fn deux_collages_successifs_ne_laissent_que_le_second() {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");

    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"premier"}"#);
    session.dispatch_controle_de_test(r#"{"type":"clipboard","v":3,"text":"second"}"#);

    assert_eq!(session.pending_clipboard.as_deref(), Some("second"));
}

/// Preuve d'intégration que `Event::KeyframeRequest` (émis par str0m
/// quand le pair envoie un PLI/FIR RTCP — ce que fait un navigateur après
/// une perte de paquet détectée par son décodeur) est bien relayé jusqu'à
/// `VideoSource::request_keyframe`, sans passer par un mock du trait
/// `Event` : le pair local ici est un vrai second `Rtc` str0m, comme dans
/// `atteint_la_cadence_video_visee_avec_un_pair_local`.
///
/// N'exerce PAS le chemin `WindowsSource`/`H264Encoder::request_keyframe`
/// réel (`#![cfg(windows)]`, indisponible sur la machine de compilation
/// Linux) : seul le relais `handle_event` → `Session::source` est prouvé
/// ici. Le câblage `WindowsSource::request_keyframe` →
/// `H264Encoder::request_keyframe` (`SetValue` sur
/// `CODECAPI_AVEncVideoForceKeyFrame`) reste vérifié par lecture et par
/// la compilation croisée Windows, pas par un test automatisé.
#[test]
fn relaie_une_demande_d_image_cle_du_pair_vers_la_source() {
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;
    use std::thread;
    use str0m::change::SdpAnswer;
    use str0m::media::{Direction, KeyframeRequestKind, MediaKind};

    /// Enveloppe `FileSource` en comptant les appels à
    /// `request_keyframe`, seule façon d'observer depuis ce test que le
    /// relais a bien eu lieu (le compteur est partagé via `Arc` avant que
    /// la source ne soit déplacée dans `Session`, qui la possède ensuite
    /// depuis le thread dédié de `Session::run`).
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
    // Recvonly côté pair == la piste vidéo que le navigateur reçoit
    // réellement de l'agent ; c'est sur ce `mid` que `writer(...)` émettra
    // le PLI plus bas (str0m nomme cet accès « writer » indépendamment du
    // sens du média — c'est l'API par laquelle la rétroaction RTCP sort).
    let video_mid = api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
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
                    // Exactement ce que fait un navigateur après une
                    // perte de paquet détectée par son décodeur : demander
                    // une image clé via un PLI RTCP. `fb_pli` est vrai par
                    // défaut pour un codec vidéo dans str0m (voir
                    // `format::payload_params::PayloadParams::new`), donc
                    // cette négociation n'a rien de spécial à activer côté
                    // offre/réponse SDP.
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

/// Construit une session nue et lui remet des `MediaAdded` synthétiques.
///
/// `MediaAdded` porte des champs tous publics (`str0m::media::MediaAdded`),
/// ce qui permet d'exercer la discrimination directement, sans négociation
/// SDP complète. Le fait que str0m rende bien la direction LOCALE, lui, est
/// établi par la MESURE : la sonde 1 (`transport::sonde_montante`) voit le
/// récepteur annoncer `RecvOnly` pour une piste offerte en `SendOnly`.
fn session_avec_pistes(pistes: &[(MediaKind, Direction)]) -> Session {
    let source = Box::new(fixtures::video_test_source());
    let mut session = Session::new(source, fixtures::local_ip(), Instant::now(), 12_000_000)
        .expect("session");
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

/// `evenements.rs` posait `audio_mid` pour TOUTE piste audio, quelle que
/// soit sa direction. Avec deux m-lines audio — celle du chantier A
/// (agent → navigateur) et celle du micro (navigateur → agent) —, la
/// seconde ÉCRASAIT la première, et le son descendant partait sur une
/// piste `recvonly`, c'est-à-dire nulle part. **MUET, sans un `WARN`.**
///
/// Ce test est un filet de non-régression sur un défaut qui existait DÉJÀ,
/// et il a été vu rouge sur le code d'avant.
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

/// Le même fait pris dans l'ordre inverse : le micro négocié AVANT le son
/// descendant. L'ordre des m-lines n'est pas sous notre contrôle — c'est
/// le navigateur qui offre.
#[test]
fn une_piste_audio_sendonly_reste_la_piste_de_sortie_meme_apres_le_micro() {
    let s = session_avec_pistes(&[
        (MediaKind::Audio, Direction::RecvOnly),
        (MediaKind::Audio, Direction::SendOnly),
    ]);
    assert_eq!(s.audio_mid, Some("m1".into()));
    assert_eq!(s.mic_mid, Some("m0".into()));
}

/// `SendRecv` reste une piste d'ÉMISSION pour nous : c'est ce que
/// négocierait un pair qui ne distingue pas les deux sens. La ranger du
/// côté du micro couperait le son descendant.
#[test]
fn une_piste_audio_sendrecv_est_une_piste_de_sortie() {
    let s = session_avec_pistes(&[(MediaKind::Audio, Direction::SendRecv)]);
    assert_eq!(s.audio_mid, Some("m0".into()));
    assert_eq!(s.mic_mid, None);
}

/// `Inactive` n'est NI l'une NI l'autre. Sans ce bras, une piste éteinte
/// par le pair prendrait la place d'une piste vivante.
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
   L'AIGUILLAGE DES CANAUX DE DONNÉES — sous-bloc F1, tâche 17.

   🔴 LE DÉFAUT CORRIGÉ, ET POURQUOI IL N'EST PAS LÉGUÉ. `dispatch_channel_data`
   aiguillait sur le SEUL drapeau `data.binary` : toute trame binaire, quel que
   soit son canal, partait dans `InputMessage::decode`. Le label était pourtant
   disponible dans `Event::ChannelOpen(id, label)` et simplement inutilisé —
   seul `"control"` y était reconnu, pour mémoriser son `ChannelId`.

   La `PeerConnection` dédiée du pont fichiers (décision D4) rend ce défaut sans
   objet POUR F1 : le canal `fichiers` vit dans une autre `PeerConnection`, dans
   un autre processus. Mais elle ne le REFERME pas — il reste entier dans la
   `PeerConnection` de chaque enfant, dormant parce qu'aujourd'hui seul `input`
   y est binaire, et c'est le piège exact où tombera la première personne qui
   jugera la voie du « troisième canal » assez bon marché. Son seul symptôme
   serait un `WARN "message d'entrée invalide"` par trame.

   Il est donc corrigé, pour trois raisons dont la troisième décide : il est
   TESTABLE SUR L'HÔTE, donc le contrôle peut être vu rouge ; il coûte une
   dizaine de lignes ; et ce dépôt a payé QUATRE fois le bras catch-all de
   `capteur/pont_media.rs` (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
   `PleinEcran`) pour apprendre qu'un aiguillage qui ne nomme pas ses cas se
   paie à chaque message neuf.
   ═══════════════════════════════════════════════════════════════════════════ */

/// La décision d'aiguillage, éprouvée avec un identifiant de STAND-IN.
///
/// 🔴 C'EST CE TEST QUI COUVRE « une trame arrivée AVANT le `ChannelOpen` de
/// son canal », cas qu'aucun montage à pair local ne peut produire : str0m émet
/// toujours `ChannelOpen` avant la première `ChannelData`. C'est pourtant
/// l'état INITIAL de toute session, où les deux champs valent `None` — et il
/// n'est éprouvable que parce que `destination` est générique sur son
/// identifiant, `str0m::channel::ChannelId` étant délibérément inconstructible
/// hors du crate de str0m.
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

    let answer_sdp = session.accept_offer(&offer.to_sdp_string()).expect("offre acceptée");
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
        if repit.is_none() && recus.lock().expect("verrou").iter().any(|m| *m == attendu) {
            repit = Some(maintenant + Duration::from_millis(400));
        }

        match peer_rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(Duration::from_millis(50));
                if fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, maintenant, attente)
                {
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
    assert_eq!(recus.len(), 1, "une seule entrée attendue, reçues : {recus:?}");
}
