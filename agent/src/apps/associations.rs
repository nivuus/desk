//! The file associations of an application, read on the VM.
//!
//! 🔴 WHAT IS PURE LIVES HERE; ONLY THE REGISTRY READ IS
//! `#[cfg(windows)]` (`associations/registre.rs`). It is the split G2
//! established twice — `apps/icone/ressource.rs`, `apps/installation` — and its
//! benefit is the same: **without it, the part that DECIDES would have no
//! host test**, and the only way to test it would be to rebuild a
//! binary for the VM.
//!
//! 🔴 MATCHING IS DONE BY PATH IDENTITY, NEVER BY NAME
//! SUBSTRING (decision D12 of the G5 plan). The model the design cites,
//! `src/app.js:15-37`, matches the ProgID to the application **by substring on
//! its name**, after stripping digits. It is **the same heuristic
//! G1 already replaced for identifiers, and for the same reason**:
//! "Nsight 2020.3" and "Nsight 2024.6" produced the same key there. Taking
//! it up here would attribute `.cu` to the wrong version of one of them, and
//! nobody would see it.
//!
//! The chain, for one extension:
//!   ① `HKCU\…\FileExts\<ext>\UserChoice\ProgId` — THE USER'S REAL
//!      CHOICE, and it wins;
//!   ② failing that, the default value of `HKCR\.<ext>`;
//!   ③ then `HKCR\<ProgID>\shell\open\command`, from which the path of the
//!      executable is EXTRACTED;
//!   ④ which is NORMALISED with the normalisation already written
//!      (`raccourci::normaliser_chemin`) — never a second one —, and which is
//!      compared with `application.cible`.

#[cfg(windows)]
pub mod registre;

#[cfg(test)]
#[path = "associations/tests.rs"]
mod tests;

use crate::apps::raccourci::normaliser_chemin;

/// The association table of THIS machine, or an EMPTY table outside Windows.
///
/// 🔴 A SINGLE ENTRY POINT FOR THE PRODUCT, AND IT COMPILES EVERYWHERE. The
/// `#[cfg]` lives here and nowhere else: the caller does not need to know which
/// system it runs on, and the discovery loop stays readable on the
/// host as on the VM.
pub fn table_de_la_machine() -> std::collections::BTreeMap<String, Vec<String>> {
    #[cfg(windows)]
    {
        table(registre::couples())
    }
    #[cfg(not(windows))]
    {
        // ⚠️ EMPTY, NOT A PANIC: the agent compiles on the host for its
        // tests, and an empty table is the truth there — that machine has no
        // Windows registry.
        std::collections::BTreeMap::new()
    }
}

/// Extracts the path of the executable from a Windows command line.
///
/// 🔴 THE RULE IS WINDOWS'S OWN, NOT A CONVENIENT APPROXIMATION: if the
/// line starts with a quote, the path runs **up to the closing
/// quote** and may therefore contain spaces; otherwise it runs **up to the
/// first space**. That is what distinguishes
/// `"C:\Program Files\App\a.exe" "%1"` — a path with spaces — from
/// `C:\Windows\notepad.exe %1`.
///
/// ⚠️ AN UNQUOTED LINE WHOSE PATH CONTAINS A SPACE IS THEREFORE TRUNCATED, and
/// that is **Windows's own behaviour**, not a gap here: the
/// system then tries several splits. We do not try them — a
/// badly matched association would be worse than a missing one, and
/// silence is the honest answer here.
///
/// Returns `None` on an empty line, or on an opening quote never closed.
pub fn executable_de_commande(commande: &str) -> Option<String> {
    let size = commande.trim();
    if size.is_empty() {
        return None;
    }
    let chemin = if let Some(reste) = size.strip_prefix('"') {
        // ⚠️ An unclosed opening quote is refused rather than taking
        // all the rest: a malformed line is not a path.
        reste.split_once('"').map(|(before, _)| before)?
    } else {
        size.split_whitespace().next()?
    };
    if chemin.is_empty() {
        return None;
    }
    Some(chemin.to_string())
}

