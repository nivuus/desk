//! What the write thread does with the DISK: resolve a path, read a
//! chunk, append a line to the journal, compact it.
//!
//! **Extracted from [`super`] at 494 lines for a gate of 500, margin 6.**
//!
//! ⚠️ **The ceiling was NOT crossed, and this extraction is therefore not a
//! catch-up** — but a margin of six is untenable: sub-block **F3**
//! must touch this file (its two interleaving rules — drain before a
//! renaming, forget on a deletion), and this repository paid four times for the
//! lesson "the margin regained by an extraction is lost again the next round
//! if treated as acquired". **Never a compression**, which
//! `CLAUDE.md` forbids by name and which D9 had to undo twice.
//!
//! **The split is by NATURE, not by size**: the state machine stays
//! in [`super`], everything that touches a file system comes here. It is
//! the same gesture as `service/verbes.rs`, which carries the `unsafe` where
//! `service.rs` carries the loop.
//!
//! 🔵 **This whole file is PURE**: ordinary `std::fs`, portable, exercised
//! on the host by the tests of [`super`]. After
//! `FILE_HANDLE_CLOSED_FILE_MODIFIED`, the file is **complete** in the
//! root — ProjFS only calls `GetFileData` on a **placeholder**.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::pont::decoupe::Morceau;
use crate::pont::journal::Journal;

/// Le chemin local d'un chemin logique du protocole.
///
/// ⚠️ **Component by component, never by `join` of a whole string**: a
/// logical path carries `/`s, which `Path::join` would interpret as an
/// ABSOLUTE path on a `/foo`. The normalisation in `pont::chemins` has already
/// refused `..` and `:` upstream — it is the barrier, not this.
pub(super) fn local(racine: &Path, chemin: &str) -> PathBuf {
    let mut local = racine.to_path_buf();
    for composant in chemin.split('/').filter(|c| !c.is_empty()) {
        local.push(composant);
    }
    local
}

/// The size of the local file. **Zero for a directory**, which has no
/// byte to push, and zero for an absent path — the thread will notice at
/// read time, where the error can be named.
pub(super) fn taille_de(racine: &Path, chemin: &str) -> u64 {
    std::fs::metadata(local(racine, chemin))
        .map(|m| if m.is_dir() { 0 } else { m.len() })
        .unwrap_or(0)
}

/// Reads a chunk of the local file.
pub(super) fn lire(racine: &Path, chemin: &str, morceau: Morceau) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    if morceau.longueur == 0 {
        // An empty file: nothing to read, and the stream closes on a chunk
        // without bytes. `File::open` would still fail if the file had
        // disappeared in the meantime, which we want to know.
        std::fs::File::open(local(racine, chemin))?;
        return Ok(Vec::new());
    }
    let mut fichier = std::fs::File::open(local(racine, chemin))?;
    fichier.seek(SeekFrom::Start(morceau.position))?;
    let mut tampon = vec![0u8; morceau.longueur as usize];
    let lus = fichier.read(&mut tampon)?;
    // ⚠️ **The ANNOUNCED length must be the one ACTUALLY read.** The file may
    // have shrunk between the `metadata` and the read; announcing the request
    // would make the header diverge from the payload, and the browser would write
    // padding zeros.
    tampon.truncate(lus);
    Ok(tampon)
}

/// Appends a line to the journal, **and flushes it to disk**.
///
/// ⚠️ **`sync_all` and not a mere `write`**: a line left in the system's
/// cache does not survive an abrupt stop, and that is exactly the case
/// this journal exists to cover.
pub(super) fn ajouter(chemin_journal: &Path, ligne: &str) -> std::io::Result<()> {
    let mut fichier = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(chemin_journal)?;
    fichier.write_all(ligne.as_bytes())?;
    fichier.sync_all()
}

/// Truncates the journal **if it is empty** and has grown.
///
/// 🔴 **`compactable()` tests `est_vide()` AND the size, never the size
/// alone.** Truncating a file that still carries a due entry would lose the data
/// **exactly when it matters**: a large journal is a journal where many
/// writes failed.
pub(super) fn compacter_si_possible(chemin_journal: &Path, journal: &Journal) {
    let Ok(meta) = std::fs::metadata(chemin_journal) else {
        return;
    };
    if !journal.compactable(meta.len()) {
        return;
    }
    if let Err(erreur) = std::fs::write(chemin_journal, b"") {
        tracing::warn!(%erreur, "compactage du journal des ecritures echoue");
    } else {
        tracing::info!(
            octets = meta.len(),
            "journal des ecritures compacte (aucune due)"
        );
    }
}
