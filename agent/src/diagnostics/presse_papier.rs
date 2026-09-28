//! P0 probe of sub-block P1 (clipboard): does the sequence counter of the
//! Windows clipboard behave as the documentation announces?
//!
//! The specification (§8) declares this point **not measured**: it relies on
//! the documentation of `GetClipboardSequenceNumber`, the VM having been busy
//! when it was written. This probe takes the measurement, and it is an
//! **elimination gate**: three of its five verdicts make the retained detection
//! mechanism (D2) not deliverable as is.
//!
//! ⚠️ **IT WRITES THE VM'S CLIPBOARD, and therefore destroys it.** Phases
//! C and D set `SetClipboardData(CF_UNICODETEXT)` to answer
//! questions Q3 and Q2. It is a PROBE gesture: **the product, for its part, never
//! writes the clipboard in P1** — it only reads it. Do not
//! confuse the two (decision D-P1-7 of the plan).
//!
//! ⚠️ **It NEVER logs the clipboard's text** — a truncated fingerprint
//! and a length, never the content. It is a private resource of
//! the VM's user, and a log committed to git is public to the repository.
//!
//! ⚠️ **It runs ALONE**, without `SUPERVISEUR`: see the comment of the
//! routing arm in `diagnostics.rs` (divergence E10 of the plan).
//!
//! No verdict is judged on a return code. Phase C judges on the
//! **movement of the counter read back**, never on the success of
//! `SetClipboardData` — it is the doctrine this repository adopted after
//! a `ChangeDisplaySettingsExW` returned `0` on an output that had
//! not moved by one pixel (sub-block D8).

use anyhow::Result;

/// Number of phase A readings, and their spacing.
const RELEVES_REPOS: usize = 3;
const PAS_REPOS: std::time::Duration = std::time::Duration::from_millis(250);
/// Phase B cadence: 4 Hz, the same as `PERIODE_PRESSE_PAPIER`.
const PAS_OBSERVATION: std::time::Duration = std::time::Duration::from_millis(250);

/// Fingerprint of a text: SHA-1 truncated to 12 hexadecimal characters.
///
/// ⚠️ **SHA-1 and not SHA-256, unlike the plan (task 1)**, and it is
/// deliberate: `sha1` is already a dependency of this crate (TURN lease),
/// `sha2` is not, and the plan states at the top that **P1 adds no
/// dependency**. The property sought is "distinguish two texts without
/// ever writing either of them", not cryptographic resistance: both
/// functions provide it equally.
fn empreinte(texte: &str) -> String {
    use sha1::{Digest, Sha1};
    let condense = Sha1::digest(texte.as_bytes());
    condense
        .iter()
        .take(6)
        .map(|octet| format!("{octet:02x}"))
        .collect()
}

/// What the probe read from the clipboard at a given instant.
struct Lecture {
    octets_utf16: usize,
    octets_utf8: usize,
    empreinte: String,
}

