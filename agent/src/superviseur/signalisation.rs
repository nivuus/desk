//! Connexion du superviseur à la session de contrôle du signaling, **et sa
//! REPRISE**.
//!
//! Distincte de `crate::signaling`, qui ne connaît que les offres et réponses
//! SDP d'une session média. Ici on envoie et reçoit des messages de contrôle,
//! et il n'y a jamais de négociation WebRTC.
//!
//! Le récepteur rendu est un `std::sync::mpsc::Receiver` et non un canal
//! tokio : la boucle du superviseur est synchrone (elle appelle des API
//! Windows bloquantes) et le sonde par `try_recv`.
//!
//! 🔴 **CE SOCKET NE SE ROUVRAIT JAMAIS, ET C'ÉTAIT UNE PANNE MUETTE
//! D'EXPLOITATION** (legs n°1 du lot 17, fermé ici). Après un redémarrage du
//! service `desk-plateforme`, les deux tâches de fond sortaient de leur
//! boucle, journalisaient `émission vers la shell échouée` puis `connexion de
//! contrôle au signaling perdue`, et **plus rien** : `rx_shell` était fermé,
//! `envoyer` écrivait dans un canal sans consommateur (`let _ = …`), et la
//! boucle du superviseur continuait de tourner en croyant parler à quelqu'un.
//! Aucune fenêtre ne pouvait plus être annoncée **ni réannoncée**, ce qui
//! rend inopérante la correction du lot 17 (`pair-present`) — elle a besoin
//! de ce socket pour être délivrée. Le seul remède était de relancer l'agent,
//! geste qui depuis le lot 32I **orpheline toutes les fenêtres**.
//!
//! **Ce que les deux tâches deviennent : UNE SEULE**, qui possède la
//! connexion, la sert par un `select!`, et la ROUVRE quand elle tombe. Les
//! deux extrémités que l'appelant tient — `rx_shell` et `envoyer` — sont
//! créées une fois et **survivent aux reconnexions** : `superviseur.rs` et
//! `boucle.rs` sont inchangés, et n'ont jamais à savoir qu'une reprise a eu
//! lieu. La décision (quand retenter, quand réarmer le repli) est PURE et
//! vit dans [`super::reprise_controle`], qui se teste sur l'hôte Linux —
//! ce fichier-ci est `#![cfg(windows)]` et ne l'est pas.
//!
//! 🔴 **CE QUI EST ÉMIS PENDANT LA COUPURE EST JETÉ, PAS REJOUÉ**, et c'est
//! délibéré : une annonce de fenêtre vieille de trente secondes est une
//! COPIE d'une vérité qui vit dans la table du superviseur, et rien ici ne
//! saurait l'expirer — l'argument mot pour mot de
//! `plateforme/src/signaling/pair-present.ts`. Ce qui répare l'état après une
//! reprise n'est pas une file, c'est la RÉANNONCE déclenchée par
//! `pair-present` : on ne rejoue pas la vérité d'hier, on prévient celui qui
//! la détient qu'on la lui redemande maintenant.
//!
//! 🔴 **LE JETON EST RELU À CHAQUE TENTATIVE, JAMAIS CELUI DU DÉMARRAGE.**
//! `main.rs` l'écrit déjà en toutes lettres pour le LANCEUR : « un
//! superviseur vit des heures ; le jeton d'agent, lui, dure dix minutes et se
//! renouvelle à chaque battement ». Une reconnexion qui présenterait
//! `config.jeton` — l'instantané du démarrage — serait refusée par la garde
//! (`poignée de main refusée : jeton refusé (expire)`, ligne réellement
//! observée dans le journal de production), **à chaque tentative, pour
//! toujours** : un remède qui aurait l'air de fonctionner sur une coupure
//! d'une minute et ne fonctionnerait plus jamais après dix. La veille
//! d'identité (`plateforme::Canal::veille_identite`) est donc lue à CHAQUE
//! ouverture.

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
    /// La connexion est tombée — il faut en rouvrir une.
    ConnexionPerdue,
    /// Le superviseur lui-même a disparu : ses deux extrémités sont fermées,
    /// il n'y a plus personne à servir et rouvrir un socket serait une fuite.
    SuperviseurArrete,
}

