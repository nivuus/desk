//! The **eight** callbacks ProjFS calls, and the only place in the bridge where
//! code runs on a thread the SYSTEM owns.
//!
//! **THREE of them are ASYNCHRONOUS** — `GetPlaceholderInfo`,
//! `GetFileData`, `GetDirectoryEnumeration` (plus `QueryFileName`, which borrows
//! the same request as the first): they register a command, push a
//! request, return `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` and return control.
//!
//! ⚠️ **Spec §8 announces "the FIVE mandatory callbacks in
//! asynchronous mode"; it is a wording gap, not a design one, and its own
//! table §4.3 says so**: `StartDirectoryEnumeration` and
//! `EndDirectoryEnumeration` appear there as "synchronous, `S_OK`", since they
//! never consult the browser. **Five are implemented, three are
//! asynchronous.**
//!
//! # What each callback is allowed to do, and nothing more
//!
//! See the threading discipline at the head of [`super`]. In short, a callback:
//!
//! - wraps **all** its body in [`std::panic::catch_unwind`] and returns
//!   `E_UNEXPECTED` — a Rust panic crossing an `extern "system"`
//!   boundary is a **process abort**, and that is exactly
//!   what decision D2 (the bridge in its own process) makes
//!   bearable rather than fatal;
//! - only writes into the locked states of [`super::Etat`], never during
//!   an I/O;
//! - **NEVER calls `PrjCompleteCommand`** — it is the bridge thread that
//!   completes;
//! - returns control **immediately**.

mod listage;
mod notification;

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_CANCEL_COMMAND_CB, PRJ_GET_FILE_DATA_CB, PRJ_GET_PLACEHOLDER_INFO_CB,
    PRJ_QUERY_FILE_NAME_CB,
};

use crate::pont::chemins;
use crate::pont::erreurs::{Erreur, EN_COURS};
use crate::pont::projfs::{ContexteProjFs, Etat, FluxDonnees};
use crate::pont::table::{Attendue, DELAI_ATTRIBUTS};
use proto::fichiers::entetes;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 THE ONLY ABI GUARD THIS REPOSITORY HAS, and it covers ONLY these eight
// functions — those we WRITE, a set DISJOINT from the thirteen
// we CALL (see the header of `chargement.rs`).
//
// ⚠️ **A `const _` and not a `#[test]`, and it is a DELIBERATE divergence
// from F1's plan** (task 12, step 4b), which writes this check as a
// `#[test]` claiming "it is therefore covered by `cargo check --target
// x86_64-pc-windows-gnu`". **That is wrong**: `cargo check` without `--tests` does not
// compile test code, and a `#[cfg(test)]` on a target never
// tested is compiled by NOTHING. The plan's check could not have
// failed. As a `const _`, it is checked by ordinary cross
// compilation, every time.
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_GET_PLACEHOLDER_INFO_CB = Some(info_marqueur);
const _: PRJ_GET_FILE_DATA_CB = Some(donnees_fichier);
const _: PRJ_QUERY_FILE_NAME_CB = Some(nom_fichier);
const _: PRJ_CANCEL_COMMAND_CB = Some(annulation);

/// Enveloppe commune : `catch_unwind`, et `E_UNEXPECTED` sur panique.
///
/// **The message names the callback.** Without it, the only trace of a panic
/// would be an `E_UNEXPECTED` returned to an application, that is, an I/O
/// error with no readable cause — the defect `pont::erreurs` exists not to
/// replay.
pub(super) fn garde(
    rappel: &'static str,
    corps: impl FnOnce() -> HRESULT + std::panic::UnwindSafe,
) -> HRESULT {
    match std::panic::catch_unwind(corps) {
        Ok(resultat) => resultat,
        Err(_) => {
            tracing::error!(
                rappel,
                "panique dans un rappel ProjFS : rattrapée avant la frontière FFI, \
                 E_UNEXPECTED rendu à l'application"
            );
            E_UNEXPECTED
        }
    }
}

