//! Client WebSocket vers le serveur de signaling.
//!
//! L'agent est toujours le répondant : il reçoit une offre SDP et renvoie une
//! réponse. Aucun trickle ICE — les candidats hôtes voyagent dans le SDP.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
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
                        if offer_tx.send(sdp.to_string()).await.is_err() {
                            break;
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
    });

    // Émission : réponses SDP.
    tokio::spawn(async move {
        while let Some(sdp) = answer_rx.recv().await {
            let payload = serde_json::json!({ "type": "answer", "sdp": sdp });
            if sink.send(Message::Text(payload.to_string())).await.is_err() {
                break;
            }
        }
    });

    Ok(SignalingHandle { offers, answers })
}
