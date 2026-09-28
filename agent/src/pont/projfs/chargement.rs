//! Runtime resolution of the **thirteen** entry points of `ProjectedFSLib.dll`, and
//! the thirteen signatures transcribed by hand.
//!
//! # Why this module exists — decision D1 of the spec
//!
//! The `Prj*` wrappers of the `windows` crate go through
//! `windows_core::link!`, which expands to
//! `#[link(name = …, kind = "raw-dylib", modifiers = "+verbatim")]`
//! (`windows-link-0.2.1/src/lib.rs:22`, the `not(target_arch = "x86")` branch,
//! that of our x86_64 target; the x86 branch is at lines 5-15 and carries
//! `import_name_type = "undecorated"` in addition). Calling a single one would set up a
//! **static** import of `ProjectedFSLib.dll` in the PE.
//!
//! Yet `agent.exe` is **a single binary for all modes**. An unresolved
//! import would not kill "the bridge": it would kill capture, video and
//! input on any VM lacking ProjFS — the exact state of this VM before
//! sub-block F0. Hence `LoadLibraryW` + `GetProcAddress`.
//!
//! **This module therefore takes the TYPES of the `windows` crate and never its
//! WRAPPERS.** The types (`PRJ_CALLBACKS`, `PRJ_CALLBACK_DATA`, the eight
//! `PRJ_*_CB`, the constants) are `struct`/`type`/`const`: they
//! emit no imported symbol.
//!
//! # 🔴 The risk this module carries, and which nothing closes — R7 of the spec
//!
//! **The thirteen signatures below are transcribed by hand, and the
//! compiler can no longer say anything about them.** A forgotten parameter, an invented
//! return type, a `*const` mistaken for a `*mut`: none of these defects
//! is detected before the call, and the call corrupts the stack without diagnostic.
//!
//! What EXISTS as a guard, and it must be said exactly:
//!
//! 1. **Each transcription carries, in a comment, the `link!` line it
//!    is a copy of, VERBATIM**, with its line number in
//!    `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`
//!    (read by command on August 19th, 2026, a 621-line file). The
//!    review is therefore a text-to-text comparison, never a
//!    reconstruction from memory.
//! 2. **`pont::resolution` checks that the thirteen NAMES exist**, and names
//!    the one missing — thirteen paths swept at each `cargo test`. It
//!    also pins the FIELD ↔ ENTRY pairing, and the construction below
//!    goes through a macro that writes each field name only once: two
//!    swapped entries are therefore structurally impossible here.
//! 3. The eight **callbacks** we provide, for their part, are checked by the
//!    compiler: `pont::projfs` assigns them to their `PRJ_*_CB` type in a
//!    `const _`, and an ABI divergence does not compile.
//!
//! ⚠️ **What these three guards do NOT cover, and it is the heart of the risk:
//! none of them bears on the ABI of the thirteen IMPORTED entry points.** Point 3
//! concerns the eight functions we WRITE, a set **disjoint** from the
//! thirteen we CALL — there is no type in `windows-rs` to
//! compare `PrjWriteFileData` against, nor any of the twelve others. **The thirteen
//! rest entirely on point 1.** Saying so rather than letting it be believed
//! that "eight out of thirteen are covered".
//!
//! ⚠️ **It is not a fear, it is a MEASURED fact** (August 20th, 2026, log
//! `journaux-pont-fichiers/f1-tache12-mutations.txt`): three ABI mutations
//! played on the declarations below — removing the fifth parameter of
//! `PrjStartVirtualizing`, giving `PrjStopVirtualizing` an `HRESULT` return,
//! changing the return of `PrjAllocateAlignedBuffer` to
//! `HRESULT` — **ALL THREE survived** `cargo check --target
//! x86_64-pc-windows-gnu` and the whole host suite. **Nothing in this repository
//! can catch a faulty transcription.** Point 1's text-to-text review
//! is the only guard, and running on the VM the only judge.
//!
//! # The check proving the absence of static import, and its reach
//!
//! ```text
//! cd agent && cargo build --release --target x86_64-pc-windows-gnu
//! x86_64-w64-mingw32-objdump -x target/x86_64-pc-windows-gnu/release/agent.exe \
//!   | grep -i 'projectedfslib'
//! ```
//!
//! ⚠️ **It bears on the host's `-gnu` binary, not on the `msvc` binary
//! shipped on the VM.** It therefore does NOT prove the absence of import in the
//! deliverable. The check that bears on the real binary is in the acceptance run (task 18)
//! and it is of another nature: rename `ProjectedFSLib.dll`, and check
//! that the supervisor, the capturer and a child start anyway.
//! **One is not worth the other.**

