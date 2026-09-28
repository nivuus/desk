//! The supervisor's connection to the signaling control session, **and its
//! RESUMPTION**.
//!
//! Distinct from `crate::signaling`, which only knows the SDP offers and answers
//! of a media session. Here we send and receive control messages,
//! and there is never any WebRTC negotiation.
//!
//! The returned receiver is a `std::sync::mpsc::Receiver` and not a tokio
//! channel: the supervisor loop is synchronous (it calls blocking
//! Windows APIs) and polls it with `try_recv`.
//!
//! 🔴 **THIS SOCKET NEVER REOPENED, AND IT WAS A MUTE OPERATIONS
//! FAILURE** (legacy no. 1 of batch 17, closed here). After a restart of the
//! `desk-plateforme` service, the two background tasks left their
//! loop, logged "send to the shell failed" then "control
//! connection to signaling lost", and **then nothing**: `rx_shell` was closed,
//! `envoyer` wrote into a channel without a consumer (`let _ = …`), and the
//! supervisor loop kept running believing it talked to someone.
//! No window could be announced **nor re-announced** any more, which
//! makes batch 17's fix (`pair-present`) inoperative — it needs
//! this socket to be delivered. The only remedy was to restart the agent,
//! a gesture that since batch 32I **orphans all windows**.
//!
//! **What the two tasks become: A SINGLE ONE**, which owns the
//! connection, serves it through a `select!`, and REOPENS it when it drops. The
//! two ends the caller holds — `rx_shell` and `envoyer` — are
//! created once and **survive reconnections**: `superviseur.rs` and
//! `boucle.rs` are unchanged, and never have to know a resumption took
//! place. The decision (when to retry, when to re-arm the fallback) is PURE and
//! lives in [`super::reprise_controle`], which is tested on the Linux host —
//! this file is `#![cfg(windows)]` and is not.
//!
//! 🔴 **WHAT IS EMITTED DURING THE OUTAGE IS DROPPED, NOT REPLAYED**, and it is
//! deliberate: a thirty-second-old window announcement is a
//! COPY of a truth that lives in the supervisor's table, and nothing here
//! could expire it — the argument word for word of
//! `plateforme/src/signaling/pair-present.ts`. What repairs the state after a
//! resumption is not a queue, it is the RE-ANNOUNCEMENT triggered by
//! `pair-present`: we do not replay yesterday's truth, we tell whoever
//! holds it that we are asking for it again now.
//!
//! 🔴 **THE TOKEN IS REREAD AT EACH ATTEMPT, NEVER THE STARTUP ONE.**
//! `main.rs` already spells it out for the LAUNCHER: "a
//! supervisor lives for hours; the agent token, for its part, lasts ten minutes and
//! renews at each heartbeat". A reconnection presenting
//! `config.jeton` — the startup snapshot — would be refused by the guard
//! ("handshake refused: token refused (expired)", a line really
//! observed in the production log), **at each attempt, forever**:
//! a remedy that would seem to work on a
//! one-minute outage and would never work again after ten. The identity
//! watch (`plateforme::Canal::veille_identite`) is therefore read at EACH
//! opening.

#![cfg(windows)]

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::protocole::{DepuisLaShell, VersLaShell};
use super::reprise_controle::Reprise;
use crate::plateforme::Identite;

type Flux = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Pourquoi `servir` a rendu la main.
enum Fin {
    /// The connection dropped — a new one must be opened.
    ConnexionPerdue,
    /// The supervisor itself is gone: both its ends are closed,
    /// there is no one left to serve and reopening a socket would be a leak.
    SuperviseurArrete,
}

