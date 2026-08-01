//! Connexion du superviseur à la session de contrôle du signaling.
//!
//! Distincte de `crate::signaling`, qui ne connaît que les offres et réponses
//! SDP d'une session média. Ici on envoie et reçoit des messages de contrôle,
//! et il n'y a jamais de négociation WebRTC.
//!
//! Le récepteur rendu est un `std::sync::mpsc::Receiver` et non un canal
//! tokio : la boucle du superviseur est synchrone (elle appelle des API
//! Windows bloquantes) et le sonde par `try_recv`.

#![cfg(windows)]

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::protocole::{DepuisLaShell, VersLaShell};

/// Ouvre la connexion, se déclare comme `agent` sur la session donnée, et rend
/// de quoi envoyer et recevoir.
pub async fn connecter(
    url: &str,
    session: &str,
) -> Result<(
    std::sync::mpsc::Receiver<DepuisLaShell>,
    impl Fn(&VersLaShell) + Send + Sync + 'static,
)> {
    let (stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    let (mut sortant, mut entrant) = stream.split();

    sortant
        .send(Message::Text(
            serde_json::json!({ "role": "agent", "session": session }).to_string(),
        ))
        .await
        .context("déclaration du superviseur au signaling")?;
    tracing::info!(session, "superviseur enregistré sur la session de contrôle");

    // Émission : une tâche tokio consomme une file, pour que l'envoi reste
    // appelable depuis la boucle synchrone du superviseur.
    let (tx_sortant, mut rx_sortant) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(texte) = rx_sortant.recv().await {
            if let Err(erreur) = sortant.send(Message::Text(texte)).await {
                tracing::warn!(%erreur, "émission vers la shell échouée");
                break;
            }
        }
    });

    // Réception : les messages que la shell nous adresse.
    let (tx_entrant, rx_entrant) = std::sync::mpsc::channel();
    tokio::spawn(async move {
        while let Some(recu) = entrant.next().await {
            let Ok(Message::Text(texte)) = recu else { continue };
            match serde_json::from_str::<DepuisLaShell>(&texte) {
                Ok(message) => {
                    if tx_entrant.send(message).is_err() {
                        break; // le superviseur s'arrête
                    }
                }
                // Le signaling envoie aussi `ice-config` et `peer-gone`, qui
                // ne concernent pas la session de contrôle : les ignorer est
                // le comportement voulu, pas un défaut.
                Err(_) => tracing::debug!(texte, "message ignoré sur la session de contrôle"),
            }
        }
        tracing::warn!("connexion de contrôle au signaling perdue");
    });

    let envoyer = move |message: &VersLaShell| match serde_json::to_string(message) {
        Ok(texte) => {
            let _ = tx_sortant.send(texte);
        }
        Err(erreur) => tracing::error!(%erreur, "sérialisation d'un message de contrôle"),
    };

    Ok((rx_entrant, envoyer))
}
