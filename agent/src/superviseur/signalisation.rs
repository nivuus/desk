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
///
/// `jeton` porte le jeton d'agent délivré par le canal `/agent`
/// (`crate::plateforme`). **`None` fait refuser la poignée de main par la
/// plateforme depuis le sous-bloc P3** : la garde n'accepte plus un
/// `{"role":"agent"}` anonyme.
pub async fn connecter(
    url: &str,
    session: &str,
    jeton: Option<&str>,
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
            serde_json::json!({ "role": "agent", "session": session, "jeton": jeton })
                .to_string(),
        ))
        .await
        .context("déclaration du superviseur au signaling")?;
    // 🔴 CETTE TRACE DISAIT « superviseur ENREGISTRÉ », ET C'ÉTAIT UN
    // MENSONGE — relevé par la recette de P3, sur pièces
    // (`journaux-plateforme-p3/vm-{1,2}-agent-sans-identite-plat.log`). Elle
    // sortait à l'ÉMISSION de la poignée de main, donc AVANT que la
    // plateforme ait pu la refuser ; aux deux exécutions sans secret
    // d'enrôlement, elle s'affichait alors qu'AUCUNE session ne s'établissait,
    // et la connexion tombait 5 ms plus tard. **Qui la cherchait au `grep`
    // pour savoir si une session tient concluait l'inverse de la vérité.**
    //
    // Elle dit désormais ce qu'elle SAIT : la déclaration est partie. Ce que
    // la plateforme en fait arrive plus tard, et sur un autre fil — d'où le
    // bras `error` de la boucle de réception ci-dessous, qui est ce qui rend
    // le refus observable.
    tracing::info!(
        session,
        "déclaration du superviseur émise sur la session de contrôle (acceptation encore inconnue)"
    );

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
                //
                // 🔴 MAIS `error` N'EST PAS DE CEUX-LÀ, et le traiter comme
                // tel a coûté un diagnostic à la recette de P3 : le refus de
                // poignée de main de la plateforme arrive sous cette forme
                // (`{"type":"error","reason":…,"motif":…}`, `relais.ts`), et
                // il partait en `debug!` — donc invisible sous le
                // `RUST_LOG=info` de l'exploitation. Le seul signe restant
                // était `connexion de contrôle au signaling perdue`, qui se
                // lit comme une panne réseau et non comme un refus.
                Err(_) => match serde_json::from_str::<serde_json::Value>(&texte) {
                    Ok(valeur) if valeur.get("type").and_then(|t| t.as_str()) == Some("error") => {
                        tracing::warn!(
                            motif = %valeur.get("motif").and_then(|m| m.as_str()).unwrap_or("(absent)"),
                            raison = %valeur.get("reason").and_then(|r| r.as_str()).unwrap_or("(absente)"),
                            "session de contrôle REFUSÉE par la plateforme"
                        );
                    }
                    _ => tracing::debug!(texte, "message ignoré sur la session de contrôle"),
                },
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
