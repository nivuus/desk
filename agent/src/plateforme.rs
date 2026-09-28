//! Client of the `/agent` channel: the agent enrols on it, beats its heart on it, and receives
//! from it its session prefix and its agent token — and, since sub-block G1,
//! it PUSHES its application catalogue on it and RECEIVES launch
//! orders from it.
//!
//! ⚠️ THIS CHANNEL THEREFORE NO LONGER CARRIES ONLY AN IDENTITY, contrary to what
//! the next paragraph says, written in sub-block P3 and kept for its
//! reasoning. Two paths have gone through it since: a bounded emission queue
//! (`FILE_EMISSION`) for what goes up, and an `mpsc` of orders for what
//! goes down.
//!
//! ⚠️ **THIS CHANNEL IS NOT THE SIGNALING**, even though it lives on the same server.
//! `crate::signaling` and `crate::superviseur::signalisation` negotiate a
//! media session; this one carries an IDENTITY. Without it, the
//! platform's guard refuses the handshake of the two others (sub-block P3), and
//! no session is established.
//!
//! 🔴 **RECONNECTION IS NEW BEHAVIOUR, and it is the raison d'être of this
//! file.** Neither `signaling.rs` nor `signalisation.rs` has any: their fall
//! is only logged, which is assumed there because the media no longer
//! depends on signaling once the offer has been exchanged. **That reasoning does
//! not carry over here**: this channel carries the heartbeat, hence `vu_a`.
//! Without reconnection, the FIRST network cut would make the VM `injoignable`
//! permanently, and the platform would punish a network cut as an
//! agent failure.

pub mod identite;
pub mod repli;

// Tests extracted into a neighbouring file (same mechanism and same reason as
// `superviseur/table.rs`): they hold a real local WebSocket server and
// weigh as much as the client itself.
#[cfg(test)]
#[path = "plateforme/tests.rs"]
mod tests;

// 🔴 A SECOND TEST FILE, BORN FROM CROSSING BY **ONE** LINE (501).
// Compressing for an overflow of one would be exactly the gesture that
// sub-block D9 paid for: `sommeil.rs` brought back to 499 by compression, then extracted
// on review demand. Crossing by one line is a crossing.
#[cfg(test)]
#[path = "plateforme/tests_installation.rs"]
mod tests_installation;

use std::time::Duration;

use proto::plateforme::VersLaPlateforme;
use tokio::sync::{mpsc, watch};

/// Heartbeat period.
///
/// ⚠️ **NOT CALIBRATED**, but **NOT free**: it must stay clearly below
/// the platform's unreachability threshold (`SEUIL_INJOIGNABLE_MS`,
/// 90 s in sub-block P3), otherwise a perfectly alive VM would be
/// declared unreachable between two beats. A factor of 3 leaves room for
/// two lost beats. **The two constants live in different code
/// repositories and nothing links them mechanically**: changing one requires
/// rereading the other.
pub const PERIODE_BATTEMENT: Duration = Duration::from_secs(30);

/// What the platform delivers, and what the rest of the agent reads: the prefix
/// that names its sessions, and the token that opens its handshakes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identite {
    pub prefixe: String,
    pub jeton: String,
    /// In milliseconds, like every timestamp of the platform.
    pub expire_a: i64,
}

/// Combien de messages montants peuvent attendre leur socket.
///
/// 🔴 BOUNDED, AND NOT UNLIMITED: this channel can stay cut for hours, and an
/// unlimited queue behind a dead socket is a memory leak whose name nothing
/// says. What overflows is LOST — see [`Canal::emettre`], which carries
/// the reason why that is acceptable.
///
/// The value comes from the real traffic: one catalogue per reconciliation, that is one
/// message every thirty seconds, plus one `Lancee` per order.
/// Thirty-two covers a quarter of an hour of outage. **NOT CALIBRATED.**
const FILE_EMISSION: usize = 32;

