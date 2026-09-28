//! WebSocket client to the signaling server.
//!
//! The agent is always the responder: it receives an SDP offer and sends back an
//! answer. No trickle ICE — host candidates travel in the SDP.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

/// What signaling tells us about the relay to use.
#[derive(Debug, Clone)]
pub struct ConfigIce {
    /// TURN server address, extracted from the `turn:host:port` URL.
    pub serveur: std::net::SocketAddr,
    pub username: String,
    pub credential: String,
}

pub struct SignalingHandle {
    /// SDP offers received from the browser.
    pub offers: mpsc::Receiver<String>,
    /// SDP answers to send back to the browser.
    pub answers: mpsc::Sender<String>,
    /// Switches to `true` as soon as one of the two background tasks (receive or
    /// send) observes the end of the connection. It is the only way for
    /// the caller to detect a signaling loss after the initial exchange
    /// (I6 of the review): without it, neither the discarded `JoinHandle`s nor the absence of
    /// rereading the `offers` channel after the first one made a signaling
    /// drop visible.
    pub closed: watch::Receiver<bool>,
    /// ICE configuration delivered by the server right after the role
    /// declaration. `watch` rather than `mpsc`: it is a STATE, of which only the
    /// last value counts, and the caller must be able to read it even if it
    /// arrives after the emission.
    pub ice_config: watch::Receiver<Option<ConfigIce>>,
    /// 🔴 **NEW — FIX FOR THE MISSING-BRAKES LEGACY (fix
    /// round 1, critical ②), August 25th, 2026.** The number of seconds
    /// the relay asks to wait before retrying, when the refusal carries
    /// the `trop-de-requetes` reason (`plateforme/src/signaling/relais.ts`,
    /// field `retryApresS`). `None` as long as no such refusal has arrived, or
    /// if the field was absent/unreadable — a relay from a version
    /// earlier than this batch sends none.
    ///
    /// ⚠️ **IT IS NOT AN ORDER, IT IS INFORMATION**: nothing here makes
    /// anyone wait. It is up to the caller to consult it before
    /// dying — see `pont.rs::executer`, which sleeps that time BEFORE returning
    /// its `Err`, so that the supervisor (`surveillance_pont.rs`), which
    /// measures the spacing since the last LAUNCH and not since the process's
    /// death, never restarts earlier than what the relay
    /// asked — without any channel crossing the process boundary.
    pub retry_apres_s: watch::Receiver<Option<u64>>,
    /// Kept so that the two background tasks are not
    /// completely abandoned: `demarrage.rs` does not wait for them in
    /// normal operation (the transport no longer depends on signaling once
    /// the offer/answer is exchanged), but discarding them silently
    /// would mask a possible panic inside one of them.
    // `#[allow(dead_code)]`: never read in this single-session task (see
    // the comment above), but kept on purpose — the lint does not
    // know that.
    #[allow(dead_code)]
    pub receiver_task: JoinHandle<()>,
    #[allow(dead_code)]
    pub sender_task: JoinHandle<()>,
}

/// Builds the relay URL from the service one.
///
/// 🔴 `SIGNALING_URL` IS THE SERVICE BASE, NEVER THE RELAY URL, AND THAT IS
/// WHAT KEEPS `scripts/run-agent.sh` UNCHANGED. The same variable serves to
/// derive the enrolment channel (`plateforme::url_du_canal`, which adds
/// `/agent`) and the HTTP address for icon uploads
/// (`apps::icone::televersement::base_http`, which strips any path). Writing
/// `/signal` in it would break the former: `ws://h:8080/signal/agent` is not
/// `/agent`, and the platform compares the path EXACTLY.
///
/// The `trim_end_matches` has the same reason as in its twin: `ws://h:8080/`
/// followed by `/signal` would give `//signal`, refused the same way.
pub fn url_du_relais(signaling_url: &str) -> String {
    format!("{}/signal", signaling_url.trim_end_matches('/'))
}

