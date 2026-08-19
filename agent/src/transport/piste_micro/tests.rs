//! Les tests de `piste_micro.rs`.
//!
//! Deux familles, et elles répondent à des questions différentes :
//! les tests unitaires exercent la garde d'exclusivité et les avertissements
//! uniques sur une `Session` nue ; le test d'intégration fait traverser un
//! vrai paquet Opus à travers un vrai `Rtc` str0m jusqu'au puits.
//!
//! ⚠️ La sonde 1 (`transport::sonde_montante`) répondait sur str0m NU. Le test
//! d'intégration d'ici répond sur NOTRE chemin. Les deux coexistent : le
//! premier dit ce que fait la bibliothèque, le second ce que fait l'agent.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use str0m::media::{Direction, Frequency, MediaKind, MediaTime};
use str0m::{Event, Output};

use super::*;
use crate::micro::{PuitsMicro, TrameMicro};
use crate::transport::fixtures;

/// Puits qui enregistre ce qu'on lui donne, et accepte ou refuse à volonté.
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

/// Spec §10 : « piste montante non négociée → aucun paquet attendu,
/// avertissement UNIQUE » — calqué sur `warn_audio_negotiation_once`.
///
/// Le compte porte sur les lignes RÉELLEMENT émises (`journaux_micro` n'est
/// incrémenté qu'au moment du `tracing::warn!`), pas sur le nombre d'appels :
/// c'est ce qui rend l'assertion capable de tomber.
#[test]
fn sans_piste_micro_negociee_l_avertissement_ne_sort_qu_une_fois() {
    let mut s = session_nue();
    assert!(!s.micro_disponible());
    for _ in 0..50 {
        s.avertir_micro_une_fois("essai");
    }
    assert_eq!(
        s.journaux_micro, 1,
        "l'avertissement de négociation est sorti {} fois",
        s.journaux_micro
    );
}

/// Spec §9 : un second flux montant est refusé, journalisé UNE fois, et sa
/// piste ignorée. **Le refus vient du PUITS** (`deposer` rend `false`) : le
/// transport ne connaît aucun mutex, et c'est la couture que E2 remplira.
#[test]
fn un_puits_qui_refuse_ne_fait_journaliser_qu_une_fois_et_ne_tue_rien() {
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

    assert_eq!(recues.lock().unwrap().len(), 50, "les trames n'ont pas atteint le puits");
    assert_eq!(
        s.journaux_micro, 1,
        "le refus a été journalisé {} fois au lieu d'une",
        s.journaux_micro
    );
    // …et la session n'est pas en train de se terminer : un micro refusé ne
    // compromet rien.
    assert!(!s.ending);
}

/// Le micro ne tue JAMAIS une session qui fonctionne (spec §10).
///
/// La propriété est en partie STRUCTURELLE — `deposer_micro` rend `()`, donc
/// ne peut rien propager — et ce test vérifie la partie qui, elle, pourrait
/// changer : l'état de la session est intact après un puits qui refuse tout,
/// y compris la piste vidéo, qui est ce qu'on protège.
#[test]
fn un_micro_refusant_ne_compromet_pas_la_video() {
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
    assert_eq!(s.video_mid, Some("v0".into()), "la piste vidéo a été perdue");
    assert!(!s.ending, "la session s'est terminée à cause du micro");
}

/// La sonde 1 répondait sur str0m nu. Celui-ci répond sur NOTRE chemin : un
/// paquet Opus écrit par le pair est retrouvé dans le puits de la session,
/// avec la durée LUE du paquet et l'horodatage RTP du pair (spec §11).
#[test]
fn un_paquet_opus_montant_atteint_le_puits_de_la_session() {
    use crate::opus::OpusEncoder;

    // Une vraie trame Opus de 10 ms : c'est elle qui donne son sens à
    // `echantillons`, qu'une charge utile arbitraire rendrait illisible.
    let mut enc = OpusEncoder::new().expect("encodeur");
    let pcm: Vec<i16> = (0..crate::opus::FRAME_INTERLEAVED)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect();
    let charge = enc.encode(&pcm).expect("encodage");

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
    // Le micro : le NAVIGATEUR émet, donc l'agent reçoit.
    let mid_micro = api.add_media(MediaKind::Audio, Direction::SendOnly, None, None, None);
    let (offer, pending) = api.apply().expect("offre non vide");

    let answer_sdp = session
        .accept_offer(&offer.to_sdp_string())
        .expect("offre acceptée");
    let answer = str0m::change::SdpAnswer::from_sdp_string(&answer_sdp).expect("réponse SDP");
    peer_rtc
        .sdp_api()
        .accept_answer(pending, answer)
        .expect("réponse acceptée");

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
            "aucune trame micro n'a atteint le puits en 15 s"
        );
        match peer_rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                // Écrire à CHAQUE échéance : la première écriture peut précéder
                // l'établissement SRTP, et un unique paquet perdu ferait échouer
                // un test dont la réponse est « oui ».
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
                fixtures::poll_peer_socket(&mut peer_rtc, &peer_socket, peer_addr, maintenant, attente);
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
        "piste micro : {} octets, rtp_48k={}, echantillons={}",
        trame.opus.len(),
        trame.rtp_48k,
        trame.echantillons
    );
    assert_eq!(trame.opus, charge, "la charge utile n'a pas traversé octet pour octet");
    assert_eq!(
        trame.echantillons,
        crate::opus::FRAME_SAMPLES,
        "la durée n'a pas été LUE du paquet"
    );
}
