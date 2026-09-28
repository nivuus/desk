//! The child-side pipe client: it connects to the sensor, attaches, and
//! returns a `SourceDistante` ready to serve the transport loop.
//!
//! **Two connections, a single direction per end.** The command connection (B)
//! is written then read by the calling thread alone, in strict alternation; the
//! media connection (A) only carries an identity frame at opening, then
//! is only read, by the dispatcher thread alone. No file object
//! therefore ever carries a concurrent read and write — see
//! task 10 of sub-block D4.

#![cfg(windows)]

use std::io::{BufReader, Write};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::capteur::distante::{Canal, Rattachee, Recu, SourceDistante};
use crate::capteur::horloge::lire_qpc;
use crate::capteur::pont_media::lire_le_media;
use crate::capteur::protocole::{
    lire_trame, write_json, DepuisCapteur, Trame, VersCapteur, NOM_TUBE,
};
use crate::capteur::reprise::DUREE_FENETRE_CANAL;

/// Depth of the frame queue between the reader thread and `next_frame`.
///
/// **It is what exerts back-pressure on the whole chain**: queue
/// full → the reader thread blocks → the pipe buffer fills up → the sensor's
/// write blocks, and its window thread waits. An access unit cannot
/// be thrown away without corrupting the stream, so blocking is the only correct
/// way out. 8 units ≈ 90 ms of video at 90 fps: enough to absorb a
/// scheduling hiccup, too little to let a session drift
/// silently.
const CAPACITE_FILE: usize = 8;

/// Step between two connection attempts. Same value as the resumption step
/// of `capture/reprise.rs`, and for the same reason: small enough not to
/// delay the real resumption, large enough for the trace to stay rare.
const PAS_CONNEXION: Duration = Duration::from_millis(150);

/// Opening window of the media connection, once the attach is accepted.
///
/// The sensor only recreates its listening instance after accepting the
/// previous one: an immediate opening may hit `ERROR_PIPE_BUSY`.
/// One second amply covers this race without delaying anything — the attach
/// has just succeeded, so the sensor is alive and its server is running.
///
/// **Public because the sensor derives its own wait budget from it**
/// (`serveur::DELAI_CONNEXION_MEDIA`): the two patiences must be a
/// single decision, otherwise the sensor holds a DXGI duplication long
/// after the child has given up.
pub const DUREE_OUVERTURE_MEDIA: Duration = Duration::from_secs(1);

pub fn connecter(
    session: &str,
    hwnd: u64,
    sortie: &str,
    fps: u32,
    debit: u32,
    // The KEPT size the supervisor set on this window — not
    // the output size. `(u32::MAX, u32::MAX)` in its absence (single-window
    // path): `retained_size`, on the sensor side (task 8), then brings it back
    // to the output size.
    size: (u32, u32),
    clock_origin: Instant,
) -> Result<SourceDistante> {
    let signalement = Signalement {
        session: session.to_string(),
        hwnd,
        sortie: sortie.to_string(),
        fps,
        debit,
        size,
        clock_origin,
    };
    // The FIRST opening is patient: the child may start before the
    // sensor has opened its pipe. The reopenings of `rattacher`, on the other hand,
    // are not — they run from the transport loop.
    let commandes = open_within(DUREE_FENETRE_CANAL)?;
    let attachee = attacher_sur(commandes, &signalement)?;
    let (largeur, hauteur) = (attachee.largeur, attachee.hauteur);
    Ok(SourceDistante::new(
        Box::new(CanalTube {
            commandes: Mutex::new(attachee.commandes),
            signalement,
        }),
        attachee.images,
        largeur,
        hauteur,
    ))
}

/// Attaches the child to the sensor on an already open command pipe, then
/// opens the media connection. **Shared by `connecter` and `rattacher`**: the
/// two must not carry two copies of this sequence.
///
/// The order is imposed: commands first (the attach returns the
/// dimensions there), media next (the identity pairs the connection with the
/// already attached session there). The sensor could not pair the reverse.
fn attacher_sur(mut commandes: std::fs::File, signalement: &Signalement) -> Result<Attachee> {
    // `clock_origin` was created by `demarrage.rs` BEFORE this call — the
    // connection may have waited for the sensor for several seconds, and a
    // re-attachment happens much later still. Reading QPC now and
    // sending it as is would shift the video by that whole gap relative to
    // the audio, which shares `clock_origin`. We therefore CORRECT by the elapsed time, which
    // makes the origin exact at every attach.
    let frequence = crate::capteur::horloge::frequence_qpc()?;
    let ecoule_tics = (signalement.clock_origin.elapsed().as_nanos() * frequence as u128
        / 1_000_000_000)
        .min(i64::MAX as u128) as i64;
    let origine_qpc = lire_qpc().context("reading QPC before the attach")? - ecoule_tics;

    // Write THEN read, on THIS thread, without any write buffer: it is
    // already the discipline of `commander`, and the attach inaugurates it.
    write_json(
        &mut commandes,
        &VersCapteur::Attache {
            session: signalement.session.clone(),
            hwnd: signalement.hwnd,
            sortie: signalement.sortie.clone(),
            fps: signalement.fps,
            debit: signalement.debit,
            size: signalement.size,
            origine_qpc,
        },
    )?;
    commandes.flush()?;

    let (largeur, hauteur) = match lire_trame(&mut commandes).context("answer to the attach")? {
        Trame::Json(octets) => match serde_json::from_slice::<DepuisCapteur>(&octets)? {
            DepuisCapteur::Attachee { largeur, hauteur } => (largeur, hauteur),
            // A refusal makes the attach fail LOUDLY: without it the child
            // would wait for a frame that will never come.
            DepuisCapteur::Refus { motif } => bail!("the sensor refused the attach: {motif}"),
            autre => bail!("unexpected answer to the attach: {autre:?}"),
        },
        Trame::Image(_) => bail!("the sensor answered an image to the attach"),
    };

    // The MEDIA connection. We write our identity there — the one and only frame
    // this end will ever write there — then hand it to the dispatcher thread, which
    // only reads. That is what makes it impossible for a read and a
    // write to cross there.
    let mut media = open_within(DUREE_OUVERTURE_MEDIA).context("opening the media connection")?;
    write_json(
        &mut media,
        &VersCapteur::Identite {
            session: signalement.session.clone(),
        },
    )?;
    media.flush()?;

    tracing::info!(
        session = %signalement.session,
        sortie = %signalement.sortie,
        largeur, hauteur,
        "attached to the sensor"
    );

    let (tx_images, rx_images) = sync_channel::<Recu>(CAPACITE_FILE);
    std::thread::spawn(move || lire_le_media(BufReader::new(media), tx_images));

    Ok(Attachee {
        commandes,
        images: rx_images,
        largeur,
        hauteur,
    })
}