/// Retrieves the bridge state from `InstanceContext`.
///
/// # Safety
///
/// The caller guarantees that `donnees` is the `PRJ_CALLBACK_DATA` ProjFS
/// has just provided, and that its `InstanceContext` is the pointer entrusted to
/// `PrjStartVirtualizing` — an `Arc<Etat>` that [`super::Virtualisation`] keeps
/// alive until after `PrjStopVirtualizing`. ProjFS guarantees that no callback
/// runs after that call returns, that is, before the `Arc` is
/// taken back.
pub(super) unsafe fn etat<'a>(donnees: *const PRJ_CALLBACK_DATA) -> Option<&'a Etat> {
    if donnees.is_null() {
        return None;
    }
    let contexte = unsafe { (*donnees).InstanceContext } as *const Etat;
    if contexte.is_null() {
        return None;
    }
    Some(unsafe { &*contexte })
}

/// The path delivered by ProjFS, in its TWO forms: the one the File System
/// Access API expects (logical, `/`-separated, **normalised and checked**), and
/// the one ProjFS will take back as is.
///
/// ⚠️ **Normalisation is not a comfort: it is the only barrier.** The
/// virtualisation root is traversed by any application of
/// the Windows session, including hostile ones — `..` climbs, NTFS alternate
/// streams, reserved device names. `pont::chemins` refuses them, and it is
/// PURE, hence exercised on the host.
pub(super) unsafe fn chemins_de(donnees: *const PRJ_CALLBACK_DATA) -> Option<(String, Vec<u16>)> {
    let brut = unsafe { donnees.as_ref() }?.FilePathName;
    if brut.is_null() {
        // The root itself: empty path on both sides.
        return Some((String::new(), vec![0u16]));
    }
    // SAFETY: ProjFS guarantees a null-terminated `PCWSTR`.
    let unites: Vec<u16> = unsafe { brut.as_wide() }.to_vec();
    let logique = match chemins::normaliser_utf16(&unites) {
        Ok(logique) => logique,
        Err(refus) => {
            tracing::warn!(?refus, "chemin ProjFS refusé par la normalisation");
            return None;
        }
    };
    let projfs = unites.into_iter().chain(std::iter::once(0)).collect();
    Some((logique, projfs))
}

/// The GUID of an enumeration, in the form [`crate::pont::table`] uses.
///
/// `[u8; 16]` and not `GUID`: the table is **pure** and does not know
/// `windows`. The conversion goes through `to_u128`, so it is total and
/// reversible — no interpretation of the GUID's fields is made here.
pub(super) unsafe fn identifiant(guid: *const GUID) -> Option<[u8; 16]> {
    if guid.is_null() {
        return None;
    }
    Some(unsafe { (*guid).to_u128() }.to_le_bytes())
}

/// Returns the metadata of an entry.
///
/// ❌ *Announced `ERROR_FILE_NOT_FOUND`: task 13's state, refuted by 14.*
unsafe extern "system" fn info_marqueur(donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("GetPlaceholderInfo", || {
        let Some(etat) = (unsafe { etat(donnees) }) else {
            return E_UNEXPECTED;
        };
        let Some((chemin, chemin_projfs)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(etat.compteurs.rendre(Erreur::CheminIntrouvable));
        };
        let entete = match serde_json::to_string(&entetes::Chemin {
            chemin: chemin.clone(),
        }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Attributs { chemin },
            std::time::Instant::now() + DELAI_ATTRIBUTS,
            ContexteProjFs::Attributs { chemin_projfs },
            proto::fichiers::TYPE_ATTRIBUTS,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(etat.compteurs.rendre(Erreur::CanalFerme))
        }
    })
}