/// Connects to signaling and starts the exchange loop as a background task.
/// `url` is already the RELAY URL (`url_du_relais`), never the service one:
/// `demarrage.rs` and `pont.rs` derive it before calling this function.
/// `jeton` carries the agent token delivered by the `/agent` channel
/// (`crate::plateforme`). **`None` makes the platform refuse the handshake
/// since sub-block P3**: the guard no longer accepts an anonymous
/// `{"role":"agent"}`, and the socket closes without any session
/// being established.
pub async fn run_signaling(
    url: &str,
    session: &str,
    jeton: Option<&str>,
) -> Result<SignalingHandle> {
    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    let (mut sink, mut source) = stream.split();

    let hello = serde_json::json!({ "role": "agent", "session": session, "jeton": jeton });
    sink.send(Message::Text(hello.to_string())).await?;
    // 🔴 "agent REGISTERED" WAS WRONG, exactly like its twin in
    // `superviseur/signalisation.rs`: the trace comes out at EMISSION, hence before
    // any verdict from the platform, and it showed in both acceptance
    // runs of P3 where NO session was established. Here the refusal is
    // already made visible below (`Some("error") => tracing::error!`);
    // it is the only label that misled.
    tracing::info!(
        session,
        "déclaration de l'agent émise au signaling (acceptation encore inconnue)"
    );

    let (offer_tx, offers) = mpsc::channel::<String>(4);
    let (answers, mut answer_rx) = mpsc::channel::<String>(4);
    // End-of-connection signal, shared between the two tasks: whichever
    // one detects the connection loss first, the other notices
    // and ends in turn instead of staying blocked
    // indefinitely (see the two `select!`s below).
    let (closed_tx, closed_rx) = watch::channel(false);
    let (ice_tx, ice_config) = watch::channel::<Option<ConfigIce>>(None);
    let (retry_apres_s_tx, retry_apres_s) = watch::channel::<Option<u64>>(None);
    let closed_tx_sender_side = closed_tx.clone();
    let closed_rx_sender_side = closed_rx.clone();

    // Receive: offers and errors coming from signaling.
    let receiver_task = tokio::spawn(async move {
        while let Some(message) = source.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(frame)) => {
                    tracing::info!(?frame, "signaling fermé par le serveur");
                    break;
                }
                Err(e) => {
                    tracing::warn!(erreur = %e, "connexion de signaling perdue");
                    break;
                }
                Ok(_) => continue,
            };
            let parsed: serde_json::Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    tracing::warn!(erreur = %e, "message de signaling illisible");
                    continue;
                }
            };
            match parsed["type"].as_str() {
                Some("offer") => {
                    if let Some(sdp) = parsed["sdp"].as_str() {
                        // `try_send`, never `.send(...).await`: this loop
                        // also serves `peer-gone` and `error` just below,
                        // so it must never fall asleep on a full
                        // channel for lack of a consumer. An offer arriving
                        // while a previous one has not been
                        // consumed yet is deliberately discarded (and logged)
                        // rather than freezing reception.
                        match offer_tx.try_send(sdp.to_string()) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                tracing::warn!(
                                    "offre écartée : la précédente n'a pas encore été consommée"
                                );
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => break,
                        }
                    }
                }
                Some("ice-config") => match analyser_config_ice(&parsed) {
                    Some(config) => {
                        tracing::info!(serveur = %config.serveur, "configuration TURN reçue");
                        let _ = ice_tx.send(Some(config));
                    }
                    None => tracing::warn!(
                        "configuration ICE reçue mais inexploitable : session sans relais"
                    ),
                },
                Some("peer-gone") => tracing::info!("le client s'est déconnecté"),
                Some("error") => {
                    // 🔴 `retryApresS` IS ONLY CARRIED ON THE VOLUME REFUSAL
                    // (`motif: "trop-de-requetes"`) — see `relais.ts`. A
                    // handshake refusal (absent or invalid token)
                    // carries none, and `as_u64()` then returns `None` without
                    // needing to test the reason here: basing the
                    // decision on the field's presence ALONE, never on the
                    // reason text, is what keeps this arm correct if the
                    // relay one day gains a second reason carrying a wait.
                    let retry = parsed["retryApresS"].as_u64();
                    if let Some(s) = retry {
                        let _ = retry_apres_s_tx.send(Some(s));
                    }
                    tracing::error!(
                        raison = %parsed["reason"],
                        retry_apres_s = ?retry,
                        "erreur de signaling"
                    )
                }
                other => tracing::debug!(?other, "message de signaling ignoré"),
            }
        }
        tracing::info!("boucle de réception du signaling terminée");
        // Wakes the send task so that it ends in turn instead
        // of waiting indefinitely on a still-open `answers` channel.
        let _ = closed_tx.send(true);
    });

    // Send: SDP answers. Ends either when `answers` is closed
    // (no sender left on the caller side), or when one of the two
    // tasks has observed the end of the connection — never waiting
    // forever for a message that will not come any more (that was the defect before this
    // fix: `while let Some(sdp) = answer_rx.recv().await` alone).
    let sender_task = tokio::spawn(async move {
        let mut closed_rx = closed_rx_sender_side;
        loop {
            tokio::select! {
                sdp = answer_rx.recv() => {
                    let Some(sdp) = sdp else { break };
                    let payload = serde_json::json!({ "type": "answer", "sdp": sdp });
                    if sink.send(Message::Text(payload.to_string())).await.is_err() {
                        tracing::warn!("échec d'envoi de la réponse SDP, connexion de signaling perdue");
                        let _ = closed_tx_sender_side.send(true);
                        break;
                    }
                }
                _ = closed_rx.changed() => {
                    tracing::info!("boucle d'émission du signaling terminée (connexion fermée)");
                    break;
                }
            }
        }
    });

    Ok(SignalingHandle {
        offers,
        answers,
        closed: closed_rx,
        ice_config,
        retry_apres_s,
        receiver_task,
        sender_task,
    })
}

