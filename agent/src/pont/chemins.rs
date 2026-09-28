//! Normalisation of a path delivered by ProjFS into a logical path for the File
//! System Access API. **PURE**: no `cfg`, no dependency on `windows`,
//! fully tested on the host.
//!
//! ProjFS delivers `PRJ_CALLBACK_DATA.FilePathName`: a path **relative to the
//! virtualisation root**, with backslashes, without a drive letter. This
//! module turns it into a logical path — components separated by `/` — or
//! **refuses** it. It never trusts what it receives: the virtualisation
//! root is traversed by any application of the Windows
//! session, including hostile ones.
//!
//! ⚠️ **CASE is the structural trap of this module, and F1 does not solve
//! it.** Windows is case-insensitive; the File System Access API is
//! **not**: `getFileHandle("Rapport.txt")` fails where NTFS would have
//! opened `rapport.txt`. This module **preserves the case** as ProjFS
//! delivered it — folding it would be worse, since the FSA would find nothing
//! at all anymore — and the defect is **documented, not masked**.
//!
//! ✅ **THE REMEDY ARRIVED IN F3, AND IT IS NOT A LOOKUP
//! TABLE.** *(These lines announced "a lookup table
//! fed by enumeration, which alone knows the real case of the disk",
//! and gave it as belonging "to F3 or later".)* F3 delivers
//! `client/src/fichiers/noms.ts`, which **enumerates the parent at EACH
//! resolution, WITHOUT ANY CACHE** — a cache nothing invalidates is the defect
//! of the old bridge (`src/file.js`, cache WITHOUT TTL). ✅ **`Rafraichir` IS
//! DELIVERED SINCE F5** (21 August 2026): a button of the shell page empties the bridge's
//! enumeration cache **and** ProjFS's negative cache.
//! ⚠️ **`noms.ts` DOES NOT BENEFIT FROM IT, and that must be said**: it still has no
//! cache, hence nothing to empty. F5's `Rafraichir` empties the
//! ENUMERATION cache, which is another object.
//!
//! ⚠️ **THIS MODULE HAS NOT CHANGED FOR ALL THAT, and it is deliberate**: it
//! still preserves the case as ProjFS delivered it. It is the BROWSER
//! that folds, because **it alone sees the local workstation**. What F3 adds
//! here is [`avec_dernier_composant`], which brings the canonical name down
//! to `PrjWritePlaceholderInfo`.
//!
//! ⚠️ **AND THE VM HALF OF THE DEFECT IS NOT REPAIRABLE**, neither here nor elsewhere:
//! when NTFS resolves the case on an ALREADY hydrated file, we are not
//! consulted. *Declaring it solved without having measured it would be exactly the
//! gesture this repository criticises in its uncalibrated constants.*
//!
//! ❌ **THIS MODULE ANNOUNCED "will therefore get `Introuvable`", AND F1'S ACCEPTANCE RUN
//! REFUTED IT: the real defect is WORSE, because it is SILENT.**
//! Measured on the VM, **three runs out of three** (`mesure-exec{1,2,5}.txt`,
//! under `docs/superpowers/plans/journaux-pont-fichiers/`): with `Casse.txt`
//! on the local workstation, `casse.txt` **and** `CASSE.TXT` both return the
//! CONTENT of `Casse.txt`, without error — the application receives the wrong
//! file and cannot know it. **And the behaviour is not consistent
//! with itself**: in the same run, `GROS.BIN` does return
//! "not found".
//!
//! ⚠️ **The mechanism is a HYPOTHESIS consistent with the evidence, not a
//! measurement.** The gap follows exactly HYDRATION: the trace
//! `racine hydratee … octets=42 entrees=1` says that a single entry of 42
//! bytes — `Casse.txt`, read by the probe just before — lived locally, and
//! NTFS, case-insensitive, then finds it **without ever reaching this
//! module**; `gros.bin`, never hydrated, falls back on the callback, which asks
//! the browser for a case it does not know. **Nothing establishes it**: it
//! would take a run where the hydration order is reversed.
//!
//! The comparison, for its part, **does fold case** where Windows does:
//! `CON.txt`, `con.txt` and `Con.TXT` all designate the same reserved
//! device, and all three are refused.

/// Why a path is refused. One cause per variant: two causes that
/// shared a variant would make the log useless the day
/// one of the two occurred in production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheminRefuse {
    /// A `..` component — the traversal spec §4.4 names.
    Remontee,
    /// An NTFS alternate data stream (`fichier.txt:Zone.Identifier`).
    FluxAlternatif,
    /// A reserved device name (`CON`, `NUL`, `COM1`…).
    NomReserve,
    /// A drive letter, a root, or a UNC path.
    Absolu,
    /// An empty component, produced by a doubled separator.
    Vide,
    /// The UTF-16 units delivered by ProjFS do not form valid text.
    NonUtf16Valide,
}