/// Returns the content of a file.
///
/// ❌ *Announced `ERROR_FILE_NOT_FOUND`: task 13's state, refuted by 14.*
unsafe extern "system" fn donnees_fichier(
    donnees: *const PRJ_CALLBACK_DATA,
    position: u64,
    longueur: u32,
) -> HRESULT {
    garde("GetFileData", || {
        let Some(etat) = (unsafe { etat(donnees) }) else {
            return E_UNEXPECTED;
        };
        let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(etat.compteurs.rendre(Erreur::CheminIntrouvable));
        };
        // ⚠️ **The whole file NEVER enters memory**: the range is
        // split by `pont::decoupe`, PURE and tested.
        //
        // ✅ **AND THERE ARE NOW UP TO `MORCEAUX_EN_VOL` IN FLIGHT.** *(These
        // lines said "only one chunk is in flight at a time in F1. Flow
        // control through `bufferedAmount` is a deliverable of F3;
        // implementing it halfway here would be worse." F3 delivered it — and NOT
        // halfway: the bridge's window AND the browser's back-pressure are
        // both there, because with a single chunk in flight the rule of
        // spec §7.3 could NEVER bite.)*
        let morceaux: std::collections::VecDeque<_> = crate::pont::decoupe::decouper(
            position,
            u64::from(longueur),
            proto::fichiers::TAILLE_TRAME_MAX,
        )
        .into();
        let mut fenetre = crate::pont::lecture::Fenetre::nouvelle(morceaux);
        let lot = fenetre.a_demander();
        if lot.is_empty() {
            // Zero length: nothing to write, and nothing to request. Complete
            // right away rather than register a command that would
            // never get a response.
            return S_OK;
        }
        let flux = unsafe { (*donnees).DataStreamId };
        let commande = unsafe { (*donnees).CommandId };
        // 🔴 **A SINGLE WINDOW, SHARED BY THE *N* CORRELATIONS.** Cloning
        // it would make each response see its own copy and
        // request the same chunks again — the file would be written *N* times,
        // or truncated depending on the order.
        let fenetre = std::sync::Arc::new(std::sync::Mutex::new(fenetre));
        // ⚠️ **A FAILURE MID-BATCH LEAVES NOTHING IN FLIGHT**: `demander`
        // removes what it just registered when the transport is gone, and the
        // correlations already emitted will expire on their budget. We stop at the
        // first refusal rather than emit more towards a dead channel.
        let mut au_moins_une = false;
        for morceau in lot {
            let entete = match serde_json::to_string(&entetes::Lire {
                chemin: chemin.clone(),
                position: morceau.position,
                longueur: morceau.longueur,
            }) {
                Ok(entete) => entete,
                Err(_) => return E_UNEXPECTED,
            };
            let demandee = etat.demander(
                commande,
                Attendue::Lire {
                    chemin: chemin.clone(),
                    position: morceau.position,
                    longueur: morceau.longueur,
                },
                std::time::Instant::now() + crate::pont::table::DELAI_LIRE,
                ContexteProjFs::Lecture {
                    flux: FluxDonnees(flux),
                    fenetre: std::sync::Arc::clone(&fenetre),
                },
                proto::fichiers::TYPE_LIRE,
                &entete,
            );
            if !demandee {
                break;
            }
            au_moins_une = true;
        }
        if au_moins_une {
            HRESULT(EN_COURS)
        } else {
            HRESULT(etat.compteurs.rendre(Erreur::CanalFerme))
        }
    })
}

