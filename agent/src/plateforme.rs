//! Client du canal `/agent` : l'agent s'y enrôle, y bat le cœur, et en reçoit
//! son préfixe de session et son jeton d'agent.
//!
//! ⚠️ **CE CANAL N'EST PAS LE SIGNALING**, même s'il vit sur le même serveur.
//! `crate::signaling` et `crate::superviseur::signalisation` négocient une
//! session média ; celui-ci porte une IDENTITÉ. Sans lui, la garde de la
//! plateforme refuse la poignée de main des deux autres (sous-bloc P3), et
//! aucune session ne s'établit.
//!
//! 🔴 **LA REPRISE EST DU COMPORTEMENT NEUF, et c'est la raison d'être de ce
//! fichier.** Ni `signaling.rs` ni `signalisation.rs` n'en ont : leur chute
//! est seulement journalisée, ce qui est assumé là-bas parce que le média ne
//! dépend plus du signaling une fois l'offre échangée. **Ce raisonnement ne
//! se transpose pas ici** : ce canal porte le battement de cœur, donc `vu_a`.
//! Sans reprise, la PREMIÈRE coupure réseau rendrait la VM `injoignable`
//! définitivement, et la plateforme punirait une coupure de réseau comme une
//! panne d'agent.

pub mod repli;

// Tests extraits dans un fichier voisin (même mécanisme et même raison que
// `superviseur/table.rs`) : ils tiennent un vrai serveur WebSocket local et
// pèsent autant que le client lui-même.
#[cfg(test)]
#[path = "plateforme/tests.rs"]
mod tests;

use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, MotifCanal, VersLaPlateforme};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;

/// Période du battement de cœur.
///
/// ⚠️ **NON CALIBRÉE**, mais **PAS libre** : elle doit rester nettement sous
/// le seuil d'injoignabilité de la plateforme (`SEUIL_INJOIGNABLE_MS`,
/// 90 s au sous-bloc P3), sans quoi une VM parfaitement vivante serait
/// déclarée injoignable entre deux battements. Un facteur 3 laisse la place à
/// deux battements perdus. **Les deux constantes vivent dans des dépôts de
/// code différents et rien ne les lie mécaniquement** : changer l'une exige
/// de relire l'autre.
pub const PERIODE_BATTEMENT: Duration = Duration::from_secs(30);

/// Ce que la plateforme délivre, et que le reste de l'agent lit : le préfixe
/// qui nomme ses sessions, et le jeton qui ouvre ses poignées de main.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identite {
    pub prefixe: String,
    pub jeton: String,
    /// En millisecondes, comme tout horodatage de la plateforme.
    pub expire_a: i64,
}

/// Le canal ouvert, et le fil qui le tient. **Le lâcher arrête le battement
/// de cœur** : `main` le garde vivant pour toute la durée du processus.
pub struct Canal {
    identite: watch::Receiver<Option<Identite>>,
    /// Le fil de reprise. Jamais attendu — il ne se termine que sur un refus
    /// définitif —, mais conservé pour ne pas être abandonné en silence.
    #[allow(dead_code)]
    tache: tokio::task::JoinHandle<()>,
}

/// Pourquoi une session du canal s'est terminée.
enum Fin {
    /// Il n'y a rien à réessayer : la même tentative rendrait le même refus.
    Definitive,
    /// Le socket est tombé, ou la plateforme a refusé pour une raison qui
    /// peut changer (une VM peut être enrôlée après coup).
    Reprenable,
}

/// Compose l'URL du canal à partir de celle du signaling.
///
/// Le `trim_end_matches` n'est pas de la coquetterie : `ws://h:8080/` suivi
/// de `/agent` donnerait `ws://h:8080//agent`, et la montée de la plateforme
/// compare le chemin **exactement** — `//agent` n'est pas `/agent`, et le
/// socket serait fermé sur un `404` que rien du côté agent n'expliquerait.
pub fn url_du_canal(signaling_url: &str) -> String {
    format!("{}/agent", signaling_url.trim_end_matches('/'))
}

/// Ouvre le canal et le tient : enrôlement, battement, et reprise.
///
/// Rend immédiatement — l'identité arrive plus tard, par `attendre_identite`.
pub fn ouvrir(signaling_url: &str, vm: String, secret: String) -> Canal {
    let url = url_du_canal(signaling_url);
    let (tx, identite) = watch::channel(None);
    let tache = tokio::spawn(async move {
        // La tentative repart de ZÉRO après chaque enrôlement réussi : un
        // agent connecté depuis trois jours qui perd son réseau une seconde
        // doit reprendre en une demi-seconde, pas en trente.
        let mut tentative = 0u32;
        loop {
            let reussite_precedente = tx.borrow().is_some();
            if reussite_precedente {
                tentative = 0;
            }
            match une_session(&url, &vm, &secret, &tx).await {
                Fin::Definitive => {
                    tracing::warn!(
                        url,
                        "canal /agent abandonné DÉFINITIVEMENT : aucune reprise ne le rattrapera"
                    );
                    return;
                }
                Fin::Reprenable => {}
            }
            let delai = repli::delai_de_repli(tentative);
            tracing::info!(url, tentative, delai_ms = delai, "reprise du canal /agent");
            tentative = tentative.saturating_add(1);
            tokio::time::sleep(Duration::from_millis(delai)).await;
        }
    });
    Canal { identite, tache }
}

impl Canal {
    /// Attend la première identité. Rend `None` si la boucle de reprise a
    /// renoncé — c'est-à-dire si l'attente est vaine, et non « pas encore ».
    pub async fn attendre_identite(&mut self) -> Option<Identite> {
        loop {
            let courante = self.identite.borrow_and_update().clone();
            if courante.is_some() {
                return courante;
            }
            if self.identite.changed().await.is_err() {
                return None;
            }
        }
    }
}