/// Ouvre la connexion, se déclare comme `agent` sur la session donnée, et rend
/// de quoi envoyer et recevoir. **La connexion est ensuite ROUVERTE
/// automatiquement à chaque chute**, sans que l'appelant ait rien à faire.
///
/// `url` est déjà l'URL du RELAIS (`crate::signaling::url_du_relais`) : c'est
/// `superviseur.rs` qui la dérive avant d'appeler cette fonction, jamais elle.
///
/// `jeton_initial` porte le jeton d'agent connu au démarrage
/// (`crate::plateforme`). **`None` fait refuser la poignée de main par la
/// plateforme depuis le sous-bloc P3** : la garde n'accepte plus un
/// `{"role":"agent"}` anonyme.
///
/// `identite` est la VEILLE d'identité, relue à chaque RECONNEXION : voir
/// l'en-tête de ce fichier. `None` quand l'agent n'a pas ouvert de canal
/// `/agent` (jeton hérité, ou aucune identité) — `jeton_initial` sert alors
/// à chaque tentative, exactement comme avant ce lot.
///
/// 🔴 **LA PREMIÈRE OUVERTURE RESTE FATALE, ET C'EST DÉLIBÉRÉ.** Elle rend
/// `Err`, donc `superviseur::executer` échoue et le processus s'arrête, comme
/// avant ce lot : une `SIGNALING_URL` fautive doit se voir tout de suite, pas
/// se transformer en boucle de reprise silencieuse. Ce lot ne corrige que la
/// perte d'une connexion **déjà établie**, qui est le défaut mesuré ; élargir
/// la reprise au tout premier essai est un autre changement, non mesuré,
/// délibérément laissé de côté.
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

    // Les DEUX extrémités que l'appelant garde. Créées une seule fois : elles
    // survivent à toutes les reconnexions, c'est ce qui laisse `boucle.rs`
    // ignorant du mécanisme.
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
        Err(erreur) => tracing::error!(%erreur, "sérialisation d'un message de contrôle"),
    };

    Ok((rx_entrant, envoyer))
}

/// Ouvre UNE connexion et y émet la déclaration de rôle.
///
/// ⚠️ Rendre `Ok` ne prouve PAS que la plateforme a accepté : le verdict
/// arrive plus tard, sous forme d'un message `{"type":"error"}` suivi d'une
/// fermeture. C'est le piège que la recette de P3 a payé — voir la trace
/// ci-dessous, qui dit ce qu'elle SAIT et rien de plus.
async fn ouvrir(url: &str, session: &str, jeton: Option<&str>) -> Result<Flux> {
    let (mut stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connexion au signaling {url}"))?;
    // `WebSocketStream` est lui-même un `Sink<Message>` : on émet la
    // déclaration SANS scinder le flux, pour n'avoir pas à le recoller
    // ensuite. `servir` le scindera une fois, et une seule.
    stream
        .send(Message::Text(
            serde_json::json!({ "role": "agent", "session": session, "jeton": jeton }).to_string(),
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
        jeton_present = jeton.is_some(),
        "déclaration du superviseur émise sur la session de contrôle (acceptation encore inconnue)"
    );
    Ok(stream)
}

/// Tient la session de contrôle : la sert, et la ROUVRE tant que le
/// superviseur est là.
///
/// ⚠️ **CHAQUE TENTATIVE EST TRACÉE, SANS CONDITION.** Ce dépôt vient de
/// payer l'inverse (lot 33 : des traces derrière des `return` précoces, et
/// l'observabilité qui avait permis le diagnostic détruite par son propre
/// remède). Le volume est borné par le plafond du repli : une plateforme
/// morte pendant une heure coûte ~120 lignes, à comparer aux dizaines de
/// milliers qu'`agent.log` écrit dans le même temps.
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
        let courant = match flux.take() {
            Some(f) => f,
            None => {
                let delai_ms = reprise.delai_ms();
                tokio::time::sleep(std::time::Duration::from_millis(delai_ms)).await;
                reprise.tentative_lancee();
                // 🔴 LE JETON EST RELU ICI, PAS CAPTURÉ AU DÉMARRAGE : voir
                // l'en-tête. `borrow()` rend la dernière identité connue,
                // rafraîchie à chaque battement par `plateforme/session.rs`.
                let jeton = jeton_courant(&identite, &jeton_initial);
                match ouvrir(&url, &session, jeton.as_deref()).await {
                    Ok(f) => {
                        // ⚠️ INCONDITIONNELLE, et elle porte le compte de
                        // messages JETÉS — y compris `0`, qui est le témoin
                        // négatif : une reprise sans perte se distingue ainsi
                        // d'une reprise qui a mangé trois annonces.
                        let jetes = vider(&mut rx_sortant);
                        tracing::warn!(
                            session = %session,
                            tentative = reprise.tentative(),
                            delai_ms,
                            messages_jetes = jetes,
                            "session de contrôle RÉTABLIE (les messages émis pendant la coupure \
                             sont jetés, jamais rejoués : la réannonce sur pair-present est ce \
                             qui répare l'état)"
                        );
                        f
                    }
                    Err(erreur) => {
                        tracing::warn!(
                            %erreur,
                            session = %session,
                            tentative = reprise.tentative(),
                            delai_ms,
                            "reconnexion de la session de contrôle ÉCHOUÉE : aucune fenêtre ne \
                             peut être annoncée ni réannoncée tant qu'elle n'aboutit pas"
                        );
                        continue;
                    }
                }
            }
        };

        let debut = std::time::Instant::now();
        let fin = servir(courant, &tx_entrant, &mut rx_sortant).await;
        let vecu_ms = debut.elapsed().as_millis() as u64;
        if let Fin::SuperviseurArrete = fin {
            tracing::info!(
                session = %session,
                "session de contrôle abandonnée : le superviseur n'est plus là"
            );
            return;
        }
        let rearme = reprise.connexion_terminee(vecu_ms);
        // ⚠️ INCONDITIONNELLE elle aussi, et c'est elle qui remplace le
        // `connexion de contrôle au signaling perdue` d'avant ce lot : celle-
        // là ne disait pas que plus rien ne reviendrait, celle-ci dit quand
        // la suite arrive.
        tracing::warn!(
            session = %session,
            vecu_ms,
            repli_rearme = rearme,
            prochaine_tentative_dans_ms = reprise.delai_ms(),
            "session de contrôle PERDUE : reconnexion programmée"
        );
    }
}

