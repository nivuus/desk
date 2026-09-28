//! The bridge's transport: a `PeerConnection` **without media**, carrying a
//! single `fichiers` data channel, reliable and ordered.
//!
//! **Mixed, but without Windows**: the str0m loop and the UDP socket are
//! portable, and this module is therefore entirely tested on the host. It knows
//! **neither ProjFS nor Windows** — it carries opaque, correlated bytes, and
//! nothing else. That is what allows exercising it with a real str0m peer in
//! local loopback, without any virtualisation root.
//!
//! **Why a DEDICATED connection** (decision D4): the video session already
//! carries `control` and `input`, and the signaling relay only accepts one
//! `agent` and one `client` per identifier. Above all, mixing the files channel with
//! the video session would mean a reconnection of one would carry away the other —
//! which principle 4 of the framing forbids.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, Input, Output, Rtc};

/// The label of the bridge's data channel. **It is the browser that creates the
/// channel** (`createDataChannel('fichiers')`); the agent is the responder.
pub const LABEL_FICHIERS: &str = "fichiers";

/// UDP receive buffer size. A bridge frame fits in
/// `TAILLE_TRAME_MAX` plus its header, but SCTP fragments: this buffer bounds
/// a datagram, not an application message.
const TAMPON_UDP: usize = 2048;

/// Maximum wait of a loop turn when str0m has no close
/// deadline. Bounds the latency of taking into account a request dropped into
/// `sortant` — without it, a request arriving just after a `recv_timeout`
/// would wait for the next str0m deadline.
const ATTENTE_MAX: Duration = Duration::from_millis(20);

/// What the bridge sends to the browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersNavigateur {
    Requete { correlation: u32, trame: Vec<u8> },
}

/// What the bridge receives from the browser, or learns of the channel state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuNavigateur {
    Reponse {
        correlation: u32,
        trame: Vec<u8>,
    },
    CanalOuvert,
    /// The channel is gone: tab closed, page reloaded, WebRTC down.
    /// The caller translates to [`crate::pont::erreurs::Erreur::CanalFerme`],
    /// that is, `ERROR_IO_DEVICE` — "the standard I/O error" of the framing.
    CanalFerme,
}

/// Builds the UDP socket and the `Rtc` of a **data-only** endpoint.
///
/// Compared to `transport::initialisation::construire_rtc`, **dropped** are:
/// `enable_h264`, `enable_opus`, `enable_bwe`, `set_stats_interval` and
/// `set_desired_bitrate` — there is no track, hence nothing to estimate or
/// probe. **Kept** are the non-blocking UDP socket (whose reason is measured:
/// `set_read_timeout`'s delay overshoots massively under Windows), the setting of the
/// cryptographic provider, and the host candidate.
///
/// ⚠️ **`clear_codecs()` without any `enable_*` is deliberate, and it works**:
/// exercised by this module's tests, which really negotiate and exchange
/// on a data channel with a str0m peer without any codec being
/// enabled on either side. The browser offers no track; there is therefore nothing
/// to match.
pub fn construire_rtc_donnees(local_ip: IpAddr) -> Result<(UdpSocket, Rtc)> {
    let socket =
        UdpSocket::bind(SocketAddr::new(local_ip, 0)).context("ouverture du socket UDP du pont")?;
    socket
        .set_nonblocking(true)
        .context("passage du socket UDP du pont en non bloquant")?;
    let addr = socket.local_addr()?;
    tracing::info!(%addr, "socket UDP du pont fichiers");

    // Idempotent: `OnceLock::set` silently ignores a second call. The
    // bridge is a separate process, so in practice it is the first — but
    // this module's tests build several.
    str0m::crypto::from_feature_flags().install_process_default();

    let mut rtc = Rtc::builder().clear_codecs().build(Instant::now());
    rtc.add_local_candidate(
        Candidate::host(addr, "udp").map_err(|e| anyhow!("candidat hôte invalide : {e}"))?,
    );
    Ok((socket, rtc))
}