use anyhow::{Context, Result};
use windows::core::{GUID, PCSTR, PCWSTR};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACKS, PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS, PRJ_DIR_ENTRY_BUFFER_HANDLE,
    PRJ_FILE_BASIC_INFO, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, PRJ_PLACEHOLDER_INFO,
    PRJ_PLACEHOLDER_VERSION_INFO, PRJ_STARTVIRTUALIZING_OPTIONS, PRJ_UPDATE_FAILURE_CAUSES,
    PRJ_UPDATE_TYPES,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::pont::resolution;

// ────────────────────────────────────────────────────────────────────────────
// The thirteen signatures. Each carries the `link!` line it is a copy of.
//
// ⚠️ THREE traps found in the module, not assumed:
//
//   1. `PrjStartVirtualizing` has FIVE parameters in the `link!` and FOUR in
//      the wrapper: the wrapper hides the OUTPUT parameter
//      `namespacevirtualizationcontext` and returns it as a `Result`. Transcribing
//      the wrapper would produce a call missing its last argument.
//   2. `PrjStopVirtualizing` and `PrjFreeAlignedBuffer` RETURN NOTHING. Giving
//      them an `HRESULT` return is an ABI defect.
//   3. `PrjAllocateAlignedBuffer` returns a POINTER, not an `HRESULT`: `NULL`
//      is the failure.
//
// ⚠️ The return type is `windows::core::HRESULT` where the `link!` writes it,
// and never `i32`: `HRESULT` is `#[repr(transparent)]` over an `i32`, so
// the ABI is identical, but writing `i32` would make the review lose the
// literal comparison point 1 above promises it.
// ────────────────────────────────────────────────────────────────────────────

/// `mod.rs:3` — `fn PrjAllocateAlignedBuffer(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, size : usize) -> *mut core::ffi::c_void`
type AllouerTamponAligne = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    size: usize,
) -> *mut core::ffi::c_void;

/// `mod.rs:8` — `fn PrjClearNegativePathCache(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, totalentrynumber : *mut u32) -> windows_core::HRESULT`
type ViderCacheNegatif = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    totalentrynumber: *mut u32,
) -> windows::core::HRESULT;

/// `mod.rs:13` — `fn PrjCompleteCommand(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, commandid : i32, completionresult : windows_core::HRESULT, extendedparameters : *const PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS) -> windows_core::HRESULT`
type CompleterCommande = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    commandid: i32,
    completionresult: windows::core::HRESULT,
    extendedparameters: *const PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS,
) -> windows::core::HRESULT;

/// `mod.rs:21` — `fn PrjDeleteFile(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, destinationfilename : windows_core::PCWSTR, updateflags : PRJ_UPDATE_TYPES, failurereason : *mut PRJ_UPDATE_FAILURE_CAUSES) -> windows_core::HRESULT`
type SupprimerFichier = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    destinationfilename: PCWSTR,
    updateflags: PRJ_UPDATE_TYPES,
    failurereason: *mut PRJ_UPDATE_FAILURE_CAUSES,
) -> windows::core::HRESULT;

/// `mod.rs:38` — `fn PrjFileNameCompare(filename1 : windows_core::PCWSTR, filename2 : windows_core::PCWSTR) -> i32`
type ComparerNoms = unsafe extern "system" fn(filename1: PCWSTR, filename2: PCWSTR) -> i32;

/// `mod.rs:47` — `fn PrjFileNameMatch(filenametocheck : windows_core::PCWSTR, pattern : windows_core::PCWSTR) -> bool`
///
/// ⚠️ The return is `bool` **because the `link!` writes it `bool`**, and not for
/// convenience: the Win32 function returns a one-byte `BOOLEAN`, and Rust's `bool`
/// is one byte too. Writing `i32` or `BOOL` would read three more bytes
/// than the function wrote.
type ApparierNom = unsafe extern "system" fn(filenametocheck: PCWSTR, pattern: PCWSTR) -> bool;

