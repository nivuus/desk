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

use proto::plateforme::VersLaPlateforme;
use tokio::sync::{mpsc, watch};

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
/// Le vocabulaire des ordres descendants vit dans un module enfant — voir son
/// en-tête pour la déclaration du franchissement de plafond qui l'a produit.
mod ordre;
pub use ordre::Ordre;

/// L'ordre d'INSTALLATION voyage dans sa propre file, et l'en-tête de ce module
/// dit pourquoi : ce n'est pas le même consommateur.
mod installation;
pub use installation::Installation;

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
    /// Les ordres d'INSTALLATION. Même règle du consommateur unique, et pour
    /// la même raison — mais un consommateur DIFFÉRENT : le fil d'installation
    /// tourne sur `tokio`, quand `ordres` est drainée par le fil COM de la
    /// découverte. Voir `plateforme/installation.rs`.
    installations: Option<mpsc::UnboundedReceiver<Installation>>,
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
    let (installations_tx, installations_rx) = mpsc::unbounded_channel();
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
            match une_session(
                &url, &vm, &secret, &tx, &mut a_emettre, &ordres_tx, &installations_tx,
            )
            .await
            {
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
        installations: Some(installations_rx),
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

    /// Prend la file des installations. Rend `None` au second appel.
    ///
    /// 🔴 MÊME PROPRIÉTÉ QU'[`Self::ordres`], ET POUR LA MÊME RAISON : deux
    /// consommateurs se voleraient les ordres l'un à l'autre, et le symptôme
    /// serait « une installation sur deux ne part pas ». La différence est
    /// qu'ici le consommateur est le fil `tokio` d'installation, jamais le fil
    /// COM de la découverte — c'est cette différence qui justifie la seconde
    /// file plutôt qu'une variante d'[`Ordre`].
    pub fn installations(&mut self) -> Option<mpsc::UnboundedReceiver<Installation>> {
        self.installations.take()
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

/// Une session du canal, de la connexion à sa chute, extraite AVANT que le
/// sous-bloc G3 n'ajoute sa seconde file : ce fichier était à 490 lignes,
/// marge 10.
mod session;
use session::une_session;
