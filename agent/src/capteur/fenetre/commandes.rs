//! Serving the child's commands, and the media back-pressure.
//!
//! **Extracted from `fenetre.rs`** for the same reason as `sommeil.rs`: the
//! sub-block D5 would have taken the parent file beyond the project's 500-line
//! cap.
//!
//! All functions here take `source: Option<&mut WindowsSource>`:
//! `None` means "the window sleeps". **They serve the commands in
//! both cases** — refusing everything during sleep would push failures up
//! to the child's network adaptation, which would close the session through a
//! path foreign to sleep.

use std::sync::mpsc::{SyncSender, TryRecvError, TrySendError};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::source::VideoSource;
use crate::windows_source::WindowsSource;

use super::{AEcrire, Contexte, Fin, PAS_A_VIDE};

/// Drains the queue of pending commands and sends each reply back to the command
/// thread. **Never blocks**: `try_recv` on one side, an unbounded `Sender`
/// on the other.
pub(super) fn servir_les_commandes(source: Option<&mut WindowsSource>, ctx: &Contexte) -> Fin {
    let mut source = source;
    loop {
        match ctx.commandes.try_recv() {
            Ok(message) => {
                // `as_deref_mut` and not `source`: the borrow would be consumed
                // by the call, and the next round needs it.
                let reponse = executer_commande(source.as_deref_mut(), ctx, message);
                // The reply goes back through the channel, never through a direct
                // write: this thread touches no file object.
                if ctx.reponses.send(reponse).is_err() {
                    return Fin::Terminer("the command thread is gone");
                }
            }
            Err(TryRecvError::Empty) => return Fin::Continuer,
            // The child closed its command connection: the window is finished.
            Err(TryRecvError::Disconnected) => {
                return Fin::Terminer("the child closed the channel")
            }
        }
    }
}

/// Drops a payload for the media connection's writer thread.
///
/// ⚠️ **It is the ONLY point where the window thread can wait, and that is what
/// guarantees it can never wait without serving the commands.** The queue
/// is bounded so that back-pressure goes up to the capture; when
/// it is full, we do not block on it — we serve the commands, take a breath
/// for one step, and try again. A blocking `send` here would recreate exactly
/// the deadlock task 10 of sub-block D4 was meant to remove: child
/// frozen in `commander` → child's frame queue full → pipe buffer
/// full → sensor's write blocked → command never served → child frozen.
///
/// **Including for a state pushed by sleep**: a window falling asleep
/// while the queue is full waits here, serving its commands.
pub(super) fn deposer(
    charge: AEcrire,
    ecritures: &SyncSender<AEcrire>,
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
) -> Fin {
    let mut charge = charge;
    let mut source = source;
    loop {
        match ecritures.try_send(charge) {
            Ok(()) => return Fin::Continuer,
            Err(TrySendError::Full(rendue)) => {
                charge = rendue;
                if let Fin::Terminer(motif) = servir_les_commandes(source.as_deref_mut(), ctx) {
                    return Fin::Terminer(motif);
                }
                std::thread::sleep(PAS_A_VIDE);
            }
            // The writer thread is dead: the media connection is lost.
            Err(TrySendError::Disconnected(_)) => {
                return Fin::Terminer("the media connection is closed")
            }
        }
    }
}