/// `mod.rs:55` — `fn PrjFillDirEntryBuffer(filename : windows_core::PCWSTR, filebasicinfo : *const PRJ_FILE_BASIC_INFO, direntrybufferhandle : PRJ_DIR_ENTRY_BUFFER_HANDLE) -> windows_core::HRESULT`
type RemplirTamponEntrees = unsafe extern "system" fn(
    filename: PCWSTR,
    filebasicinfo: *const PRJ_FILE_BASIC_INFO,
    direntrybufferhandle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> windows::core::HRESULT;

/// `mod.rs:68` — `fn PrjFreeAlignedBuffer(buffer : *const core::ffi::c_void)`
///
/// ⚠️ **No return** (trap 2 above).
type RendreTamponAligne = unsafe extern "system" fn(buffer: *const core::ffi::c_void);

/// `mod.rs:93` — `fn PrjMarkDirectoryAsPlaceholder(rootpathname : windows_core::PCWSTR, targetpathname : windows_core::PCWSTR, versioninfo : *const PRJ_PLACEHOLDER_VERSION_INFO, virtualizationinstanceid : *const windows_core::GUID) -> windows_core::HRESULT`
type MarquerRacine = unsafe extern "system" fn(
    rootpathname: PCWSTR,
    targetpathname: PCWSTR,
    versioninfo: *const PRJ_PLACEHOLDER_VERSION_INFO,
    virtualizationinstanceid: *const GUID,
) -> windows::core::HRESULT;

/// `mod.rs:101` — `fn PrjStartVirtualizing(virtualizationrootpath : windows_core::PCWSTR, callbacks : *const PRJ_CALLBACKS, instancecontext : *const core::ffi::c_void, options : *const PRJ_STARTVIRTUALIZING_OPTIONS, namespacevirtualizationcontext : *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT) -> windows_core::HRESULT`
///
/// ⚠️ **FIVE parameters** (trap 1 above). The fifth is the OUTPUT
/// parameter the wrapper hides behind its `Result`.
type DemarrerVirtualisation = unsafe extern "system" fn(
    virtualizationrootpath: PCWSTR,
    callbacks: *const PRJ_CALLBACKS,
    instancecontext: *const core::ffi::c_void,
    options: *const PRJ_STARTVIRTUALIZING_OPTIONS,
    namespacevirtualizationcontext: *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
) -> windows::core::HRESULT;

/// `mod.rs:109` — `fn PrjStopVirtualizing(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT)`
///
/// ⚠️ **No return** (trap 2 above).
type ArreterVirtualisation =
    unsafe extern "system" fn(namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT);

/// `mod.rs:122` — `fn PrjWriteFileData(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, datastreamid : *const windows_core::GUID, buffer : *const core::ffi::c_void, byteoffset : u64, length : u32) -> windows_core::HRESULT`
type EcrireDonnees = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    datastreamid: *const GUID,
    buffer: *const core::ffi::c_void,
    byteoffset: u64,
    length: u32,
) -> windows::core::HRESULT;

/// `mod.rs:130` — `fn PrjWritePlaceholderInfo(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, destinationfilename : windows_core::PCWSTR, placeholderinfo : *const PRJ_PLACEHOLDER_INFO, placeholderinfosize : u32) -> windows_core::HRESULT`
type EcrireInfoMarqueur = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    destinationfilename: PCWSTR,
    placeholderinfo: *const PRJ_PLACEHOLDER_INFO,
    placeholderinfosize: u32,
) -> windows::core::HRESULT;

/// Converts an address into a typed function pointer.
///
/// # Safety
///
/// The caller guarantees that `adresse` is the address actually exported for
/// the entry point `T` is the transcription of. **This is where risk R7 is
/// taken**: neither this function nor the compiler can check that `T`
/// describes the signature of the function living at this address.
///
/// The size assertion changes nothing to that, and it is not decorative for
/// all that: it catches a `T` that would **not** be a bare function
/// pointer — an `Option<fn>` inhabiting a niche, a fat reference —, a case
/// `transmute_copy` would silently accept **by reading beyond the source
/// variable**.
unsafe fn depuis_adresse<T: Copy>(adresse: usize) -> T {
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<usize>(),
        "la cible d'une transcription ProjFS n'est pas un pointeur de fonction nu"
    );
    // SAFETY: the sizes are equal (assertion above), and the caller
    // guarantees the signature match.
    unsafe { std::mem::transmute_copy(&adresse) }
}

