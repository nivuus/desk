//! The media connection's writer thread: what the window thread hands it, the
//! bounded depth of their queue, and the writing itself.
//!
//! Split out of `fenetre.rs` when `cargo fmt` pushed that file past 500 lines.

use std::io::Write;
use std::sync::mpsc::Receiver;

use crate::capteur::protocole::{write_image, write_json, DepuisCapteur};
use crate::h264::AccessUnit;

/// Depth of the queue between the window thread and the media connection's
/// writer thread.
///
/// **Bounded on purpose**: a free queue would let access units accumulate
/// without limit that a child no longer reading will never take. It is the
/// exact counterpart of `CAPACITE_FILE` on the child side, and back-pressure therefore
/// keeps going up to the capture — but it now goes up in
/// `deposer`, which serves the commands at every wait round.
pub(super) const CAPACITE_ECRITURES: usize = 8;

/// What the window thread hands to the media connection's writer thread.
pub(super) enum AEcrire {
    Image(AccessUnit),
    Etat(DepuisCapteur),
}

/// The media connection's writer thread: it only writes, and it is the
/// only one touching this file object. Nobody reads it.
pub(super) fn write_media<E: Write>(mut ecrivain: E, charges: Receiver<AEcrire>, session: &str) {
    for charge in charges {
        let written = match charge {
            AEcrire::Image(unite) => write_image(&mut ecrivain, &unite),
            AEcrire::Etat(message) => write_json(&mut ecrivain, &message),
        };
        // `flush` at every payload: in front of a still window, the next
        // payload may never come, and the child would wait for this one in
        // a buffer. Same lesson as the attach reply of task 9.
        if let Err(error) = written.and_then(|()| ecrivain.flush()) {
            tracing::warn!(%session, %error, "writing to the media connection interrupted");
            return;
        }
    }
}
