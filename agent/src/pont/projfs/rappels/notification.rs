//! The NOTIFICATION callback, extracted from [`super`] **before** F2
//! made it grow, and not after.
//!
//! `rappels.rs` was at **488** lines for a margin of **12**, and this callback
//! is exactly what F2 weighs down: it must read `isdirectory`, route
//! **five** more notifications, and push an event towards the write
//! thread. The extraction therefore comes first — a gesture invented by D9
//! (`capteur/serveur/instances.rs`) and replayed three times by D10 —, **never
//! a compression**, which `CLAUDE.md` forbids by name.
//!
//! # What this extraction is NOT
//!
//! **It adds no behaviour.** The transposition is VERBATIM. What
//! made it grow comes from task 12, in a separate commit, so that the
//! review can compare one with the other.
//!
//! # ✅ F3 HAS ARRIVED, AND IT READS THE TWO PARAMETERS F2 IGNORED
//!
//! `_est_repertoire` and `_destination` were prefixed with an underscore because
//! F2 refused renaming and deletion. **Both are now read** —
//! one is carried as is in the header, the other normalised by
//! `pont::chemins`. `_params`, on the other hand, **stays `_params`**: see
//! below, it is still a union.

use windows::core::HRESULT;
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_NOTIFICATION, PRJ_NOTIFICATION_CB, PRJ_NOTIFICATION_PARAMETERS,
};

use super::{chemins_de, etat, garde};
use crate::pont::ecriture::{fil::Ordre, Evenement};
use crate::pont::notifications;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 THE `const _` LEAVES WITH ITS FUNCTION — this repository's only ABI guard,
// checked by ORDINARY cross compilation and not by a `#[test]` (see
// the header of [`super`], which explains why a `#[cfg(test)]` on a target
// never tested is compiled by NOTHING).
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_NOTIFICATION_CB = Some(notification);

