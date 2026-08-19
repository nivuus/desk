//! Tests d'intégration du transport du pont, sur un vrai pair str0m en boucle
//! locale.
//!
//! ⚠️ **Échafaudage LOCAL, et c'est délibéré** : `transport::fixtures` est
//! `pub(super)` — `pont/` ne peut pas l'employer. Le hisser toucherait le
//! chemin vidéo pour un besoin de test, ce qui est exactement le genre
//! d'échange que ce dépôt refuse. L'échafaudage ci-dessous est donc minimal :
//! un second `Rtc` sur loopback qui crée ses canaux et échange l'offre.

use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use str0m::change::SdpAnswer;
use str0m::channel::ChannelId;
use str0m::net::{DatagramRecv, Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

use super::*;

/// Budget dur d'un test. Généreux : une poignée de main DTLS+ICE en boucle
/// locale prend quelques dizaines de millisecondes, mais un échec doit rendre
/// un message parlant plutôt qu'un blocage.
const BUDGET: Duration = Duration::from_secs(10);

/// Le pair « navigateur » : son socket, son adresse, son `Rtc`, et les
/// identifiants des canaux qu'il a créés.
struct Pair {
    socket: UdpSocket,
    adresse: SocketAddr,
    rtc: Rtc,
    canaux: Vec<ChannelId>,
}

/// Monte un pont et un pair, négocie `labels` comme canaux de données, et rend
/// le pair prêt ainsi que les deux bouts de la boucle du pont.
///
/// **Aucun codec n'est activé d'aucun côté** — c'est le point que la doc de
/// `construire_rtc_donnees` annonce éprouvé : `clear_codecs()` sans le moindre
/// `enable_*` négocie bien une connexion de données seules.
fn monter(labels: &[&str]) -> (Pair, Sender<VersNavigateur>, Receiver<DuNavigateur>) {
    let local_ip = "127.0.0.1".parse().unwrap();
    let (socket_pont, mut rtc_pont) = construire_rtc_donnees(local_ip).expect("pont construit");

    let socket = UdpSocket::bind(SocketAddr::new(local_ip, 0)).expect("socket du pair");
    let adresse = socket.local_addr().unwrap();
    let mut rtc = Rtc::builder().clear_codecs().build(Instant::now());
    rtc.add_local_candidate(Candidate::host(adresse, "udp").unwrap());

    // C'est le NAVIGATEUR qui crée les canaux : le pont est répondant.
    let mut api = rtc.sdp_api();
    let canaux: Vec<ChannelId> = labels.iter().map(|l| api.add_channel((*l).to_string())).collect();
    let (offre, en_attente) = api.apply().expect("offre non vide");

    let reponse = rtc_pont
        .sdp_api()
        .accept_offer(offre)
        .expect("le pont accepte une offre de données seules");
    let reponse = SdpAnswer::from_sdp_string(&reponse.to_sdp_string()).expect("réponse SDP valide");
    rtc.sdp_api().accept_answer(en_attente, reponse).expect("réponse acceptée");

    let (tx_sortant, rx_sortant) = channel();
    let (tx_entrant, rx_entrant) = channel();
    thread::spawn(move || {
        let _ = tourner(rtc_pont, socket_pont, rx_sortant, tx_entrant);
    });

    (Pair { socket, adresse, rtc, canaux }, tx_sortant, rx_entrant)
}

/// **LE** pilote de ces tests : il pompe le pair ET récolte ce que le pont
/// fait remonter, dans la MÊME boucle.
///
/// ⚠️ Les deux ne peuvent pas être séparés, et une première rédaction l'a
/// appris à ses dépens : attendre un message sur `entrant` dans une fonction à
/// part BLOQUAIT le seul fil qui pompe le pair, donc ICE et DTLS n'aboutissaient
/// jamais, donc le canal ne s'ouvrait jamais. Les quatre tests échouaient à la
/// même ligne avec le même message — la signature d'un défaut d'échafaudage, et
/// non de quatre défauts de produit.
///
/// `agir` court à chaque tour, avec le `Rtc` du pair et tout ce qui a remonté
/// jusque-là. `fini` décide de l'arrêt. Rend ce qui a remonté du pont, et ce
/// que le pair a reçu sur ses canaux.
fn echanger(
    pair: &mut Pair,
    entrant: &Receiver<DuNavigateur>,
    mut agir: impl FnMut(&mut Rtc, &[DuNavigateur]),
    mut fini: impl FnMut(&[DuNavigateur], &[(ChannelId, bool, Vec<u8>)]) -> bool,
    quoi: &str,
) -> (Vec<DuNavigateur>, Vec<(ChannelId, bool, Vec<u8>)>) {
    let limite = Instant::now() + BUDGET;
    let mut remontees: Vec<DuNavigateur> = Vec::new();
    let mut recus: Vec<(ChannelId, bool, Vec<u8>)> = Vec::new();
    loop {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        if fini(&remontees, &recus) {
            return (remontees, recus);
        }
        let maintenant = Instant::now();
        assert!(
            maintenant < limite,
            "budget dépassé sans {quoi} (remontées : {remontees:?}, reçues : {})",
            recus.len()
        );
        agir(&mut pair.rtc, &remontees);
        match pair.rtc.poll_output().expect("poll_output du pair") {
            Output::Timeout(t) => {
                let attente = t
                    .saturating_duration_since(maintenant)
                    .min(limite.saturating_duration_since(maintenant))
                    // Bornée court : le fil du pont peut avoir quelque chose à
                    // nous dire à tout instant, et `entrant` n'est relu qu'en
                    // haut de ce tour.
                    .min(Duration::from_millis(5));
                if attente.is_zero() {
                    let _ = pair.rtc.handle_input(Input::Timeout(maintenant));
                    continue;
                }
                pair.socket.set_read_timeout(Some(attente)).unwrap();
                let mut tampon = vec![0u8; 2000];
                match pair.socket.recv_from(&mut tampon) {
                    Ok((n, source)) => {
                        if let Ok(contenu) = DatagramRecv::try_from(&tampon[..n]) {
                            let recu = Receive {
                                proto: Protocol::Udp,
                                source,
                                destination: pair.adresse,
                                contents: contenu,
                            };
                            let _ = pair.rtc.handle_input(Input::Receive(Instant::now(), recu));
                        }
                    }
                    Err(_) => {
                        let _ = pair.rtc.handle_input(Input::Timeout(Instant::now()));
                    }
                }
            }
            Output::Transmit(t) => {
                let _ = pair.socket.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::ChannelData(data)) => {
                recus.push((data.id, data.binary, data.data.clone()));
            }
            Output::Event(_) => {}
        }
    }
}

/// Vrai dès que le pont a annoncé son canal ouvert.
fn canal_ouvert(remontees: &[DuNavigateur]) -> bool {
    remontees.contains(&DuNavigateur::CanalOuvert)
}

#[test]
fn une_trame_emise_par_le_pont_arrive_au_pair_en_binaire() {
    let (mut pair, sortant, entrant) = monter(&[LABEL_FICHIERS]);

    let trame = proto::fichiers::encoder(
        proto::fichiers::TYPE_LIRE,
        0x1234_5678,
        r#"{"chemin":"a.txt"}"#,
        &[0x00, 0xFF],
    );
    let a_emettre = trame.clone();
    let mut emise = false;
    let (_, recus) = echanger(
        &mut pair,
        &entrant,
        |_, remontees| {
            // Émettre SEULEMENT une fois le canal annoncé ouvert : émettre
            // avant ferait perdre la requête, et le test ne mesurerait plus
            // que sa propre course.
            if !emise && canal_ouvert(remontees) {
                emise = true;
                sortant
                    .send(VersNavigateur::Requete {
                        correlation: 0x1234_5678,
                        trame: a_emettre.clone(),
                    })
                    .unwrap();
            }
        },
        |_, recus| !recus.is_empty(),
        "que la trame du pont n'arrive au pair",
    );

    let (_, binaire, octets) = recus.first().expect("une trame reçue").clone();
    // ⚠️ `binary = true`, à l'inverse du canal `control` qui écrit `false` : la
    // charge est faite d'octets bruts, et le mode texte la ferait passer par
    // une validation UTF-8 côté navigateur — 0x00 et 0xFF n'y survivraient pas.
    assert!(binaire, "la trame doit être écrite en BINAIRE");
    assert_eq!(octets, trame, "la trame doit arriver octet pour octet");
}

#[test]
fn une_trame_du_pair_remonte_avec_sa_correlation() {
    let (mut pair, _sortant, entrant) = monter(&[LABEL_FICHIERS]);
    let reponse =
        proto::fichiers::encoder(proto::fichiers::TYPE_DONNEES, 0x0BAD_F00D, "{}", &[1, 2, 3, 4]);

    let canal = pair.canaux[0];
    let a_envoyer = reponse.clone();
    let mut envoye = false;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !envoye && canal_ouvert(remontees) {
                if let Some(mut c) = rtc.channel(canal) {
                    envoye = c.write(true, &a_envoyer).is_ok();
                }
            }
        },
        |remontees, _| remontees.iter().any(|m| matches!(m, DuNavigateur::Reponse { .. })),
        "que la réponse du pair ne remonte",
    );

    let reponse_recue = remontees
        .iter()
        .find_map(|m| match m {
            DuNavigateur::Reponse { correlation, trame } => Some((*correlation, trame.clone())),
            _ => None,
        })
        .expect("une réponse remontée");
    assert_eq!(reponse_recue.0, 0x0BAD_F00D, "la corrélation doit traverser intacte");
    assert_eq!(reponse_recue.1, reponse, "la trame doit remonter octet pour octet");
}

