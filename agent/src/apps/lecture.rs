//! Reading a Windows shortcut through `IShellLinkW`, and discovering the four
//! roots through `SHGetKnownFolderPath`.
//!
//! 🔴 THIS MODULE IS `#[cfg(windows)]` IN ITS ENTIRETY, AND IT CARRIES NO
//! RULE. It returns a [`Brut`] and nothing else; what decides whether to keep it
//! lives in `apps::raccourci`, which is pure and is judged on the host against the
//! VM's 218 real shortcuts. The split is that of spec §6, not an
//! implementation trade-off.
//!
//! ⚠️ IT IS CHECKED ONLY BY `cargo check --target x86_64-pc-windows-gnu`,
//! which covers types, borrows, visibility and lifetimes — and NOT
//! linking, the real target being `msvc`. No host test can
//! cover it.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::{
    FOLDERID_CommonStartMenu, FOLDERID_Desktop, FOLDERID_PublicDesktop, FOLDERID_StartMenu,
    IShellLinkW, SHGetKnownFolderPath, ShellLink, KF_FLAG_DEFAULT, SLGP_RAWPATH, SLGP_UNCPRIORITY,
};

use super::raccourci::Brut;

/// The `IShellLinkW` buffers: the interface documentation bounds
/// none of these strings, but a shortcut argument cannot in any case
/// exceed the Windows command line (32,767 UTF-16 units). The
/// buffer is therefore sized so that no silent truncation is
/// possible, rather than on `MAX_PATH` — which long paths exceed.
const TAMPON: usize = 32_768;

/// Joins the calling thread's single-threaded COM apartment.
///
/// 🔴 `APARTMENTTHREADED`, AND NOT `MULTITHREADED` like `wasapi.rs`: the Shell
/// and above all `ShellExecuteExW` (`apps::lancement`) require an STA, and the
/// two run on the SAME dedicated thread. Choosing the MTA here would make the
/// launch fail later, far from its cause.
///
/// 🔴 THE `HRESULT` IS CHECKED, NOT IGNORED — precedent `wasapi.rs`. `S_OK`
/// (this thread has just joined an STA) and `S_FALSE` (it was already a member)
/// are acceptable; `RPC_E_CHANGED_MODE` means this thread already belongs
/// to an apartment of another model, and carrying on would mean calling
/// COM vtables from the wrong apartment without marshalling — undefined
/// behaviour that "works" most of the time, hence that no run
/// reliably reveals.
///
/// No matching `CoUninitialize`: this thread lives as long as the
/// reconciliation loop, and COM requires the release to happen on the
/// thread that initialised — it is the same reasoning, and the same precedent,
/// as in `agent/src/wasapi.rs`.
pub fn initialize_com() -> Result<()> {
    // SAFETY: FFI call. The only contract is that this thread has not already
    // joined an apartment of another model, which the check below
    // verifies rather than assumes.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        bail!(
            "CoInitializeEx(APARTMENTTHREADED) refused ({hr:?}): the application discovery \
             thread already belongs to a COM apartment of another model. \
             `IShellLinkW` and `ShellExecuteExW` both require an STA, and run \
             on this very thread."
        );
    }
    Ok(())
}

