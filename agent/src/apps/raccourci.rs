//! What a Windows shortcut becomes, and what makes it enter the catalogue.
//!
//! 🔴 THIS MODULE IS PURE: no `#[cfg]`, no COM object, no file system
//! access. That is what lets it be judged on the Linux host
//! against `agent/testdata/gapps-corpus-vm.json`, the 218 real shortcuts of
//! the VM. The COM read lives in `apps::lecture`, which is `#[cfg(windows)]`.
//!
//! ⚠️ THE TARGET'S EXISTENCE IS AN INJECTED PREDICATE, and that is the only
//! reason the filtering rule is testable here: written with a
//! hardcoded `Path::exists()`, it would return `false` for the 167 targets of the
//! corpus — none exists on the host — and the test would be inert while
//! staying green.

use proto::plateforme::{Application, SourceMax};

use super::sha256;

/// A shortcut as the Shell returns it, before any decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brut {
    /// The name of the `.lnk`, without its extension.
    pub nom: String,
    /// The path of the `.lnk` itself.
    pub chemin: String,
    pub cible: String,
    pub arguments: String,
    pub repertoire: String,
}

/// Why a shortcut does not enter the catalogue.
///
/// ⚠️ THREE REASONS, AND NONE IS A FILTER BY NAME OR BY PATH. An
/// "Uninstall" pattern depends on the language — this VM is in French, and one of its
/// uninstallers is called `maintenancetool.exe` — and it would silently
/// discard legitimate applications. A system-path filter would lose
/// Notepad and Paint, whose targets are two of the 63 under `C:\Windows`.
/// The 15 uninstallers therefore enter the catalogue, and it is the user who
/// hides them, through an explicit and reversible gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ecart {
    /// Shell namespace target (PIDL), without a file path —
    /// `Paramètres Windows`, `Ce PC`, the Recycle Bin. **This is not an
    /// error**: 7 of the VM's 218 shortcuts are in that case.
    CibleVide,
    /// The observed extension, folded to lowercase. Empty if the target
    /// carries none.
    Extension(String),
    /// The target is named but the file is not there — 3 of the VM's 170 `.exe`,
    /// all under a Python 3.13 installation that has vanished.
    CibleAbsente,
}

/// The only target extension that enters the catalogue in v1.
///
/// ⚠️ IT IS AN ALLOW LIST, NOT A DENY LIST, and the difference
/// is about the extensions we have not seen: a deny list would
/// let them all in. `.msc` and `.url` are launchable and
/// deliberately excluded — `.msc` opens an MMC console whose window is
/// that of `mmc.exe`, which work stream D's window attachment never
/// tested; `.url` opens the default browser, that is an
/// application that is not the one you think you are launching. Out of v1 scope,
/// named and not forgotten.
const EXTENSIONS_RETENUES: &[&str] = &["exe"];

/// The extension of the last component of a Windows path, folded to
/// lowercase. Empty if the file name carries none.
fn extension_de(chemin: &str) -> String {
    let fichier = chemin.rsplit(['\\', '/']).next().unwrap_or(chemin);
    match fichier.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    }
}

/// The filtering rule, in the order the spec writes it.
///
/// `existe` is INJECTED: see the header of this module.
pub fn retenir(brut: &Brut, existe: &dyn Fn(&str) -> bool) -> Result<(), Ecart> {
    if brut.cible.trim().is_empty() {
        return Err(Ecart::CibleVide);
    }
    let extension = extension_de(&brut.cible);
    if !EXTENSIONS_RETENUES.contains(&extension.as_str()) {
        return Err(Ecart::Extension(extension));
    }
    if !existe(&brut.cible) {
        return Err(Ecart::CibleAbsente);
    }
    Ok(())
}