/// Le jeton à présenter MAINTENANT : celui de la veille d'identité s'il y en a
/// une, sinon celui du démarrage.
fn jeton_courant(
    identite: &Option<watch::Receiver<Option<Identite>>>,
    jeton_initial: &Option<String>,
) -> Option<String> {
    identite
        .as_ref()
        .and_then(|veille| veille.borrow().as_ref().map(|i| i.jeton.clone()))
        .or_else(|| jeton_initial.clone())
}

/// Jette ce qui attendait d'être émis, et rend combien. Voir l'en-tête pour la
/// raison : une annonce périmée est pire que pas d'annonce.
fn vider(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> usize {
    let mut jetes = 0usize;
    while rx.try_recv().is_ok() {
        jetes += 1;
    }
    jetes
}

/// Sert UNE connexion jusqu'à sa fin : lit ce qui arrive, écrit ce qui part.
///
/// **Un seul `select!` là où il y avait deux tâches**, parce que les deux
/// moitiés partagent désormais un destin : quand la connexion tombe, il faut
/// que la même boucle en rouvre une. Deux tâches indépendantes obligeraient à
/// les coordonner pour savoir laquelle reconnecte.
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
                    // ⚠️ `Err` REND LA MAIN, il ne `continue` pas. Avant ce
                    // lot, un `let Ok(Message::Text(_)) = recu else
                    // { continue }` traitait une erreur de flux comme une
                    // trame à ignorer : sur une erreur PERSISTANTE, cette
                    // boucle tournait à vide en consommant un cœur, sans une
                    // trace. Non observé en production — la connexion se
                    // terminait par `None` — mais le chemin existait.
                    Err(erreur) => {
                        tracing::warn!(%erreur, "lecture de la session de contrôle en erreur");
                        return Fin::ConnexionPerdue;
                    }
                    Ok(Message::Close(_)) => return Fin::ConnexionPerdue,
                    // Ping/Pong/Binary : `tokio-tungstenite` répond seul aux
                    // Ping, il n'y a rien à faire ici.
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
                if let Err(erreur) = sortant.send(Message::Text(texte)).await {
                    tracing::warn!(%erreur, "émission vers la shell échouée");
                    return Fin::ConnexionPerdue;
                }
            }
        }
    }
}

/// Ce qu'une trame texte veut dire, ou rien.
fn analyser(texte: &str) -> Option<DepuisLaShell> {
    match serde_json::from_str::<DepuisLaShell>(texte) {
        Ok(message) => Some(message),
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
        //
        // ⚠️ **CE BRAS COMPTE DOUBLE DEPUIS QUE LA CONNEXION SE REPREND** :
        // un jeton expiré présenté par une reconnexion se dit ici, et
        // c'est la SEULE façon de distinguer « la plateforme est morte »
        // de « la plateforme refuse ce que je lui présente ».
        Err(_) => match serde_json::from_str::<serde_json::Value>(texte) {
            Ok(valeur) if valeur.get("type").and_then(|t| t.as_str()) == Some("error") => {
                tracing::warn!(
                    motif = %valeur.get("motif").and_then(|m| m.as_str()).unwrap_or("(absent)"),
                    raison = %valeur.get("reason").and_then(|r| r.as_str()).unwrap_or("(absente)"),
                    "session de contrôle REFUSÉE par la plateforme"
                );
                None
            }
            _ => {
                tracing::debug!(texte, "message ignoré sur la session de contrôle");
                None
            }
        },
    }
}
