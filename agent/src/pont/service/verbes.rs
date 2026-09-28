//! The ProjFS calls the bridge thread emits: complete, write a marker,
//! write data, fill an entry buffer.
//!
//! Extracted from [`super`] for the same reason `projfs/racine.rs` was from
//! `projfs.rs`: **before** the addition made the extraction necessary, and
//! not after. This file carries the `unsafe`s, [`super`] carries the loop.

use windows::core::{GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS, PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0,
    PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0_1, PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION,
    PRJ_DIR_ENTRY_BUFFER_HANDLE, PRJ_FILE_BASIC_INFO, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    PRJ_PLACEHOLDER_INFO,
};

use crate::pont::entetes::filetime_depuis_ms;
use crate::pont::enumeration::{Entree, Session};
use crate::pont::projfs::{chargement::ProjFs, Contexte, Etat};

/// `FILE_ATTRIBUTE_DIRECTORY` / `FILE_ATTRIBUTE_NORMAL`.
///
/// ⚠️ **`NORMAL` (or `DIRECTORY`) for EVERYTHING, and it is declared**: the File
/// System Access API exposes no attribute, there is **nothing to carry**
/// (spec §3.5.2). It is not an approximation for lack of better, it is
/// the absence of a source.
const ATTRIBUT_REPERTOIRE: u32 = 0x0000_0010; // Storage/FileSystem/mod.rs, FILE_ATTRIBUTE_DIRECTORY
const ATTRIBUT_NORMAL: u32 = 0x0000_0080; // Storage/FileSystem/mod.rs, FILE_ATTRIBUTE_NORMAL

/// `HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER)` — what
/// `PrjFillDirEntryBuffer` returns when the buffer is full. **It is not an
/// error**: it is the signal to stop there and complete, the rest going
/// at the next `GetDirectoryEnumeration`.
const TAMPON_PLEIN: HRESULT = HRESULT(0x8007_007Au32 as i32); // Foundation/mod.rs:2813, value 122

/// An aligned buffer, released by its `Drop`.
///
/// ⚠️ **`PrjAllocateAlignedBuffer` / `PrjFreeAlignedBuffer` are the first
/// source of memory leaks in a ProjFS provider** (spec §4.3). The pair is
/// therefore encapsulated here, **never called by hand** — and it is released by `Drop`,
/// including when `PrjWriteFileData` fails, which a `free` written after
/// the call would not do.
struct TamponAligne<'a> {
    pointeur: *mut core::ffi::c_void,
    projfs: &'a ProjFs,
}

impl<'a> TamponAligne<'a> {
    /// `None` if the allocation fails — `PrjAllocateAlignedBuffer` returns a
    /// POINTER, and `NULL` is the failure (transcription trap no. 3).
    fn allouer(
        projfs: &'a ProjFs,
        contexte: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        size: usize,
    ) -> Option<Self> {
        // SAFETY: valid context, non-zero size. Transcription of the `link!`
        // at `mod.rs:3`.
        let pointeur = unsafe { (projfs.allouer_tampon_aligne)(contexte, size) };
        if pointeur.is_null() {
            return None;
        }
        Some(Self { pointeur, projfs })
    }
}

impl Drop for TamponAligne<'_> {
    fn drop(&mut self) {
        // SAFETY: `pointeur` comes from `PrjAllocateAlignedBuffer` and has been
        // handed to no one. `PrjFreeAlignedBuffer` returns NOTHING (`mod.rs:68`).
        unsafe { (self.projfs.rendre_tampon_aligne)(self.pointeur) };
    }
}

