//! The sensor's named pipe server.
//!
//! Each child connects to it and describes itself in its first frame:
//! there is therefore NO supervisor → sensor channel, and no state table
//! shared between three processes. Closing the pipe IS the end-of-life
//! signal of a window.
//!
//! **Two connections per window, and a single direction per end.** A child first opens
//! the command connection (it writes its attach there and reads the
//! replies), then the media connection (it writes its only `Identite` there, and
//! only reads frames from then on). No file object therefore ever carries a
//! concurrent read and write — see task 10 of sub-block D4:
//! the acceptance run established that a write by the sensor did not complete as long
//! as a blocking read was pending on the same pipe instance.

#![cfg(windows)]

use std::io::{BufReader, BufWriter, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

use crate::capteur::fenetre::Fenetre;
use crate::capteur::protocole::{ecrire_json, lire_trame, DepuisCapteur, Trame, VersCapteur};
use crate::capteur::tube::DUREE_OUVERTURE_MEDIA;

mod attentes;
use attentes::{attendre_le_media, oublier};

mod instances;
use instances::{connecter, creer_instance, SOUFFLE_CREATION_INSTANCE};

/// Maximum wait for the media connection after an accepted attach.
///
/// **Aligned on the child's patience (`tube::DUREE_OUVERTURE_MEDIA`), and
/// not on `DUREE_FENETRE_CANAL`.** Task 10 prescribed 15 s "so that a
/// child giving up and a sensor giving in discover each other at the same
/// moment"; but on THIS connection, what governs the child's giving up
/// is `DUREE_OUVERTURE_MEDIA`, not the resumption window of an already
/// established session. Past its own budget, the child fails and dies: waiting
/// longer would catch nobody.
///
/// ⚠️ **And waiting is costly here**: at this point the `Fenetre` is ALREADY
/// built, so this output's DXGI duplication is already taken. Yet DXGI
/// only allows one per output: every second of needless waiting is a
/// second during which any restart of this window would be refused with
/// `0x80070057` — and with `RELANCES_MAX = 3` a window can burn its four
/// attempts in such a gap. Hence the one-second margin, and no more.
const DELAI_CONNEXION_MEDIA: Duration =
    DUREE_OUVERTURE_MEDIA.saturating_add(Duration::from_secs(1));

/// Maximum wait for a reply from the window thread — at attach as at
/// every command.
///
/// **Accepted upper bound, strictly above the documented worst case.** `resize`
/// may rebuild a complete encoding chain, and `Drop for H264Encoder`
/// carries a part bounded at 8 s in the worst case (`2 × DELAI_BARRIERE +
/// 2 × DELAI_ARRET_MFT`); 12 s leave 4 s for the rebuild. Too
/// short, we would declare dead a window that is working.
///
/// **It is the belt, not the braces.** The remedy for the deadlock found
/// by the review of task 10 is structural (the writer thread of
/// `fenetre.rs`); this bound exists so that a window thread silent for an
/// unforeseen reason returns an **error** to the child — which the transport
/// loop already absorbs through a `warn!` — instead of freezing it forever.
/// The child itself no longer has any timeout on its read.
const DELAI_REPONSE_FENETRE: Duration = Duration::from_secs(12);

pub fn servir() -> Result<()> {
    // True as soon as a `creer_instance` failure has been reported — see below.
    let mut echec_signale = false;
    loop {
        // A NEW instance per client. `PIPE_UNLIMITED_INSTANCES` only makes
        // sense because this loop NEVER blocks on a child's attach
        // frame (see below): without this detachment, a
        // single instance listened at a time despite its name, and a
        // connected child that never sends its attach blocked the welcome of
        // all the following windows.
        // **Not fatal, like the two other errors of this loop**
        // (fix I5 from the final branch review). A `?` here brought
        // down the WHOLE sensor — hence the N sessions — for a failure of
        // `CreateNamedPipeW` that may be transient (momentary exhaustion
        // of a system resource), whereas the loop already knows how to survive
        // a connection refusal and a failed attach. The breather avoids
        // making it a tight loop if the cause persists: without it, a
        // permanent failure would produce thousands of lines per second on the
        // CIFS share.
        //
        // The line is reported only once per series of failures, same reason
        // as `Enfant::etat_illisible_signale` (`superviseur/lanceur.rs`): the
        // breather alone would not be enough to bound the log if the cause lasts.
        let tube = match creer_instance() {
            Ok(tube) => {
                if echec_signale {
                    echec_signale = false;
                    tracing::info!("création d'instances de tube rétablie");
                }
                tube
            }
            Err(erreur) => {
                if !echec_signale {
                    echec_signale = true;
                    tracing::warn!(
                        %erreur,
                        "création d'une instance de tube refusée, réessais \
                         (signalé une seule fois tant que l'échec se répète)"
                    );
                }
                std::thread::sleep(SOUFFLE_CREATION_INSTANCE);
                continue;
            }
        };

        match connecter(tube) {
            // Connected: move ALL of the welcome — reading the first
            // frame included — onto its own thread, DETACHED and never joined,
            // for the same reason as the window thread: tearing down a
            // `WindowsSource` can freeze, and the accept loop must
            // never be able to. It is also what loops back immediately
            // to listen on the next instance, instead of waiting for this
            // child — and each child now opens TWO connections.
            Ok(()) => {
                // `HANDLE` carries a raw pointer and is therefore not `Send` —
                // it crosses the thread boundary as an integer, without
                // risk: this pipe instance is no longer touched by the
                // accept loop once the thread is launched, so no
                // aliasing between the two threads.
                let brut = tube.0 as usize;
                std::thread::spawn(move || {
                    let tube = HANDLE(brut as *mut _);
                    // A failed attach does NOT bring the server down: the
                    // other windows carry on. That is the whole point
                    // of having a sensor that outlives its windows.
                    if let Err(erreur) = accueillir(tube) {
                        tracing::warn!(%erreur, "attache d'un enfant refusée");
                    }
                });
            }
            // Real connection failure (not the benign race isolated by
            // `connecter`): the refused pipe is closed so as not to leak, and the
            // loop recreates a new instance. Does not bring the
            // server down either.
            Err(erreur) => {
                if let Err(fermeture) = unsafe { CloseHandle(tube) } {
                    tracing::warn!(%fermeture, "fermeture d'un tube refusé également en échec");
                }
                tracing::warn!(%erreur, "connexion d'un enfant refusée");
            }
        }
    }
}

/// Reads the first frame and ROUTES on its type: an attach opens a
/// command connection, an identity pairs a media connection with an
/// already attached session.
fn accueillir(tube: HANDLE) -> Result<()> {
    // `std::fs::File` from the handle: it gives `Read`/`Write` without writing
    // a wrapper, and closing it closes the pipe.
    use std::os::windows::io::FromRawHandle;
    let fichier = unsafe { std::fs::File::from_raw_handle(tube.0 as *mut _) };
    // The reader works on a DUPLICATED handle: `fichier` stays whole and
    // can be handed as is to the window thread if it is a media
    // connection. Closing the duplicate does not close the pipe instance.
    let mut lecteur = BufReader::new(fichier.try_clone().context("clone du tube en lecture")?);

    let premiere = match lire_trame(&mut lecteur).context("première trame de l'enfant")? {
        Trame::Json(octets) => {
            serde_json::from_slice::<VersCapteur>(&octets).context("première trame illisible")?
        }
        Trame::Image(_) => bail!("le premier message d'un enfant ne peut pas être une image"),
    };

    match premiere {
        VersCapteur::Attache { .. } => ouvrir_les_commandes(lecteur, fichier, premiere),
        // The media connection: this thread only hands it over to the
        // window thread already waiting. No read will remain pending on it,
        // and the child will write nothing more on it — hence the `lecteur` we let
        // drop right after.
        VersCapteur::Identite { session } => {
            // `retirer_pour_identite` ignores the generation: see its
            // comment in `attentes.rs`.
            let attendue = attentes::retirer_pour_identite(&session);
            match attendue {
                Some(media) => {
                    drop(lecteur);
                    if media.send(fichier).is_err() {
                        tracing::warn!(
                            %session,
                            "fil de fenêtre disparu avant sa connexion média, connexion abandonnée"
                        );
                    }
                }
                // Never a panic, never silence: an orphan identity
                // names itself, and its connection closes on leaving here.
                None => tracing::warn!(
                    %session,
                    "connexion média pour une session inconnue, abandonnée"
                ),
            }
            Ok(())
        }
        autre => bail!("première trame inattendue d'un enfant : {autre:?}"),
    }
}

/// Handles a **command** connection: builds the window, replies to
/// the attach, then loops in strict alternation read → execute →
/// write. **This thread is the only one touching this connection.**
fn ouvrir_les_commandes(
    lecteur: BufReader<std::fs::File>,
    mut ecrivain: std::fs::File,
    attache: VersCapteur,
) -> Result<()> {
    let VersCapteur::Attache { ref session, .. } = attache else {
        bail!("le premier message d'un enfant doit être une attache");
    };
    let session = session.clone();

    let (media, attente_media) = channel::<std::fs::File>();
    // The insertion and its replacement log now live in
    // `attendre_le_media`, which returns the generation of THIS registration — the
    // value this thread must hand back as is to `oublier`.
    let generation = attendre_le_media(&session, media);

    let (commandes, receveur_commandes) = channel::<VersCapteur>();
    let (reponses, receveur_reponses) = channel::<DepuisCapteur>();

    // WINDOW thread: it holds the `WindowsSource`. DETACHED, never joined — the
    // accept loop must not be blockable by an encoder
    // teardown (`Drop for H264Encoder` can freeze).
    let session_fenetre = session.clone();
    std::thread::spawn(move || {
        if let Err(erreur) = tenir_la_fenetre(
            attache,
            session_fenetre,
            generation,
            attente_media,
            receveur_commandes,
            reponses,
        ) {
            // `cause::chaine`: `tenir_la_fenetre` propagates
            // contextualised errors from end to end of the wake-up path. See
            // `crate::cause`.
            tracing::warn!(
                erreur = %crate::cause::chaine(&erreur),
                "fil de fenêtre terminé sur erreur"
            );
        }
    });

    // **The attach reply goes out BEFORE any command read.** The child
    // reads it on this same connection, and it only opens its media connection
    // after reading it: entering the read loop directly would
    // leave it blocked forever. Bounded, so that a source construction
    // that did not return does not freeze this thread.
    let premiere = match receveur_reponses.recv_timeout(DELAI_REPONSE_FENETRE) {
        Ok(reponse) => reponse,
        Err(erreur) => {
            // The sender removes the entry it registered: if the window
            // thread stayed stuck in its source construction, it will never reach
            // its own wait, hence never its own removal.
            oublier(&session, generation);
            tracing::warn!(%session, %erreur, "aucune réponse à l'attache, canal abandonné");
            // An explicit REFUSAL rather than a silent close: the child
            // reads it and fails loudly, instead of interpreting an end of pipe.
            let _ = ecrire_json(
                &mut ecrivain,
                &DepuisCapteur::Refus {
                    motif: format!("aucune réponse du fil de fenêtre en {DELAI_REPONSE_FENETRE:?}"),
                },
            );
            let _ = ecrivain.flush();
            return Ok(());
        }
    };
    // `ecrivain` is a BARE `File`, without a write buffer: the acceptance run of
    // task 9 had seen an attach reply sleep in a `BufWriter`
    // until the first frame — which never comes in front of a still
    // window. The `flush` stays, but it is the absence of a buffer that guarantees it.
    let refusee = matches!(premiere, DepuisCapteur::Refus { .. });
    ecrire_json(&mut ecrivain, &premiere).context("réponse à l'attache")?;
    ecrivain.flush().context("réponse à l'attache")?;
    if refusee {
        // The refusal is written, the child will read it; nothing else will come on
        // this connection. Closing it on the way out is the normal end.
        return Ok(());
    }

    boucler_les_commandes(lecteur, ecrivain, &session, commandes, receveur_reponses);
    Ok(())
}

/// The window thread, from end to end: build the source, announce the
/// result to the command thread, wait for the media connection, serve.
fn tenir_la_fenetre(
    attache: VersCapteur,
    session: String,
    generation: u64,
    attente_media: Receiver<std::fs::File>,
    commandes: Receiver<VersCapteur>,
    reponses: Sender<DepuisCapteur>,
) -> Result<()> {
    let fenetre = match Fenetre::ouvrir(attache) {
        Ok(fenetre) => fenetre,
        Err(erreur) => {
            // The refusal is ANNOUNCED to the child, never silent: without this
            // message it would wait for a frame that will not come.
            let _ = reponses.send(DepuisCapteur::Refus {
                motif: format!("{erreur:#}"),
            });
            oublier(&session, generation);
            return Err(erreur);
        }
    };
    let (largeur, hauteur) = fenetre.dimensions();
    if reponses
        .send(DepuisCapteur::Attachee { largeur, hauteur })
        .is_err()
    {
        oublier(&session, generation);
        bail!("le fil de commandes de {session} est parti avant la réponse à l'attache");
    }

    // The child only opens its media connection AFTER reading `Attachee`: that is
    // what guarantees it knows its dimensions, and that is why this
    // wait comes here and not earlier.
    let media = match attente_media.recv_timeout(DELAI_CONNEXION_MEDIA) {
        Ok(media) => media,
        Err(RecvTimeoutError::Timeout) => {
            // The receiver removes ITS own entry: a child dying between its
            // two connections would otherwise leave an eternal entry.
            oublier(&session, generation);
            tracing::warn!(
                %session,
                delai = ?DELAI_CONNEXION_MEDIA,
                "aucune connexion média, fenêtre abandonnée"
            );
            return Ok(());
        }
        // The sender was replaced in the registry (re-attachment) or
        // dropped: the current entry no longer belongs to us, we must certainly
        // not remove it.
        Err(RecvTimeoutError::Disconnected) => {
            tracing::warn!(%session, "attente de connexion média rompue, fenêtre abandonnée");
            return Ok(());
        }
    };
    fenetre.servir(commandes, reponses, BufWriter::new(media))
}

/// The command connection loop: read a command, have it
/// executed by the window thread, write its reply. **Strict alternation on
/// a single thread**: never a pending read while writing.
fn boucler_les_commandes(
    mut lecteur: BufReader<std::fs::File>,
    mut ecrivain: std::fs::File,
    session: &str,
    commandes: Sender<VersCapteur>,
    reponses: Receiver<DepuisCapteur>,
) {
    loop {
        let message = match lire_trame(&mut lecteur) {
            Ok(Trame::Json(octets)) => match serde_json::from_slice::<VersCapteur>(&octets) {
                Ok(message) => message,
                Err(erreur) => {
                    tracing::warn!(%session, %erreur, "commande illisible, canal abandonné");
                    return;
                }
            },
            Ok(Trame::Image(_)) => {
                tracing::warn!(%session, "un enfant a envoyé une image, canal abandonné");
                return;
            }
            // End of pipe: the child has gone. Dropping `commandes`
            // signals `Disconnected` to the window thread, which tears down its source.
            Err(_) => return,
        };
        if commandes.send(message).is_err() {
            return; // the window thread has gone
        }
        // **Bounded, and it is the safety belt of the whole channel.**
        // The child waits for this reply without any timeout: without a bound here, a
        // silent window thread would freeze it forever. On expiry we
        // return an `Erreur` to it, which `SourceDistante` passes up and the transport
        // loop already absorbs through a `warn!` — then we close the connection.
        let reponse = match reponses.recv_timeout(DELAI_REPONSE_FENETRE) {
            Ok(reponse) => reponse,
            Err(RecvTimeoutError::Timeout) => {
                tracing::warn!(
                    %session,
                    delai = ?DELAI_REPONSE_FENETRE,
                    "le fil de fenêtre n'a pas répondu, canal clos"
                );
                let motif =
                    format!("le fil de fenêtre n'a pas répondu en {DELAI_REPONSE_FENETRE:?}");
                let _ = ecrire_json(&mut ecrivain, &DepuisCapteur::Erreur { motif });
                let _ = ecrivain.flush();
                return;
            }
            // The window thread has gone: the end of pipe will tell the child.
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if ecrire_json(&mut ecrivain, &reponse)
            .and_then(|()| ecrivain.flush())
            .is_err()
        {
            return;
        }
    }
}
