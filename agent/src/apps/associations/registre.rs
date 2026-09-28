//! Reading the registry — the ONLY `#[cfg(windows)]` half of the associations.
//!
//! 🔴 IT DECIDES NOTHING. It reads strings; matching,
//! normalisation and order live in the parent module, which is PURE and
//! tested on the host. It is the split G2 established twice
//! (`apps/icone/ressource.rs`, `apps/installation`), and its benefit is the
//! same: without it, the part that decides would have **no** host test.
//!
//! ⚠️ THIS MODULE HAS NO TEST, AND IT IS DECLARED: it only compiles on the VM.
//! `cargo check --target x86_64-pc-windows-gnu` checks its **types, borrows and
//! lifetimes** — never its behaviour.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, MAX_PATH};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CLASSES_ROOT,
    HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ,
};

/// 🔴 THE CAP EXISTS SO THAT THE WORST CASE IS BOUNDED, and it is declared.
/// `FileExts` typically holds a few dozen entries; a messed-up registry
/// could hold many more, and the reconciliation runs
/// every `PERIODE_RECONCILIATION`. **Not calibrated**: it is a safeguard
/// against a read that would never end, never a target.
const EXTENSIONS_MAX: usize = 512;

/// The key where Windows stores THE USER'S REAL CHOICE.
const FILE_EXTS: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts";

fn en_pcwstr(s: &str) -> HSTRING {
    HSTRING::from(s)
}

/// Reads a `REG_SZ` value, or returns `None`.
///
/// ⚠️ `RegGetValueW` IS PREFERRED TO `RegQueryValueExW`: it opens, reads, checks
/// the type and closes in one call, and it **guarantees null termination** of what
/// it returns — which `RegQueryValueEx` does not, and which is a
/// classic source of reads past the buffer.
fn value(racine: HKEY, subkey: &str, nom: Option<&str>) -> Option<String> {
    let mut tampon = [0u16; 2048];
    let mut octets = (tampon.len() * 2) as u32;
    let cle = en_pcwstr(subkey);
    let nom_h = nom.map(en_pcwstr);
    let code = unsafe {
        RegGetValueW(
            racine,
            PCWSTR(cle.as_ptr()),
            nom_h
                .as_ref()
                .map_or(PCWSTR::null(), |h| PCWSTR(h.as_ptr())),
            RRF_RT_REG_SZ,
            None,
            Some(tampon.as_mut_ptr().cast()),
            Some(&mut octets),
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    // `octets` counts BYTES, terminator included: we go back to `u16` and
    // drop the terminator.
    let unites = (octets as usize / 2).min(tampon.len());
    let sans_zero = tampon[..unites]
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(unites);
    let texte = String::from_utf16_lossy(&tampon[..sans_zero]);
    if texte.is_empty() {
        None
    } else {
        Some(texte)
    }
}

/// The extensions the user once chose, as they are.
fn extensions_connues() -> Vec<String> {
    let mut cle = HKEY::default();
    let chemin = en_pcwstr(FILE_EXTS);
    let ouvert = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            Some(0),
            KEY_READ,
            &mut cle,
        )
    };
    if ouvert != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut noms = Vec::new();
    let mut index = 0u32;
    loop {
        if noms.len() >= EXTENSIONS_MAX {
            tracing::warn!(
                plafond = EXTENSIONS_MAX,
                "extension cap reached: reading the associations stops there"
            );
            break;
        }
        let mut tampon = [0u16; MAX_PATH as usize];
        let mut size = tampon.len() as u32;
        let code = unsafe {
            RegEnumKeyExW(
                cle,
                index,
                Some(windows::core::PWSTR(tampon.as_mut_ptr())),
                &mut size,
                None,
                None,
                None,
                None,
            )
        };
        if code != ERROR_SUCCESS {
            break;
        }
        noms.push(String::from_utf16_lossy(&tampon[..size as usize]));
        index += 1;
    }
    unsafe {
        let _ = RegCloseKey(cle);
    };
    noms
}

/// The ProgID kept for an extension: **the user's choice first**,
/// the default value of `HKCR` second.
///
/// 🔴 THE ORDER IS THE RULE, NOT A CONVENIENCE: `UserChoice` is what the
/// person really chose, and `HKCR` what the last installation
/// set. Swapping them would attribute the association to the most
/// recently installed software rather than to the one in use.
fn progid(extension_brute: &str) -> Option<String> {
    let choix = value(
        HKEY_CURRENT_USER,
        &format!(r"{FILE_EXTS}\{extension_brute}\UserChoice"),
        Some("ProgId"),
    );
    if choix.is_some() {
        return choix;
    }
    let with_dot = if extension_brute.starts_with('.') {
        extension_brute.to_string()
    } else {
        format!(".{extension_brute}")
    };
    value(HKEY_CLASSES_ROOT, &with_dot, None)
}

/// The `(extension, command line)` pairs the registry holds —
/// **a single read, for all applications**.
///
/// 🔴 WHAT IT RETURNS IS RAW, AND THAT IS ITS WHOLE PURPOSE. Grouping,
/// normalisation and order are PURE and TESTED rules
/// (`super::table`); this module only reads.
///
/// ⚠️ THE SCOPE IS THAT OF `FileExts`, AND IT IS NARROWER THAN "ALL
/// THE MACHINE'S ASSOCIATIONS": these are the extensions that
/// THE USER once opened or chose. Enumerating `HKEY_CLASSES_ROOT`
/// entirely — several thousand keys, re-read at every reconciliation —
/// would be paying dearly for a set nobody asked for. **Declared,
/// not discovered.**
pub fn couples() -> Vec<(String, String)> {
    let mut couples = Vec::new();
    for brute in extensions_connues() {
        let Some(id) = progid(&brute) else { continue };
        let Some(commande) = value(
            HKEY_CLASSES_ROOT,
            &format!(r"{id}\shell\open\command"),
            None,
        ) else {
            continue;
        };
        couples.push((brute, commande));
    }
    couples
}
