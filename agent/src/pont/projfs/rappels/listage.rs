//! The THREE directory enumeration callbacks, extracted from [`super`].
//!
//! # Why this file is called `listage` and not `enumeration`
//!
//! Spec §9 writes: "if it approaches 500, **the enumeration callbacks move
//! to `projfs/enumeration.rs`**, never by compression". **Two things have
//! changed since**:
//!
//! 1. the callbacks no longer live in `projfs.rs` but in
//!    `projfs/rappels.rs` — extracted by F1's task 13 —, so that the
//!    landing point is `projfs/rappels/…`;
//! 2. **`agent/src/pont/enumeration.rs` ALREADY EXISTS**, and it is **PURE** (the
//!    enumeration session, 140 lines). A `projfs/rappels/enumeration.rs`
//!    would create two homonymous modules, one pure and the other
//!    `#[cfg(windows)]` — the kind of homonymy one only notices when
//!    rereading a cross-reference, six months later.
//!
//! Hence **`listage`**. ⚠️ *Sub-block F3 names the same file in its plan,
//! under this same name, precisely so that there is never a third
//! split of `rappels.rs`.*
//!
//! # What this extraction is NOT
//!
//! **It adds no behaviour.** The transposition is VERBATIM, and
//! it was done by cutting the file by LINE NUMBERS rather than by
//! copying by hand. It comes **BEFORE** F2's addition and not after:
//! `rappels.rs` was at **488** lines, margin **12**, and the notification
//! callback is exactly what F2 makes grow. It is the gesture D9
//! invented (`capteur/serveur/instances.rs`, margin returned from 10 to 65) and that
//! D10 played three times — **never a compression**, which `CLAUDE.md`
//! forbids by name.

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_DIR_ENTRY_BUFFER_HANDLE, PRJ_END_DIRECTORY_ENUMERATION_CB,
    PRJ_GET_DIRECTORY_ENUMERATION_CB, PRJ_START_DIRECTORY_ENUMERATION_CB,
};

use super::{chemins_de, etat, garde, identifiant};
use crate::pont::errors::{Error, EN_COURS};
use crate::pont::projfs::{ContexteProjFs, TamponEntrees};
use crate::pont::table::{Attendue, DELAI_LISTER};
use proto::files::entetes;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 THE THREE `const _`s LEAVE WITH THEIR FUNCTIONS, and it is not
// tidying: it is this repository's ONLY ABI guard (see the header of
// [`super`]). Leaving it behind would make it a declaration pointing
// elsewhere. D9 set the rule when extracting `capteur/serveur/instances.rs` —
// *the comment leaves with its constant*, and the review compared word for word.
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_START_DIRECTORY_ENUMERATION_CB = Some(debut_enumeration);
const _: PRJ_END_DIRECTORY_ENUMERATION_CB = Some(fin_enumeration);
const _: PRJ_GET_DIRECTORY_ENUMERATION_CB = Some(suite_enumeration);

/// Opens an enumeration session. **Synchronous, `S_OK`** — there is nothing to
/// ask the browser to open a session (spec §4.3).
pub(super) unsafe extern "system" fn debut_enumeration(
    data: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
) -> HRESULT {
    garde("StartDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(data) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        // ⚠️ The session is indexed by the ENUMERATION GUID, never by the
        // path: two applications listing the same directory at the same
        // time open two distinct sessions, and indexing by path would make
        // the second overwrite the first — one of the two would receive an
        // empty directory (spec §7.2).
        etat.sessions
            .lock()
            .expect("verrou des sessions")
            .entry(id)
            .or_default();
        S_OK
    })
}

/// Closes an enumeration session. **Synchronous, `S_OK`.**
pub(super) unsafe extern "system" fn fin_enumeration(
    data: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
) -> HRESULT {
    garde("EndDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(data) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        // The session dies here: its entries do NOT outlive the enumeration.
        // It is what distinguishes a session from an enumeration cache, which is
        // NOT delivered in F1 (see `pont::enumeration`).
        etat.sessions
            .lock()
            .expect("verrou des sessions")
            .remove(&id);
        S_OK
    })
}