/// The open channel, and the thread that holds it. **Dropping it stops the
/// heartbeat — and, since G1, APPLICATION DISCOVERY with it**: the
/// `apps` loop exits on `TryRecvError::Disconnected` and logs that the
/// /agent channel is closed and application discovery stopped. `main` keeps it alive
/// for the whole duration of the process.
///
/// ❌ **"TWO MECHANISMS INSTEAD OF ONE" BECAME FALSE IN SUB-BLOCK G3: THERE
/// ARE THREE.** The INSTALLATION thread stops too — it drains
/// `installations()`, whose sender dies with the channel, and it logs that
/// the /agent channel is closed and the installation thread stopped. The count is corrected
/// rather than removed: it is what says what is lost by dropping this field.
/// The vocabulary of downstream orders lives in a child module — see its
/// header for the declaration of the ceiling crossing that produced it.
mod ordre;
pub use ordre::Ordre;

/// The INSTALLATION order travels in its own queue, and the header of this module
/// says why: it is not the same consumer.
mod installation;
pub use installation::Installation;

pub struct Canal {
    identite: watch::Receiver<Option<Identite>>,
    /// The reconnection thread. Never awaited — it only ends on a final
    /// refusal —, but kept so as not to be silently abandoned.
    ///
    /// ⚠️ THE `allow` REMAINS JUSTIFIED, BUT NOT FOR THE SAME REASON ON BOTH
    /// TARGETS — noted by REMOVING it, on each:
    ///   - `--target x86_64-pc-windows-gnu`: "field `tache` is never read".
    ///     It is the HISTORICAL reason, and the only one that remains on the
    ///     real target; `emission` and `ordres` are indeed read, by the
    ///     discovery loop;
    ///   - on the host: "fields `tache`, `emission`, and `ordres` are never
    ///     read", because `apps::demarrer` is a stub there that returns `None`
    ///     without touching anything.
    ///
    /// An `allow` that became useless is an assertion that became false: this one
    /// is to be reread the day nothing would call `apps::brancher` anymore.
    #[allow(dead_code)]
    tache: tokio::task::JoinHandle<()>,
    /// The upstream queue, drained in the `select!` of [`une_session`].
    emission: mpsc::Sender<VersLaPlateforme>,
    /// The downstream orders. `Option` because only one consumer can
    /// take it: two would steal orders from one another, and each would only
    /// see part of them.
    ordres: Option<mpsc::UnboundedReceiver<Ordre>>,
    /// The INSTALLATION orders. Same single-consumer rule, and for
    /// the same reason — but a DIFFERENT consumer: the installation thread
    /// runs on `tokio`, while `ordres` is drained by the COM thread of
    /// discovery. See `plateforme/installation.rs`.
    installations: Option<mpsc::UnboundedReceiver<Installation>>,
    /// The signaling URL as it was received — sub-block G2.
    ///
    /// ⚠️ IT IS KEPT RATHER THAN REREAD FROM THE ENVIRONMENT: the
    /// icon upload derives its HTTP address from it, and rereading
    /// `SIGNALING_URL` elsewhere would make the same value live in two places,
    /// hence diverge the day one of the two were changed.
    signaling_url: String,
}

/// What is needed to emit without holding the whole [`Canal`].
#[derive(Clone)]
pub struct Emetteur {
    file: mpsc::Sender<VersLaPlateforme>,
}

impl Emetteur {
    /// Queues an upstream message. **Never blocks, and returns no
    /// error.**
    ///
    /// 🔴 A MESSAGE QUEUED WHILE THE SOCKET IS DOWN IS LOST, AND
    /// IT IS INTENDED. This channel is a WebSocket `push`: it has no delivery
    /// guarantee, in either direction. Making it blocking would turn the
    /// queue into a memory leak on a channel that can stay cut for hours;
    /// making it fatal would kill the channel on an ordinary network cut.
    ///
    /// **What makes the loss acceptable lies elsewhere, and a single thing
    /// makes it acceptable**: the agent sends its COMPLETE catalogue again
    /// (`complet = true`) at each re-enrolment, so any divergence born
    /// from a lost message has an END. Removing this complete resend would make this
    /// loss silent and permanent.
    pub fn emettre(&self, message: VersLaPlateforme) {
        if let Err(erreur) = self.file.try_send(message) {
            tracing::warn!(%erreur, "message montant abandonné : canal coupé ou file pleine");
        }
    }
}