fn executer_commande(
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
    message: VersCapteur,
) -> DepuisCapteur {
    // The four messages that do not touch the source are handled BEFORE
    // it, so that their reply is the same asleep and awake:
    // `Visibilite` and `AudioMort` because they COMMAND or feed
    // the sleep arbitration — ignoring them during sleep would forbid
    // any wake-up or any promotion of a neighbour —, the two others because
    // a protocol violation does not stop being one during
    // sleep.
    match message {
        VersCapteur::Visibilite { visible, focalisee } => {
            // The effect does NOT come back through this reply: arbitration is global
            // and may concern ANOTHER window than this one. It comes back through
            // `DepuisCapteur::Sommeil`, pushed on the media connection.
            crate::capteur::sommeil::signaler(ctx.session, visible, focalisee);
            return DepuisCapteur::Fait;
        }
        VersCapteur::AudioMort => {
            // The effect does NOT come back through this reply: arbitration is global
            // and may concern ANOTHER window of the same PID group. It
            // comes back through `DepuisCapteur::Audio`, pushed on the media
            // connection. Same pattern as `Visibilite` just above.
            crate::capteur::sommeil::audio_mort(ctx.session);
            return DepuisCapteur::Fait;
        }
        // Sub-block D10: the PROOF (a real packet) that the audio capture of
        // THIS session has restarted. Handled at the same rank as `AudioMort` —
        // before the source, not after — for the same reason: it touches
        // neither encoder nor duplication, only the sleep registry.
        // Unlike `AudioMort`, it re-arbitrates nothing and never comes
        // back through a push on the media connection — it only
        // resets this session's re-arm counter.
        VersCapteur::AudioVivant => {
            crate::capteur::sommeil::signaler_audio_vivant(ctx.session);
            return DepuisCapteur::Fait;
        }
        // Sub-block P2: the paste coming from the browser. Stage 0 like its
        // neighbours — it touches neither encoder nor duplication —, and handled
        // BEFORE the source for the same reason as them: ignoring it during
        // sleep would mean a sleeping window could no longer paste anything,
        // whereas the VM's clipboard is global and has nothing to do
        // with its encoder.
        //
        // 🔴 **It is the ONLY one of this family whose `Fait` carries the effect**,
        // and the only one that can return `Error`: the child waits for this
        // reply to know whether it must inject `Ctrl+V`. See the doc of the
        // variant, which carries D6's whole ordering.
        VersCapteur::ClipboardWrite { texte } => {
            return match crate::capteur::sommeil::write_clipboard(&texte) {
                Ok(()) => DepuisCapteur::Fait,
                Err(error) => DepuisCapteur::Error {
                    motif: format!("{error:#}"),
                },
            };
        }
        VersCapteur::Attache { .. } => {
            return DepuisCapteur::Error {
                motif: "second attach on an already attached channel".into(),
            }
        }
        // `Identite` belongs only to the media connection, where it is the
        // first and only frame: seeing it here signals a child confusing
        // its two connections.
        VersCapteur::Identite { session: autre } => {
            return DepuisCapteur::Error {
                motif: format!("identity of {autre} on the command connection"),
            }
        }
        _ => {}
    }

    // Everything else requires the source. Asleep, there is neither an encoder to tune nor a
    // duplication to resize: we reply as if it were done, rather than an
    // error that would go up to the child's network adaptation and be
    // logged there as a refusal — pointless noise.
    //
    // ⚠️ **What is accepted this way is neither applied nor kept.** Wake-up
    // rebuilds the source through `sur_sortie`, hence at the full encoding
    // size and at the attach bitrate.
    //
    // **Re-read at task 8 of sub-block D10: still true, and "full"
    // now designates the KEPT size** (`superviseur::placement::retained_size`),
    // not the raw size of the DXGI output — which may be larger on
    // a polluted registry. It is even more accurate than before: the "full
    // resolution" rebuilt on wake-up is the one the window
    // actually asked for, never the potentially inflated one of the
    // output. The bitrate catches up by itself —
    // `appliquer_decision` pushes it again at every controller decision; the
    // encoding size, on the other hand, only catches up at the next change of
    // rung, the comparison with `encode_size_appliquee` on the child side believing the
    // target already applied. Consequence: after a wake-up, a frame encoded at
    // full resolution at a reduced rung's bitrate, hence degraded — never a
    // bitrate overrun.
    //
    // **It is NOT fixed, and it is a decision — but its reason has changed.**
    // The technical obstacle has fallen: `set_encode_size` destroys the current
    // encoder before building a new one (`windows_source/encodage.rs`,
    // task 10 of D5), so it no longer exceeds the cap. What remains is a scope
    // trade-off, weaker: re-applying would cost an encoder construction at
    // the moment of wake-up — the one where the session most needs its first
    // frame — for a degradation that resolves itself at the next rung.
    // **To be reopened outside this sub-block**, its obstacle no longer existing.
    let Some(source) = source else {
        return match message {
            // The kept size, as is: a sleeping window no longer has
            // either capture or encoder, there is nothing to resize. A `Fait`
            // would make `SourceDistante::resize` fail, which expects a `Size`.
            //
            // ⚠️ **This comment has successively said two false things, and
            // it was the CROSS-CUTTING end-of-branch review of D9 that caught it —
            // no per-task review could.** It first claimed that
            // "`resize` has no effect anyway on a source in
            // `SortieEntiere` mode"; D8 refuted it by making the output follow the
            // viewport; the comment was therefore rewritten to announce, as an
            // accepted consequence, that a switch to fullscreen requested during
            // sleep would be **lost**. ❌ **That second wording has been
            // stale since task 3 of sub-block D9**, which removed the
            // tailored output mode change (see the finding at the head
            // of `capteur/plein_ecran.rs`): `resize` had again become, without
            // reservation, without effect in `SortieEntiere`, and there was therefore
            // NO fullscreen left to lose through this path.
            //
            // ❌ **THIS THIRD WORDING IS IN TURN FALSE SINCE
            // BATCH 33 — the FOURTH on this single comment, and the repository
            // warned that "the lifetime of a *this remains true* is one
            // sub-block".** `resize` is no longer without effect in
            // `SortieEntiere`: it makes the crop and the window follow the
            // viewport (`ModeCapture::suit_le_viewport`). **A
            // resize requested during SLEEP is therefore once again
            // LOST** — this arm returns the kept size as is, without
            // applying anything.
            //
            // 🔵 **And it is acceptable, for a reason that is not a
            // wish**: wake-up rebuilds the source through `sur_sortie` on
            // `self.dimensions()`, which `boucler` now keeps up to date (see
            // its point 3); and the client REPLAYS — `RejeuResize` re-emits any
            // size observed but not confirmed, and the browser's `ResizeObserver`
            // has not stopped observing during the VM's
            // sleep. The size lost here comes back with the next `Resize`.
            // **Not measured**: no acceptance run has exercised "resizing during
            // sleep", and it is said as such.
            //
            // What remains true, and why this arm exists: returning a
            // `Size` rather than a `Fait`, because `SourceDistante::resize`
            // expects a `Size`.
            VersCapteur::Redimensionner { .. } => DepuisCapteur::Size {
                largeur: ctx.size.0,
                hauteur: ctx.size.1,
            },
            _ => DepuisCapteur::Fait,
        };
    };

    let result = match message {
        VersCapteur::Redimensionner { largeur, hauteur } => {
            return match source.resize(largeur, hauteur) {
                Ok(()) => {
                    let (largeur, hauteur) = source.dimensions();
                    DepuisCapteur::Size { largeur, hauteur }
                }
                Err(error) => DepuisCapteur::Error {
                    motif: format!("{error:#}"),
                },
            }
        }
        VersCapteur::EncodeSize { largeur, hauteur } => source.set_encode_size(largeur, hauteur),
        VersCapteur::Debit { bps } => source.set_bitrate(bps),
        VersCapteur::ImageCle => source.request_keyframe(),
        // Handled above, before the source, hence never reached here. An
        // `Error` rather than an `unreachable!`: a panic on this thread
        // would take the window down for a drafting mistake.
        VersCapteur::Attache { .. }
        | VersCapteur::Identite { .. }
        | VersCapteur::Visibilite { .. }
        | VersCapteur::AudioMort
        | VersCapteur::AudioVivant
        | VersCapteur::ClipboardWrite { .. } => {
            return DepuisCapteur::Error {
                motif: "command already handled outside the source".into(),
            }
        }
    };
    match result {
        Ok(()) => DepuisCapteur::Fait,
        Err(error) => DepuisCapteur::Error {
            motif: format!("{error:#}"),
        },
    }
}
