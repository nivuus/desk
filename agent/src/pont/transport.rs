//! Le transport du pont : une `PeerConnection` **sans média**, portant un
//! unique canal de données `fichiers`, fiable et ordonné.
//!
//! **Mixte, mais sans Windows** : la boucle str0m et le socket UDP sont
//! portables, et ce module se teste donc entièrement sur l'hôte. Il ne connaît
//! **ni ProjFS ni Windows** — il transporte des octets opaques, corrélés, et
//! rien d'autre. C'est ce qui permet de l'éprouver avec un vrai pair str0m en
//! boucle locale, sans la moindre racine de virtualisation.
//!
//! **Pourquoi une connexion DÉDIÉE** (décision D4) : la session vidéo porte
//! déjà `control` et `input`, et le relais de signaling n'accepte qu'un
//! `agent` et un `client` par identifiant. Surtout, mêler le canal fichiers à
//! la session vidéo ferait qu'une reconnexion de l'une emporterait l'autre —
//! ce que le principe 4 du cadrage interdit.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

/// Le label du canal de données du pont. **C'est le navigateur qui crée le
/// canal** (`createDataChannel('fichiers')`) ; l'agent est répondant.
pub const LABEL_FICHIERS: &str = "fichiers";

/// Taille du tampon de réception UDP. Une trame du pont tient dans
/// `TAILLE_TRAME_MAX` plus son en-tête, mais SCTP fragmente : ce tampon borne
/// un datagramme, pas un message applicatif.
const TAMPON_UDP: usize = 2048;

/// Attente maximale d'un tour de boucle quand str0m n'a pas d'échéance
/// proche. Borne la latence de prise en compte d'une requête déposée dans
/// `sortant` — sans elle, une requête arrivée juste après un `recv_timeout`
/// attendrait l'échéance str0m suivante.
const ATTENTE_MAX: Duration = Duration::from_millis(20);

/// Ce que le pont envoie au navigateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersNavigateur {
    Requete { correlation: u32, trame: Vec<u8> },
}

/// Ce que le pont reçoit du navigateur, ou apprend de l'état du canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuNavigateur {
    Reponse { correlation: u32, trame: Vec<u8> },
    CanalOuvert,
    /// Le canal est parti : onglet fermé, page rechargée, WebRTC tombé.
    /// L'appelant traduit en [`crate::pont::erreurs::Erreur::CanalFerme`],
    /// c'est-à-dire `ERROR_IO_DEVICE` — « l'erreur I/O standard » du cadrage.
    CanalFerme,
}

/// Construit le socket UDP et le `Rtc` d'un point d'accès **données seules**.
///
/// Par rapport à `transport::initialisation::construire_rtc`, **tombent** :
/// `enable_h264`, `enable_opus`, `enable_bwe`, `set_stats_interval` et
/// `set_desired_bitrate` — il n'y a aucune piste, donc rien à estimer ni à
/// sonder. **Restent** le socket UDP non bloquant (dont la raison est mesurée :
/// le délai de `set_read_timeout` déborde massivement sous Windows), la pose du
/// fournisseur cryptographique, et le candidat hôte.
///
/// ⚠️ **`clear_codecs()` sans aucun `enable_*` est délibéré, et il fonctionne** :
/// éprouvé par les tests de ce module, qui négocient et échangent réellement
/// sur un canal de données avec un pair str0m sans qu'aucun codec ne soit
/// activé d'aucun côté. Le navigateur n'offre aucune piste ; il n'y a donc rien
/// à apparier.
pub fn construire_rtc_donnees(local_ip: IpAddr) -> Result<(UdpSocket, Rtc)> {
    let socket =
        UdpSocket::bind(SocketAddr::new(local_ip, 0)).context("ouverture du socket UDP du pont")?;
    socket
        .set_nonblocking(true)
        .context("passage du socket UDP du pont en non bloquant")?;
    let addr = socket.local_addr()?;
    tracing::info!(%addr, "socket UDP du pont fichiers");

    // Idempotent : `OnceLock::set` ignore silencieusement un second appel. Le
    // pont est un processus à part, donc c'est en pratique le premier — mais
    // les tests de ce module en construisent plusieurs.
    str0m::crypto::from_feature_flags().install_process_default();

    let mut rtc = Rtc::builder().clear_codecs().build(Instant::now());
    rtc.add_local_candidate(
        Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
    );
    Ok((socket, rtc))
}

