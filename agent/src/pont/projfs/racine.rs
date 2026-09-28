//! The virtualisation root: where it lives, and how it is marked
//! **only once**.
//!
//! Extracted from [`super`] **before** task 14's addition, and not after:
//! `projfs.rs` was at 498 lines, margin 2, and this repository paid four times for the
//! lesson "the margin regained by an extraction is lost again the next round
//! if treated as acquired". The two files sub-block D9
//! handled AFTER the fact were **compressed**, a gesture `CLAUDE.md` forbids
//! by name, then extracted anyway.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use windows::core::{GUID, PCWSTR};

use super::chargement;

/// The root folder's name, in the user's profile.
///
/// **ProjFS virtualises a DIRECTORY, never a volume**: the "Mes
/// Fichiers drive" is a usage name, not a letter (decision D6). No letter
/// is assigned — `subst`/`DefineDosDevice` would have a logon session
/// lifetime, hence one more mechanism to watch, to set again after
/// a restart and to remove cleanly, **for zero gain**: a profile folder
/// is browsed from an `IFileOpenDialog` exactly like a volume, and
/// it appears in Explorer's navigation pane with no work.
const NOM_RACINE: &str = "Mes Fichiers";

/// Where the instance GUID is persisted, **outside the root**.
///
/// ⚠️ **Constraint 3 of spec §3.6, and it is not a tidiness
/// preference**: a state that lived in the root would itself be a projected
/// object — hence dependent on the bridge to be read, which is **circular** —
/// and it would disappear with the root the day it had to be recreated,
/// that is, **exactly the day it matters**.
const STATE_SUBFOLDER: &str = r"Guacamole\pont";
const GUID_FILE: &str = "instance.guid";

/// `%USERPROFILE%\Mes Fichiers`.
pub(super) fn racine() -> Result<PathBuf> {
    let profil = std::env::var("USERPROFILE")
        .context("USERPROFILE absent : impossible de situer la racine du pont fichiers")?;
    Ok(PathBuf::from(profil).join(NOM_RACINE))
}

/// `%LOCALAPPDATA%\Guacamole\pont`.
///
/// ⚠️ **`pub` since F2**: the **due writes journal** lives there
/// too, for the reason written at the head of [`STATE_SUBFOLDER`] — a state that
/// lived IN the root would itself be a projected object, hence dependent on the
/// bridge to be read, and it would disappear with the root **exactly the day
/// it matters**.
pub fn dossier_etat() -> Result<PathBuf> {
    let local = std::env::var("LOCALAPPDATA")
        .context("LOCALAPPDATA absent : impossible de situer l'état du pont fichiers")?;
    Ok(PathBuf::from(local).join(STATE_SUBFOLDER))
}