/// Reads a `.lnk` and returns its five fields, plus the `nShow` the launch
/// will reuse.
///
/// 🔴 `IShellLink::Resolve` IS CALLED NOWHERE, AND IT IS A DECISION.
/// It may query the network, and above all **trigger the on-demand
/// installation of an advertised MSI shortcut** — during a routine
/// reconciliation, every thirty seconds, without anyone having asked.
/// The cost of refusing is named: a shortcut whose target has moved keeps its
/// old path, and it is the launch that catches up, through its fallback.
pub fn lire(chemin: &Path) -> Result<Raccourci> {
    let chemin_w = vers_utf16(&chemin.to_string_lossy());

    // SAFETY: FFI calls. `CoCreateInstance` returns a reference-counted
    // interface that `windows-rs` releases through `Drop`.
    let lien: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .context("CoCreateInstance(ShellLink)")?;
    let persistance: IPersistFile = lien.cast().context("IShellLinkW -> IPersistFile")?;
    unsafe { persistance.Load(PCWSTR(chemin_w.as_ptr()), STGM_READ) }
        .with_context(|| format!("IPersistFile::Load({})", chemin.display()))?;

    // `SLGP_RAWPATH` returns the path AS STORED, environment
    // variables included and unexpanded: that is what avoids
    // calling `Resolve` implicitly. `SLGP_UNCPRIORITY` prefers the UNC
    // path to the mapped-drive path, which depends on the session.
    //
    // ⚠️ The third argument is a BARE `u32`, while the constants
    // live in the newtype `SLGP_FLAGS(pub i32)`: the conversion is
    // explicit, and it was re-read in the bindings.
    let drapeaux = (SLGP_RAWPATH.0 | SLGP_UNCPRIORITY.0) as u32;
    let mut tampon = vec![0u16; TAMPON];
    // The second argument may be null: we do not want the
    // `WIN32_FIND_DATAW`, and asking for it would cost one disk access per
    // shortcut — 218 per round.
    let cible = match unsafe { lien.GetPath(&mut tampon, std::ptr::null_mut(), drapeaux) } {
        Ok(()) => developper(&depuis_utf16(&tampon)),
        // ⚠️ AN UNREADABLE TARGET IS NOT AN ERROR: Shell namespace
        // targets (Recycle Bin, "This PC", Control Panel) have
        // no file path, and there are 7 of them on this VM. The filtering
        // rule discards them through `Ecart::CibleVide`; failing here would
        // turn them into read failures.
        Err(_) => String::new(),
    };

    let mut tampon = vec![0u16; TAMPON];
    let arguments = match unsafe { lien.GetArguments(&mut tampon) } {
        Ok(()) => depuis_utf16(&tampon),
        Err(_) => String::new(),
    };

    let mut tampon = vec![0u16; TAMPON];
    let repertoire = match unsafe { lien.GetWorkingDirectory(&mut tampon) } {
        Ok(()) => developper(&depuis_utf16(&tampon)),
        Err(_) => String::new(),
    };

    // The window style the shortcut asks for — minimised, maximised,
    // normal. The launch replays it as is, so that a double-click in
    // the agent and a double-click in Explorer give the same thing.
    let montrer = unsafe { lien.GetShowCmd() }.map(|c| c.0).unwrap_or(1);

    // 🔴 THE `IconLocation` IN RAW `<path>,<index>` FORMAT — sub-block G2.
    // It is from it that `apps::icone::source` draws the image's PROVENANCE, and
    // that is why it is neither expanded nor normalised here: the module that
    // reads it is PURE, and it must see exactly what the shortcut carries.
    //
    // ⚠️ AN EMPTY PATH IS NOT AN ABSENCE OF ICON: it refers to the TARGET,
    // and **92 of the 153 shortcuts kept on this VM are in that case**. A
    // read error therefore returns the empty string, which carries exactly that
    // meaning.
    let mut tampon = vec![0u16; TAMPON];
    let mut index = 0i32;
    let icone = match unsafe { lien.GetIconLocation(&mut tampon, &mut index) } {
        Ok(()) => {
            let chemin = depuis_utf16(&tampon);
            if chemin.is_empty() && index == 0 {
                String::new()
            } else {
                format!("{chemin},{index}")
            }
        }
        Err(_) => String::new(),
    };

    let nom = chemin
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    Ok(Raccourci {
        brut: Brut {
            nom,
            chemin: chemin.to_string_lossy().into_owned(),
            cible,
            arguments,
            repertoire,
        },
        montrer,
        icone,
    })
}

/// What the read returns: the five fields, plus the `nShow`.
///
/// `montrer` takes part NEITHER in the identity NOR in the message: it only serves the
/// launch, and putting it in [`Brut`] would make it travel on the channel for
/// nothing.
pub struct Raccourci {
    pub brut: Brut,
    pub montrer: i32,
    /// The RAW `IconLocation`, `<path>,<index>` — empty when the shortcut
    /// declares none, which refers to the target. Sub-block G2.
    pub icone: String,
}