/// **The write decision, and the only F2 callback that changes what an
/// application gets.**
///
/// ⚠️ **It DECIDES nothing itself**: the decision lives in
/// [`crate::pont::notifications`], which is **PURE** and exercised on the host. This
/// callback translates, it does not arbitrate.
///
/// # 🔴 What it NEVER does, and why
///
/// **No file read, no lock held, no waiting.** It runs
/// on a thread the SYSTEM owns: opening the hydrated file there would do an
/// I/O on that thread, and the read would cross the root — hence our own
/// callbacks. All it does is **push an event on an `mpsc` and
/// return `S_OK` immediately**; it is the dedicated write thread that reads.
///
/// # ⚠️ `PRJ_NOTIFICATION_PARAMETERS` IS NOT DEREFERENCED, AND IT IS DELIBERATE
///
/// It is a **UNION** (`mod.rs:352-356`, members described at `mod.rs:364-376`),
/// and **reading the wrong member is undefined behaviour**. F2 needs
/// none of the three: `PostCreate.NotificationMask` and
/// `FileRenamed.NotificationMask` serve to **change the mask** for that
/// file, which F2 does not do, and `FileDeletedOnHandleClose.IsFileModified`
/// concerns deletion, which is **F3**. The parameter therefore stays
/// `_params` — **not reading it at all is the only safe way**, and saying so
/// prevents a successor from seeing an oversight in it.
///
/// ✅ **`destination` IS READ SINCE F3, and it is NOT a member of the union.**
/// It is a **DIRECT parameter** of the callback (`mod.rs:334`,
/// `destinationfilename: PCWSTR`). Saying so prevents a successor from looking
/// for it in `PRJ_NOTIFICATION_PARAMETERS.FileRenamed`, which only carries a
/// notification mask.
///
/// ⚠️ **It only carries a name for `PRE_RENAME` and `FILE_RENAMED`.** For
/// the mask's seven other notifications, it is empty or null — and that is
/// why [`destination_de`] returns a [`notifications::Cible`] rather than a
/// path: "there is no destination" is a legitimate state, distinct from
/// "the destination is unacceptable".
pub(super) unsafe extern "system" fn notification(
    data: *const PRJ_CALLBACK_DATA,
    est_repertoire: bool,
    notification: PRJ_NOTIFICATION,
    destination: windows::core::PCWSTR,
    _params: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT {
    garde("Notification", || {
        let Some(etat) = (unsafe { etat(data) }) else {
            return E_UNEXPECTED;
        };
        // SAFETY: `destination` is a `PCWSTR` ProjFS provided; it is
        // either null, or null-terminated.
        let vers = unsafe { destination_de(destination) };
        let cible = match &vers {
            None => notifications::Cible::SansObjet,
            Some(Ok(_)) => notifications::Cible::InRoot,
            Some(Err(())) => notifications::Cible::HorsRacine,
        };
        // 🔵 **THE TRACE PROBE S1 READS, AND IT IS A `debug!` ON PURPOSE.**
        //
        // It carries the callback's FOUR raw fields — the code, `isdirectory`,
        // the path, the destination —, that is, exactly what S1
        // needs to answer its three questions **without a single byte
        // going to the local workstation**.
        //
        // ⚠️ **`debug!` and not `info!`, unlike the census of codes**
        // : this callback runs on a thread the system owns, at each
        // notification. It is NOT the bridge's hottest path — reads
        // produce none, `FILE_HANDLE_CLOSED_NO_MODIFICATION`
        // being deliberately not requested —, but one `info!` line per
        // file creation would flood an operations log for a
        // bench need.
        //
        // ⚠️ **It is read with a TARGETED filter**, never a global
        // `RUST_LOG=debug`: that would make one line per chunk read, which is the
        // "never trace per packet" trap of the TURN work item. The narrowest filter
        // that yields it is
        // `agent::pont::projfs::rappels::notification=debug`. ⚠️ **F3's acceptance
        // run used `RUST_LOG=info,agent::pont=debug`**, which is wider:
        // measured over the eight recorded runs, it produces **no** line
        // per chunk read — the read path does not call `debug!`. The
        // largest record is 3,100 lines for a 90 s session.
        tracing::debug!(
            code = notification.0,
            est_repertoire,
            chemin = %chemin_brut(data),
            destination = ?vers,
            ?cible,
            "notification ProjFS"
        );
        match notifications::decider(notification.0, etat.etat_de_notification(), cible) {
            notifications::Reponse::Refuser(cause) => HRESULT(etat.compteurs.rendre(cause)),
            // 🔵 Writing is allowed. **There is nothing more to do
            // here**: the bytes only concern us when the
            // handle closes, through a POST.
            notifications::Reponse::Autoriser => S_OK,
            notifications::Reponse::Pousser(quoi) => {
                // ⚠️ **`pont::chemins`' normalisation stays the ONLY
                // barrier** against `..` climbs, NTFS alternate
                // streams (`:`) and reserved device names. It is
                // PURE, hence exercised on the host.
                let Some((chemin, _)) = (unsafe { chemins_de(data) }) else {
                    // A path refused by normalisation: we push
                    // NOTHING, and `chemins_de` has already logged the refusal. Returning
                    // S_OK is the only choice — the notification is a POST,
                    // and refusing would prevent nothing.
                    return S_OK;
                };
                let evenement = match quoi {
                    notifications::Poussee::Creation => Evenement::Cree {
                        chemin,
                        repertoire: est_repertoire,
                    },
                    notifications::Poussee::Contenu => Evenement::Modified { chemin },
                    // ── F3'S TWO PUSHES ───────────────────────────────────
                    notifications::Poussee::Renommage => {
                        // 🔴 **TWO INVARIANTS THAT REFUSE RATHER THAN
                        // GUESS, and it is the safeguard against F3's most serious
                        // risk (R-F3-1).** Getting the DIRECTION wrong would produce
                        // no error: the renaming would happen, backwards,
                        // and the destination would overwrite the source.
                        //
                        // ⚠️ **This safeguard DEPENDS ON NO MEASUREMENT.** Probe
                        // S1 records from evidence which ProjFS field carries
                        // what; this one holds even if the probe has never been
                        // played.
                        let Some(Ok(vers)) = vers else {
                            tracing::warn!(
                                de = %chemin,
                                destination_lisible = vers.is_some(),
                                "renommage sans destination utilisable : RIEN n'est pousse"
                            );
                            return S_OK;
                        };
                        if vers.is_empty() || vers == chemin {
                            tracing::warn!(
                                de = %chemin,
                                vers = %vers,
                                "renommage dont la destination est vide ou egale a la source : \
                                 RIEN n'est pousse"
                            );
                            return S_OK;
                        }
                        Evenement::Renomme {
                            de: chemin,
                            vers,
                            repertoire: est_repertoire,
                        }
                    }
                    notifications::Poussee::Suppression => Evenement::Deleted {
                        chemin,
                        repertoire: est_repertoire,
                    },
                };
                // ────────────────────────────────────────────────────────
                // 🔴 **F5 — FIRST HALF OF INVALIDATION: what the VM
                // changed.** The directory containing the mutated entry stops
                // being served from memory, otherwise a creation made
                // IN the VM would stay invisible at the next listing — the exact
                // defect spec §7.4 reproaches the old bridge for.
                //
                // ⚠️ **A RENAMING INVALIDATES BOTH PARENTS**, source and
                // destination: `a/x` → `b/y` removes an entry from `a` and
                // adds one to `b`. Invalidating only one would let the other
                // lie, and the direction of the error would depend on which — hence
                // would be irregular, hence harder to see.
                //
                // ⚠️ **WHAT I DO NOT KNOW, AND DO NOT CLAIM
                // TO KNOW**: does the ProjFS filter merge local entries
                // itself with what the provider enumerates, or does it
                // call us back?
                //
                // ✅ **GATE P3 WAS PLAYED (2 runs), AND THIS ARM
                // IS REACHED.** *These lines said "it was not played,
                // the VM was off": it was true at the time I wrote
                // them, and the VM was handed back an hour later.* The
                // log carries a `code=4` ProjFS notification for the file
                // created outside the browser, and **the relisting that
                // follows costs 49 ms — a round trip, not the 7-8 ms of a
                // cache hit**. The directory's memory had therefore indeed
                // been forgotten HERE.
                //
                // ⚠️ **WHAT THIS DOES NOT SEPARATE**: whether the filter *also*
                // merges by itself. Our invalidation precedes it, and nothing
                // in this setup says what would have happened without it. The
                // two halves stay in place, because one is
                // **indispensable** if the filter calls us back — which it does —
                // and **harmless** if it merges: the cost of being wrong
                // is not symmetric.
                // ────────────────────────────────────────────────────────
                if etat.cache_arme {
                    if let Ok(mut cache) = etat.cache.lock() {
                        match &evenement {
                            Evenement::Renomme { de, vers, .. } => {
                                cache.invalider(de);
                                cache.invalider(vers);
                            }
                            autre => cache.invalider(autre.chemin()),
                        }
                    }
                }
                if etat.vers_ecriture.send(Ordre::Survenu(evenement)).is_err() {
                    // 🔴 **The write thread is gone, and the application has ALREADY
                    // saved.** Nothing more can be said to it: it is
                    // the absence of back-pressure the header of
                    // `pont::notifications` describes. The `warn!` is all that
                    // remains.
                    tracing::warn!(
                        "fil d'ecriture du pont parti : une ecriture ne sera JAMAIS poussee"
                    );
                }
                S_OK
            }
            // A notification the mask should not have delivered. Accepting
            // SILENTLY would let a mask widened by mistake go
            // unnoticed.
            notifications::Reponse::AccepterSansAttendre => {
                tracing::warn!(
                    code = notification.0,
                    "notification ProjFS non attendue par le masque de F2 : acceptee sans effet"
                );
                S_OK
            }
        }
    })
}

/// The destination of a notification, normalised.
///
/// Three outcomes, and **the distinction between the last two is what makes
/// [`notifications::Cible`] more honest than a `bool`**:
///
/// - `None` — the parameter is null or empty: **there is no destination**,
///   which is the case of the mask's seven notifications other than
///   `PRE_RENAME` and `FILE_RENAMED`;
/// - `Some(Ok(chemin))` — an acceptable destination, normalised into a logical
///   path;
/// - `Some(Err(()))` — a destination `pont::chemins` refuses: `..`
///   climb, NTFS alternate stream, reserved device name, absolute path.
///   **It is also what we get from a target outside the root**, ProjFS only
///   delivering paths relative to it.
///
/// # Safety
///
/// The caller guarantees that `brut` is the `destinationfilename` ProjFS has just
/// provided: null, or null-terminated.
unsafe fn destination_de(brut: windows::core::PCWSTR) -> Option<Result<String, ()>> {
    if brut.is_null() {
        return None;
    }
    // SAFETY: the caller's guarantee.
    let unites = unsafe { brut.as_wide() };
    if unites.is_empty() {
        return None;
    }
    match crate::pont::chemins::normaliser_utf16(unites) {
        Ok(logique) => Some(Ok(logique)),
        Err(refus) => {
            tracing::warn!(
                ?refus,
                "destination de renommage refusee par la normalisation"
            );
            Some(Err(()))
        }
    }
}

/// The callback's `FilePathName`, **as is**, for probe S1's trace.
///
/// ⚠️ **It is NOT the normalised path**: the probe needs to see what
/// ProjFS delivered, including what `pont::chemins` would refuse. A path
/// refused by normalisation produces no trace elsewhere, and S1 must
/// be able to observe that it arrived.
///
/// # Safety
///
/// The caller guarantees that `data` is the `PRJ_CALLBACK_DATA` ProjFS
/// has just provided.
unsafe fn chemin_brut(data: *const PRJ_CALLBACK_DATA) -> String {
    let Some(brut) = (unsafe { data.as_ref() }) else {
        return String::new();
    };
    if brut.FilePathName.is_null() {
        return String::new();
    }
    // SAFETY: ProjFS guarantees a null-terminated `PCWSTR`.
    String::from_utf16_lossy(unsafe { brut.FilePathName.as_wide() })
}