/// The transport loop. Returns `Ok(())` when the connection ends or the
/// caller drops `sortant`.
///
/// `sortant` carries the requests to emit, `entrant` returns the responses and the
/// channel state changes.
pub fn tourner(
    mut rtc: Rtc,
    socket: UdpSocket,
    sortant: Receiver<VersNavigateur>,
    entrant: Sender<DuNavigateur>,
) -> Result<()> {
    let adresse = socket
        .local_addr()
        .context("adresse locale du socket du pont")?;
    let mut canal: Option<ChannelId> = None;
    let mut tampon = vec![0u8; TAMPON_UDP];

    loop {
        if !rtc.is_alive() {
            // The channel goes with the connection: saying so explicitly, otherwise
            // commands in flight would wait for a delay rather than an error.
            let _ = entrant.send(DuNavigateur::CanalFerme);
            return Ok(());
        }

        // Drain `poll_output` until `Output::Timeout`: same invariant as
        // the video loop (`transport.rs`), and for the same reason — every
        // mutation of `Rtc` must be followed by a complete drain.
        let echeance = loop {
            match rtc
                .poll_output()
                .map_err(|e| anyhow!("poll_output du pont : {e}"))?
            {
                Output::Timeout(t) => break t,
                Output::Transmit(t) => {
                    // A failing write is not fatal: str0m
                    // will retransmit. Logging it per datagram would be —
                    // "never trace per packet in the transport
                    // loop" is a lesson this repository paid for with a
                    // whole session (18,619 lines in a few seconds,
                    // written to a CIFS share).
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

        // A request to emit? We wait at most until the str0m deadline.
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

        // Then the socket, without blocking (it is non-blocking), and finally the
        // passing time.
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

/// Handles a str0m event. Returns `Some(..)` when the loop must stop.
fn traiter(
    evenement: Event,
    canal: &mut Option<ChannelId>,
    entrant: &Sender<DuNavigateur>,
) -> Option<Result<()>> {
    match evenement {
        Event::Connected => {
            tracing::info!("pont fichiers connecté au navigateur");
        }
        // 🔴 **Defect found by the closing test, not by
        // review.** Without these two arms, the loop only noticed the peer's
        // departure at ICE expiry — that is, tens of seconds after
        // the tab closed, during which any command in flight
        // would have waited for its DELAY instead of returning `ERROR_IO_DEVICE` right
        // away. It is exactly fix I1 of `transport/evenements.rs`,
        // which had had to be made there for the same reason; replaying it here
        // would have been the fifth time this repository paid for an event fallen
        // into a catch-all arm.
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
            // ⚠️ **ROUTING IS BY LABEL, FROM DAY ONE.**
            //
            // ❌ *This comment described IN THE PRESENT TENSE a defect of
            // `transport/evenements.rs::dispatch_channel_data` — "routes
            // on `data.binary` alone and ignores `data.id`". Task 17 of
            // the SAME branch fixed it: it now calls
            // `destination(data.id, …)`, so it looks at the channel.* The reason
            // for this block does not change: keeping the id of the expected label, and
            // refusing
            // everything else, costs three lines now and a whole acceptance
            // run later.
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
                // Naming the id: without it, this `warn!` does not allow saying
                // WHICH channel spoke, and the trace would be unusable in
                // an acceptance run. "An unattributable trace costs a
                // re-attribution" (D6).
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
                        .send(DuNavigateur::Reponse {
                            correlation,
                            trame: data.data.to_vec(),
                        })
                        .is_err()
                    {
                        tracing::info!("plus personne ne lit les réponses : arrêt du transport");
                        return Some(Ok(()));
                    }
                }
                // An unreadable frame is THROWN AWAY, never guessed: its
                // correlation is precisely what cannot be read, so
                // nothing would allow linking it to a command.
                Err(erreur) => tracing::warn!(%erreur, "trame du navigateur illisible, jetée"),
            }
        }
        _ => {}
    }
    None
}

/// Writes a request on the channel, if the channel exists.
fn emettre(rtc: &mut Rtc, canal: Option<ChannelId>, correlation: u32, trame: &[u8]) {
    let Some(id) = canal else {
        // It is not a programming anomaly: the channel can go down
        // between a command's registration and its emission. The caller
        // will learn it through its table's expiry — it is what the table
        // exists to cover.
        tracing::warn!(
            correlation,
            "requête non émise : aucun canal du pont ouvert"
        );
        return;
    };
    let Some(mut sortie) = rtc.channel(id) else {
        tracing::warn!(
            correlation,
            ?id,
            "requête non émise : canal introuvable côté str0m"
        );
        return;
    };
    // ⚠️ `binary = true`, **unlike the `control` channel** which writes `false`:
    // a files frame's payload is made of raw bytes, and writing it
    // in text mode would put it through UTF-8 validation on the browser side.
    if let Err(erreur) = sortie.write(true, trame) {
        tracing::warn!(%erreur, correlation, "écriture d'une requête du pont échouée");
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_fermeture;