/// Waits for the delay the relay suggested (`retryApresS`, refusal
/// `trop-de-requetes`) if it sent one — zero cost otherwise.
///
/// 🔴 **FIX FOR THE MISSING-BRAKES LEGACY (fix round 1,
/// critical ②).** It is the caller that decides WHEN to consult this value —
/// typically right before returning a fatal error, once it is established that
/// the session will not open. Nothing here makes `run_signaling`
/// itself wait: signaling stays a pure information relay.
///
/// ⚠️ **BOUNDED BY `REPLI_MAX_MS`**, never the server's raw value: a
/// relay sending an aberrant value (bug, or compromised server) must
/// not be able to freeze indefinitely a process whose sole purpose,
/// at this stage, is to die fast so that its supervisor retries.
///
/// 🔴 **DECLARED, NOT FIXED (review, fix round 2)**: this ceiling
/// (`REPLI_MAX_MS` = 30 s) can be STRICTLY LOWER than what the relay
/// suggests (`retryApresS` can be up to `FENETRE_REQUETES_MS / 1000` =
/// 60 s, `plateforme/src/securite/frein.ts`). Consequence: an agent that
/// honours a 60 s suggestion only sleeps 30, retries, and — the
/// window's budget not having expired yet — gets refused a SECOND time
/// before the window really empties. One `/signal` connection
/// wasted per prolonged refusal window, never more: the caller's
/// exponential fallback (`relance_pont::EtatRelance`, or
/// `plateforme::repli` for the `/agent` channel) keeps growing
/// meanwhile, so it never degenerates into hammering.
pub async fn honorer_retry_suggere(retry_apres_s: &watch::Receiver<Option<u64>>) {
    let Some(secondes) = *retry_apres_s.borrow() else {
        return;
    };
    let bornees = secondes.min(crate::plateforme::repli::REPLI_MAX_MS / 1000);
    tracing::info!(
        secondes = bornees,
        secondes_demandees = secondes,
        "attente du délai suggéré par le relais (retryApresS) avant de céder la main"
    );
    tokio::time::sleep(std::time::Duration::from_secs(bornees)).await;
}

