//! The tests of the channel client, against a REAL local WebSocket server.
//!
//! Extracted into a neighbouring file on the precedent of
//! `superviseur/table.rs`: the client already holds 285 lines, and these tests
//! add as many. Same mechanism, same reason.
//!
//! 🔴 **THE CUT IS REAL, AND THAT IS THE WHOLE POINT.** The fake channel drops
//! its socket without a close frame — a pulled cable, not a polite
//! `Close`. A test that only exercised the nominal path would say NOTHING about
//! reconnection, which is the only new behaviour of this file.

use super::*;
// ⚠️ THESE TWO NAMES WERE INHERITED THROUGH `use super::*`. The extraction of
// `session.rs` (sub-block G3) took out of the parent the only production uses
// that justified them, and the parent stopped importing them: the test
// module must therefore name them itself. It is the price, tiny and declared,
// of a `use super::*` — it makes invisible what one depends on.
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, IssueLancement, MotifCanal};
use tokio_tungstenite::tungstenite::Message;

use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// What the fake channel plays on a given connection.
enum Scenario {
    /// Enrols, then CUTS abruptly.
    EnroleEtCoupe(&'static str),
    /// Enrols and keeps the connection open indefinitely.
    EnroleEtTient(&'static str),
    /// Refuses, with its reason, then closes.
    Refuse(MotifCanal),
    /// Refuses by writing the RAW frame, without going through our encoders.
    ///
    /// 🔴 IT IS THE ONLY WAY TO PLAY A PLATFORM OF A VERSION OTHER THAN
    /// OURS. `DepuisLaPlateforme::refus` always sets
    /// `PLATEFORME_VERSION`: a scenario that used it could not
    /// go red on the defect measured in acceptance run G1, where both ends have
    /// precisely different versions.
    RefuseBrut(&'static str),
}

/// A fake `/agent` channel. Returns its URL and the queue of received enrolment
/// messages — one per connection, which makes the NUMBER of
/// connections observable, and hence reconnection assertable.
async fn faux_canal(mut scenarios: Vec<Scenario>) -> (String, mpsc::UnboundedReceiver<String>) {
    let ecoute = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("écoute locale");
    let port = ecoute.local_addr().expect("adresse locale").port();
    let (tx, rx) = mpsc::unbounded_channel();
    scenarios.reverse();
    tokio::spawn(async move {
        loop {
            let Ok((flux, _)) = ecoute.accept().await else {
                return;
            };
            let scenario = scenarios.pop();
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(flux)
                    .await
                    .expect("montée WebSocket");
                if let Some(Ok(Message::Text(texte))) = ws.next().await {
                    let _ = tx.send(texte);
                }
                match scenario {
                    Some(Scenario::EnroleEtCoupe(prefixe)) => {
                        envoyer_enrole(&mut ws, prefixe).await;
                        // Let the byte go before pulling the cable:
                        // without this pause, the test would measure a
                        // TCP race and not reconnection.
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
                    }
                    Some(Scenario::EnroleEtTient(prefixe)) => {
                        envoyer_enrole(&mut ws, prefixe).await;
                        std::future::pending::<()>().await;
                    }
                    Some(Scenario::RefuseBrut(trame)) => {
                        let _ = ws.send(Message::Text(trame.to_string())).await;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
                    }
                    Some(Scenario::Refuse(motif)) => {
                        let texte = serde_json::to_string(&DepuisLaPlateforme::refus(motif))
                            .expect("sérialisation du refus");
                        let _ = ws.send(Message::Text(texte)).await;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        drop(ws);
                    }
                    // One connection too many: the test observes it through the queue, and
                    // it is precisely what a version refusal must
                    // never produce.
                    None => std::future::pending::<()>().await,
                }
            });
        }
    });
    (format!("ws://127.0.0.1:{port}"), rx)
}

async fn envoyer_enrole<S>(ws: &mut tokio_tungstenite::WebSocketStream<S>, prefixe: &str)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let texte = serde_json::to_string(&DepuisLaPlateforme::enrole(
        prefixe,
        "jeton-jwt",
        1_787_136_774_000,
    ))
    .expect("sérialisation de l'enrôlement");
    ws.send(Message::Text(texte))
        .await
        .expect("envoi de l'enrôlement");
}

/// Waits for the identity to take a value different from the one already read.
async fn prochain_prefixe(canal: &mut Canal) -> String {
    loop {
        canal
            .identite
            .changed()
            .await
            .expect("le fil de reprise a renoncé au lieu de reprendre");
        let courante = canal.identite.borrow_and_update().clone();
        if let Some(identite) = courante {
            return identite.prefixe;
        }
    }
}