/// Folds a Windows path for comparison: edge whitespace removed,
/// case folded, final backslash removed.
///
/// ⚠️ CASE IS FOLDED BECAUSE NTFS IS CASE-INSENSITIVE: two
/// paths that differ only by it designate the same file, and treating them
/// as two applications would duplicate the catalogue according to the case
/// an installer wrote into its `.lnk`.
///
/// ⚠️ THE FINAL SLASH IS REMOVED EXCEPT ON A ROOT (`c:\`), which is not
/// a directory without it. 25 of the VM's 167 working directories
/// carry one; **removing it changes NEITHER of the two key counts**
/// (measured: 154 and 104 in both cases). It is therefore not a fix
/// but a precaution, and it is written so that nobody believes it measured
/// necessary.
pub fn normaliser_chemin(chemin: &str) -> String {
    let taille = chemin.trim().to_lowercase();
    if taille.len() > 3 && taille.ends_with('\\') {
        taille.trim_end_matches('\\').to_string()
    } else {
        taille
    }
}

/// The identity of an application: the fingerprint of the triple.
///
/// 🔴 THE ARGUMENTS ARE RAW, THE TARGET AND DIRECTORY ARE NORMALISED, and
/// it is the figure that imposes it: out of the VM's 167 kept shortcuts, the
/// triple yields **154** distinct keys when the target alone yields **104**.
/// The 26 `smartmontools` shortcuts all target the same `runcmdu.exe` with
/// different arguments; merging them would lose 25 applications, and
/// the total gap is 50.
///
/// Folding the case of arguments would cause the same loss more discreetly:
/// `-Mode admin` and `-mode Admin` are two distinct invocations.
///
/// The separator is the nul byte, which cannot appear in any of the three
/// fields: without it, `("ab", "", "c")` and `("a", "b", "c")` would have the same
/// fingerprint.
pub fn cle(cible: &str, arguments: &str, repertoire: &str) -> String {
    let mut matiere = Vec::new();
    matiere.extend_from_slice(normaliser_chemin(cible).as_bytes());
    matiere.push(0);
    matiere.extend_from_slice(arguments.as_bytes());
    matiere.push(0);
    matiere.extend_from_slice(normaliser_chemin(repertoire).as_bytes());
    sha256::hex(&matiere)
}

/// What travels on the channel: the key, plus the five fields.
///
/// ⚠️ `chemin` IS THAT OF THE `.lnk`, AND IT IS WHAT WE WILL LAUNCH. It does not
/// take part in the identity — a shortcut that moves from the Desktop to the
/// Start menu remains the same application — but it must travel, because
/// the launch goes through the shortcut and not through the rebuilt target.
pub fn depuis_brut(brut: Brut) -> Application {
    Application {
        cle: cle(&brut.cible, &brut.arguments, &brut.repertoire),
        nom: brut.nom,
        chemin: brut.chemin,
        cible: normaliser_chemin(&brut.cible),
        arguments: brut.arguments,
        repertoire: normaliser_chemin(&brut.repertoire),
        // ⚠️ THE ICON IS NOT KNOWN HERE, AND THIS MODULE MUST NOT
        // LOOK FOR IT: it is PURE, and extraction opens COM. It is
        // `apps::boucle` that fills these two fields when the key is new or
        // the `.lnk` has changed — never on every round.
        //
        // 🔴 `None` / `NonMesuree` IS THEREFORE THE STARTING STATE, AND IT IS
        // HONEST: an application without an icon is better than a missing
        // application. The reverse combination — a null icon and a measured
        // size — is FORBIDDEN, and no path writes it.
        icone: None,
        source_max: SourceMax::NonMesuree,
        // 🔴 SAME REASON AS THE ICON, AND SAME STARTING STATE. The accent is
        // derived from the icon's PIXELS — so not before it exists — and
        // associations are read from the REGISTRY, which is no more
        // pure than opening COM. It is `apps::boucle` that fills both.
        //
        // ⚠️ `None` and the EMPTY list are HONEST: an application without
        // accent and without association is better than a missing application,
        // and it is the state of the vast majority of them.
        accent: None,
        associations: Vec::new(),
    }
}

#[cfg(test)]
#[path = "raccourci/tests.rs"]
mod tests;