#[test]
fn un_channeldata_venu_d_un_autre_canal_est_refuse_et_journalise() {
    // ⚠️ Ce test ne peut se voir ROUGE que si l'on négocie DEUX canaux : sans
    // le second, il n'y a rien à envoyer sur le mauvais, et le test serait
    // VACUEUX — il passerait sur un pont qui n'aiguille sur rien du tout.
    let (mut pair, _sortant, entrant) = monter(&["autre-canal", LABEL_FICHIERS]);
    let intrus = proto::fichiers::encoder(proto::fichiers::TYPE_ECHEC, 0xDEAD_0000, "{}", b"non");
    let legitime = proto::fichiers::encoder(proto::fichiers::TYPE_META, 0x0000_BEEF, "{}", b"oui");

    let (mauvais, bon) = (pair.canaux[0], pair.canaux[1]);
    let (a, b) = (intrus.clone(), legitime.clone());
    let mut etape = 0u8;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !canal_ouvert(remontees) {
                return;
            }
            // L'INTRUS D'ABORD, la trame légitime ENSUITE. Le canal étant
            // fiable et ordonné, si la légitime remonte alors que l'intrus
            // n'est jamais apparu, c'est que l'intrus a été JETÉ — et non pas
            // seulement qu'il n'est pas encore arrivé. Sans cet ordre, le test
            // ne prouverait qu'une absence dans une fenêtre de temps.
            if etape == 0 {
                if let Some(mut c) = rtc.channel(mauvais) {
                    if c.write(true, &a).is_ok() {
                        etape = 1;
                    }
                }
            } else if etape == 1 {
                if let Some(mut c) = rtc.channel(bon) {
                    if c.write(true, &b).is_ok() {
                        etape = 2;
                    }
                }
            }
        },
        |remontees, _| remontees.iter().any(|m| matches!(m, DuNavigateur::Reponse { .. })),
        "que la trame légitime ne remonte",
    );

    let correlations: Vec<u32> = remontees
        .iter()
        .filter_map(|m| match m {
            DuNavigateur::Reponse { correlation, .. } => Some(*correlation),
            _ => None,
        })
        .collect();
    assert_eq!(
        correlations,
        vec![0x0000_BEEF],
        "seule la trame du canal `fichiers` doit remonter : la présence de \
         0xDEAD0000 signifierait que le mauvais canal a été traité"
    );
    // …et l'intrus n'arrive pas non plus après.
    assert!(
        entrant.recv_timeout(Duration::from_millis(200)).is_err(),
        "aucune autre trame ne doit remonter : l'intrus a été jeté"
    );
}

