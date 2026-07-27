//! Client WebSocket vers le serveur de signaling.
//!
//! L'agent est toujours le répondant : il reçoit une offre SDP et renvoie une
//! réponse. Aucun trickle ICE — les candidats hôtes voyagent dans le SDP.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

pub struct SignalingHandle {
    /// Offres SDP reçues du navigateur.
    pub offers: mpsc::Receiver<String>,
    /// Réponses SDP à renvoyer au navigateur.
    pub answers: mpsc::Sender<String>,
}

/// Se connecte au signaling et démarre la boucle d'échange en tâche de fond.
pub async fn run_signaling(url: &str, session: &str) -> Result<SignalingHandle> {
    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    let (mut sink, mut source) = stream.split();

    let hello = serde_json::json!({ "role": "agent", "session": session });
    sink.send(Message::Text(hello.to_string())).await?;
    tracing::info!(session, "agent enregistré auprès du signaling");

    let (offer_tx, offers) = mpsc::channel::<String>(4);
    let (answers, mut answer_rx) = mpsc::channel::<String>(4);
    // Signal de fin de connexion : la tâche d'émission ne doit pas rester
    // bloquée indéfiniment sur `answer_rx.recv()` une fois la connexion
    // morte côté réception — sans quoi elle survivrait, inutile, tant que
    // `SignalingHandle::answers` reste vivant (potentiellement toute la
    // durée du processus une fois l'agent durable, au-delà de cette tâche).
    let (closed_tx, mut closed_rx) = watch::channel(false);

    // Réception : offres et erreurs venant du signaling.
    tokio::spawn(async move {
        while let Some(message) = source.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) | Err(_) => break,
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
                        // `try_send`, jamais `.send(...).await` : cette boucle
                        // sert aussi `peer-gone` et `error` juste en dessous,
                        // elle ne doit donc jamais s'endormir sur un canal
                        // plein faute de consommateur. Une offre qui arrive
                        // alors qu'une précédente n'a pas encore été
                        // consommée est délibérément écartée (et journalisée)
                        // plutôt que de geler la réception.
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
                Some("peer-gone") => tracing::info!("le client s'est déconnecté"),
                Some("error") => {
                    tracing::error!(raison = %parsed["reason"], "erreur de signaling")
                }
                other => tracing::debug!(?other, "message de signaling ignoré"),
            }
        }
        tracing::info!("boucle de réception du signaling terminée");
        // Réveille la tâche d'émission pour qu'elle se termine à son tour au
        // lieu d'attendre indéfiniment sur un canal `answers` encore ouvert.
        let _ = closed_tx.send(true);
    });

    // Émission : réponses SDP. Se termine soit quand `answers` est fermé
    // (plus aucun expéditeur côté appelant), soit — c'est le cas manquant
    // avant ce correctif — quand la réception a constaté la fin de la
    // connexion, plutôt que d'attendre pour toujours un message qui ne
    // viendra plus.
    tokio::spawn(async move {
        loop {
            tokio::select! {
                sdp = answer_rx.recv() => {
                    let Some(sdp) = sdp else { break };
                    let payload = serde_json::json!({ "type": "answer", "sdp": sdp });
                    if sink.send(Message::Text(payload.to_string())).await.is_err() {
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

    Ok(SignalingHandle { offers, answers })
}