/// La boucle de transport. Rend `Ok(())` quand la connexion se termine ou que
/// l'appelant lâche `sortant`.
///
/// `sortant` porte les requêtes à émettre, `entrant` rend les réponses et les
/// changements d'état du canal.
pub fn tourner(
    mut rtc: Rtc,
    socket: UdpSocket,
    sortant: Receiver<VersNavigateur>,
    entrant: Sender<DuNavigateur>,
) -> Result<()> {
    let adresse = socket.local_addr().context("adresse locale du socket du pont")?;
    let mut canal: Option<ChannelId> = None;
    let mut tampon = vec![0u8; TAMPON_UDP];

    loop {
        if !rtc.is_alive() {
            // Le canal part avec la connexion : le dire explicitement, sinon
            // les commandes en vol attendraient un délai plutôt qu'une erreur.
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Ok(());
        }

        // Drainer `poll_output` jusqu'à `Output::Timeout` : même invariant que
        // la boucle vidéo (`transport.rs`), et pour la même raison — toute
        // mutation de `Rtc` doit être suivie d'un drainage complet.
        let echeance = loop {
            match rtc.poll_output().map_err(|e| anyhow!("poll_output du pont : {e}"))? {
                Output::Timeout(t) => break t,
                Output::Transmit(t) => {
                    // Une écriture qui échoue n'est pas fatale : str0m
                    // retransmettra. La journaliser par datagramme le serait —
                    // « ne jamais tracer par paquet dans la boucle de
                    // transport » est une leçon que ce dépôt a payée d'une
                    // session entière (18 619 lignes en quelques secondes,
                    // écrites sur un partage CIFS).
                    let _ = socket.send_to(&t.contents, t.destination);
                }
                Output::Event(evenement) => {
                    if let Some(fin) = traiter(evenement, &mut canal, &entrant) {
                        return fin;
                    }
                }
            }
        };

        let maintenant = Instant::now();
        let attente = echeance
            .saturating_duration_since(maintenant)
            .min(ATTENTE_MAX);

        // Une requête à émettre ? On attend au plus jusqu'à l'échéance str0m.
        match sortant.recv_timeout(attente) {
            Ok(VersNavigateur::Requete { correlation, trame }) => {
                emettre(&mut rtc, canal, correlation, &trame);
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::info!("plus personne n'émet de requête : arrêt du transport du pont");
                return Ok(());
            }
        }

        // Puis le socket, sans bloquer (il est non bloquant), et enfin le
        // temps qui passe.
        match socket.recv_from(&mut tampon) {
            Ok((taille, source)) => {
                let recu = Receive::new(Protocol::Udp, source, adresse, &tampon[..taille])
                    .map_err(|e| anyhow!("datagramme illisible : {e}"))?;
                rtc.handle_input(Input::Receive(Instant::now(), recu))
                    .map_err(|e| anyhow!("handle_input du pont : {e}"))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                rtc.handle_input(Input::Timeout(Instant::now()))
                    .map_err(|e| anyhow!("handle_input(Timeout) du pont : {e}"))?;
            }
            Err(e) => return Err(e).context("lecture du socket UDP du pont"),
        }
    }
}