#[test]
fn un_seul_canal_est_retenu_parmi_deux_et_c_est_celui_du_label() {
    // 🔴 **LE test de l'aiguillage par label, et il est né d'une MUTATION
    // SURVIVANTE.** Remplacer `if label == LABEL_FICHIERS` par `if true`
    // laissait les quatre premiers tests VERTS : celui de l'intrus négocie
    // `autre-canal` puis `fichiers`, et comme le dernier `ChannelOpen` écrase
    // le précédent, `canal` finissait quand même sur le bon — par l'ORDRE
    // d'ouverture, pas par le label. Le test ne mesurait pas ce qu'il
    // annonçait.
    //
    // Celui-ci ne dépend d'aucun ordre : sur DEUX canaux négociés, le pont ne
    // doit annoncer QU'UNE seule ouverture. Sans le filtre sur le label, il en
    // annonce deux, quel que soit l'ordre dans lequel elles arrivent.
    let (mut pair, _sortant, entrant) = monter(&[LABEL_FICHIERS, "autre-canal"]);

    // On attend que les DEUX canaux soient ouverts CÔTÉ PAIR — sinon on
    // conclurait « une seule ouverture » alors que la seconde n'est pas encore
    // arrivée, et le test redeviendrait vacueux.
    //
    // ⚠️ Une première rédaction confiait ce comptage à `echanger`, dont la
    // condition d'arrêt lisait un compteur qu'il n'incrémente jamais — un
    // contrôle incapable de RÉUSSIR, exactement le pendant du contrôle
    // incapable d'échouer. `echanger` ne remonte pas les `ChannelOpen` du
    // pair ; ce test pompe donc lui-même.
    let mut ouverts = 0usize;
    let limite = Instant::now() + BUDGET;
    let mut remontees: Vec<DuNavigateur> = Vec::new();
    while ouverts < 2 && Instant::now() < limite {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        match pair.rtc.poll_output().expect("poll_output du pair") {
            Output::Transmit(t) => {
                let _ = pair.socket.send_to(&t.contents, t.destination);
            }
            Output::Event(Event::ChannelOpen(..)) => ouverts += 1,
            Output::Event(_) => {}
            Output::Timeout(_) => {
                pair.socket.set_read_timeout(Some(Duration::from_millis(5))).unwrap();
                let mut tampon = vec![0u8; 2000];
                match pair.socket.recv_from(&mut tampon) {
                    Ok((n, source)) => {
                        if let Ok(contenu) = DatagramRecv::try_from(&tampon[..n]) {
                            let recu = Receive {
                                proto: Protocol::Udp,
                                source,
                                destination: pair.adresse,
                                contents: contenu,
                            };
                            let _ = pair.rtc.handle_input(Input::Receive(Instant::now(), recu));
                        }
                    }
                    Err(_) => {
                        let _ = pair.rtc.handle_input(Input::Timeout(Instant::now()));
                    }
                }
            }
        }
    }
    assert_eq!(ouverts, 2, "le pair doit avoir ouvert ses DEUX canaux");

    // Laisser au pont le temps d'annoncer une éventuelle seconde ouverture.
    let fin = Instant::now() + Duration::from_millis(300);
    while Instant::now() < fin {
        while let Ok(m) = entrant.try_recv() {
            remontees.push(m);
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let ouvertures = remontees.iter().filter(|m| **m == DuNavigateur::CanalOuvert).count();
    assert_eq!(
        ouvertures, 1,
        "deux canaux négociés, UNE seule ouverture annoncée : \
         en voir deux signifie que le label n'est pas filtré (remontées : {remontees:?})"
    );
}

#[test]
fn la_fermeture_du_canal_seul_remonte_canal_ferme_et_seulement_pour_le_bon_id() {
    // Le cas où le navigateur ferme SON canal de données sans fermer la
    // connexion — distinct du close_notify du test suivant, et servi par un
    // autre bras de `traiter`. Né d'une MUTATION SURVIVANTE : retirer le bras
    // `ChannelClose` laissait tous les autres tests verts, aucun n'empruntant
    // ce chemin.
    //
    // ⚠️ **Il s'écrit sur des événements SYNTHÉTIQUES, et ce n'est pas un
    // raccourci de confort — c'est la seule voie.** Vérifié dans les sources de
    // str0m 0.21 plutôt que supposé : `DirectApi::close_data_channel` appelle
    // `RtcSctp::close_stream`, qui ne fait que poser `do_close = true` sur
    // l'entrée LOCALE (`sctp/mod.rs:516-520`) ; l'événement `SctpEvent::Close`
    // qui en découle (`:838-841`) est rendu au pair qui ferme, et **rien n'est
    // émis sur le fil**. Un pair str0m ne peut donc pas provoquer de
    // `Event::ChannelClose` chez nous. Un vrai navigateur, lui, émet un
    // stream-reset SCTP que str0m traite bien (`:740`, `:790`, `:808`, `:818`
    // posent `do_close` depuis l'entrée) — le bras est donc utile en
    // production, et seulement inatteignable depuis ce banc.
    //
    // Précédent de ce dépôt pour la forme : `transport/evenements/tests.rs`
    // remet des `MediaAdded` synthétiques à une session nue, pour exercer une
    // discrimination sans négociation.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(LABEL_FICHIERS.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;

    assert!(traiter(Event::ChannelOpen(bon, LABEL_FICHIERS.to_string()), &mut canal, &tx).is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalOuvert));
    assert_eq!(canal, Some(bon));

    // Une fermeture qui ne concerne PAS notre canal ne doit RIEN remonter :
    // sans cette moitié, le test passerait sur un bras qui envoie
    // `CanalFerme` à chaque fermeture, quelle qu'elle soit — et le pont
    // déclarerait mort un canal bien vivant.
    assert!(traiter(Event::ChannelClose(autre), &mut canal, &tx).is_none());
    assert!(rx.try_recv().is_err(), "la fermeture d'un autre canal ne remonte rien");
    assert_eq!(canal, Some(bon), "et elle ne doit pas oublier le nôtre");

    assert!(traiter(Event::ChannelClose(bon), &mut canal, &tx).is_none());
    assert_eq!(rx.try_recv(), Ok(DuNavigateur::CanalFerme));
    assert_eq!(canal, None, "le canal fermé doit être oublié");
}

#[test]
fn un_canal_dont_le_label_n_est_pas_le_notre_n_est_jamais_retenu() {
    // Le pendant synthétique de `un_seul_canal_est_retenu_parmi_deux…`, et il
    // exerce ce que celui-là ne peut pas : l'ordre INVERSE d'ouverture, où le
    // canal étranger arrive EN DERNIER. Sans filtre sur le label, c'est lui qui
    // écraserait le nôtre.
    let mut faux = Rtc::builder().clear_codecs().build(Instant::now());
    let mut api = faux.sdp_api();
    let bon = api.add_channel(LABEL_FICHIERS.to_string());
    let autre = api.add_channel("autre-canal".to_string());

    let (tx, rx) = channel();
    let mut canal: Option<ChannelId> = None;
    traiter(Event::ChannelOpen(bon, LABEL_FICHIERS.to_string()), &mut canal, &tx);
    let _ = rx.try_recv();
    traiter(Event::ChannelOpen(autre, "autre-canal".to_string()), &mut canal, &tx);

    assert_eq!(canal, Some(bon), "un canal étranger ne doit jamais écraser le nôtre");
    assert!(rx.try_recv().is_err(), "et il ne doit annoncer aucune ouverture");
}

#[test]
fn la_fermeture_du_canal_remonte_canal_ferme() {
    let (mut pair, _sortant, entrant) = monter(&[LABEL_FICHIERS]);

    // Le pair s'en va — l'onglet se ferme. La boucle doit le DIRE, pas se
    // terminer en silence : sans ce message, les commandes en vol attendraient
    // leur délai plutôt que de rendre ERROR_IO_DEVICE tout de suite.
    let mut parti = false;
    let (remontees, _) = echanger(
        &mut pair,
        &entrant,
        |rtc, remontees| {
            if !parti && canal_ouvert(remontees) {
                parti = true;
                // ⚠️ `close()` et NON `disconnect()` — la différence est tout
                // le test. `disconnect()` ne fait que poser `alive = false`
                // LOCALEMENT, sans rien émettre : le pont ne l'apprendrait
                // qu'à l'expiration d'ICE, des dizaines de secondes plus tard.
                // `close()` envoie le close_notify DTLS, c'est-à-dire ce que
                // fait un navigateur dont on ferme l'onglet. Une première
                // rédaction employait `disconnect()` et échouait au budget —
                // elle ne mesurait pas ce qu'elle croyait.
                rtc.close().expect("close_notify émis");
            }
        },
        |remontees, _| remontees.contains(&DuNavigateur::CanalFerme),
        "que la fermeture ne remonte",
    );
    assert!(
        remontees.contains(&DuNavigateur::CanalOuvert),
        "le canal doit d'abord s'être ouvert, sinon le test ne mesure rien"
    );
    assert!(remontees.contains(&DuNavigateur::CanalFerme));
}
