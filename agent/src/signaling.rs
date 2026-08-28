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
    /// 🔴 **NEUF — CORRECTIF DU LEGS DES FREINS MANQUANTS (round de
    /// correction 1, critique ②), 25 août 2026.** Le nombre de secondes que
    /// le relais demande d'attendre avant de retenter, quand le refus porte
    /// le motif `trop-de-requetes` (`plateforme/src/signaling/relais.ts`,
    /// champ `retryApresS`). `None` tant qu'aucun tel refus n'est arrivé, ou
    /// si le champ était absent/illisible — un relais d'une version
    /// antérieure à ce lot n'en envoie aucun.
    ///
    /// ⚠️ **CE N'EST PAS UN ORDRE, C'EST UNE INFORMATION** : rien ici ne fait
    /// attendre qui que ce soit. C'est à l'appelant de la consulter avant de
    /// mourir — voir `pont.rs::executer`, qui dort ce temps AVANT de rendre
    /// son `Err`, de sorte que le superviseur (`surveillance_pont.rs`), qui
    /// mesure l'espacement depuis le dernier LANCEMENT et non depuis la mort
    /// du processus, ne relance jamais plus tôt que ce que le relais a
    /// demandé — sans qu'aucun canal ne franchisse la frontière de processus.
    pub retry_apres_s: watch::Receiver<Option<u64>>,
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

/// Compose l'URL du relais à partir de celle du service.
///
/// 🔴 `SIGNALING_URL` EST LA BASE DU SERVICE, JAMAIS L'URL DU RELAIS, ET C'EST
/// CE QUI REND `scripts/run-agent.sh` INCHANGÉ. La même variable sert à
/// dériver le canal d'enrôlement (`plateforme::url_du_canal`, qui ajoute
/// `/agent`) et l'adresse HTTP du téléversement d'icônes
/// (`apps::icone::televersement::base_http`, qui retire tout chemin). Y écrire
/// `/signal` casserait le premier : `ws://h:8080/signal/agent` n'est pas
/// `/agent`, et la plateforme compare le chemin EXACTEMENT.
///
/// Le `trim_end_matches` a la même raison que chez son jumeau : `ws://h:8080/`
/// suivi de `/signal` donnerait `//signal`, refusé de la même façon.
pub fn url_du_relais(signaling_url: &str) -> String {
    format!("{}/signal", signaling_url.trim_end_matches('/'))
}

/// Se connecte au signaling et démarre la boucle d'échange en tâche de fond.
/// `url` est déjà l'URL du RELAIS (`url_du_relais`), jamais celle du service :
/// `demarrage.rs` et `pont.rs` la dérivent avant d'appeler cette fonction.
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
    let (retry_apres_s_tx, retry_apres_s) = watch::channel::<Option<u64>>(None);
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
                    // 🔴 `retryApresS` N'EST PORTÉ QUE SUR LE REFUS DE VOLUME
                    // (`motif: "trop-de-requetes"`) — voir `relais.ts`. Un
                    // refus de poignée de main (jeton absent ou invalide)
                    // n'en porte aucun, et `as_u64()` rend alors `None` sans
                    // qu'il y ait besoin de tester le motif ici : caler la
                    // décision sur la SEULE présence du champ, jamais sur le
                    // texte du motif, est ce qui laisse ce bras correct si le
                    // relais gagne un jour un second motif porteur d'attente.
                    let retry = parsed["retryApresS"].as_u64();
                    if let Some(s) = retry {
                        let _ = retry_apres_s_tx.send(Some(s));
                    }
                    tracing::error!(
                        raison = %parsed["reason"],
                        retry_apres_s = ?retry,
                        "erreur de signaling"
                    )
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
        retry_apres_s,
        receiver_task,
        sender_task,
    })
}