/// Runs the probe for `secondes` of phase B.
pub fn executer(secondes_texte: &str) -> Result<()> {
    let secondes: u64 = secondes_texte.trim().parse().unwrap_or(40);

    tracing::info!(
        secondes,
        "P0 START clipboard probe — it WRITES the VM clipboard (phases C and D)"
    );

    // ---- Phase A: does the counter exist, and is it STABLE at rest? ----
    let mut repos = Vec::with_capacity(RELEVES_REPOS);
    for i in 0..RELEVES_REPOS {
        if i > 0 {
            std::thread::sleep(PAS_REPOS);
        }
        let seq = win::numero_de_sequence();
        tracing::info!(releve = i, seq, "P0 A at rest");
        repos.push(seq);
    }
    let stable_au_repos = repos.windows(2).all(|paire| paire[0] == paire[1]);

    // ---- Phase A-bis: DISAMBIGUATE a zero, never conclude from it ----
    //
    // 🔴 **Three zeros have TWO causes, and the first version of this probe
    // confused them — it returned a WRONG elimination verdict on a
    // perfectly healthy system** (run no. 1 of 20 August 2026, log
    // `p0-sonde-1.log`, kept for that reason):
    //
    // 1. `GetClipboardSequenceNumber` FAILED — the process does not have
    //    `WINSTA_ACCESSCLIPBOARD` access on its window station. The
    //    documentation only provides for this case, and it is what the first
    //    version assumed.
    // 2. The counter EXISTS and is really zero, because **nothing has
    //    ever been copied since the window station started**.
    //    Measured: the VM had just been started, the probe read `0, 0, 0` and
    //    declared `P0 NON MESURABLE`; five copies later, the same probe
    //    on the same station read **53**, stable, and all phases
    //    passed. The zero was the counter, not its absence.
    //
    // So we do not conclude: we write the clipboard ourselves — the
    // gesture of phase C, played here as a disambiguator — and read back. If it
    // moves, the counter exists and the following phases make sense. **A
    // negative verdict requires the measured thing to be ABSENT, not
    // merely zero**; it is the mirror image of D9's trap (`survit=true`
    // returned by a vanished output).
    if repos.iter().all(|&s| s == 0) {
        tracing::info!(
            "P0 A-bis: three zeros — the clipboard is written to tell \
             \"counter absent\" apart from \"nothing copied yet\""
        );
        let _ = win::write_text("sonde-presse-papier-desambiguisation");
        std::thread::sleep(PAS_REPOS);
        let apres = win::numero_de_sequence();
        tracing::info!(apres, "P0 A-bis reading after our write");
        if apres != 0 {
            // The counter exists: the station was simply blank. We
            // start again from this reading, which is the real reference state.
            repos = vec![apres; RELEVES_REPOS];
        }
    }
    let compteur_absent = repos.iter().all(|&s| s == 0);
    let q1 = if compteur_absent {
        "NOT-MEASURABLE-counter-absent"
    } else if stable_au_repos {
        "stable"
    } else {
        "UNSTABLE-at-rest"
    };
    tracing::info!(
        q1,
        seq_min = repos.iter().min(),
        seq_max = repos.iter().max(),
        "P0 A summary"
    );

    if compteur_absent {
        tracing::warn!(
            "P0 NOT MEASURABLE: GetClipboardSequenceNumber returns 0 on the three readings AND \
             after a write by the probe itself — so the process does not have \
             WINSTA_ACCESSCLIPBOARD. Phases B to D cannot measure anything, they \
             are skipped."
        );
        tracing::info!(
            q1,
            q1bis = "NON-MESUREE",
            q2 = "NON-MESUREE",
            q3 = "NON-MESUREE",
            echecs_open = 0,
            tentatives_open = 0,
            seq_debut = 0,
            seq_fin = 0,
            "P0 BILAN"
        );
        return Ok(());
    }

    // ---- Phase B: does a copy made by hand make the counter move? ----
    let seq_debut = *repos.last().expect("RELEVES_REPOS > 0");
    let mut reference = seq_debut;
    let mut mouvements = 0usize;
    let mut lectures = 0usize;
    let mut echecs_open = 0usize;
    let mut tentatives_open = 0usize;
    tracing::info!(
        secondes,
        seq_debut,
        "P0 B start — COPY NOW three distinct texts by hand in the VM, \
         then COPY the third one AGAIN identically"
    );
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(secondes);
    while std::time::Instant::now() < fin {
        std::thread::sleep(PAS_OBSERVATION);
        let seq = win::numero_de_sequence();
        if seq == reference {
            continue;
        }
        mouvements += 1;
        tentatives_open += 1;
        match win::lire_texte() {
            Ok(Some(texte)) => {
                lectures += 1;
                let lecture = mesurer(&texte);
                tracing::info!(
                    seq,
                    precedent = reference,
                    octets_utf16 = lecture.octets_utf16,
                    octets_utf8 = lecture.octets_utf8,
                    empreinte = lecture.empreinte,
                    "P0 B movement, text read"
                );
            }
            Ok(None) => {
                tracing::info!(
                    seq,
                    precedent = reference,
                    "P0 B movement, no CF_UNICODETEXT"
                );
            }
            Err(error) => {
                echecs_open += 1;
                tracing::warn!(seq, precedent = reference, %error, "P0 B movement, opening refused");
            }
        }
        reference = seq;
    }
    let q1bis = if mouvements == 0 {
        "AUCUN-MOUVEMENT"
    } else {
        "moved"
    };
    tracing::info!(q1bis, mouvements, lectures, echecs_open, "P0 B summary");

    // ---- Phase C: does our OWN write make the counter move? ----
    let nonce = format!("{:x}", std::process::id());
    let notre_texte = format!("sonde-presse-papier-{nonce}");
    let before_c = win::numero_de_sequence();
    let written_c = win::write_text(&notre_texte);
    std::thread::sleep(PAS_REPOS);
    let apres_c = win::numero_de_sequence();
    // We judge on the movement read back, never on `written_c`.
    let q3 = if apres_c != before_c {
        "moved"
    } else {
        "NO-MOVEMENT"
    };
    tracing::info!(
        q3,
        before = before_c,
        apres = apres_c,
        api_annonce_succes = written_c.is_ok(),
        empreinte = empreinte(&notre_texte),
        octets = notre_texte.len(),
        "P0 C our write"
    );

    // ---- Phase D: does an IDENTICAL rewrite make the counter move? ----
    let before_d = win::numero_de_sequence();
    let written_d = win::write_text(&notre_texte);
    std::thread::sleep(PAS_REPOS);
    let apres_d = win::numero_de_sequence();
    let q2 = if apres_d != before_d {
        "moved"
    } else {
        "NO-MOVEMENT"
    };
    tracing::info!(
        q2,
        before = before_d,
        apres = apres_d,
        api_annonce_succes = written_d.is_ok(),
        "P0 D identical rewrite"
    );

    // ---- Phase E : le bilan ----
    tracing::info!(
        q1,
        q1bis,
        q2,
        q3,
        mouvements,
        lectures,
        echecs_open,
        tentatives_open,
        seq_debut,
        seq_fin = apres_d,
        "P0 BILAN"
    );
    Ok(())
}