/// Opens the connection, declares itself as `agent` on the given session, and returns
/// what is needed to send and receive. **The connection is then REOPENED
/// automatically at each drop**, with nothing for the caller to do.
///
/// `url` is already the RELAY URL (`crate::signaling::url_du_relais`): it is
/// `superviseur.rs` that derives it before calling this function, never this function.
///
/// `jeton_initial` carries the agent token known at startup
/// (`crate::plateforme`). **`None` makes the platform refuse the handshake
/// since sub-block P3**: the guard no longer accepts an anonymous
/// `{"role":"agent"}`.
///
/// `identite` is the identity WATCH, reread at each RECONNECTION: see
/// this file's header. `None` when the agent opened no `/agent`
/// channel (inherited token, or no identity) — `jeton_initial` then serves
/// at each attempt, exactly as before this batch.
///
/// 🔴 **THE FIRST OPENING STAYS FATAL, AND IT IS DELIBERATE.** It returns
/// `Err`, so `superviseur::executer` fails and the process stops, as
/// before this batch: a wrong `SIGNALING_URL` must show right away, not
/// turn into a silent resumption loop. This batch only fixes the
/// loss of an **already established** connection, which is the measured defect; widening
/// resumption to the very first try is another change, not measured,
/// deliberately left aside.
pub async fn connecter(
    url: &str,
    session: &str,
    jeton_initial: Option<&str>,
    identite: Option<watch::Receiver<Option<Identite>>>,
) -> Result<(
    std::sync::mpsc::Receiver<DepuisLaShell>,
    impl Fn(&VersLaShell) + Send + Sync + 'static,
)> {
    let premier = ouvrir(url, session, jeton_initial).await?;

    // The TWO ends the caller keeps. Created only once: they
    // survive all reconnections, which is what leaves `boucle.rs`
    // unaware of the mechanism.
    let (tx_entrant, rx_entrant) = std::sync::mpsc::channel();
    let (tx_sortant, rx_sortant) = tokio::sync::mpsc::unbounded_channel::<String>();

    tokio::spawn(tenir(
        premier,
        url.to_string(),
        session.to_string(),
        jeton_initial.map(str::to_string),
        identite,
        tx_entrant,
        rx_sortant,
    ));

    let envoyer = move |message: &VersLaShell| match serde_json::to_string(message) {
        Ok(texte) => {
            let _ = tx_sortant.send(texte);
        }
        Err(error) => tracing::error!(%error, "serializing a control message"),
    };

    Ok((rx_entrant, envoyer))
}