/// Why a session of the channel ended.
enum Fin {
    /// There is nothing to retry: the same attempt would return the same refusal.
    Definitive,
    /// The socket went down, or the platform refused for a reason that
    /// may change (a VM can be enrolled afterwards).
    Reprenable,
}

/// Composes the channel's URL from the signaling's.
///
/// The `trim_end_matches` is not a nicety: `ws://h:8080/` followed
/// by `/agent` would give `ws://h:8080//agent`, and the platform's upgrade
/// compares the path **exactly** — `//agent` is not `/agent`, and the
/// socket would be closed on a `404` that nothing on the agent side would explain.
pub fn url_du_canal(signaling_url: &str) -> String {
    format!("{}/agent", signaling_url.trim_end_matches('/'))
}

/// Opens the channel and holds it: enrolment, heartbeat, and reconnection.
///
/// Returns immediately — the identity arrives later, through `attendre_identite`.
pub fn ouvrir(signaling_url: &str, vm: String, secret: String) -> Canal {
    let url = url_du_canal(signaling_url);
    let (tx, identite) = watch::channel(None);
    let (emission, mut a_emettre) = mpsc::channel(FILE_EMISSION);
    // ⚠️ UNBOUNDED, unlike the upstream queue, and for an opposite
    // reason: its consumer handles each order in a `spawn_blocking`
    // and must NEVER make the channel loop wait — a blocking `send`
    // here would suspend the heartbeat, and the platform would declare the VM
    // unreachable while it launches an application. The rate bounds it
    // in practice: one order per user click.
    let (ordres_tx, ordres_rx) = mpsc::unbounded_channel();
    let (installations_tx, installations_rx) = mpsc::unbounded_channel();
    let tache = tokio::spawn(async move {
        // The attempt restarts from ZERO after each successful enrolment: an
        // agent connected for three days that loses its network for one second
        // must reconnect in half a second, not in thirty.
        let mut tentative = 0u32;
        loop {
            let reussite_precedente = tx.borrow().is_some();
            if reussite_precedente {
                tentative = 0;
            }
            match une_session(
                &url,
                &vm,
                &secret,
                &tx,
                &mut a_emettre,
                &ordres_tx,
                &installations_tx,
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
    /// Waits for the first identity. Returns `None` if the reconnection loop has
    /// given up — that is if the wait is in vain, and not "not yet".
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

    /// Takes the queue of downstream orders. Returns `None` on the second call.
    ///
    /// A single consumer, because two would steal orders from one
    /// another and the symptom would be "one launch out of two does not go".
    pub fn ordres(&mut self) -> Option<mpsc::UnboundedReceiver<Ordre>> {
        self.ordres.take()
    }

    /// Takes the installations queue. Returns `None` on the second call.
    ///
    /// 🔴 SAME PROPERTY AS [`Self::ordres`], AND FOR THE SAME REASON: two
    /// consumers would steal orders from one another, and the symptom
    /// would be "one installation out of two does not go". The difference is
    /// that here the consumer is the `tokio` installation thread, never the
    /// COM thread of discovery — it is this difference that justifies the second
    /// queue rather than a variant of [`Ordre`].
    pub fn installations(&mut self) -> Option<mpsc::UnboundedReceiver<Installation>> {
        self.installations.take()
    }

    /// The signaling URL, from which the icon upload derives its HTTP
    /// address (sub-block G2).
    pub fn url_signaling(&self) -> &str {
        &self.signaling_url
    }

    /// A detachable sender, for the discovery thread.
    ///
    /// The `Canal` itself is not `Send` to a blocking thread that would
    /// keep it indefinitely: it is the queue, and it alone, that must
    /// cross. A tokio `Sender` is `Send` and `Clone`.
    pub fn emetteur(&self) -> Emetteur {
        Emetteur {
            file: self.emission.clone(),
        }
    }

    /// Observes identity changes — a re-enrolment is one.
    ///
    /// It is through it that the discovery loop knows it must send the
    /// COMPLETE catalogue again rather than a delta.
    pub fn veille_identite(&self) -> watch::Receiver<Option<Identite>> {
        self.identite.clone()
    }
}

/// One session of the channel, from connection to its fall, extracted BEFORE
/// sub-block G3 added its second queue: this file was at 490 lines,
/// margin 10.
mod session;
use session::une_session;
