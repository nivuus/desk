//! Client WebSocket vers le serveur de signaling.
//!
//! L'agent est toujours le répondant : il reçoit une offre SDP et renvoie une
//! réponse. Aucun trickle ICE — les candidats hôtes voyagent dans le SDP.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

/// Ce que le signaling nous dit du relais à employer.
#[derive(Debug, Clone)]
pub struct ConfigIce {
    /// Adresse du serveur TURN, extraite de l'URL `turn:hôte:port`.
    pub serveur: std::net::SocketAddr,
    pub username: String,
    pub credential: String,
}

pub struct SignalingHandle {
    /// Offres SDP reçues du navigateur.
    pub offers: mpsc::Receiver<String>,
    /// Réponses SDP à renvoyer au navigateur.
    pub answers: mpsc::Sender<String>,
    /// Passe à `true` dès que l'une des deux tâches de fond (réception ou
    /// émission) constate la fin de la connexion. C'est le seul moyen pour
    /// l'appelant de détecter une perte de signaling après l'échange initial
    /// (I6 de la revue) : sans lui, ni les `JoinHandle` jetés ni l'absence de
    /// relecture du canal `offers` après la première ne rendaient une chute
    /// du signaling visible.
    pub closed: watch::Receiver<bool>,
    /// Configuration ICE délivrée par le serveur juste après la déclaration
    /// de rôle. `watch` plutôt que `mpsc` : c'est un ÉTAT, dont seule la
    /// dernière valeur compte, et l'appelant doit pouvoir le lire même s'il
    /// arrive après l'émission.
    pub ice_config: watch::Receiver<Option<ConfigIce>>,
    /// Conservées pour que les deux tâches de fond ne soient pas
    /// complètement abandonnées : `demarrage.rs` ne les attend pas en
    /// fonctionnement normal (le transport ne dépend plus du signaling une
    /// fois l'offre/réponse échangée), mais les jeter silencieusement
    /// masquerait un panic éventuel à l'intérieur de l'une d'elles.
    // `#[allow(dead_code)]` : jamais lus dans cette tâche mono-session (voir
    // le commentaire ci-dessus), mais conservés à dessein — le lint ne le
    // sait pas.
    #[allow(dead_code)]
    pub receiver_task: JoinHandle<()>,
    #[allow(dead_code)]
    pub sender_task: JoinHandle<()>,
}

/// Se connecte au signaling et démarre la boucle d'échange en tâche de fond.
/// `jeton` porte le jeton d'agent délivré par le canal `/agent`
/// (`crate::plateforme`). **`None` fait refuser la poignée de main par la
/// plateforme depuis le sous-bloc P3** : la garde n'accepte plus un
/// `{"role":"agent"}` anonyme, et le socket se ferme sans qu'aucune session
/// ne s'établisse.
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
    // 🔴 « agent ENREGISTRÉ » ÉTAIT FAUX, exactement comme son jumeau de
    // `superviseur/signalisation.rs` : la trace sort à l'ÉMISSION, donc avant
    // tout verdict de la plateforme, et elle s'affichait aux deux exécutions
    // de recette de P3 où AUCUNE session ne s'établissait. Ici le refus est
    // déjà rendu visible plus bas (`Some("error") => tracing::error!`) ;
    // c'est le seul libellé qui trompait.
    tracing::info!(
        session,
        "déclaration de l'agent émise au signaling (acceptation encore inconnue)"
    );

    let (offer_tx, offers) = mpsc::channel::<String>(4);
    let (answers, mut answer_rx) = mpsc::channel::<String>(4);
    // Signal de fin de connexion, partagé entre les deux tâches : quelle que
    // soit celle qui détecte la perte de connexion en premier, l'autre s'en
    // aperçoit et se termine à son tour au lieu de rester bloquée
    // indéfiniment (voir les deux `select!` ci-dessous).
    let (closed_tx, closed_rx) = watch::channel(false);
    let (ice_tx, ice_config) = watch::channel::<Option<ConfigIce>>(None);
    let closed_tx_sender_side = closed_tx.clone();
    let closed_rx_sender_side = closed_rx.clone();

    // Réception : offres et erreurs venant du signaling.
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
                Some("ice-config") => {
                    match analyser_config_ice(&parsed) {
                        Some(config) => {
                            tracing::info!(serveur = %config.serveur, "configuration TURN reçue");
                            let _ = ice_tx.send(Some(config));
                        }
                        None => tracing::warn!(
                            "configuration ICE reçue mais inexploitable : session sans relais"
                        ),
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
    // (plus aucun expéditeur côté appelant), soit quand l'une des deux
    // tâches a constaté la fin de la connexion — jamais en attendant pour
    // toujours un message qui ne viendra plus (c'était le défaut avant ce
    // correctif : `while let Some(sdp) = answer_rx.recv().await` seul).
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
        receiver_task,
        sender_task,
    })
}

/// Extrait la première entrée TURN exploitable d'un message `ice-config`.
///
/// Résout le nom d'hôte : `Candidate::relayed` et le socket UDP veulent une
/// `SocketAddr`, pas une URL. Une résolution qui échoue rend `None` — session
/// sans relais plutôt que session sans démarrage.
fn analyser_config_ice(message: &serde_json::Value) -> Option<ConfigIce> {
    use std::net::ToSocketAddrs;

    let serveurs = message["iceServers"].as_array()?;
    for entree in serveurs {
        let urls = entree["urls"].as_str()?;
        // Forme attendue : `turn:hôte:port`. On ignore les entrées `stun:` —
        // l'adresse réflexive nous vient de la réponse Allocate elle-même.
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

    /// Rend l'URL d'un faux signaling et le premier message reçu.
    async fn premiere_poignee_de_main(jeton: Option<&str>) -> String {
        let ecoute = TcpListener::bind("127.0.0.1:0").await.expect("écoute locale");
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

    /// 🔴 Sans le jeton sur le fil, la garde de la plateforme refuse la
    /// poignée de main et AUCUNE session ne s'établit (sous-bloc P3). La
    /// mutation qui rougit ce test — retirer la clé `jeton` du `json!` — ne
    /// casse RIEN à la compilation, ni ici ni chez l'appelant : c'est une
    /// panne de bout en bout que seul le fil peut révéler.
    #[tokio::test]
    async fn la_poignee_de_main_porte_le_jeton_d_agent() {
        let poignee = premiere_poignee_de_main(Some("jwt.d.agent")).await;
        assert_eq!(
            poignee,
            r#"{"jeton":"jwt.d.agent","role":"agent","session":"P:w-1"}"#
        );
    }

    /// Sans jeton, le champ part à `null` — que la garde traite exactement
    /// comme une absence. Ce test fige la forme, pour qu'un futur repli ne
    /// puisse pas y glisser une chaîne vide qui aurait l'air d'un jeton.
    #[tokio::test]
    async fn sans_jeton_la_poignee_de_main_le_dit_au_lieu_de_l_inventer() {
        let poignee = premiere_poignee_de_main(None).await;
        assert_eq!(poignee, r#"{"jeton":null,"role":"agent","session":"P:w-1"}"#);
    }
}
