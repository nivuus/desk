//! Obtaining the RAW BYTES of an icon directory — and nothing else.
//!
//! 🔴 THIS MODULE DECIDES NOTHING. It returns a `Vec<u8>` that
//! `apps::icone::ressource`, which is PURE, parses. It is the split the
//! specification §6 calls "the most important point of this
//! list", and without it acceptance criterion ② would have no host test:
//! its only proof would be a control-flow argument — the
//! exact situation that defect F1 of sub-block D7 paid for.
//!
//! ⚠️ IT IS ONLY CHECKED BY `cargo check --target x86_64-pc-windows-gnu`,
//! which covers types, borrows, visibility and lifetimes — and NOT
//! linking, the real target being `msvc`. No host test can
//! cover it, and it is said rather than hidden.

use std::path::Path;

use anyhow::{bail, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    EnumResourceNamesW, FindResourceW, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
    LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
};

use super::super::lecture::vers_utf16;

/// `RT_GROUP_ICON` — the resource that carries the `GRPICONDIR`.
///
/// ⚠️ IT IS AN INTEGER DISGUISED AS A POINTER, and that is the API's contract: the
/// predefined resource types are passed as `PCWSTR` whose
/// numeric value is the identifier. `RT_ICON` is 3, `RT_GROUP_ICON` is
/// `3 + 11 = 14`.
const RT_GROUP_ICON: PCWSTR = PCWSTR(14 as *const u16);

/// The bytes of the `GRPICONDIR` of the FIRST icon group of a PE module.
///
/// ⚠️ **THE FIRST GROUP, AND IT IS A DECLARED APPROXIMATION.** The Shell
/// itself chooses the group that the `IconLocation` designates by its index, and a
/// negative index designates a resource by its IDENTIFIER. This module takes
/// the index when it is positive and falls back on the first group otherwise. **The
/// consequence is bounded and named**: `source_max` may then describe a
/// group neighbouring the one the image shows — never an invented size, and
/// never a convenient `256`.
///
/// 🔴 `LOAD_LIBRARY_AS_IMAGE_RESOURCE` IS ADDED TO `AS_DATAFILE` AND IS NOT
/// OPTIONAL: without it, `FindResourceW` on a module loaded as a pure data
/// file does not always find its resources. Probe M1 of the plan used
/// both, and it read 110 sources out of 110 attempted.
///
/// ⚠️ **NO CODE OF THE LOADED MODULE IS EXECUTED**: that is the whole purpose of
/// `AS_DATAFILE`, and it is what makes it acceptable to open arbitrary `.exe`
/// files from the VM's disk.
pub fn grpicondir(module: &Path, index: i32) -> Result<Vec<u8>> {
    let large = vers_utf16(&module.to_string_lossy());
    // SAFETY: FFI call. The path is a null-terminated UTF-16 buffer
    // that we own, and both flags forbid any execution of
    // the module's code.
    let handle = unsafe {
        LoadLibraryExW(
            PCWSTR(large.as_ptr()),
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .with_context(|| format!("loading module {} as a data file", module.display()))?;

    let result = lire_groupe(handle, index);

    // 🔴 `FreeLibrary` ON EVERY EXIT PATH, error paths included. A
    // leak here would be invisible for hours, on a process that
    // reconciles every thirty seconds and opens up to 110 modules per
    // tick — that is 13,200 handles per hour.
    // SAFETY: FFI call. The handle comes from `LoadLibraryExW` just
    // above and has not been released elsewhere.
    let _ = unsafe { FreeLibrary(handle) };
    result
}

/// The enumeration callback: it KEEPS the first name and stops.
///
/// SAFETY: `param` is the `*mut Option<PCWSTR>` that `lire_groupe` passes to
/// `EnumResourceNamesW`, and it lives for the whole duration of the call.
unsafe extern "system" fn premier_nom(
    _module: HMODULE,
    _type_: PCWSTR,
    nom: PCWSTR,
    param: isize,
) -> windows::core::BOOL {
    let sortie = param as *mut Option<PCWSTR>;
    if !sortie.is_null() {
        unsafe { *sortie = Some(nom) };
    }
    // `FALSE` STOPS the enumeration: we only want the first one.
    windows::core::BOOL(0)
}

fn lire_groupe(handle: HMODULE, index: i32) -> Result<Vec<u8>> {
    // 🔴 A NULL NAME IS NOT "THE FIRST RESOURCE", AND IT IS THE DEFECT
    // THE MEASUREMENT OF 21 AUGUST 2026 FOUND. A first draft passed
    // `PCWSTR(null())` to `FindResourceW` for an index of 0, believing it
    // asked for the first group: `FindResourceW` then looks for a resource
    // whose NAME is null, and finds none. **Measured: 148 of the 154
    // applications returned `no RT_GROUP_ICON resource`, including
    // modules that obviously carry one — `steam.exe`.** The catalogue
    // stayed correct and the icons were served; only the PROVENANCE
    // fell to `NonMesuree`, which is exactly what the sub-block
    // exists to measure.
    //
    // The first group is therefore obtained by ENUMERATION, as probe M1 of the
    // plan did and as task 9 prescribed — `EnumResourceNamesW`
    // was in its list of calls, and its omission is what produced the
    // defect.
    let mut premier: Option<PCWSTR> = None;
    if index <= 0 {
        // SAFETY: FFI call. `handle` is alive, and the pointer passed as
        // `param` points to a variable on this stack, which outlives the call.
        // `EnumResourceNamesW` returns `Err` when the callback stops the enumeration
        // — which ours always does —, so its result is ignored in
        // favour of what the callback KEPT.
        let _ = unsafe {
            EnumResourceNamesW(
                Some(handle),
                RT_GROUP_ICON,
                Some(premier_nom),
                &mut premier as *mut _ as isize,
            )
        };
    }
    // ⚠️ A NEGATIVE index designates a resource by its identifier; a
    // positive index is a RANK. This module can only follow the second case, and it
    // falls back on the first enumerated group for the other — see the caveat at
    // the head of [`grpicondir`].
    let nom = if index > 0 {
        PCWSTR(index as usize as *const u16)
    } else {
        match premier {
            Some(n) => n,
            None => bail!("no icon group enumerated in this module"),
        }
    };
    // SAFETY: FFI call. `handle` is alive (its `FreeLibrary` is in the
    // caller), and `nom` is either a disguised integer identifier or null.
    let bloc = unsafe { FindResourceW(Some(handle), nom, RT_GROUP_ICON) };
    if bloc.is_invalid() {
        bail!("no RT_GROUP_ICON resource in this module");
    }
    // SAFETY: FFI call. `bloc` has just been validated.
    let size = unsafe { SizeofResource(Some(handle), bloc) } as usize;
    if size == 0 {
        bail!("RT_GROUP_ICON resource of zero size");
    }
    // SAFETY: FFI call.
    let charge = unsafe { LoadResource(Some(handle), bloc) }.context("LoadResource")?;
    // SAFETY: FFI call. `LockResource` returns a pointer to the mapped
    // resource, valid as long as the module is loaded — hence until the
    // caller's `FreeLibrary`, which runs AFTER this function.
    let debut = unsafe { LockResource(charge) } as *const u8;
    if debut.is_null() {
        bail!("LockResource returned a null pointer");
    }
    // SAFETY: the resource is mapped over `size` bytes, which
    // `SizeofResource` has just returned, and the copy is made BEFORE the
    // caller's `FreeLibrary`. The resulting `Vec` no longer depends on the
    // module.
    Ok(unsafe { std::slice::from_raw_parts(debut, size) }.to_vec())
}
