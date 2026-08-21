//! Une session du canal `/agent` : de la connexion à sa chute, et le refus.
//!
//! 🔴 EXTRAIT AVANT L'ADDITION, JAMAIS APRÈS. `plateforme.rs` était à **490**
//! lignes, marge **10** ; le sous-bloc G3 y ajoute une seconde file, une
//! branche descendante et deux montantes. Le plan de G3 le relevait à 453 le
//! 20 août, marge 47 : c'est le sous-bloc G2 qui a consommé la différence, et
//! le remède reste celui que `CLAUDE.md` impose — une extraction jouée
//! d'avance, jamais une compression.
//!
//! ⚠️ CE FICHIER N'EST PAS `#[cfg(windows)]`, et la « Convention de module
//! enfant » de `CLAUDE.md` ne s'applique donc pas : c'est un `mod` ordinaire
//! déclaré chez son parent, pour la règle des 500 lignes et pour elle seule.
//!
//! 🔴 AUCUNE LIGNE DE COMPORTEMENT N'A CHANGÉ à l'extraction. Les visibilités
//! sont passées à `pub(super)` là où il le fallait, **et nulle part ailleurs**.


use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use proto::plateforme::{DepuisLaPlateforme, MotifCanal, VersLaPlateforme};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use super::{Fin, Identite, Installation, Ordre, PERIODE_BATTEMENT};


/// Une session du canal, de la connexion à sa chute.
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
            // ⚠️ CE BRAS EST CE QUI REND LE CANAL BIDIRECTIONNEL. Sans lui, la
            // file grossirait jusqu'à sa borne puis rejetterait en silence :
            // l'agent croirait émettre son catalogue, la plateforme resterait
            // vide, et RIEN ne le dirait.
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
                    Ok(DepuisLaPlateforme::Refus { version, motif }) => {
                        return sur_refus(url, version, &motif);
                    }
                    // 🔴 CE BRAS DOIT EXISTER, ET IL NE DOIT SURTOUT PAS
                    // FERMER LA SESSION. Sans lui, un ordre parfaitement
                    // valide tomberait dans le bras `Err` ci-dessous, qui rend
                    // `Fin::Reprenable` : le canal se reprendrait en boucle à
                    // chaque clic de l'utilisateur, et la trace accuserait une
                    // divergence de version qui n'existe pas.
                    Ok(DepuisLaPlateforme::Lancer { demande, cle, .. }) => {
                        tracing::info!(url, %demande, %cle, "ordre de lancement reçu");
                        // Un envoi qui échoue signifie que le consommateur
                        // n'est plus là — l'agent s'arrête, ou personne n'a
                        // pris la file. On le journalise sans tuer le canal :
                        // le battement de cœur doit continuer.
                        if ordres.send(Ordre::Lancer { demande, cle }).is_err() {
                            tracing::warn!(url, "aucun consommateur d'ordres, lancement abandonné");
                        }
                    }
                    // 🔴 CE BRAS DOIT EXISTER, POUR LA RAISON EXACTE DU BRAS
                    // CI-DESSUS. Sans lui, un inventaire parfaitement valide
                    // tomberait dans le bras `Err`, qui rend `Fin::Reprenable` :
                    // le canal se reprendrait à CHAQUE réconciliation qui
                    // annonce une icône neuve, et la trace accuserait une
                    // divergence de version qui n'existe pas.
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
                    // 🔴 CE BRAS DOIT EXISTER, POUR LA RAISON EXACTE DES DEUX
                    // BRAS CI-DESSUS — et c'est la CINQUIÈME fois que ce dépôt
                    // paie cette leçon (D5 `Sommeil`, D6 `Part`, D7 `Audio`,
                    // D8 `PleinEcran`, G2 `IconesManquantes`). Sans lui, un
                    // ordre d'installation parfaitement valide tomberait dans
                    // le bras `Err`, qui rend `Fin::Reprenable` : le canal se
                    // reprendrait à chaque installation demandée, et la trace
                    // accuserait une divergence de version qui n'existe pas.
                    //
                    // ⚠️ IL PART DANS L'AUTRE FILE, et `plateforme/installation.rs`
                    // dit pourquoi : le consommateur n'est pas le fil COM de la
                    // découverte mais un fil `tokio`, qui téléchargera plusieurs
                    // centaines de mégaoctets puis attendra un processus des
                    // minutes durant. Le mettre dans `Ordre` figerait le
                    // catalogue PENDANT L'INSTALLATION dont on attend qu'il
                    // rende compte.
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
///
/// ⚠️ **CE BRAS A ÉTÉ INATTEIGNABLE DANS LE SEUL CAS POUR LEQUEL IL EXISTE, et
/// c'était mesuré** (recette G1, 20 août 2026, UNE exécution) : pour
/// l'atteindre il fallait avoir DÉSÉRIALISÉ un `refus`, donc avoir accepté son
/// champ `v` — or la plateforme émet son refus avec SA version. Un agent v1
/// face à une plateforme v2 tombait donc dans la branche « illisible » de
/// [`une_session`], qui est reprenable, et reprenait indéfiniment.
/// **Le refus est hors versionnement depuis la correction du même jour**
/// (`proto/src/plateforme.rs`, clauses 1 à 3 de son en-tête) : ce bras est
/// désormais atteignable, et deux tests de bout en bout le jouent contre un
/// faux canal qui écrit la trame BRUTE d'une autre version.
///
/// 🔴 **`version_emise` ET `version_recue` SONT TOUTES DEUX AU JOURNAL, et
/// c'est le seul endroit du dépôt où l'écart se lit.** Une seule des deux ne
/// dirait pas dans quel sens rattraper — rebâtir l'agent, ou la plateforme.
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
            tracing::warn!(url, ?autre, version_recue, "canal /agent refusé par la plateforme");
            Fin::Reprenable
        }
        // 🔴 UN MOTIF QUE NOUS NE CONNAISSONS PAS SE JOURNALISE **VERBATIM** ET
        // SE RÉESSAIE. Le journaliser est ce qui empêche le mode de panne de
        // revenir par la porte du motif : sans cette branche, un motif ajouté
        // par une version future retomberait dans « message illisible », qui
        // n'en dit pas le nom. Le réessayer est le choix prudent — nous ne
        // savons pas s'il est définitif, et la reprise est bornée par le repli
        // exponentiel (30 s) tout en écrivant CETTE ligne à chaque tour.
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
) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
{
    let (flux, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au canal {url}"))?;
    Ok(flux)
}