/// Opens ONE connection and emits the role declaration on it.
///
/// ⚠️ Returning `Ok` does NOT prove the platform accepted: the verdict
/// arrives later, as a `{"type":"error"}` message followed by a
/// closing. It is the trap P3's acceptance run paid for — see the trace
/// below, which says what it KNOWS and nothing more.
async fn ouvrir(url: &str, session: &str, jeton: Option<&str>) -> Result<Flux> {
    let (mut stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    // `WebSocketStream` is itself a `Sink<Message>`: we emit the
    // declaration WITHOUT splitting the stream, so as not to have to glue it back
    // afterwards. `servir` will split it once, and only once.
    stream
        .send(Message::Text(
            serde_json::json!({ "role": "agent", "session": session, "jeton": jeton }).to_string(),
        ))
        .await
        .context("supervisor declaration to the signaling")?;
    // 🔴 THIS TRACE SAID "supervisor REGISTERED", AND IT WAS A
    // LIE — found by P3's acceptance run, from evidence
    // (`journaux-plateforme-p3/vm-{1,2}-agent-sans-identite-plat.log`). It
    // came out at the handshake's EMISSION, hence BEFORE the
    // platform could refuse it; in both runs without an enrolment
    // secret, it showed while NO session was established,
    // and the connection dropped 5 ms later. **Whoever grepped for it
    // to know whether a session holds concluded the opposite of the truth.**
    //
    // It now says what it KNOWS: the declaration went out. What
    // the platform does with it arrives later, and on another thread — hence the
    // `error` arm of the receive loop below, which is what makes
    // the refusal observable.
    tracing::info!(
        session,
        jeton_present = jeton.is_some(),
        "supervisor declaration sent on the control session (acceptance still unknown)"
    );
    Ok(stream)
}

/// Holds the control session: serves it, and REOPENS it as long as the
/// supervisor is there.
///
/// ⚠️ **EACH ATTEMPT IS TRACED, UNCONDITIONALLY.** This repository has just
/// paid for the opposite (batch 33: traces behind early `return`s, and
/// the observability that had allowed the diagnosis destroyed by its own
/// remedy). The volume is bounded by the fallback's ceiling: a platform
/// dead for an hour costs ~120 lines, compared with the tens of
/// thousands `agent.log` writes in the same time.
#[allow(clippy::too_many_arguments)]
async fn tenir(
    premier: Flux,
    url: String,
    session: String,
    jeton_initial: Option<String>,
    identite: Option<watch::Receiver<Option<Identite>>>,
    tx_entrant: std::sync::mpsc::Sender<DepuisLaShell>,
    mut rx_sortant: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    let mut reprise = Reprise::neuve();
    let mut flux = Some(premier);
    loop {
        let current = match flux.take() {
            Some(f) => f,
            None => {
                let delai_ms = reprise.delai_ms();
                tokio::time::sleep(std::time::Duration::from_millis(delai_ms)).await;
                reprise.tentative_lancee();
                // 🔴 THE TOKEN IS REREAD HERE, NOT CAPTURED AT STARTUP: see
                // the header. `borrow()` returns the last known identity,
                // refreshed at each heartbeat by `plateforme/session.rs`.
                let jeton = current_token(&identite, &jeton_initial);
                match ouvrir(&url, &session, jeton.as_deref()).await {
                    Ok(f) => {
                        // ⚠️ UNCONDITIONAL, and it carries the count of
                        // DROPPED messages — including `0`, which is the negative
                        // witness: a resumption without loss is thus distinguished
                        // from a resumption that ate three announcements.
                        let jetes = drain(&mut rx_sortant);
                        tracing::warn!(
                            session = %session,
                            tentative = reprise.tentative(),
                            delai_ms,
                            messages_jetes = jetes,
                            "control session RESTORED (the messages sent during the cut \
                             are dropped, never replayed: the re-announcement on pair-present is what \
                             repairs the state)"
                        );
                        f
                    }
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            session = %session,
                            tentative = reprise.tentative(),
                            delai_ms,
                            "control session reconnection FAILED: no window can \
                             be announced or re-announced until it succeeds"
                        );
                        continue;
                    }
                }
            }
        };

        let debut = std::time::Instant::now();
        let fin = servir(current, &tx_entrant, &mut rx_sortant).await;
        let vecu_ms = debut.elapsed().as_millis() as u64;
        if let Fin::SuperviseurArrete = fin {
            tracing::info!(
                session = %session,
                "control session abandoned: the supervisor is gone"
            );
            return;
        }
        let rearme = reprise.connexion_terminee(vecu_ms);
        // ⚠️ UNCONDITIONAL too, and it is the one replacing the
        // "control connection to signaling lost" line from before this batch: that
        // one did not say nothing more would come back, this one says when
        // the rest arrives.
        tracing::warn!(
            session = %session,
            vecu_ms,
            repli_rearme = rearme,
            next_attempt_in_ms = reprise.delai_ms(),
            "control session LOST: reconnection scheduled"
        );
    }
}

/// The token to present NOW: the identity watch's if there is
/// one, otherwise the startup one.
fn current_token(
    identite: &Option<watch::Receiver<Option<Identite>>>,
    jeton_initial: &Option<String>,
) -> Option<String> {
    identite
        .as_ref()
        .and_then(|veille| veille.borrow().as_ref().map(|i| i.jeton.clone()))
        .or_else(|| jeton_initial.clone())
}