/// Win32's reserved device names.
///
/// ⚠️ The list is that of **DOS devices**, not an arbitrary security
/// list: Windows resolves them BEFORE looking at the file system, at
/// any depth, and with any extension.
const NOMS_RESERVES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Decodes the UTF-16 units of a ProjFS `PCWSTR`, then normalises.
///
/// It is not a convenience: it is the only producer of
/// [`CheminRefuse::NonUtf16Valide`]. It lives here, pure and tested on the host,
/// rather than in `pont/projfs.rs` where nothing could test it.
pub fn normaliser_utf16(unites: &[u16]) -> Result<String, CheminRefuse> {
    let texte: Result<String, _> = char::decode_utf16(unites.iter().copied()).collect();
    match texte {
        Ok(t) => normaliser(&t),
        Err(_) => Err(CheminRefuse::NonUtf16Valide),
    }
}

/// Normalises a ProjFS path into a logical path, or says why it is refused.
///
/// The empty string is **lawful**: it is the path of the root itself,
/// the one ProjFS delivers for the enumeration of the root directory.
pub fn normaliser(brut: &str) -> Result<String, CheminRefuse> {
    if brut.is_empty() {
        return Ok(String::new());
    }
    // An absolute or UNC path is never relative to the root: ProjFS does not
    // deliver any, so receiving one signals that we are not on the path
    // we think — refusal, not recovery.
    if brut.starts_with('\\') || brut.starts_with('/') || brut.chars().nth(1) == Some(':') {
        return Err(CheminRefuse::Absolu);
    }

    let mut composants = Vec::new();
    for composant in brut.split(['\\', '/']) {
        if composant.is_empty() {
            return Err(CheminRefuse::Vide);
        }
        // `.` is harmless and gets dropped; `..` NEVER gets
        // resolved. Resolving `a\..\b` into `b` would already be an error: the
        // path would then cross the root on a non-existent `a`, and
        // above all `a\..\..\x` would become indistinguishable from `x` after a
        // single resolution pass. We refuse going up, we do not compute
        // it — it is the only treatment that has no edge case.
        if composant == "." {
            continue;
        }
        if composant == ".." {
            return Err(CheminRefuse::Remontee);
        }
        if composant.contains(':') {
            return Err(CheminRefuse::FluxAlternatif);
        }
        if est_reserve(composant) {
            return Err(CheminRefuse::NomReserve);
        }
        composants.push(composant);
    }
    Ok(composants.join("/"))
}

/// Does a component designate a reserved device?
///
/// The comparison folds case and ignores the extension **and** the trailing dots and
/// spaces, exactly like Win32: `con`, `CON.txt`, `Con. ` all designate
/// the console device.
fn est_reserve(composant: &str) -> bool {
    let base = composant.split('.').next().unwrap_or(composant);
    let base = base.trim_end_matches([' ', '.']);
    NOMS_RESERVES.iter().any(|r| r.eq_ignore_ascii_case(base))
}

#[cfg(test)]
mod tests;

/// Replaces the LAST component of a ProjFS path with `nom`, keeping the
/// separators and the case of everything preceding it.
///
/// 🔴 **It is consequence ① of F3's case canonicaliser**, and it is
/// PURE to be tested on the host: `PrjWritePlaceholderInfo` must receive
/// the name **STORED on the local workstation**, never the one the application typed.
///
/// Without it, a `GROS.BIN` requested on a local `gros.bin` would create a
/// placeholder named `GROS.BIN` in the root. The root being NTFS — hence
/// case-insensitive —, the opening would succeed; but **an enumeration of the
/// parent would return `gros.bin`**: two names for one file, one of which
/// exists nowhere.
///
/// ⚠️ **ProjFS's separator is `\`**, never `/`: it is not the normalised
/// logical path, it is the one the system delivered and will take back as
/// is.
///
/// Returns `None` when there is nothing to change — empty path, or last
/// component already equal to `nom`. **The caller then keeps the original bytes**,
/// which preserves the property F1 had given itself: not reconverting a
/// path we have no reason to touch.
pub fn avec_dernier_composant(chemin_projfs: &str, nom: &str) -> Option<String> {
    if chemin_projfs.is_empty() || nom.is_empty() {
        return None;
    }
    match chemin_projfs.rfind('\\') {
        Some(i) => {
            if &chemin_projfs[i + 1..] == nom {
                return None;
            }
            Some(format!("{}{}", &chemin_projfs[..=i], nom))
        }
        None => {
            if chemin_projfs == nom {
                return None;
            }
            Some(nom.to_string())
        }
    }
}