/// Does the command line of a ProgID designate THIS target?
///
/// 🔴 AN EQUALITY OF NORMALISED PATHS, NEVER A NAME SUBSTRING.
/// `normaliser_chemin` is REUSED and not copied: two normalisations
/// would diverge the day one of them changed, and matching
/// would become wrong **on the one side that had not moved**.
#[cfg(test)]
pub fn commande_vise(commande: &str, cible: &str) -> bool {
    match executable_de_commande(commande) {
        None => false,
        Some(exe) => normaliser_chemin(&exe) == normaliser_chemin(cible),
    }
}

/// Normalises an extension: lowercase, **with** the leading dot.
///
/// ⚠️ THE DOT IS ENFORCED HERE, AND THAT IS WHAT MAKES THE WIRE FORMAT
/// PREDICTABLE: the registry writes `.txt` under `HKCR` and `txt` under `FileExts`
/// depending on the key, and letting both forms travel would force the platform
/// to choose — that is, to carry a rule that is not its own.
pub fn normaliser_extension(extension: &str) -> Option<String> {
    let size = extension.trim().trim_start_matches('.').to_lowercase();
    if size.is_empty() || size.contains(['\\', '/', ' ']) {
        return None;
    }
    Some(format!(".{size}"))
}

/// The association TABLE: **normalised** executable path → sorted
/// extensions.
///
/// 🔴 A TABLE, AND NOT A QUERY PER APPLICATION, AND IT IS A COST
/// DECISION. The VM's corpus has **156 applications**; asking
/// the registry, for each one, which extensions point at it, would re-read
/// every entry of `FileExts` **156 times per reconciliation** — and the
/// reconciliation runs every `PERIODE_RECONCILIATION`. The registry is
/// therefore read **once**, and each application **looks up** its path in it.
///
/// 🔴 AND IT IS ALSO WHAT MAKES THE RULE TESTABLE: what comes in is a
/// list of `(extension, command line)` pairs — exactly what a
/// registry returns —, and all the rest (extracting the executable, normalising,
/// grouping, sorting) is **pure** and lives here.
///
/// ⚠️ AN UNREADABLE COMMAND LINE IS DISCARDED SILENTLY, and that is
/// deliberate: a ProgID whose command cannot be read designates no
/// application, and the only other choice would be to attribute it at random.
pub fn table(couples: Vec<(String, String)>) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut brute: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (extension, commande) in couples {
        let Some(exe) = executable_de_commande(&commande) else {
            continue;
        };
        let Some(ext) = normaliser_extension(&extension) else {
            continue;
        };
        brute.entry(normaliser_chemin(&exe)).or_default().push(ext);
    }
    brute
        .into_iter()
        .map(|(exe, exts)| (exe, ranger(exts)))
        .collect()
}

/// What the table keeps for a target — the EMPTY list if it is not there.
///
/// ⚠️ THE TARGET IS NORMALISED HERE TOO, and by the SAME function: the table is
/// built on normalised paths, and querying it with a raw path would
/// never find anything — a **silent** defect, which would simply make
/// every list empty.
pub fn pour_cible(
    table: &std::collections::BTreeMap<String, Vec<String>>,
    cible: &str,
) -> Vec<String> {
    table
        .get(&normaliser_chemin(cible))
        .cloned()
        .unwrap_or_default()
}

/// Sorts and deduplicates the extensions of an application.
///
/// 🔴 THE ORDER IS ENFORCED, AND IT IS NOT AN ORNAMENT: the platform compares
/// the received catalogue with the one it knows to decide what it writes.
/// A registry enumeration order — guaranteed by nothing — would make
/// two IDENTICAL lists diverge, hence write on every tick and log
/// a change that did not happen.
pub fn ranger(extensions: Vec<String>) -> Vec<String> {
    let mut rangees: Vec<String> = extensions
        .iter()
        .filter_map(|e| normaliser_extension(e))
        .collect();
    rangees.sort();
    rangees.dedup();
    rangees
}
