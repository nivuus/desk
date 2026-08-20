//! Client du canal `/agent` : l'agent s'y enrôle, y bat le cœur, et en reçoit
//! son préfixe de session et son jeton d'agent — et, depuis le sous-bloc G1,
//! il y POUSSE son catalogue d'applications et en REÇOIT des ordres de
//! lancement.
//!
//! ⚠️ CE CANAL NE PORTE DONC PLUS SEULEMENT UNE IDENTITÉ, contrairement à ce
//! que dit le paragraphe suivant, écrit au sous-bloc P3 et conservé pour son
//! raisonnement. Deux voies l'ont traversé depuis : une file d'émission
//! bornée (`FILE_EMISSION`) pour ce qui monte, et une `mpsc` d'ordres pour ce
//! qui descend.
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

pub mod identite;
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
use tokio::sync::{mpsc, watch};
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

/// Combien de messages montants peuvent attendre leur socket.
///
/// 🔴 BORNÉE, ET NON ILLIMITÉE : ce canal peut rester coupé des heures, et une
/// file illimitée derrière un socket mort est une fuite mémoire dont rien ne
/// dit le nom. Ce qui déborde est PERDU — voir [`Canal::emettre`], qui porte
/// la raison pour laquelle c'est acceptable.
///
/// La valeur tient au trafic réel : un catalogue par réconciliation, soit un
/// message toutes les trente secondes, plus une `Lancee` par ordre.
/// Trente-deux couvre un quart d'heure de coupure. **NON CALIBRÉE.**
const FILE_EMISSION: usize = 32;

/// Le canal ouvert, et le fil qui le tient. **Le lâcher arrête le battement
/// de cœur — et, depuis G1, la DÉCOUVERTE D'APPLICATIONS avec lui** : la
/// boucle d'`apps` sort sur `TryRecvError::Disconnected` et journalise « canal
/// /agent fermé : découverte d'applications arrêtée ». `main` le garde vivant
/// pour toute la durée du processus, et c'est désormais vrai pour deux
/// mécanismes au lieu d'un.
/// Ce que la plateforme demande à la boucle d'applications.
///
/// 🔴 UN ENUM PLUTÔT QU'UNE SECONDE FILE, ET C'EST UNE DÉCISION. Les deux
/// messages descendants vont au MÊME consommateur — la boucle d'apps, sur son
/// fil COM dédié —, et deux files l'obligeraient à interroger les deux à
/// chaque tour d'attente, avec le risque qu'un ajout futur en oublie une. Un
/// enum rend l'exhaustivité vérifiable par le compilateur là où deux files la
/// laisseraient à la vigilance.
///
/// ⚠️ IL NE TRANSPORTE AUCUN OCTET D'IMAGE. `IconesManquantes` ne porte qu'un
/// inventaire d'empreintes ; les images montent par `PUT /icone/:sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ordre {
    /// Lancer une application, par sa clé. `demande` apparie la réponse.
    Lancer { demande: String, cle: String },
    /// Téléverser les icônes que la plateforme n'a pas.
    IconesManquantes { empreintes: Vec<String> },
}

pub struct Canal {
    identite: watch::Receiver<Option<Identite>>,
    /// Le fil de reprise. Jamais attendu — il ne se termine que sur un refus
    /// définitif —, mais conservé pour ne pas être abandonné en silence.
    ///
    /// ⚠️ L'`allow` RESTE JUSTIFIÉ, MAIS PAS POUR LA MÊME RAISON SUR LES DEUX
    /// CIBLES — relevé en le RETIRANT, sur chacune :
    ///   - `--target x86_64-pc-windows-gnu` : « field `tache` is never read ».
    ///     C'est la raison HISTORIQUE, et la seule qui reste sur la cible
    ///     réelle ; `emission` et `ordres` sont bien lus, par la boucle de
    ///     découverte ;
    ///   - sur l'hôte : « fields `tache`, `emission`, and `ordres` are never
    ///     read », parce que `apps::demarrer` y est un talon qui rend `None`
    ///     sans rien toucher.
    ///
    /// Un `allow` devenu inutile est une affirmation devenue fausse : celui-ci
    /// est à relire le jour où plus rien n'appellerait `apps::brancher`.
    #[allow(dead_code)]
    tache: tokio::task::JoinHandle<()>,
    /// La file montante, drainée dans le `select!` de [`une_session`].
    emission: mpsc::Sender<VersLaPlateforme>,
    /// Les ordres descendants. `Option` parce qu'un seul consommateur peut la
    /// prendre : deux se voleraient les ordres l'un à l'autre, et chacun n'en
    /// verrait qu'une partie.
    ordres: Option<mpsc::UnboundedReceiver<Ordre>>,
    /// L'URL du signaling telle qu'on l'a reçue — sous-bloc G2.
    ///
    /// ⚠️ ELLE EST RETENUE PLUTÔT QUE RELUE DE L'ENVIRONNEMENT : le
    /// téléversement d'icônes en dérive son adresse HTTP, et relire
    /// `SIGNALING_URL` ailleurs ferait vivre la même valeur à deux endroits,
    /// donc diverger le jour où l'un des deux serait changé.
    signaling_url: String,
}

