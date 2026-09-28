//! One session of the `/agent` channel: from connection to its fall, and the refusal.
//!
//! 🔴 EXTRACTED BEFORE THE ADDITION, NEVER AFTER. `plateforme.rs` was at **490**
//! lines, margin **10**; sub-block G3 adds to it a second queue, a
//! downstream branch and two upstream ones. G3's plan noted it at 453 on
//! 20 August, margin 47: it is sub-block G2 that consumed the difference, and
//! the remedy remains the one `CLAUDE.md` imposes — an extraction played
//! in advance, never a compression.
//!
//! ⚠️ THIS FILE IS NOT `#[cfg(windows)]`, and the "Child module
//! convention" of `CLAUDE.md` therefore does not apply: it is an ordinary `mod`
//! declared in its parent, for the 500-line rule and for it alone.
//!
//! 🔴 NO LINE OF BEHAVIOUR CHANGED in the extraction. Visibilities
//! were switched to `pub(super)` where needed, **and nowhere else**.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, MotifCanal, VersLaPlateforme};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use super::{Fin, Identite, Installation, Ordre, PERIODE_BATTEMENT};

/// One session of the channel, from connection to its fall.
pub(super) async fn une_session(
    url: &str,
    vm: &str,
    secret: &str,
    tx: &watch::Sender<Option<Identite>>,
    a_emettre: &mut mpsc::Receiver<VersLaPlateforme>,
    ordres: &mpsc::UnboundedSender<Ordre>,
    installations: &mpsc::UnboundedSender<Installation>,
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
        // A serialisation that fails is a code defect, not a random event:
        // retrying it would return the same error indefinitely.
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
    // The first `tick` of a tokio `interval` is IMMEDIATE: without this
    // consumption, a heartbeat would leave before even the enrolment
    // response, and the platform would refuse it as `sequence`.
    battement.tick().await;
    let mut prefixe: Option<String> = None;

    loop {
        tokio::select! {
            // ⚠️ THIS ARM IS WHAT MAKES THE CHANNEL BIDIRECTIONAL. Without it, the
            // queue would grow up to its bound then silently reject:
            // the agent would believe it emits its catalogue, the platform would stay
            // empty, and NOTHING would say so.
            Some(message) = a_emettre.recv() => {
                let Ok(texte) = serde_json::to_string(&message) else {
                    tracing::error!("sérialisation d'un message montant impossible");
                    continue;
                };
                if let Err(erreur) = socket.send(Message::Text(texte)).await {
                    tracing::warn!(url, %erreur, "message montant non émis");
                    return Fin::Reprenable;
                }
            }
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
                        // A heartbeat BEFORE any enrolment has no
                        // prefix to carry: ignore it rather than invent a
                        // nameless identity.
                        let Some(prefixe) = prefixe.clone() else {
                            tracing::warn!(url, "battement reçu avant tout enrôlement, ignoré");
                            continue;
                        };
                        tracing::debug!(url, expire_a, "jeton d'agent rafraîchi");
                        let _ = tx.send(Some(Identite { prefixe, jeton, expire_a }));
                    }
                    Ok(DepuisLaPlateforme::Refus { version, motif }) => {
                        return sur_refus(url, version, &motif);
                    }
                    // 🔴 THIS ARM MUST EXIST, AND ABOVE ALL IT MUST NOT
                    // CLOSE THE SESSION. Without it, a perfectly
                    // valid order would fall into the `Err` arm below, which returns
                    // `Fin::Reprenable`: the channel would reconnect in a loop at
                    // each user click, and the trace would accuse a
                    // version divergence that does not exist.
                    Ok(DepuisLaPlateforme::Lancer { demande, cle, .. }) => {
                        tracing::info!(url, %demande, %cle, "ordre de lancement reçu");
                        // A send that fails means the consumer
                        // is no longer there — the agent is stopping, or no one
                        // took the queue. We log it without killing the channel:
                        // the heartbeat must continue.
                        if ordres.send(Ordre::Lancer { demande, cle }).is_err() {
                            tracing::warn!(url, "aucun consommateur d'ordres, lancement abandonné");
                        }
                    }
                    // 🔴 THIS ARM MUST EXIST, FOR THE EXACT REASON OF THE ARM
                    // ABOVE. Without it, a perfectly valid inventory
                    // would fall into the `Err` arm, which returns `Fin::Reprenable`:
                    // the channel would reconnect at EACH reconciliation that
                    // announces a new icon, and the trace would accuse a
                    // version divergence that does not exist.
                    Ok(DepuisLaPlateforme::IconesManquantes { empreintes, .. }) => {
                        tracing::info!(
                            url, manquantes = empreintes.len(),
                            "inventaire d'icones manquantes reçu"
                        );
                        if ordres.send(Ordre::IconesManquantes { empreintes }).is_err() {
                            tracing::warn!(
                                url,
                                "aucun consommateur d'ordres, televersement d'icones abandonne"
                            );
                        }
                    }
                    // 🔴 THIS ARM MUST EXIST, FOR THE EXACT REASON OF THE TWO
                    // ARMS ABOVE — and it is the FIFTH time this repository
                    // pays this lesson (D5 `Sommeil`, D6 `Part`, D7 `Audio`,
                    // D8 `PleinEcran`, G2 `IconesManquantes`). Without it, a
                    // perfectly valid installation order would fall into
                    // the `Err` arm, which returns `Fin::Reprenable`: the channel would
                    // reconnect at each requested installation, and the trace
                    // would accuse a version divergence that does not exist.
                    //
                    // ⚠️ IT GOES INTO THE OTHER QUEUE, and `plateforme/installation.rs`
                    // says why: the consumer is not the COM thread of
                    // discovery but a `tokio` thread, which will download several
                    // hundred megabytes then wait for a process for
                    // minutes. Putting it in `Ordre` would freeze the
                    // catalogue DURING THE INSTALLATION it is expected
                    // to report on.
                    Ok(DepuisLaPlateforme::Installer {
                        installation, url: source, nom, taille, sha256, ..
                    }) => {
                        tracing::info!(
                            url, %installation, %nom, taille,
                            "ordre d'installation reçu"
                        );
                        let ordre = Installation {
                            id: installation, url: source, nom, taille, sha256,
                        };
                        if installations.send(ordre).is_err() {
                            tracing::warn!(
                                url,
                                "aucun consommateur d'installations, ordre abandonne"
                            );
                        }
                    }
                    // 🔴 THIS CASE IS VERY PROBABLY A VERSION DIVERGENCE,
                    // and it is retried anyway — deliberately.
                    // `verifie_version` refuses at deserialisation, so a
                    // more recent platform lands here and not in the
                    // `Refus` arm. Retrying it cannot resolve the
                    // divergence, but the backoff is BOUNDED (30 s) and each
                    // attempt writes THIS message, distinct from all the
                    // others: a version incompatibility therefore does not disguise
                    // itself as a silent reconnection loop, which is the
                    // failure mode this channel exists to avoid. And a
                    // platform redeployed at the right version recovers on its own.
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

/// 🔴 **`version` IS NOT RETRIED. All the other reasons are.**
///
/// It is the asymmetry of decision D4 of the plan, and it has a reason: a
/// divergent version will return the same refusal at the millionth attempt, whereas
/// a refused enrolment stops being refused as soon as the operator enrols the VM,
/// without anyone having to restart the agent.
///
/// ⚠️ **THIS ARM WAS UNREACHABLE IN THE ONLY CASE IT EXISTS FOR, and
/// it was measured** (acceptance run G1, 20 August 2026, ONE run): to
/// reach it one had to have DESERIALISED a `refus`, hence to have accepted its
/// `v` field — yet the platform emits its refusal with ITS version. A v1 agent
/// facing a v2 platform therefore fell into the "unreadable" branch of
/// [`une_session`], which is recoverable, and reconnected indefinitely.
/// **The refusal has been outside versioning since the fix of the same day**
/// (`proto/src/plateforme.rs`, clauses 1 to 3 of its header): this arm is
/// now reachable, and two end-to-end tests play it against a
/// fake channel that writes the RAW frame of another version.
///
/// 🔴 **`version_emise` AND `version_recue` ARE BOTH IN THE LOG, and
/// it is the only place in the repository where the gap can be read.** Only one of the two
/// would not say in which direction to catch up — rebuild the agent, or the platform.
pub(super) fn sur_refus(url: &str, version_recue: u8, motif: &str) -> Fin {
    match MotifCanal::depuis_mot(motif) {
        Some(MotifCanal::Version) => {
            tracing::warn!(
                url,
                version_emise = proto::plateforme::PLATEFORME_VERSION,
                version_recue,
                "la plateforme REFUSE la version du canal /agent : aucune reprise, \
                 il faut rebâtir l'agent ou la plateforme"
            );
            Fin::Definitive
        }
        Some(autre) => {
            tracing::warn!(
                url,
                ?autre,
                version_recue,
                "canal /agent refusé par la plateforme"
            );
            Fin::Reprenable
        }
        // 🔴 A REASON WE DO NOT KNOW IS LOGGED **VERBATIM** AND
        // IS RETRIED. Logging it is what prevents the failure mode from
        // coming back through the door of the reason: without this branch, a reason added
        // by a future version would fall back into "unreadable message", which
        // does not say its name. Retrying it is the prudent choice — we do not
        // know whether it is final, and the reconnection is bounded by the exponential
        // backoff (30 s) while writing THIS line at each round.
        None => {
            tracing::warn!(
                url,
                motif,
                version_recue,
                version_emise = proto::plateforme::PLATEFORME_VERSION,
                "canal /agent refusé pour un motif que cette version de l'agent ne \
                 connaît pas : réessai, et le motif est journalisé tel quel"
            );
            Fin::Reprenable
        }
    }
}

async fn connecter(
    url: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let (flux, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au canal {url}"))?;
    Ok(flux)
}