/// Traite un événement str0m. Rend `Some(..)` quand la boucle doit s'arrêter.
fn traiter(
    evenement: Event,
    canal: &mut Option<ChannelId>,
    entrant: &Sender<DuNavigateur>,
) -> Option<Result<()>> {
    match evenement {
        Event::Connected => {
            tracing::info!("pont fichiers connecté au navigateur");
        }
        // 🔴 **Défaut trouvé par le test de fermeture, et non par la
        // relecture.** Sans ces deux bras, la boucle ne remarquait le départ du
        // pair qu'à l'expiration d'ICE — soit des dizaines de secondes après
        // que l'onglet s'est fermé, pendant lesquelles toute commande en vol
        // aurait attendu son DÉLAI au lieu de rendre `ERROR_IO_DEVICE` tout de
        // suite. C'est exactement le correctif I1 de `transport/evenements.rs`,
        // qui avait dû être fait là-bas pour la même raison ; le rejouer ici
        // aurait été la cinquième fois que ce dépôt paie un événement tombé
        // dans un bras fourre-tout.
        Event::Closed => {
            tracing::info!("connexion du pont fermée par le pair (close_notify DTLS)");
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Some(Ok(()));
        }
        Event::IceConnectionStateChange(str0m::IceConnectionState::Disconnected) => {
            tracing::warn!("ICE déconnecté sur le pont fichiers");
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Some(Ok(()));
        }
        Event::ChannelOpen(id, label) => {
            // ⚠️ **L'AIGUILLAGE EST PAR LABEL, DÈS LE PREMIER JOUR.**
            //
            // `transport/evenements.rs::dispatch_channel_data` aiguille sur le
            // seul `data.binary` et ignore `data.id`, pourtant disponible : un
            // canal tiers y serait traité comme `control` ou `input` selon son
            // seul mode d'écriture. Le défaut est réel et corrigé ailleurs ; on
            // ne le rejoue pas ici. Retenir l'id du label attendu, et refuser
            // tout le reste, coûte trois lignes maintenant et une recette
            // entière plus tard.
            if label == LABEL_FICHIERS {
                tracing::info!(%label, "canal du pont fichiers ouvert");
                *canal = Some(id);
                let _ = entrant.send(DuNavigateur::CanalOuvert);
            } else {
                tracing::warn!(
                    %label, ?id,
                    "canal de données ignoré : le pont ne sert que le label attendu"
                );
            }
        }
        Event::ChannelClose(id) => {
            if *canal == Some(id) {
                tracing::info!(?id, "canal du pont fichiers fermé");
                *canal = None;
                let _ = entrant.send(DuNavigateur::CanalFerme);
            }
        }
        Event::ChannelData(data) => {
            if *canal != Some(data.id) {
                // Nommer l'id : sans lui, ce `warn!` ne permet pas de dire
                // QUEL canal a parlé, et la trace serait inexploitable en
                // recette. « Une trace non attribuable coûte une
                // ré-imputation » (D6).
                tracing::warn!(
                    id = ?data.id, attendu = ?canal, octets = data.data.len(),
                    "données reçues sur un canal qui n'est pas celui du pont : ignorées"
                );
                return None;
            }
            match proto::fichiers::decoder(&data.data) {
                Ok(trame) => {
                    let correlation = trame.correlation;
                    if entrant
                        .send(DuNavigateur::Reponse { correlation, trame: data.data.to_vec() })
                        .is_err()
                    {
                        tracing::info!("plus personne ne lit les réponses : arrêt du transport");
                        return Some(Ok(()));
                    }
                }
                // Une trame illisible est JETÉE, jamais devinée : sa
                // corrélation est justement ce qu'on ne peut pas lire, donc
                // rien ne permettrait de la rattacher à une commande.
                Err(erreur) => tracing::warn!(%erreur, "trame du navigateur illisible, jetée"),
            }
        }
        _ => {}
    }
    None
}

/// Écrit une requête sur le canal, si le canal existe.
fn emettre(rtc: &mut Rtc, canal: Option<ChannelId>, correlation: u32, trame: &[u8]) {
    let Some(id) = canal else {
        // Ce n'est pas une anomalie de programmation : le canal peut tomber
        // entre l'inscription d'une commande et son émission. L'appelant
        // l'apprendra par l'expiration de sa table — c'est ce que la table
        // existe pour couvrir.
        tracing::warn!(correlation, "requête non émise : aucun canal du pont ouvert");
        return;
    };
    let Some(mut sortie) = rtc.channel(id) else {
        tracing::warn!(correlation, ?id, "requête non émise : canal introuvable côté str0m");
        return;
    };
    // ⚠️ `binary = true`, **à l'inverse du canal `control`** qui écrit `false` :
    // la charge d'une trame de fichiers est faite d'octets bruts, et l'écrire
    // en mode texte la ferait passer par une validation UTF-8 côté navigateur.
    if let Err(erreur) = sortie.write(true, trame) {
        tracing::warn!(%erreur, correlation, "écriture d'une requête du pont échouée");
    }
}

#[cfg(test)]
mod tests;