/// Extracts the first usable TURN entry of an `ice-config` message.
///
/// Resolves the host name: `Candidate::relayed` and the UDP socket want a
/// `SocketAddr`, not a URL. A failing resolution returns `None` — session
/// without relay rather than session without startup.
fn analyser_config_ice(message: &serde_json::Value) -> Option<ConfigIce> {
    use std::net::ToSocketAddrs;

    let serveurs = message["iceServers"].as_array()?;
    for entree in serveurs {
        let urls = entree["urls"].as_str()?;
        // Expected shape: `turn:host:port`. `stun:` entries are ignored —
        // the reflexive address comes to us from the Allocate response itself.
        let Some(reste) = urls.strip_prefix("turn:") else {
            continue;
        };
        let Ok(mut adresses) = reste.to_socket_addrs() else {
            continue;
        };
        let serveur = adresses.next()?;
        return Some(ConfigIce {
            serveur,
            username: entree["username"].as_str()?.to_string(),
            credential: entree["credential"].as_str()?.to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Returns the URL of a fake signaling and the first message received.
    async fn premiere_poignee_de_main(jeton: Option<&str>) -> String {
        let ecoute = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("écoute locale");
        let port = ecoute.local_addr().expect("adresse locale").port();
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (flux, _) = ecoute.accept().await.expect("connexion entrante");
            let mut ws = tokio_tungstenite::accept_async(flux)
                .await
                .expect("montée WebSocket");
            if let Some(Ok(Message::Text(texte))) = ws.next().await {
                let _ = tx.send(texte);
            }
            std::future::pending::<()>().await;
        });
        let _handle = run_signaling(&format!("ws://127.0.0.1:{port}"), "P:w-1", jeton)
            .await
            .expect("connexion au faux signaling");
        tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("aucune poignée de main en 5 s")
            .expect("le faux signaling n'a rien reçu")
    }

    /// 🔴 Without the token on the wire, the platform's guard refuses the
    /// handshake and NO session is established (sub-block P3). The
    /// mutation that turns this test red — removing the `jeton` key from the `json!` — breaks
    /// NOTHING at compile time, neither here nor at the caller: it is an
    /// end-to-end failure only the wire can reveal.
    #[tokio::test]
    async fn la_poignee_de_main_porte_le_jeton_d_agent() {
        let poignee = premiere_poignee_de_main(Some("jwt.d.agent")).await;
        assert_eq!(
            poignee,
            r#"{"jeton":"jwt.d.agent","role":"agent","session":"P:w-1"}"#
        );
    }

    /// Without a token, the field goes out as `null` — which the guard treats exactly
    /// like an absence. This test freezes the shape, so that a future fallback cannot
    /// slip an empty string into it that would look like a token.
    #[tokio::test]
    async fn sans_jeton_la_poignee_de_main_le_dit_au_lieu_de_l_inventer() {
        let poignee = premiere_poignee_de_main(None).await;
        assert_eq!(
            poignee,
            r#"{"jeton":null,"role":"agent","session":"P:w-1"}"#
        );
    }

    #[test]
    fn le_relais_derive_du_signaling() {
        assert_eq!(url_du_relais("ws://h:8080"), "ws://h:8080/signal");
    }

    /// ⚠️ SAME REASON AS `url_du_canal`: `ws://h:8080/` followed by `/signal`
    /// would give `//signal`, and the platform compares the path EXACTLY.
    #[test]
    fn la_barre_finale_ne_double_pas() {
        assert_eq!(url_du_relais("ws://h:8080/"), "ws://h:8080/signal");
    }

    /// 🔴 THE TEST THAT FREEZES THE PLAN'S GAP ①: `SIGNALING_URL` stays the BASE,
    /// so the `/agent` channel keeps being derived correctly from a clean
    /// base.
    ///
    /// ⚠️ **THIS COMMENT PROMISED MORE THAN THIS TEST HOLDS, and it is
    /// corrected rather than erased (cross-cutting review, August 21st, 2026).** It said:
    /// "without it, someone could one day put `/signal` in the variable
    /// and break enrolment without any test flinching". **This test does
    /// NOT close that case**: it passes `"ws://h:8080"`, a CLEAN base, so it
    /// cannot see what would happen to `"ws://h:8080/signal"` — from which
    /// `url_du_canal` would derive `ws://h:8080/signal/agent`, and enrolment
    /// would fail. **The `SIGNALING_URL` contract is therefore frozen by NO
    /// test**, and it is listed in `CLAUDE.md`'s "Open legacies". What this
    /// test does establish, and it is already useful, is that the addition of `/signal` by
    /// `url_du_relais` did not contaminate `url_du_canal`.
    #[test]
    fn le_canal_agent_n_est_pas_affecte() {
        assert_eq!(
            crate::plateforme::url_du_canal("ws://h:8080"),
            "ws://h:8080/agent"
        );
    }

    /// 🔴 `SIGNALING_URL` IS THE SERVICE BASE, NEVER THE RELAY URL.
    /// `url_du_relais` adds `/signal` to it, `url_du_canal` adds `/agent`.
    /// Writing `/signal` in it would break enrolment — `ws://h:8080/signal/agent` —
    /// and NO test said so: the neighbouring one (`le_canal_agent_n_est_pas_
    /// affecte`, just above) passes a CLEAN base, so never exercises
    /// this case, while its comment claimed to close it.
    ///
    /// ⚠️ **THIS TEST FREEZES, IT DOES NOT FIX** (round of August 25th, 2026,
    /// task 4 of the no-VM legacy work): `url_du_canal` is a pure concatenation
    /// (`format!("{}/agent", …trim_end_matches('/'))`), without a guard on the
    /// base's content — played on `"ws://h:8080/signal"`, it returns
    /// `"ws://h:8080/signal/agent"` **exactly as documented here**, and the
    /// test already passed before this task (CHECKED: the red only exists by
    /// mutating `url_du_canal`, never on today's product). The
    /// contract this test closes is therefore not "the product refuses a wrong
    /// base" — it does not, and nothing in this task makes it
    /// do so — but "the behaviour on a wrong base is KNOWN and fixed
    /// by a test", where yesterday no test looked at it.
    #[test]
    fn une_base_portant_deja_signal_casse_le_canal_agent() {
        // The RIGHT base: `url_du_canal` adds `/agent` to it.
        assert_eq!(
            crate::plateforme::url_du_canal("ws://h:8080"),
            "ws://h:8080/agent"
        );
        // 🔴 THE WRONG BASE, the one no test exercised: it already carries
        // the relay's suffix, and enrolment then goes to a path that
        // does not exist on the platform side. It is THIS case the contract fixes here.
        assert_eq!(
            crate::plateforme::url_du_canal("ws://h:8080/signal"),
            "ws://h:8080/signal/agent",
            "le contrat de SIGNALING_URL a changé : cette égalité documentait \
             que le produit accepte une base déjà suffixée EN SILENCE"
        );
    }

    /// 🔴 FIX FOR THE MISSING-BRAKES LEGACY (fix round 1,
    /// critical ②) — `honorer_retry_suggere`, exercised on the host. FROZEN
    /// clock (`start_paused`): without it, these three tests would really
    /// wait whole seconds, and an assertion on the elapsed time
    /// would become measurement noise rather than a fact.
    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_n_attend_rien_sans_valeur() {
        let (_tx, rx) = watch::channel::<Option<u64>>(None);
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(
            tokio::time::Instant::now(),
            debut,
            "aucun refus de volume n'est jamais arrivé : rien à attendre"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_attend_exactement_la_valeur_recue() {
        let (_tx, rx) = watch::channel(Some(5u64));
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(
            tokio::time::Instant::now() - debut,
            std::time::Duration::from_secs(5)
        );
    }

    /// 🔴 The test that matters: without this ceiling, a relay sending an
    /// aberrant value (bug, or compromise) would freeze indefinitely a
    /// process whose sole purpose, at this stage, is to die fast so
    /// that its supervisor retries.
    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_est_bornee_au_plafond_de_repli() {
        let (_tx, rx) = watch::channel(Some(999_999u64));
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(
            tokio::time::Instant::now() - debut,
            std::time::Duration::from_millis(crate::plateforme::repli::REPLI_MAX_MS),
        );
    }
}