/// Attend le délai que le relais a suggéré (`retryApresS`, refus
/// `trop-de-requetes`) s'il en a envoyé un — coût nul sinon.
///
/// 🔴 **CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
/// critique ②).** C'est l'appelant qui décide QUAND consulter cette valeur —
/// typiquement juste avant de rendre une erreur fatale, une fois établi que
/// la session ne s'ouvrira pas. Rien ici ne fait attendre `run_signaling`
/// elle-même : le signaling reste un pur relais d'information.
///
/// ⚠️ **BORNÉE À `REPLI_MAX_MS`**, jamais la valeur brute du serveur : un
/// relais qui enverrait une valeur aberrante (bogue, ou serveur compromis) ne
/// doit pas pouvoir geler indéfiniment un processus dont la seule vocation,
/// à ce stade, est de mourir vite pour que son superviseur retente.
///
/// 🔴 **DÉCLARÉ, PAS CORRIGÉ (revue, round de correction 2)** : ce plafond
/// (`REPLI_MAX_MS` = 30 s) peut être STRICTEMENT INFÉRIEUR à ce que le relais
/// suggère (`retryApresS` peut valoir jusqu'à `FENETRE_REQUETES_MS / 1000` =
/// 60 s, `plateforme/src/securite/frein.ts`). Conséquence : un agent qui
/// honore une suggestion de 60 s ne dort que 30, retente, et — le budget de
/// la fenêtre n'ayant pas encore expiré — se fait refuser une SECONDE fois
/// avant que la fenêtre ne se vide réellement. Une connexion `/signal`
/// gaspillée par fenêtre de refus prolongé, jamais plus : le repli
/// exponentiel de l'appelant (`relance_pont::EtatRelance`, ou
/// `plateforme::repli` pour le canal `/agent`) continue de croître par
/// ailleurs, donc cela ne dégénère jamais en martèlement.
pub async fn honorer_retry_suggere(retry_apres_s: &watch::Receiver<Option<u64>>) {
    let Some(secondes) = *retry_apres_s.borrow() else { return };
    let bornees = secondes.min(crate::plateforme::repli::REPLI_MAX_MS / 1000);
    tracing::info!(
        secondes = bornees,
        secondes_demandees = secondes,
        "attente du délai suggéré par le relais (retryApresS) avant de céder la main"
    );
    tokio::time::sleep(std::time::Duration::from_secs(bornees)).await;
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

    #[test]
    fn le_relais_derive_du_signaling() {
        assert_eq!(url_du_relais("ws://h:8080"), "ws://h:8080/signal");
    }

    /// ⚠️ MÊME RAISON QUE `url_du_canal` : `ws://h:8080/` suivi de `/signal`
    /// donnerait `//signal`, et la plateforme compare le chemin EXACTEMENT.
    #[test]
    fn la_barre_finale_ne_double_pas() {
        assert_eq!(url_du_relais("ws://h:8080/"), "ws://h:8080/signal");
    }

    /// 🔴 LE TEST QUI FIGE L'ÉCART ① DU PLAN : `SIGNALING_URL` reste la BASE,
    /// donc le canal `/agent` continue de se dériver juste depuis une base
    /// propre.
    ///
    /// ⚠️ **CE COMMENTAIRE A PROMIS PLUS QUE CE TEST NE TIENT, et c'est
    /// corrigé plutôt qu'effacé (revue transverse, 21 août 2026).** Il disait :
    /// « sans lui, quelqu'un pourrait un jour mettre `/signal` dans la variable
    /// et casser l'enrôlement sans qu'aucun test ne bronche ». **Ce test ne
    /// ferme PAS ce cas** : il passe `"ws://h:8080"`, une base PROPRE, donc il
    /// ne peut pas voir ce qui arriverait à `"ws://h:8080/signal"` — d'où
    /// `url_du_canal` tirerait `ws://h:8080/signal/agent`, et l'enrôlement
    /// tomberait. **Le contrat de `SIGNALING_URL` reste donc figé par AUCUN
    /// test**, et c'est inscrit aux « Legs ouverts » de `CLAUDE.md`. Ce que ce
    /// test-ci établit, et c'est déjà utile, est que l'ajout de `/signal` par
    /// `url_du_relais` n'a pas contaminé `url_du_canal`.
    #[test]
    fn le_canal_agent_n_est_pas_affecte() {
        assert_eq!(crate::plateforme::url_du_canal("ws://h:8080"), "ws://h:8080/agent");
    }

    /// 🔴 `SIGNALING_URL` EST LA BASE DU SERVICE, JAMAIS L'URL DU RELAIS.
    /// `url_du_relais` y ajoute `/signal`, `url_du_canal` y ajoute `/agent`.
    /// Y écrire `/signal` casserait l'enrôlement — `ws://h:8080/signal/agent` —
    /// et AUCUN test ne le disait : celui d'à côté (`le_canal_agent_n_est_pas_
    /// affecte`, juste au-dessus) passe une base PROPRE, donc n'éprouve jamais
    /// ce cas, alors que son commentaire prétendait le fermer.
    ///
    /// ⚠️ **CE TEST FIGE, IL NE CORRIGE PAS** (round du 25 août 2026,
    /// `legs-sans-vm` tâche 4) : `url_du_canal` est une concaténation pure
    /// (`format!("{}/agent", …trim_end_matches('/'))`), sans garde sur le
    /// contenu de la base — jouée sur `"ws://h:8080/signal"`, elle rend
    /// `"ws://h:8080/signal/agent"` **exactement comme documenté ici**, et le
    /// test passait déjà avant cette tâche (VÉRIFIÉ : la rouge n'existe qu'en
    /// mutant `url_du_canal`, jamais sur le produit d'aujourd'hui). Le
    /// contrat que ce test ferme n'est donc pas « le produit refuse une base
    /// fausse » — il ne le fait pas, et rien dans cette tâche ne le lui fait
    /// faire — mais « le comportement sur une base fausse est CONNU et fixé
    /// par un test », là où hier aucun test ne le regardait.
    #[test]
    fn une_base_portant_deja_signal_casse_le_canal_agent() {
        // La base JUSTE : `url_du_canal` y ajoute `/agent`.
        assert_eq!(crate::plateforme::url_du_canal("ws://h:8080"), "ws://h:8080/agent");
        // 🔴 LA BASE FAUSSE, celle qu'aucun test n'éprouvait : elle porte déjà
        // le suffixe du relais, et l'enrôlement part alors vers un chemin qui
        // n'existe pas côté plateforme. C'est CE cas que le contrat fixe ici.
        assert_eq!(
            crate::plateforme::url_du_canal("ws://h:8080/signal"),
            "ws://h:8080/signal/agent",
            "le contrat de SIGNALING_URL a changé : cette égalité documentait \
             que le produit accepte une base déjà suffixée EN SILENCE"
        );
    }

    /// 🔴 CORRECTIF DU LEGS DES FREINS MANQUANTS (round de correction 1,
    /// critique ②) — `honorer_retry_suggere`, éprouvée sur l'hôte. Horloge
    /// GELÉE (`start_paused`) : sans elle, ces trois tests attendraient
    /// réellement des secondes entières, et une assertion sur la durée
    /// écoulée deviendrait un bruit de mesure plutôt qu'un fait.
    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_n_attend_rien_sans_valeur() {
        let (_tx, rx) = watch::channel::<Option<u64>>(None);
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(
            tokio::time::Instant::now(),
            debut,
            "aucun refus de volume n'est jamais arrivé : rien à attendre"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_attend_exactement_la_valeur_recue() {
        let (_tx, rx) = watch::channel(Some(5u64));
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(tokio::time::Instant::now() - debut, std::time::Duration::from_secs(5));
    }

    /// 🔴 Le test qui compte : sans ce plafond, un relais qui enverrait une
    /// valeur aberrante (bogue, ou compromis) gèlerait indéfiniment un
    /// processus dont la seule vocation, à ce stade, est de mourir vite pour
    /// que son superviseur retente.
    #[tokio::test(start_paused = true)]
    async fn honorer_retry_suggere_est_bornee_au_plafond_de_repli() {
        let (_tx, rx) = watch::channel(Some(999_999u64));
        let debut = tokio::time::Instant::now();
        honorer_retry_suggere(&rx).await;
        assert_eq!(
            tokio::time::Instant::now() - debut,
            std::time::Duration::from_millis(crate::plateforme::repli::REPLI_MAX_MS),
        );
    }
}