/// Creates the root if needed, and marks it **only once**.
///
/// ⚠️ **The THREE constraints of spec §3.6, and none is decorative:**
///
/// 1. **mark only once** — re-marking an already marked root
///    **fails**. The instance GUID is therefore persisted, and its PRESENCE is what
///    says the root has already been marked;
/// 2. **the root must contain no data at marking time** — an
///    explicit refusal beats an `HRESULT` no one will know how to read;
/// 3. **nothing of our state lives in the root** (see [`STATE_SUBFOLDER`]).
pub(super) fn preparer(projfs: &chargement::ProjFs, racine: &Path) -> Result<()> {
    std::fs::create_dir_all(racine)
        .with_context(|| format!("création de la racine « {} »", racine.display()))?;
    let etat = dossier_etat()?;
    let empreinte = etat.join(GUID_FILE);

    let deja_marquee = empreinte.exists();
    let guid = match std::fs::read_to_string(&empreinte) {
        Ok(texte) => lire_guid(texte.trim()).with_context(|| {
            format!(
                "« {} » ne porte pas un GUID lisible : le retirer À LA MAIN ferait re-marquer \
                 une racine déjà marquée, ce que ProjFS refuse — il faut retirer la RACINE aussi",
                empreinte.display()
            )
        })?,
        // SAFETY: `CoCreateGuid` has no precondition and takes no
        // pointer; it can only fail for lack of resources.
        Err(_) => unsafe { windows::Win32::System::Com::CoCreateGuid() }
            .context("génération du GUID d'instance de la racine")?,
    };

    if !deja_marquee {
        // Constraint 2. Counted rather than assumed: a root inherited from an
        // earlier run whose fingerprint was lost probably contains
        // hydrated files, and marking would fail with
        // a code no one would be able to link to this cause.
        let entrees = std::fs::read_dir(racine)
            .with_context(|| format!("lecture de la racine « {} »", racine.display()))?
            .count();
        if entrees != 0 {
            bail!(
                "la racine « {}» contient {entrees} entrée(s) : ProjFS refuse de marquer un \
                 répertoire non vide, et « {} » n'existe pas — vider la racine, ou restaurer \
                 l'empreinte de l'instance qui l'a marquée",
                racine.display(),
                empreinte.display()
            );
        }
    }

    let chemin = utf16(racine);
    // SAFETY: `chemin` is valid and null-terminated; the two optional
    // pointers are null (no target path, no version
    // information); `guid` lives until the end of the function. Signature:
    // transcription of the `link!` at `mod.rs:93`.
    let issue = unsafe {
        (projfs.marquer_racine)(
            PCWSTR(chemin.as_ptr()),
            PCWSTR::null(),
            std::ptr::null(),
            &guid,
        )
    };
    if issue.is_err() {
        // ⚠️ **We do NOT test a particular `HRESULT`, and it is deliberate**:
        // the exact code ProjFS returns for "already marked" has been measured
        // on NO machine of this repository, and inventing it would do exactly
        // what this repository reproaches its unrecorded claims for.
        //
        // ❌ **"What is known is that re-marking FAILS" WAS WRONG, AND
        // F5 MEASURED IT (August 21st, 2026, gate P2).** On this VM,
        // `PrjMarkDirectoryAsPlaceholder` **SUCCEEDS** on an already marked
        // root whose fingerprint exists: three runs, one of which
        // (`p2-dues`) without purge and **without a restart in between**, all
        // three going through the success branch.
        //
        // 🔴 **TWO CONSEQUENCES, and the second cost a measurement**:
        //
        // 1. **the branch below has NEVER run** — it is tolerant and
        //    harmless, but it is not exercised, and it must not be
        //    read as an exercised path;
        // 2. **the success trace said "marked for the FIRST time" even
        //    when the root was already marked**, since it is the `else`
        //    of a call that always succeeds. F5's gate P2 asked it
        //    whether the marking had survived a restart: *it returned the
        //    same sentence in all three cases*, so it could not
        //    answer. It now says what it KNOWS — whether a fingerprint
        //    already existed — rather than what it assumes.
        //
        // ⚠️ **The behaviour is NOT changed**: tolerating the failure when
        // the fingerprint exists stays right, and making it fatal on the strength of
        // three runs on ONE machine would be exactly the opposite of what
        // this paragraph reproaches.
        if deja_marquee {
            tracing::info!(
                %issue,
                racine = %racine.display(),
                "marquage refusé sur une racine dont l'empreinte existe déjà : \
                 tenue pour déjà marquée (le code exact de « déjà marquée » n'est \
                 mesuré sur aucune machine de ce dépôt, donc il n'est pas testé)"
            );
        } else {
            bail!(
                "PrjMarkDirectoryAsPlaceholder sur « {} » : {issue}",
                racine.display()
            );
        }
    } else {
        std::fs::create_dir_all(&etat)
            .with_context(|| format!("création de « {} »", etat.display()))?;
        // Written AFTER marking, never before: a fingerprint set on a
        // marking that fails would make a root count as marked when it
        // is not, and the next startup would fail with no readable cause.
        std::fs::write(&empreinte, format!("{guid:?}"))
            .with_context(|| format!("écriture de « {} »", empreinte.display()))?;
        // ⚠️ **`deja_marquee` IS REPORTED, and it is what makes this trace
        // able to answer a question.** It asserted "first
        // time" without knowing; it now returns the observation it
        // rests on, and the interpretation stays with the reader.
        tracing::info!(
            racine = %racine.display(),
            empreinte_preexistante = deja_marquee,
            "racine marquée (le marquage RÉUSSIT même sur une racine déjà marquée : mesuré, F5 P2)"
        );
    }
    Ok(())
}

/// Rereads the GUID written by [`preparer`], in the `Debug` format of
/// `windows::core::GUID`.
fn lire_guid(texte: &str) -> Result<GUID> {
    GUID::try_from(texte).with_context(|| format!("GUID « {texte} » illisible"))
}

/// A null-terminated UTF-16 string, for a `PCWSTR`.
pub(super) fn utf16(chemin: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    chemin
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
