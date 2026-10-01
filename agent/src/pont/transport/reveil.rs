//! **What wakes the transport loop**: a request to emit, or a datagram
//! received — merged into ONE channel, so that the loop has ONE place to wait.
//!
//! # 🔴 WHY THIS MODULE EXISTS: THE 33 KiB/s CEILING
//!
//! The loop used to wait on TWO sources it could not watch together: the
//! request channel (`sortant.recv_timeout`) and the UDP socket (non-blocking
//! `recv_from`). It therefore blocked up to `ATTENTE_MAX` on the first, THEN
//! read **one** datagram from the second, and started again. F4 measured the
//! result — ~1,200 bytes per ~35 ms turn, **30 to 33 KiB/s, linear** — and
//! established the attribution by mutation (`ATTENTE_MAX` 20 ms → 1 ms doubles
//! the throughput, and no more: the fixed cost of the turn then dominates).
//! See `docs/superpowers/plans/2026-08-21-pont-fichiers-f4-resultats.md` §7.2. (policy: allow-fr, real file path)
//!
//! The remedy F4 named is "a change of design of the loop, not a setting":
//! two small relay threads turn each source into a [`Reveil`] on a single
//! channel. A datagram now wakes the loop **as soon as it arrives**, and a
//! burst of datagrams is drained without any wait between them.
//!
//! ⚠️ **The socket goes back to BLOCKING mode, with a read timeout**, and the
//! timeout is NOT on the data path: a datagram ends the wait the moment it
//! lands. The timeout only bounds how long the reader takes to notice the
//! stop — which is why its overshoot under Windows (the measured reason the
//! socket was non-blocking) no longer costs anything.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};

use super::{VersNavigateur, TAMPON_UDP};

/// How long the reader blocks before checking whether the loop is gone.
/// **Off the data path**: see the module header.
const PERIODE_LECTEUR: Duration = Duration::from_millis(100);

/// The longest a send may block the loop. ⚠️ The socket is blocking for its
/// reader, and the blocking mode is shared with the loop's sends: a full OS
/// send buffer would otherwise stall the loop and its timers. Past this, the
/// send fails and is dropped — exactly what the non-blocking socket did with
/// `WouldBlock` — and str0m retransmits.
const ATTENTE_ENVOI: Duration = Duration::from_millis(5);

/// How many wake-ups may wait for the loop at most.
///
/// 🔴 **BOUNDED, AND IT IS NOT A DETAIL**: the reader drains the socket faster
/// than the loop may process, so an unbounded channel would move the kernel's
/// BOUNDED receive buffer into UNBOUNDED process memory — any sender reaching
/// the host candidate could make it grow. Full, the channel blocks the reader;
/// the datagrams then wait in the kernel, which drops them past its own
/// buffer, exactly as before this module. SCTP retransmits. 1,024 datagrams
/// of at most `TAMPON_UDP` bytes is ~2 MiB at worst.
const CAPACITE: usize = 1024;

/// One reason for the transport loop to take a turn.
#[derive(Debug)]
pub(super) enum Reveil {
    Requete(VersNavigateur),
    Datagram {
        source: SocketAddr,
        bytes: Vec<u8>,
    },
    /// The socket failed for good: the loop stops on this error, as it did
    /// when it read the socket itself.
    Socket(std::io::Error),
    /// Every `Sender<VersNavigateur>` is gone: nobody emits requests any more.
    PlusDeRequetes,
}

/// The receiving end. **Dropping it stops the reader** at its next period.
pub(super) struct Reveils {
    recu: Receiver<Reveil>,
    arret: Arc<AtomicBool>,
}

impl Reveils {
    pub(super) fn attendre(&self, attente: Duration) -> Result<Reveil, RecvTimeoutError> {
        self.recu.recv_timeout(attente)
    }
}

impl Drop for Reveils {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}

/// Starts the two relays and returns their merged channel.
///
/// ⚠️ **The relay of `sortant` is NOT stopped by [`Reveils`]' drop**: it is
/// blocked in `recv()`, and leaves on the next request (whose send then fails)
/// or when the last `Sender` goes. That is the same lifetime the `Receiver`
/// had when the loop held it: the bridge's stop releases every `Sender`
/// (`pont.rs`), so nothing outlives the process.
pub(super) fn armer(socket: &UdpSocket, sortant: Receiver<VersNavigateur>) -> Result<Reveils> {
    let (envoi, recu) = sync_channel(CAPACITE);
    let arret = Arc::new(AtomicBool::new(false));

    let lecture = socket
        .try_clone()
        .context("duplicating the bridge UDP socket for its reader")?;
    // ⚠️ The blocking mode is a property of the SOCKET, shared by both handles:
    // the loop's sends become blocking too, hence their bound.
    lecture
        .set_nonblocking(false)
        .context("switching the bridge UDP socket back to blocking")?;
    lecture
        .set_write_timeout(Some(ATTENTE_ENVOI))
        .context("bounding the bridge's sends")?;
    lecture
        .set_read_timeout(Some(PERIODE_LECTEUR))
        .context("bounding the bridge reader's wait")?;

    let envoi_lecteur = envoi.clone();
    let arret_lecteur = Arc::clone(&arret);
    std::thread::Builder::new()
        .name("pont-transport-lecteur".into())
        .spawn(move || lire(lecture, envoi_lecteur, arret_lecteur))
        .context("launching the bridge socket reader")?;

    let relais = std::thread::Builder::new()
        .name("pont-transport-relais".into())
        .spawn(move || {
            for requete in sortant.iter() {
                if envoi.send(Reveil::Requete(requete)).is_err() {
                    return;
                }
            }
            let _ = envoi.send(Reveil::PlusDeRequetes);
        });
    if let Err(error) = relais {
        // The reader is already running: stop it, or it would loop on its
        // timeouts for the life of the process.
        arret.store(true, Ordering::Relaxed);
        return Err(error).context("launching the bridge request relay");
    }

    Ok(Reveils { recu, arret })
}

fn lire(socket: UdpSocket, envoi: SyncSender<Reveil>, arret: Arc<AtomicBool>) {
    let mut tampon = vec![0u8; TAMPON_UDP];
    while !arret.load(Ordering::Relaxed) {
        let reveil = match socket.recv_from(&mut tampon) {
            Ok((size, source)) => Reveil::Datagram {
                source,
                bytes: tampon[..size].to_vec(),
            },
            // The period elapsed: `WouldBlock` on Unix, `TimedOut` on Windows.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                continue
            }
            Err(e) => {
                let _ = envoi.send(Reveil::Socket(e));
                return;
            }
        };
        if envoi.send(reveil).is_err() {
            return;
        }
    }
}