/// The thirteen entry points, resolved once.
///
/// **None is `Option`**: [`charger`] fails if a single one is missing, so a
/// `ProjFs` that exists has them all. Making them optional would put on
/// each call site a decision that belongs to loading.
pub struct ProjFs {
    pub allouer_tampon_aligne: AllouerTamponAligne,
    /// ⚠️ **Loaded without a caller, DELIBERATELY.** It empties ProjFS's negative
    /// cache, which no F1 path requests: the only way to
    /// trigger it is `Rafraichir`. It is resolved right now so that
    /// F5 does not have to reopen this layer — and because a missing entry point
    /// must be discovered at LOADING, with a message naming it,
    /// never at the first call.
    ///
    /// ✅ **IT HAS HAD ITS FIRST PRODUCTION CALLER SINCE F5**
    /// (`service::annonces::rafraichir`), and F1's bet therefore held: the
    /// layer was not reopened. **R7 closes by ONE entry point out of five** —
    /// not "R7 is closed".
    /// 🔵 **And its `totalentrynumber` is TRACED**: the negative cache has
    /// become OBSERVABLE for the first time. Recorded, 3 runs:
    /// `cache_negatif_purge=1`. F4 could only measure a **zero**
    /// differential, since no probe reached the provider.
    pub vider_cache_negatif: ViderCacheNegatif,
    pub completer_commande: CompleterCommande,
    /// ⚠️ **Loaded without a caller, DELIBERATELY**, for the same reason:
    /// **no eviction policy in F1**. Each file read is hydrated on
    /// the VM's disk and stays there (spec §6.4, case 4). Setting a policy
    /// without measurement would be exactly the gesture this repository reproaches its
    /// uncalibrated constants for; the measurement belongs to F5, and the entry point is
    /// ready for it.
    ///
    /// ⛔ **F5 MEASURED, AND SET NO POLICY** — it was its decision
    /// D9, written in advance. **This entry point therefore stays without a caller, and
    /// sub-project ③ closes behind it**: there will be no F6. Four
    /// of the five ProjFS entry points without a `PRJ_*_CB` twin stay so.
    #[allow(dead_code)]
    pub supprimer_fichier: SupprimerFichier,
    pub comparer_noms: ComparerNoms,
    pub apparier_nom: ApparierNom,
    pub remplir_tampon_entrees: RemplirTamponEntrees,
    pub rendre_tampon_aligne: RendreTamponAligne,
    pub marquer_racine: MarquerRacine,
    pub demarrer_virtualisation: DemarrerVirtualisation,
    pub arreter_virtualisation: ArreterVirtualisation,
    pub ecrire_donnees: EcrireDonnees,
    pub ecrire_info_marqueur: EcrireInfoMarqueur,
}

/// The library's name, in null-terminated UTF-16.
///
/// Written out in full rather than through the `w!` macro so that the name is
/// `grep`-able as is: it is the exact pattern the `objdump` check and the
/// renaming acceptance run (task 18) both look for.
const BIBLIOTHEQUE: &str = "ProjectedFSLib.dll";

/// Loads `ProjectedFSLib.dll` and resolves the thirteen entry points.
///
/// **A single missing entry point makes loading fail, and the error
/// names it** — see [`crate::pont::resolution`], where this rule lives and is tested
/// on the host. Only what no host test can reach remains here.
pub fn charger() -> Result<ProjFs> {
    let nom: Vec<u16> = BIBLIOTHEQUE
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // `LoadLibraryW` and not `LoadLibraryA`: the name is text, and nothing
    // guarantees the Windows session's ANSI code page.
    let module = unsafe { LoadLibraryW(PCWSTR(nom.as_ptr())) }
        .with_context(|| format!("chargement de {BIBLIOTHEQUE}"))?;

    let adresses = resolution::resoudre(|entree| {
        // `GetProcAddress` takes a null-terminated ANSI name. The thirteen
        // names are pure ASCII (`resolution::NOMS`), so the conversion cannot
        // lose a character; the `ok()?` covers the only remaining
        // case, an inner null, which can only come from a faulty
        // edit of `NOMS`.
        let c = std::ffi::CString::new(entree).ok()?;
        // SAFETY: `module` is a valid handle returned by `LoadLibraryW`, and
        // `c` lives until the end of this expression, hence beyond the call.
        unsafe { GetProcAddress(module, PCSTR(c.as_ptr() as *const u8)) }.map(|f| f as usize)
    })
    .with_context(|| {
        format!(
            "{BIBLIOTHEQUE} est chargée mais n'exporte pas les treize entrées que le pont \
             fichiers appelle : cette VM porte probablement une génération antérieure de ProjFS"
        )
    })?;

    // SAFETY: each address comes from `GetProcAddress` on the name that
    // `resolution::Adresses` associates with this field — a pairing pinned by
    // `resolution::chaque_champ_recoit_l_adresse_de_son_entree` — and each
    // type above is the literal copy of the `link!` cited next to its
    // declaration. **It is here, and nowhere else, that risk R7 is
    // taken**: if a transcription diverges, this `transmute` is valid for the
    // compiler and wrong for the machine.
    //
    // ⚠️ **The field name is written ONLY ONCE per entry point**, by the macro
    // below, and it is the SAME in `ProjFs` and in
    // `resolution::Adresses`. It is what makes swapping two entry points
    // structurally impossible here: the first draft indexed a
    // `[usize; 13]` by rank constants, and a mutation swapping
    // two of these ranks SURVIVED the whole test suite.
    macro_rules! transcrire {
        ($($champ:ident),+ $(,)?) => {
            ProjFs { $( $champ: unsafe { depuis_adresse(adresses.$champ) }, )+ }
        };
    }
    Ok(transcrire!(
        allouer_tampon_aligne,
        vider_cache_negatif,
        completer_commande,
        supprimer_fichier,
        comparer_noms,
        apparier_nom,
        remplir_tampon_entrees,
        rendre_tampon_aligne,
        marquer_racine,
        demarrer_virtualisation,
        arreter_virtualisation,
        ecrire_donnees,
        ecrire_info_marqueur,
    ))
}