/// The four shortcut roots, resolved by the system.
///
/// 🔴 NEVER LITERAL PATHS. The spec measures
/// `C:\Users\Administrateur\Desktop` on ONE machine: hardcoding it would make
/// discovery wrong on any other installation, and silent about it.
///
/// ⚠️ A ROOT THAT FAILS TO RESOLVE IS SKIPPED WITH ITS TRACE, NEVER
/// FATAL. `FOLDERID_StartMenu` may not exist on a fresh profile, and a
/// reconciliation that failed entirely on a missing root would make the
/// WHOLE catalogue disappear — spec §7 says so by name.
pub fn racines() -> Vec<PathBuf> {
    const RACINES: [(&str, GUID); 4] = [
        ("Desktop", FOLDERID_Desktop),
        ("Public desktop", FOLDERID_PublicDesktop),
        ("Start menu", FOLDERID_StartMenu),
        ("common Start menu", FOLDERID_CommonStartMenu),
    ];
    let mut sortie = Vec::new();
    for (nom, id) in RACINES {
        match dossier_connu(&id) {
            Ok(chemin) if chemin.is_dir() => sortie.push(chemin),
            Ok(chemin) => tracing::warn!(
                racine = nom,
                chemin = %chemin.display(),
                "shortcut root absent from the disk, skipped"
            ),
            Err(error) => {
                tracing::warn!(racine = nom, %error, "shortcut root not resolved, skipped")
            }
        }
    }
    sortie
}

fn dossier_connu(id: &GUID) -> Result<PathBuf> {
    // SAFETY: FFI call. The returned buffer is allocated by the Shell, and it is
    // up to us to free it through `CoTaskMemFree` — hence the copy before.
    let brut = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
        .context("SHGetKnownFolderPath")?;
    if brut.is_null() {
        bail!("SHGetKnownFolderPath returned a null pointer");
    }
    let chemin = unsafe { brut.to_string() }.context("known folder path not valid UTF-16");
    unsafe { CoTaskMemFree(Some(brut.0 as *const _)) };
    Ok(PathBuf::from(chemin?))
}

/// Walks a root RECURSIVELY and returns its `.lnk` files.
///
/// ⚠️ RECURSIVE BECAUSE THE START MENU IS A TREE: it carries 203 of the
/// VM's 218 shortcuts, almost all in per-publisher subfolders.
/// A flat walk would see almost none of them.
///
/// ⚠️ AN UNREADABLE DIRECTORY IS SKIPPED WITH ITS TRACE, like a root.
pub fn lnk_under(racine: &Path) -> Vec<PathBuf> {
    let mut sortie = Vec::new();
    let mut pile = vec![racine.to_path_buf()];
    while let Some(dossier) = pile.pop() {
        let entrees = match std::fs::read_dir(&dossier) {
            Ok(e) => e,
            Err(error) => {
                tracing::warn!(dossier = %dossier.display(), %error, "unreadable directory, skipped");
                continue;
            }
        };
        for entree in entrees.flatten() {
            let chemin = entree.path();
            match entree.file_type() {
                // Symbolic links are not followed: a link that
                // pointed to an ancestor would make this walk loop without
                // end, every thirty seconds.
                Ok(t) if t.is_dir() => pile.push(chemin),
                Ok(t) if t.is_file() => {
                    let est_lnk = chemin
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
                    if est_lnk {
                        sortie.push(chemin);
                    }
                }
                _ => {}
            }
        }
    }
    sortie
}

/// Expands the environment variables of a path.
///
/// `SLGP_RAWPATH` returns the path as stored — `%ProgramFiles%\…`
/// for many installer shortcuts. Without this expansion, the rule
/// "the target file exists" would discard them all, and the identity key
/// would depend on the way it was written rather than on the file targeted.
fn developper(value: &str) -> String {
    if !value.contains('%') {
        return value.to_string();
    }
    let source = vers_utf16(value);
    let mut tampon = vec![0u16; TAMPON];
    // SAFETY: FFI call. Returns the number of units written, zero on
    // failure — in which case we keep the unexpanded value rather than
    // returning an empty string, which would read as "Shell namespace
    // target" and would change the discard reason.
    let written = unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut tampon)) };
    if written == 0 {
        return value.to_string();
    }
    depuis_utf16(&tampon)
}

/// ⚠️ `pub(super)` SINCE SUB-BLOCK G2, and it is a DELIBERATE reuse
/// rather than a copy: `apps::icone` and `apps::icone::lecture_pe`
/// need it, and two identical encodings would diverge the day one of them
/// stopped appending its terminating nul — a failure nothing would report before a
/// buffer overrun on the Windows side.
pub(super) fn vers_utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// ⚠️ STOPS AT THE FIRST NUL. The `IShellLinkW` buffers are not
/// filled: they carry a nul-terminated string followed by 32,000
/// zeros, and a naive conversion would return a 32,768-character string
/// whose equality and fingerprint would be wrong.
fn depuis_utf16(tampon: &[u16]) -> String {
    let fin = tampon.iter().position(|&c| c == 0).unwrap_or(tampon.len());
    String::from_utf16_lossy(&tampon[..fin])
}