/// Une session du canal, de la connexion à sa chute.
async fn une_session(
    url: &str,
    vm: &str,
    secret: &str,
    tx: &watch::Sender<Option<Identite>>,
) -> Fin {
    let mut socket = match connecter(url).await {
        Ok(socket) => socket,
        Err(erreur) => {
            tracing::warn!(url, %erreur, "ouverture du canal /agent échouée");
            return Fin::Reprenable;
        }
    };

    let enroler = match serde_json::to_string(&VersLaPlateforme::enroler(vm, secret)) {
        Ok(texte) => texte,
        // Une sérialisation qui échoue est un défaut de code, pas un aléa :
        // la réessayer rendrait la même erreur indéfiniment.
        Err(erreur) => {
            tracing::error!(%erreur, "sérialisation de l'enrôlement impossible");
            return Fin::Definitive;
        }
    };
    if let Err(erreur) = socket.send(Message::Text(enroler)).await {
        tracing::warn!(url, %erreur, "envoi de l'enrôlement échoué");
        return Fin::Reprenable;
    }

    let mut battement = tokio::time::interval(PERIODE_BATTEMENT);
    // Le premier `tick` d'un `interval` tokio est IMMÉDIAT : sans cette
    // consommation, un battement partirait avant même la réponse
    // d'enrôlement, et la plateforme le refuserait en `sequence`.
    battement.tick().await;
    let mut prefixe: Option<String> = None;

    loop {
        tokio::select! {
            _ = battement.tick() => {
                let Ok(texte) = serde_json::to_string(&VersLaPlateforme::battement()) else {
                    return Fin::Definitive;
                };
                if let Err(erreur) = socket.send(Message::Text(texte)).await {
                    tracing::warn!(url, %erreur, "battement de cœur non émis");
                    return Fin::Reprenable;
                }
            }
            recu = socket.next() => {
                let texte = match recu {
                    Some(Ok(Message::Text(texte))) => texte,
                    Some(Ok(Message::Close(cadre))) => {
                        tracing::warn!(url, ?cadre, "canal /agent fermé par la plateforme");
                        return Fin::Reprenable;
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(erreur)) => {
                        tracing::warn!(url, %erreur, "canal /agent perdu");
                        return Fin::Reprenable;
                    }
                    None => {
                        tracing::warn!(url, "canal /agent clos sans message de fermeture");
                        return Fin::Reprenable;
                    }
                };
                match serde_json::from_str::<DepuisLaPlateforme>(&texte) {
                    Ok(DepuisLaPlateforme::Enrole { prefixe: p, jeton, expire_a, .. }) => {
                        tracing::info!(url, prefixe = %p, expire_a, "agent enrôlé auprès de la plateforme");
                        prefixe = Some(p.clone());
                        let _ = tx.send(Some(Identite { prefixe: p, jeton, expire_a }));
                    }
                    Ok(DepuisLaPlateforme::BattementRecu { jeton, expire_a, .. }) => {
                        // Un battement AVANT tout enrôlement n'a pas de
                        // préfixe à porter : l'ignorer plutôt qu'inventer une
                        // identité sans nom.
                        let Some(prefixe) = prefixe.clone() else {
                            tracing::warn!(url, "battement reçu avant tout enrôlement, ignoré");
                            continue;
                        };
                        tracing::debug!(url, expire_a, "jeton d'agent rafraîchi");
                        let _ = tx.send(Some(Identite { prefixe, jeton, expire_a }));
                    }
                    Ok(DepuisLaPlateforme::Refus { motif, .. }) => {
                        return sur_refus(url, motif);
                    }
                    // 🔴 CE CAS EST TRÈS PROBABLEMENT UNE DIVERGENCE DE
                    // VERSION, et il se réessaie quand même — délibérément.
                    // `verifie_version` refuse à la désérialisation, donc une
                    // plateforme plus récente atterrit ici et non dans le bras
                    // `Refus`. Le réessayer ne peut pas résoudre la
                    // divergence, mais le repli est BORNÉ (30 s) et chaque
                    // tentative écrit CE message-ci, distinct de tous les
                    // autres : une incompatibilité de version ne se déguise
                    // donc pas en boucle de reconnexion muette, qui est le
                    // mode de panne que ce canal existe pour éviter. Et une
                    // plateforme redéployée à la bonne version reprend seule.
                    Err(erreur) => {
                        tracing::warn!(
                            url, %erreur, texte,
                            "message de la plateforme illisible (version divergente ?)"
                        );
                        return Fin::Reprenable;
                    }
                }
            }
        }
    }
}

/// 🔴 **`version` NE SE RÉESSAIE PAS. Tous les autres motifs, si.**
///
/// C'est l'asymétrie de la décision D4 du plan, et elle a une raison : une
/// version divergente rendra le même refus à la millionième tentative, alors
/// qu'un enrôlement refusé cesse de l'être dès que l'exploitant enrôle la VM,
/// sans que personne n'ait à redémarrer l'agent.
fn sur_refus(url: &str, motif: MotifCanal) -> Fin {
    match motif {
        MotifCanal::Version => {
            tracing::warn!(
                url,
                version_emise = proto::plateforme::PLATEFORME_VERSION,
                "la plateforme REFUSE la version du canal /agent : aucune reprise, \
                 il faut rebâtir l'agent ou la plateforme"
            );
            Fin::Definitive
        }
        autre => {
            tracing::warn!(url, ?autre, "canal /agent refusé par la plateforme");
            Fin::Reprenable
        }
    }
}

async fn connecter(
    url: &str,
) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
{
    let (flux, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au canal {url}"))?;
    Ok(flux)
}
