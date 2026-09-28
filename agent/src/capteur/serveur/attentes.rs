//! The registry of media connection waits — extracted from `serveur.rs`
//! (review of task 16, D10): `serveur.rs` was at exactly 500 lines,
//! and this registry — the static, its accessors, `attendre_le_media`, `oublier` —
//! is LITERALLY the thing transposed from `sommeil/registre.rs`.
//! Extracting it here makes that parallel structural rather than merely asserted
//! in a comment, and the remedy for the finding below (the single lock)
//! benefits directly: it is the same gesture, in the same move.
//! Same set-up as `sommeil.rs` → `sommeil/registre.rs`.
//!
//! **Sessions attached on their command connection and waiting for their
//! media connection.** Key: the session identifier; value: the sending
//! channel of the media connection to come, and the generation of this wait.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// The complete state of the registry — the content AND the generation counter,
/// under ONE SINGLE lock.
///
/// ⚠️ **This is NOT the first form of this file.** The first version
/// (review of task 16) put the generation in an `AtomicU64` separate from the
/// map's `Mutex`, arguing that "the monotonicity of a `fetch_add` does not
/// depend on any other [data]". **It was the wrong invariant.** What
/// matters is not that the COUNTER is monotonic — it always is — but
/// that the generation STORED IN THE MAP is that of the LAST `insert`. A
/// `fetch_add` and an `insert` under two distinct locks are two distinct critical
/// sections, which can interleave:
///
/// ```text
/// T1 (attach A): fetch_add -> 1
/// T2 (attach B): fetch_add -> 2
/// T2: lock, insert (media2, 2)
/// T1: lock, insert (media1, 1)   // overwrites the MOST RECENT with 1
/// ```
///
/// The registry would then carry generation 1 while 2 is the
/// latest attach — an inversion `sommeil/registre.rs` CANNOT
/// produce, because `prochaine_generation += 1` and the `insert` are under
/// the SAME guard there (`sommeil/registre.rs::inscrire`). The window is narrow —
/// it takes two threads in `attendre_le_media` for the SAME session, hence a
/// child re-attaching while its previous `ouvrir_les_commandes` is
/// still running — and the consequence is not fatal (at worst a stale generation
/// wrongly accepted), but it is an inversion the transposed pattern
/// does not have, and which therefore has no reason to exist here either.
///
/// **The remedy**: a single lock for both, exactly like
/// `sommeil::registre::Etat` (`canaux` and `prochaine_generation` already live
/// there side by side, under the same `Mutex`).
struct Etat {
    attentes: HashMap<String, (Sender<std::fs::File>, u64)>,
    prochaine_generation: u64,
}

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

/// Locks the state while surviving poisoning: a window thread
/// that panics must not take down the welcome of all the following ones.
fn etat() -> MutexGuard<'static, Etat> {
    ETAT.get_or_init(|| {
        Mutex::new(Etat {
            attentes: HashMap::new(),
            prochaine_generation: 0,
        })
    })
    .lock()
    .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// Registers a session's media connection wait, and returns the generation
/// that has just been assigned to it.
///
/// **The generation is stamped HERE, at ATTACH — not transmitted by the
/// protocol, not taken at process launch.** Same fix as
/// the one the review of sub-block D9 imposed on
/// `capteur::sommeil::registre::inscrire` (see its comment, and the one
/// below, on `oublier`): the F5 race plays out between two successive
/// attaches of the SAME session (a child re-attaching after a
/// pipe break says the same name again), never between two process
/// launches — a child restarted by the supervisor receives a NEW name
/// (`Table::compteur`, `superviseur/table.rs`), hence no race. Only a
/// generation stamped at attach distinguishes the two registrations that
/// CAN overlap.
///
/// The caller (`ouvrir_les_commandes`, in `serveur.rs`) keeps the returned value
/// while serving this connection and hands it back as is
/// to `oublier`.
pub(super) fn attendre_le_media(session: &str, media: Sender<std::fs::File>) -> u64 {
    let mut garde = etat();
    // Stamped and inserted UNDER THE SAME GUARD: that is the whole remedy for the
    // finding above, see the comment on `Etat`.
    garde.prochaine_generation += 1;
    let generation = garde.prochaine_generation;
    // A replacement is logged: it signals a child re-attaching
    // without the previous wait having been settled. Dropping the old
    // sender immediately wakes up the corresponding window thread.
    if garde
        .attentes
        .insert(session.to_string(), (media, generation))
        .is_some()
    {
        tracing::warn!(%session, "attente de connexion média remplacée pour cette session");
    }
    generation
}

/// Removes a session's media connection wait — **if and only if**
/// the generation presented is indeed the current one.
///
/// ✅ **It is LITERALLY the F5 race (D7), closed here on the SECOND
/// registry (D10, hand-over 2).** Sub-block D9 had closed it on the sleep
/// registry alone (`capteur/sommeil/registre.rs`: `inscrire` stamps a
/// monotonic generation, `retirer` steps aside if the recorded one is more
/// recent) — its task 10 brief only named `sommeil`, and no
/// per-task review could see this twin. Found by the cross-cutting review
/// at the end of branch D9, closed here with the same pattern.
///
/// Without this comparison, an unconditional `remove` issued by a late thread
/// — an abandonment expiring at the precise instant the same session has just
/// re-attached — would take away the NEW wait: the child that has just
/// re-attached would then wait for media nobody would deliver any more.
pub(super) fn oublier(session: &str, generation: u64) {
    let mut garde = etat();
    if garde
        .attentes
        .get(session)
        .is_some_and(|(_, g)| *g != generation)
    {
        tracing::info!(%session, generation, "oubli périmé ignoré");
        return;
    }
    garde.attentes.remove(session);
}

/// Removes and returns the media sender a session is waiting for, **whatever
/// its generation** — called when the media connection itself arrives
/// (`accueillir`, `VersCapteur::Identite` branch, in `serveur.rs`).
///
/// The generation has nothing to say here: this removal pairs the media connection
/// with the CURRENT wait, whatever it is — this is NOT the path of the
/// F5 race, which only plays out between two calls to `oublier` (see its
/// comment). A media connection arriving for a stale generation
/// is no longer awaited by anyone anyway: its `Receiver` was
/// abandoned with the entry replaced by the re-attachment.
pub(super) fn retirer_pour_identite(session: &str) -> Option<Sender<std::fs::File>> {
    etat()
        .attentes
        .remove(session)
        .map(|(media, _generation)| media)
}

#[cfg(test)]
mod tests;