/// Says whether a name exists. Constantly consulted by Windows for paths
/// that do not exist (`desktop.ini`, `Thumbs.db`, application
/// manifests) — hence the negative cache armed at startup.
///
/// ❌ *Announced `ERROR_FILE_NOT_FOUND`: task 13's state, refuted by 14.*
unsafe extern "system" fn nom_fichier(donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("QueryFileName", || {
        let Some(etat) = (unsafe { etat(donnees) }) else {
            return E_UNEXPECTED;
        };
        let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(etat.compteurs.rendre(Erreur::CheminIntrouvable));
        };
        let entete = match serde_json::to_string(&entetes::Chemin {
            chemin: chemin.clone(),
        }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        // Same request as `GetPlaceholderInfo`: "does this name exist?" and
        // "what are its metadata?" have the same answer on the
        // browser side. ProjFS's negative cache
        // (`PRJ_FLAG_USE_NEGATIVE_PATH_CACHE`) is what prevents Windows's
        // constant probes — `desktop.ini`, `Thumbs.db`,
        // `folder.jpg`, application manifests — from each becoming a
        // browser round trip (spec §7.4).
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Attributs { chemin },
            std::time::Instant::now() + DELAI_ATTRIBUTS,
            ContexteProjFs::Existence,
            proto::fichiers::TYPE_ATTRIBUTS,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(etat.compteurs.rendre(Erreur::CanalFerme))
        }
    })
}

/// Removes a command abandoned by the application.
///
/// ⚠️ **Mandatory from F1.** `PRJ_CANCEL_COMMAND_CB` is only optional for
/// a SYNCHRONOUS provider; ours is not (spec §4.3). Without it, an
/// application abandoning its I/O would leave us an orphan command
/// in the table, and its late response would be applied to a buffer the
/// system has taken back.
unsafe extern "system" fn annulation(donnees: *const PRJ_CALLBACK_DATA) {
    // This callback returns NOTHING (`PRJ_CANCEL_COMMAND_CB`), so `garde` — which returns
    // an `HRESULT` — does not apply as is. The `catch_unwind` is written by
    // hand: it is the same requirement, and forgetting it here would be exactly
    // as fatal.
    let issue = std::panic::catch_unwind(|| {
        let Some(etat) = (unsafe { etat(donnees) }) else {
            return;
        };
        let commande = unsafe { (*donnees).CommandId };
        // 🔴 **ALL THE CORRELATIONS, and it is F3's read window
        // that requires it**: a read can have up to
        // `pont::lecture::MORCEAUX_EN_VOL` in flight. Letting one survive
        // would make `PrjCompleteCommand` be called on an ALREADY completed command,
        // when its budget expires — a system call on an
        // identifier that belongs to someone else.
        let correlations = etat
            .table
            .lock()
            .expect("verrou de la table")
            .annuler(commande);
        for correlation in &correlations {
            // ⚠️ **The ProjFS context leaves WITH the table entry, otherwise it
            // leaks.** `Table::annuler` only knows the table — it is PURE
            // — and the enumeration buffer or the data stream of a cancelled
            // command would otherwise stay in `en_attente` for the whole life of the
            // bridge, with nothing ever reading it.
            if let Ok(mut attente) = etat.en_attente.lock() {
                attente.remove(correlation);
            }
        }
        if !correlations.is_empty() {
            tracing::debug!(
                commande,
                en_vol = correlations.len(),
                "commande ProjFS annulée par l'application"
            );
        }
    });
    if issue.is_err() {
        tracing::error!(
            rappel = "CancelCommand",
            "panique dans un rappel ProjFS : rattrapée avant la frontière FFI"
        );
    }
}

/// The block of the eight callbacks, as `PrjStartVirtualizing` expects it.
pub(super) fn bloc() -> windows::Win32::Storage::ProjectedFileSystem::PRJ_CALLBACKS {
    windows::Win32::Storage::ProjectedFileSystem::PRJ_CALLBACKS {
        StartDirectoryEnumerationCallback: Some(listage::debut_enumeration),
        EndDirectoryEnumerationCallback: Some(listage::fin_enumeration),
        GetDirectoryEnumerationCallback: Some(listage::suite_enumeration),
        GetPlaceholderInfoCallback: Some(info_marqueur),
        GetFileDataCallback: Some(donnees_fichier),
        QueryFileNameCallback: Some(nom_fichier),
        NotificationCallback: Some(notification::notification),
        CancelCommandCallback: Some(annulation),
    }
}