/// What we measure of a text read — never the text itself.
fn mesurer(texte: &str) -> Lecture {
    Lecture {
        octets_utf16: texte.encode_utf16().count() * 2,
        octets_utf8: texte.len(),
        empreinte: empreinte(texte),
    }
}

mod win {
    use anyhow::{Context, Result};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    pub fn numero_de_sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Ouvre le presse-papier, lit `CF_UNICODETEXT`, referme.
    ///
    /// `Ok(None)` = the clipboard carries no Unicode text (an image,
    /// for example); `Err` = opening was refused, which is NORMAL
    /// under Windows (another application holds it) and not a failure.
    pub fn lire_texte() -> Result<Option<String>> {
        unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
        let result = (|| unsafe {
            let poignee = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
                Ok(poignee) if !poignee.is_invalid() => poignee,
                _ => return Ok(None),
            };
            let global = HGLOBAL(poignee.0);
            let pointeur = GlobalLock(global) as *const u16;
            if pointeur.is_null() {
                anyhow::bail!("GlobalLock returned a null pointer");
            }
            let mut length = 0usize;
            while *pointeur.add(length) != 0 {
                length += 1;
            }
            let unites = std::slice::from_raw_parts(pointeur, length);
            let texte = String::from_utf16_lossy(unites);
            let _ = GlobalUnlock(global);
            Ok(Some(texte))
        })();
        let _ = unsafe { CloseClipboard() };
        result
    }

    /// Writes `texte` to the clipboard — **probe gesture only**.
    pub fn write_text(texte: &str) -> Result<()> {
        let mut unites: Vec<u16> = texte.encode_utf16().collect();
        unites.push(0);
        unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
        let result = (|| unsafe {
            EmptyClipboard().context("EmptyClipboard")?;
            let octets = unites.len() * std::mem::size_of::<u16>();
            let global = GlobalAlloc(GMEM_MOVEABLE, octets).context("GlobalAlloc")?;
            let pointeur = GlobalLock(global) as *mut u16;
            if pointeur.is_null() {
                anyhow::bail!("GlobalLock returned a null pointer");
            }
            std::ptr::copy_nonoverlapping(unites.as_ptr(), pointeur, unites.len());
            let _ = GlobalUnlock(global);
            // The clipboard takes ownership of the block: do not free it.
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0)))
                .context("SetClipboardData")?;
            Ok(())
        })();
        let _ = unsafe { CloseClipboard() };
        result
    }
}