/// Completes a ProjFS command, without extended parameters.
///
/// 🔴 **AN ABSENT `command_id` IS NOT AN ERROR: it is a WRITE.**
/// It arises from a POST notification, which has already returned control to
/// the application — there is therefore **no callback to complete**. Calling
/// `PrjCompleteCommand(0)` would complete a command belonging to
/// someone else.
///
/// ⚠️ **The case is logged at `debug!`, never kept quiet.** Silence would make an
/// unexpected `None` — coming from a read whose identifier was lost —
/// indistinguishable from the nominal case.
pub(super) fn completer(etat: &Etat, commande: Option<i32>, result: HRESULT) {
    let Some(commande) = commande else {
        tracing::debug!(%result, "aucune commande ProjFS a completer : c'est une ecriture");
        return;
    };
    let Some(Contexte(contexte)) = etat.contexte() else {
        tracing::warn!(
            commande,
            "complétion impossible : aucun contexte de virtualisation"
        );
        return;
    };
    // SAFETY: context valid as long as virtualisation runs — the bridge
    // thread stops before `Virtualisation`'s `Drop`. The null fourth
    // parameter means "no extended parameter" (`mod.rs:14`).
    let issue =
        unsafe { (etat.projfs.completer_commande)(contexte, commande, result, std::ptr::null()) };
    if issue.is_err() {
        tracing::warn!(commande, %issue, %result, "PrjCompleteCommand refusée");
    }
}

/// Completes an **enumeration**, which requires extended parameters.
///
/// ⚠️ An enumeration completed without `PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION` and
/// without its `DirEntryBufferHandle` would return an empty directory (spec §4.3):
/// ProjFS would not know which buffer to reread.
pub(super) fn completer_enumeration(
    etat: &Etat,
    commande: i32,
    tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    result: HRESULT,
) {
    let Some(Contexte(contexte)) = etat.contexte() else {
        tracing::warn!(
            commande,
            "complétion d'énumération impossible : aucun contexte"
        );
        return;
    };
    let params = PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS {
        CommandType: PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION,
        Anonymous: PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0 {
            Enumeration: PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0_1 {
                DirEntryBufferHandle: tampon,
            },
        },
    };
    // SAFETY: `params` lives until the end of the expression, hence beyond
    // the call.
    let issue = unsafe { (etat.projfs.completer_commande)(contexte, commande, result, &params) };
    if issue.is_err() {
        tracing::warn!(commande, %issue, "PrjCompleteCommand (énumération) refusée");
    }
}

/// The basic information block of an entry.
fn info_de_base(repertoire: bool, size: u64, modified_ms: i64) -> PRJ_FILE_BASIC_INFO {
    let horodatage = filetime_depuis_ms(modified_ms);
    PRJ_FILE_BASIC_INFO {
        IsDirectory: repertoire,
        // `i64` on the ProjFS side, `u64` on the protocol side: a size above
        // 8 EiB does not exist, but saturating it beats a negative, which
        // ProjFS would read as an absurd size.
        FileSize: size.min(i64::MAX as u64) as i64,
        // ⚠️ **The four fields carry the SAME timestamp, and it is
        // declared**: the File System Access API only exposes
        // `File.lastModified` (spec §3.5.2). Inventing a distinct creation
        // date would be fabricated data.
        CreationTime: horodatage,
        LastAccessTime: horodatage,
        LastWriteTime: horodatage,
        ChangeTime: horodatage,
        FileAttributes: if repertoire {
            ATTRIBUT_REPERTOIRE
        } else {
            ATTRIBUT_NORMAL
        },
    }
}

/// Writes an entry's marker — the response to `GetPlaceholderInfo`.
pub(super) fn write_placeholder(
    etat: &Etat,
    chemin: &[u16],
    repertoire: bool,
    size: u64,
    modified_ms: i64,
) -> HRESULT {
    let Some(Contexte(contexte)) = etat.contexte() else {
        return HRESULT(
            etat.compteurs
                .rendre(crate::pont::errors::Error::Inattendue),
        );
    };
    let info = PRJ_PLACEHOLDER_INFO {
        FileBasicInfo: info_de_base(repertoire, size, modified_ms),
        ..Default::default()
    };
    // SAFETY: `chemin` is null-terminated (set by the callback from ProjFS's
    // `FilePathName`), `info` lives until the end of the function.
    // `PRJ_PLACEHOLDER_INFO` ends with a flexible-size `VariableData: [u8; 1]`:
    // the announced size is that of the structure, since
    // we write no variable data.
    unsafe {
        (etat.projfs.write_placeholder_info)(
            contexte,
            PCWSTR(chemin.as_ptr()),
            &info,
            std::mem::size_of::<PRJ_PLACEHOLDER_INFO>() as u32,
        )
    }
}