/// Une seule tentative d'ouverture d'une instance du tube.
fn ouvrir_une_instance() -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(NOM_TUBE)
}

/// Retries the opening within a bounded window: the child may start before
/// the sensor has opened its pipe (at the very first launch, or during
/// a sensor restart), and a listening instance may be momentarily
/// busy between two welcomes.
fn open_within(fenetre: Duration) -> Result<std::fs::File> {
    let debut = Instant::now();
    let mut derniere = None;
    while debut.elapsed() <= fenetre {
        match ouvrir_une_instance() {
            Ok(file) => return Ok(file),
            Err(error) => {
                derniere = Some(error);
                std::thread::sleep(PAS_CONNEXION);
            }
        }
    }
    Err(anyhow::Error::from(derniere.expect("at least one attempt"))
        .context(format!("no sensor on {NOM_TUBE} after {fenetre:?}")))
}

struct CanalTube {
    /// The command connection. `Mutex` and not `&mut`: `Canal::commander`
    /// takes `&mut self`, but it is the write → read alternation that must
    /// stay indivisible, and nothing promises the caller will always be the
    /// same thread.
    commandes: Mutex<std::fs::File>,
    /// What is needed to re-attach to a restarted sensor. Kept at connection:
    /// at the moment of the break, nothing else carries these values.
    signalement: Signalement,
}

/// What must be repeated to the sensor to re-attach.
///
/// `clock_origin` is kept and NOT frozen in QPC ticks: each attach
/// recomputes `origine_qpc` from it, so that the origin stays
/// exact whatever the time elapsed since the child started.
struct Signalement {
    session: String,
    hwnd: u64,
    sortie: String,
    fps: u32,
    debit: u32,
    /// The KEPT size requested by the supervisor, repeated at every
    /// attach and every re-attachment (`connecter`, `Canal::rattacher`).
    size: (u32, u32),
    clock_origin: Instant,
}

/// The fruit of a successful attach, child side.
struct Attachee {
    commandes: std::fs::File,
    images: Receiver<Recu>,
    largeur: u32,
    hauteur: u32,
}

impl Canal for CanalTube {
    /// Reopens BOTH connections to the sensor (restarted by the
    /// supervisor) and re-sends the attach. Replaces the command connection of
    /// THIS `CanalTube`, and returns the new frame queue.
    ///
    /// **Without this method, the resumption window of `SourceDistante` would
    /// only delay the death of sessions by 15 s**: nothing else
    /// ever opens a second pipe. See task 3bis.
    ///
    /// A single attempt on the command connection, without internal
    /// patience: it is `SourceDistante` that holds the budget and the spacing
    /// (`PAS_RATTACHEMENT`). A patient opening would block the transport
    /// loop for up to 15 s.
    fn rattacher(&mut self) -> Result<Rattachee> {
        let commandes = ouvrir_une_instance().context("reopening the sensor pipe")?;
        let attachee = attacher_sur(commandes, &self.signalement)?;
        // Replace THIS channel's connection: the old one points to a dead
        // pipe, and `commander` would still use it.
        self.commandes = Mutex::new(attachee.commandes);
        Ok(Rattachee {
            images: attachee.images,
            largeur: attachee.largeur,
            hauteur: attachee.hauteur,
        })
    }

    /// ⚠️ **This read has NO timeout, and it is accepted.** The `recv_timeout`
    /// that used to bound the wait disappeared with the reader thread, and there
    /// is no equivalent of `SO_RCVTIMEO` for a synchronous named
    /// pipe. The chosen countermeasure is that the sensor closes its pipes when
    /// dying — the job object guarantees its death, and the closing of its
    /// handles with it —, which makes this read return an **error**
    /// rather than hang. **To be explicitly checked in the acceptance run**:
    /// kill the sensor while commands are flowing, and observe that
    /// `commander` returns an error.
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur> {
        let mut commandes = self
            .commandes
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner());
        // Write THEN read on the same thread: that is the discipline that
        // makes blocking impossible. Never introduce a reader thread
        // on this connection — see task 10 of sub-block D4.
        write_json(&mut *commandes, &message)?;
        commandes.flush()?;
        match lire_trame(&mut *commandes).context("sensor answer")? {
            Trame::Json(octets) => Ok(serde_json::from_slice(&octets)?),
            Trame::Image(_) => bail!("the sensor answered an image to a command"),
        }
    }
}