#[test]
fn l_url_du_canal_ne_double_jamais_la_barre() {
    assert_eq!(url_du_canal("ws://h:8080"), "ws://h:8080/agent");
    assert_eq!(url_du_canal("ws://h:8080/"), "ws://h:8080/agent");
}

/// 🔴 THE RECONNECTION TEST. Mutation that turns it red: replace the loop of
/// `ouvrir` with a single call to `une_session` — the agent would lose its channel at
/// the first cut, `vu_a` would stop advancing, and the platform
/// would declare the VM `injoignable` **permanently**, while the network
/// has been back for a long time.
#[tokio::test]
async fn une_coupure_reelle_du_socket_fait_reprendre_le_canal() {
    let (url, mut connexions) = faux_canal(vec![
        Scenario::EnroleEtCoupe("PREMIER"),
        Scenario::EnroleEtTient("SECOND"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let premiere = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune identité en 5 s")
        .expect("la boucle a renoncé avant tout enrôlement");
    assert_eq!(premiere.prefixe, "PREMIER");

    let seconde = tokio::time::timeout(Duration::from_secs(5), prochain_prefixe(&mut canal))
        .await
        .expect("aucune reprise en 5 s après la coupure : le canal ne se reprend PAS");
    assert_eq!(seconde, "SECOND");

    // And it is indeed a SECOND connection, with the same enrolment: the
    // reconnection presents itself again, it does not merely beat.
    let premier = connexions.recv().await.expect("premier enrôlement");
    let second = connexions.recv().await.expect("second enrôlement");
    assert!(
        premier.contains(r#""vm":"vm-1""#),
        "enrôlement reçu : {premier}"
    );
    assert_eq!(
        premier, second,
        "la reprise doit re-présenter le MÊME enrôlement"
    );
}

/// 🔴 D4'S EXCEPTION, direction 1. Mutation that turns it red: return
/// `Fin::Reprenable` on `MotifCanal::Version` — a version incompatibility
/// would then disguise itself as a reconnection loop, which is the most
/// costly failure mode to diagnose in this whole repository.
///
/// ⚠️ **TWO TESTS AND NOT A SINGLE ONE WITH TWO ASSERTIONS**: `expect` stops at
/// the first failure, and the second half — "no second connection" — would
/// then NEVER be tested alone. It is lesson ①A-bis of P2, applied
/// in advance. Measured: under the mutation, both go red, each on its
/// own message.
#[tokio::test]
async fn un_refus_de_version_rend_l_attente_vaine() {
    let (url, _connexions) = faux_canal(vec![Scenario::Refuse(MotifCanal::Version)]).await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let verdict = tokio::time::timeout(Duration::from_secs(3), canal.attendre_identite())
        .await
        .expect("la boucle doit RENONCER, pas attendre indéfiniment");
    assert!(
        verdict.is_none(),
        "un refus de version doit rendre l'attente vaine"
    );
}

/// The other half: the channel does not reopen.
#[tokio::test]
async fn a_version_refusal_opens_no_second_connection() {
    let (url, mut connexions) = faux_canal(vec![Scenario::Refuse(MotifCanal::Version)]).await;
    let _canal = ouvrir(&url, "vm-1".into(), "chut".into());

    connexions.recv().await.expect("premier enrôlement");
    // The minimal backoff (500 ms) has elapsed three times: if there were to be
    // a second attempt, it would be there.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        connexions.try_recv().is_err(),
        "une seconde connexion a eu lieu : le refus de version a été réessayé"
    );
}

/// 🔴 D4'S EXCEPTION, direction 2 — and it is the other half, without which
/// "not retrying" could be obtained by NEVER retrying anything.
/// A refused enrolment stops being refused as soon as the operator enrols the VM,
/// without anyone restarting the agent.
#[tokio::test]
async fn un_refus_d_enrolement_se_reessaie() {
    let (url, _connexions) = faux_canal(vec![
        Scenario::Refuse(MotifCanal::Enrolement),
        Scenario::EnroleEtTient("APRES-ENROLEMENT"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let identite = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune reprise en 5 s après un refus d'enrôlement")
        .expect("la boucle a renoncé sur un refus qui n'est PAS `version`");
    assert_eq!(identite.prefixe, "APRES-ENROLEMENT");
}

// ---------------------------------------------------------------------------
// Sub-block G1 — the channel becomes BIDIRECTIONAL.
// ---------------------------------------------------------------------------

/// A scenario that enrols, holds, and SENDS BACK everything the agent pushes to it.
///
/// The harness above only reads ONE message per connection — the one of
/// enrolment — and can therefore say nothing about a catalogue emitted afterwards.
/// This one additionally opens an order channel that the test feeds.
pub(super) async fn faux_canal_bidirectionnel(
    prefixe: &'static str,
) -> (
    String,
    mpsc::UnboundedReceiver<String>,
    mpsc::UnboundedSender<String>,
) {
    let ecoute = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("écoute locale");
    let port = ecoute.local_addr().expect("adresse locale").port();
    let (recus_tx, recus_rx) = mpsc::unbounded_channel();
    let (ordres_tx, mut ordres_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let Ok((flux, _)) = ecoute.accept().await else {
            return;
        };
        let mut ws = tokio_tungstenite::accept_async(flux)
            .await
            .expect("montée WebSocket");
        if let Some(Ok(Message::Text(texte))) = ws.next().await {
            let _ = recus_tx.send(texte);
        }
        envoyer_enrole(&mut ws, prefixe).await;
        loop {
            tokio::select! {
                ordre = ordres_rx.recv() => match ordre {
                    Some(texte) => { let _ = ws.send(Message::Text(texte)).await; }
                    None => return,
                },
                recu = ws.next() => match recu {
                    Some(Ok(Message::Text(texte))) => { let _ = recus_tx.send(texte); }
                    Some(Ok(_)) => {}
                    _ => return,
                },
            }
        }
    });
    (format!("ws://127.0.0.1:{port}"), recus_rx, ordres_tx)
}

pub(super) async fn attendre_message(
    recus: &mut mpsc::UnboundedReceiver<String>,
    predicat: impl Fn(&str) -> bool,
) -> String {
    let attente = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let texte = recus.recv().await.expect("le faux canal s'est tu");
            if predicat(&texte) {
                return texte;
            }
        }
    });
    attente.await.expect("aucun message correspondant en 5 s")
}

#[tokio::test]
async fn a_message_pushed_into_the_queue_reaches_the_server() {
    // 🔴 THE RED: not draining the queue in the `select!`. It would grow
    // endlessly, the agent would believe it had emitted its catalogue, and NOTHING would
    // say so — neither error nor trace, the platform would simply stay empty.
    let (url, mut recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");

    canal
        .emetteur()
        .emettre(VersLaPlateforme::catalogue(true, Vec::new(), Vec::new()));
    let texte = attendre_message(&mut recus, |t| t.contains("catalogue")).await;
    assert_eq!(
        texte,
        r#"{"type":"catalogue","v":5,"complet":true,"applications":[],"disparues":[]}"#
    );
}

#[tokio::test]
async fn un_ordre_de_lancement_arrive_au_consommateur_et_ne_ferme_pas_la_session() {
    // 🔴 TWO REDS IN ONE. Forgetting the `Lancer` arm would make it fall into the
    // `Err` arm ("unreadable message"), which CLOSES the session: a
    // perfectly valid order would trigger a reconnection loop. The second
    // `assert` measures that the session survives — without it, an arm that
    // logged then returned `Fin::Reprenable` would pass the first.
    let (url, mut recus, ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    let mut recu_ordres = canal
        .ordres()
        .expect("la file d'ordres n'est prise qu'une fois");

    ordres
        .send(r#"{"type":"lancer","v":5,"demande":"d-7","cle":"a1b2"}"#.into())
        .expect("envoi de l'ordre");

    let ordre = tokio::time::timeout(Duration::from_secs(5), recu_ordres.recv())
        .await
        .expect("aucun ordre reçu en 5 s")
        .expect("la file d'ordres est fermée");
    assert_eq!(
        ordre,
        Ordre::Lancer {
            demande: "d-7".into(),
            cle: "a1b2".into()
        }
    );

    // The session is still alive: the next emission arrives.
    canal
        .emetteur()
        .emettre(VersLaPlateforme::lancee("d-7", IssueLancement::Raccourci));
    let texte = attendre_message(&mut recus, |t| t.contains("lancee")).await;
    assert_eq!(
        texte,
        r#"{"type":"lancee","v":5,"demande":"d-7","issue":"raccourci"}"#
    );
}

#[tokio::test]
async fn un_message_mis_en_file_alors_que_le_socket_est_tombe_est_perdu_sans_tuer_le_canal() {
    // 🔴 IT IS THE INTENDED BEHAVIOUR, AND THIS TEST HAMMERS IT HOME. Making it blocking
    // would turn the queue into a memory leak on a channel that can stay cut
    // for hours; making it fatal would kill the channel on an ordinary network
    // cut. The loss is acceptable for a single reason, written next to
    // the queue: the agent sends its COMPLETE catalogue again at each
    // re-enrolment, so any divergence has an end.
    let (url, mut recus) = faux_canal(vec![
        Scenario::EnroleEtCoupe("AAA"),
        Scenario::EnroleEtTient("BBB"),
    ])
    .await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    assert_eq!(
        canal
            .attendre_identite()
            .await
            .expect("1er enrôlement")
            .prefixe,
        "AAA"
    );

    // The cut occurs; we push while there is no socket anymore.
    for _ in 0..64 {
        canal
            .emetteur()
            .emettre(VersLaPlateforme::catalogue(false, Vec::new(), Vec::new()));
    }

    // The channel reconnects all the same: it is the proof that no emission was
    // fatal, and the second enrolment attests it.
    assert_eq!(prochain_prefixe(&mut canal).await, "BBB");
    let _ = recus.recv().await;
}

#[tokio::test]
async fn the_identity_is_reannounced_at_each_reenrolment() {
    // 🔴 THE RED: pushing the identity only ONCE. The discovery loop
    // observes this `watch` to know that a re-enrolment took place, and it is
    // what makes it send the COMPLETE catalogue again. Without this second send, a
    // restarted platform would stay divergent WITHOUT END.
    let (url, _recus) = faux_canal(vec![
        Scenario::EnroleEtCoupe("AAA"),
        Scenario::EnroleEtTient("BBB"),
    ])
    .await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    assert_eq!(canal.attendre_identite().await.expect("1er").prefixe, "AAA");
    assert_eq!(prochain_prefixe(&mut canal).await, "BBB");
}

#[tokio::test]
async fn la_file_d_ordres_ne_se_prend_qu_une_fois() {
    // Two consumers would steal orders from one another, and each
    // would only see part of them — a defect whose symptom would be "one
    // launch out of two does not go".
    let (url, _recus, _ordres) = faux_canal_bidirectionnel("PPP").await;
    let mut canal = ouvrir(&url, "w1".into(), "chut".into());
    canal.attendre_identite().await.expect("enrôlement");
    assert!(canal.ordres().is_some());
    assert!(canal.ordres().is_none());
}

// ---------------------------------------------------------------------------
// Fix of 20 August 2026 — the refusal of a platform of ANOTHER version.
// ---------------------------------------------------------------------------

/// 🔴 THE END-TO-END RED OF DEFECT 2. Acceptance run G1 noted, on a
/// v1 agent facing a v2 platform: **0** "the platform REFUSES the
/// version" lines and **10** reconnections, up to the 30 s step, without end. This test
/// plays the frame as it arrives on the wire — the SENDER's version, not
/// ours — and requires the loop to GIVE UP.
#[tokio::test]
async fn a_version_refusal_sent_in_another_version_makes_the_wait_vain() {
    let (url, _connexions) = faux_canal(vec![Scenario::RefuseBrut(
        r#"{"type":"refus","v":97,"motif":"version"}"#,
    )])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let verdict = tokio::time::timeout(Duration::from_secs(3), canal.attendre_identite())
        .await
        .expect("la boucle doit RENONCER, pas boucler : c'est le défaut mesuré en recette G1");
    assert!(
        verdict.is_none(),
        "un refus de version émis dans une autre version doit rendre l'attente vaine"
    );
}

/// The other half, written as two tests for the reason already given above
/// (`expect` stops at the first failure): no second connection.
#[tokio::test]
async fn a_version_refusal_sent_in_another_version_opens_no_second_connection() {
    let (url, mut connexions) = faux_canal(vec![Scenario::RefuseBrut(
        r#"{"type":"refus","v":97,"motif":"version"}"#,
    )])
    .await;
    let _canal = ouvrir(&url, "vm-1".into(), "chut".into());

    connexions.recv().await.expect("premier enrôlement");
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(
        connexions.try_recv().is_err(),
        "une seconde connexion a eu lieu : c'est la boucle sans terme de la recette G1"
    );
}

/// A reason that NO version of this repository knows must stay readable, be
/// logged as is, and be retried — it is the "the peer can recover from it"
/// class. Without this tolerance, the remedy above would only hold until
/// the first reason added by a future version, and the failure mode
/// would come back identically.
#[tokio::test]
async fn un_refus_a_motif_inconnu_se_reessaie_au_lieu_de_devenir_illisible() {
    let (url, _connexions) = faux_canal(vec![
        Scenario::RefuseBrut(r#"{"type":"refus","v":98,"motif":"quota-depasse"}"#),
        Scenario::EnroleEtTient("APRES-MOTIF-INCONNU"),
    ])
    .await;
    let mut canal = ouvrir(&url, "vm-1".into(), "chut".into());

    let identite = tokio::time::timeout(Duration::from_secs(5), canal.attendre_identite())
        .await
        .expect("aucune reprise en 5 s après un refus à motif inconnu")
        .expect("la boucle a renoncé sur un motif qui n'est PAS `version`");
    assert_eq!(identite.prefixe, "APRES-MOTIF-INCONNU");
}