/// De quoi émettre sans tenir le [`Canal`] entier.
#[derive(Clone)]
pub struct Emetteur {
    file: mpsc::Sender<VersLaPlateforme>,
}

impl Emetteur {
    /// Met un message montant en file. **Ne bloque jamais, et ne rend aucune
    /// erreur.**
    ///
    /// 🔴 UN MESSAGE MIS EN FILE PENDANT QUE LE SOCKET EST TOMBÉ EST PERDU, ET
    /// C'EST VOULU. Ce canal est un `push` WebSocket : il n'a aucune garantie
    /// de livraison, dans aucun des deux sens. Le rendre bloquant ferait de la
    /// file une fuite mémoire sur un canal qui peut rester coupé des heures ;
    /// le rendre fatal tuerait le canal sur une coupure réseau ordinaire.
    ///
    /// **Ce qui rend la perte acceptable est ailleurs, et une seule chose la
    /// rend acceptable** : l'agent renvoie son catalogue COMPLET
    /// (`complet = true`) à chaque réenrôlement, donc toute divergence née
    /// d'un message perdu a un TERME. Retirer ce renvoi complet rendrait cette
    /// perte silencieuse et définitive.
    pub fn emettre(&self, message: VersLaPlateforme) {
        if let Err(erreur) = self.file.try_send(message) {
            tracing::warn!(%erreur, "message montant abandonné : canal coupé ou file pleine");
        }
    }
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
    let (emission, mut a_emettre) = mpsc::channel(FILE_EMISSION);
    // ⚠️ NON BORNÉE, à l'inverse de la file montante, et pour une raison
    // opposée : son consommateur traite chaque ordre dans un `spawn_blocking`
    // et ne doit JAMAIS faire attendre la boucle du canal — un `send` bloquant
    // ici suspendrait le battement de cœur, et la plateforme déclarerait la VM
    // injoignable pendant qu'elle lance une application. Le débit la borne de
    // fait : un ordre par clic d'utilisateur.
    let (ordres_tx, ordres_rx) = mpsc::unbounded_channel();
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
            match une_session(&url, &vm, &secret, &tx, &mut a_emettre, &ordres_tx).await {
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
    Canal {
        identite,
        tache,
        emission,
        ordres: Some(ordres_rx),
        signaling_url: signaling_url.to_string(),
    }
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

    /// Prend la file des ordres descendants. Rend `None` au second appel.
    ///
    /// Un seul consommateur, parce que deux se voleraient les ordres l'un à
    /// l'autre et que le symptôme serait « un lancement sur deux ne part pas ».
    pub fn ordres(&mut self) -> Option<mpsc::UnboundedReceiver<Ordre>> {
        self.ordres.take()
    }

    /// L'URL du signaling, dont le téléversement d'icônes dérive son adresse
    /// HTTP (sous-bloc G2).
    pub fn url_signaling(&self) -> &str {
        &self.signaling_url
    }

    /// Un émetteur détachable, pour le fil de découverte.
    ///
    /// Le `Canal` lui-même n'est pas `Send` vers un fil bloquant qui le
    /// garderait indéfiniment : c'est la file, et elle seule, qui doit
    /// traverser. Un `Sender` de tokio est `Send` et `Clone`.
    pub fn emetteur(&self) -> Emetteur {
        Emetteur { file: self.emission.clone() }
    }

    /// Observe les changements d'identité — un réenrôlement en est un.
    ///
    /// C'est par lui que la boucle de découverte sait qu'elle doit renvoyer le
    /// catalogue COMPLET plutôt qu'un delta.
    pub fn veille_identite(&self) -> watch::Receiver<Option<Identite>> {
        self.identite.clone()
    }
}

/// Une session du canal, de la connexion à sa chute.
async fn une_session(
    url: &str,
    vm: &str,
    secret: &str,
    tx: &watch::Sender<Option<Identite>>,
    a_emettre: &mut mpsc::Receiver<VersLaPlateforme>,
    ordres: &mpsc::UnboundedSender<Ordre>,
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
fn sur_refus(url: &str, version_recue: u8, motif: &str) -> Fin {
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