/// Returns the entries of a directory.
///
/// ❌ *Announced "`S_OK`, empty buffer, empty root": task 13's state.
/// Task 14 of the SAME branch refuted it — `Lister` really goes out.*
pub(super) unsafe extern "system" fn suite_enumeration(
    data: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
    expression: windows::core::PCWSTR,
    tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    garde("GetDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(data) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        let Some((chemin, _)) = (unsafe { chemins_de(data) }) else {
            return HRESULT(etat.compteurs.rendre(Error::CheminIntrouvable));
        };
        let motif = if expression.is_null() {
            None
        } else {
            // SAFETY: ProjFS guarantees a null-terminated `PCWSTR`.
            unsafe { expression.to_string() }.ok()
        };
        // ⚠️ `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (`mod.rs:177`, value `1i32`)
        // **must be honoured**: it restarts the enumeration in progress. Ignoring it
        // would return an empty directory to any application that asks again
        // from the start, silently.
        let redemarrer = unsafe { (*data).Flags }.0 & 1 != 0;

        let mut sessions = match etat.sessions.lock() {
            Ok(sessions) => sessions,
            Err(_) => return E_UNEXPECTED,
        };
        let session = sessions.entry(id).or_default();
        if redemarrer {
            session.redemarrer();
        }
        if session.chargee() {
            // ⚠️ **SYNCHRONOUS path, and it is the nominal case.** ProjFS calls
            // `GetDirectoryEnumeration` again until exhaustion; only the FIRST
            // call of a session consults the browser. Going through the table again
            // at each turn would make a network round trip per full buffer,
            // for a list we already have.
            return crate::pont::service::remplir_session(etat, session, tampon);
        }
        drop(sessions);

        // ────────────────────────────────────────────────────────────────────
        // 🔴 **F5 — THE ENUMERATION CACHE IS CONSULTED HERE, AND NOWHERE
        // ELSE.**
        //
        // The point is chosen: **after** the synchronous path of the loaded
        // session (which has nothing to request), **before** registering a
        // command in the table. On a hit, the command is **never
        // registered**, hence **never completed** — and it is this detail that
        // preserves the invariant spec §7.1 states: *"A single bridge thread
        // completes commands, never a callback thread."*
        //
        // **Filling a buffer and returning `S_OK` from this thread is NOT
        // completing a command**: it is never creating one. Calling
        // `PrjCompleteCommand` from here, on the other hand, would break the invariant — and
        // it is the gesture not to make.
        //
        // ⚠️ **The memorised entries are RAW**, and `preparer` runs
        // anyway: filtering by `expression` and sorting depend on the
        // REQUEST, so that a `dir *.txt` would poison the cache for the
        // next `dir` if the prepared result were memorised.
        // ────────────────────────────────────────────────────────────────────
        if etat.cache_arme {
            let memorisees = etat.cache.lock().ok().and_then(|mut c| {
                c.lire(&chemin, std::time::Instant::now())
                    .map(<[_]>::to_vec)
            });
            if let Some(brutes) = memorisees {
                let preparees = crate::pont::enumeration::preparer(
                    brutes,
                    motif.as_deref(),
                    |nom, m| etat.apparier(nom, m),
                    |a, b| etat.comparer(a, b),
                );
                let mut sessions = match etat.sessions.lock() {
                    Ok(sessions) => sessions,
                    Err(_) => return E_UNEXPECTED,
                };
                let session = sessions.entry(id).or_default();
                session.poser(preparees);
                return crate::pont::service::remplir_session(etat, session, tampon);
            }
        }

        let entete = match serde_json::to_string(&entetes::Chemin {
            chemin: chemin.clone(),
        }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        let demandee = etat.demander(
            unsafe { (*data).CommandId },
            Attendue::Lister {
                chemin,
                enumeration: id,
            },
            std::time::Instant::now() + DELAI_LISTER,
            ContexteProjFs::Enumeration {
                tampon: TamponEntrees(tampon),
                expression: motif,
            },
            proto::files::TYPE_LISTER,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(etat.compteurs.rendre(Error::CanalFerme))
        }
    })
}