/// Writes a chunk of a file — the response to `GetFileData`.
///
/// ⚠️ **The whole file NEVER enters memory.** It is the exact opposite of
/// the old bridge, each read of which did `getFile()` + `arrayBuffer()` +
/// `.slice(...)` (`web/index.js:562-564`): a sequential read of a
/// 100 MiB file in 128 KiB blocks reread 100 MiB from disk there,
/// **eight hundred times**.
///
/// ⚠️ **Alignment assumption, declared and NOT verified**:
/// `PrjGetVirtualizationInstanceInfo` returns a `WriteAlignment` this bridge does not
/// read — the entry point is not loaded (task 12's thirteen do not
/// include it). Chunks are `MAX_FRAME_SIZE` (64 KiB), a multiple of
/// any plausible sector size, and their position derives from the one
/// ProjFS requested. **This is not a proof**: if a
/// `PrjWriteFileData` were refused for alignment, it is here that one would have to
/// load `PrjGetVirtualizationInstanceInfo` and round. Declared legacy.
pub(super) fn write_file_data(etat: &Etat, flux: GUID, position: u64, charge: &[u8]) -> HRESULT {
    let Some(Contexte(contexte)) = etat.contexte() else {
        return HRESULT(
            etat.compteurs
                .rendre(crate::pont::errors::Error::Inattendue),
        );
    };
    let Some(tampon) = TamponAligne::allouer(&etat.projfs, contexte, charge.len()) else {
        tracing::warn!(
            octets = charge.len(),
            "PrjAllocateAlignedBuffer a rendu NULL"
        );
        return HRESULT(
            etat.compteurs
                .rendre(crate::pont::errors::Error::Inattendue),
        );
    };
    // SAFETY: `tampon.pointeur` is non-null and is at least `charge.len()`
    // bytes; the two regions do not overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(charge.as_ptr(), tampon.pointeur as *mut u8, charge.len())
    };
    // SAFETY: transcription of the `link!` at `mod.rs:122`. The buffer is released
    // by `TamponAligne`'s `Drop`, including if this call fails.
    unsafe {
        (etat.projfs.write_file_data)(
            contexte,
            &flux,
            tampon.pointeur,
            position,
            charge.len() as u32,
        )
    }
}

/// Fills the entry buffer from the session, until it is full
/// or the session exhausted.
///
/// Returns the completion `HRESULT`: `S_OK` in both cases. **A full
/// buffer is not an error** — the rest goes at the next
/// `GetDirectoryEnumeration`, on the same session.
pub(super) fn remplir(
    etat: &Etat,
    session: &mut Session,
    tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    while let Some(entree) = session.prochaine() {
        let nom: Vec<u16> = entree
            .nom
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let info = info_de_base(entree.repertoire, entree.size, entree.modified_ms);
        // SAFETY: `nom` is null-terminated and lives until the end of the turn;
        // `info` likewise. Transcription of the `link!` at `mod.rs:55`.
        let issue =
            unsafe { (etat.projfs.remplir_tampon_entrees)(PCWSTR(nom.as_ptr()), &info, tampon) };
        if issue == TAMPON_PLEIN {
            // ⚠️ **Do NOT advance**: the entry was not written, and advancing
            // would lose it forever — silently, since
            // the enumeration would end normally with one file
            // fewer.
            return S_OK;
        }
        if issue.is_err() {
            tracing::warn!(nom = %entree.nom, %issue, "PrjFillDirEntryBuffer refusée");
            return issue;
        }
        session.avancer();
    }
    S_OK
}

/// Converts protocol entries into enumeration entries.
pub(super) fn entrees_depuis(json: Vec<proto::files::entetes::EntreeJson>) -> Vec<Entree> {
    json.into_iter()
        .map(|e| Entree {
            nom: e.nom,
            repertoire: e.repertoire,
            size: e.size,
            modified_ms: e.modified,
        })
        .collect()
}