/// Drops what was waiting to be emitted, and returns how much. See the header for the
/// reason: a stale announcement is worse than no announcement.
fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> usize {
    let mut jetes = 0usize;
    while rx.try_recv().is_ok() {
        jetes += 1;
    }
    jetes
}

/// Serves ONE connection until it ends: reads what arrives, writes what leaves.
///
/// **A single `select!` where there were two tasks**, because the two
/// halves now share a fate: when the connection drops, the
/// same loop must open a new one. Two independent tasks would require
/// coordinating them to know which one reconnects.
async fn servir(
    flux: Flux,
    tx_entrant: &std::sync::mpsc::Sender<DepuisLaShell>,
    rx_sortant: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
) -> Fin {
    let (mut sortant, mut entrant) = flux.split();
    loop {
        tokio::select! {
            recu = entrant.next() => {
                let Some(recu) = recu else { return Fin::ConnexionPerdue };
                let message = match recu {
                    Ok(Message::Text(texte)) => texte,
                    // ⚠️ `Err` RETURNS CONTROL, it does not `continue`. Before this
                    // batch, a `let Ok(Message::Text(_)) = recu else
                    // { continue }` treated a stream error as a
                    // frame to ignore: on a PERSISTENT error, this
                    // loop spun idle consuming a core, without a
                    // trace. Not observed in production — the connection
                    // ended with `None` — but the path existed.
                    Err(error) => {
                        tracing::warn!(%error, "reading the control session failed");
                        return Fin::ConnexionPerdue;
                    }
                    Ok(Message::Close(_)) => return Fin::ConnexionPerdue,
                    // Ping/Pong/Binary: `tokio-tungstenite` answers
                    // Pings on its own, there is nothing to do here.
                    Ok(_) => continue,
                };
                if let Some(message) = analyser(&message) {
                    if tx_entrant.send(message).is_err() {
                        return Fin::SuperviseurArrete;
                    }
                }
            }
            a_emettre = rx_sortant.recv() => {
                let Some(texte) = a_emettre else { return Fin::SuperviseurArrete };
                if let Err(error) = sortant.send(Message::Text(texte)).await {
                    tracing::warn!(%error, "sending to the shell failed");
                    return Fin::ConnexionPerdue;
                }
            }
        }
    }
}

/// What a text frame means, or nothing.
fn analyser(texte: &str) -> Option<DepuisLaShell> {
    match serde_json::from_str::<DepuisLaShell>(texte) {
        Ok(message) => Some(message),
        // Signaling also sends `ice-config` and `peer-gone`, which
        // do not concern the control session: ignoring them is
        // the intended behaviour, not a defect.
        //
        // 🔴 BUT `error` IS NOT ONE OF THEM, and treating it as
        // such cost P3's acceptance run a diagnosis: the platform's
        // handshake refusal arrives in this form
        // (`{"type":"error","reason":…,"motif":…}`, `relais.ts`), and
        // it went out as `debug!` — hence invisible under operations'
        // `RUST_LOG=info`. The only remaining sign
        // was "control connection to signaling lost", which
        // reads like a network failure and not like a refusal.
        //
        // ⚠️ **THIS ARM COUNTS DOUBLE SINCE THE CONNECTION RESUMES**:
        // an expired token presented by a reconnection is told here, and
        // it is the ONLY way to distinguish "the platform is dead"
        // from "the platform refuses what I present to it".
        Err(_) => match serde_json::from_str::<serde_json::Value>(texte) {
            Ok(value) if value.get("type").and_then(|t| t.as_str()) == Some("error") => {
                tracing::warn!(
                    motif = %value.get("motif").and_then(|m| m.as_str()).unwrap_or("(absent)"),
                    raison = %value.get("reason").and_then(|r| r.as_str()).unwrap_or("(absente)"),
                    "control session REFUSED by the platform"
                );
                None
            }
            _ => {
                tracing::debug!(texte, "message ignored on the control session");
                None
            }
        },
    }
}
