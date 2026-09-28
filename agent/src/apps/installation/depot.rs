//! Where the installer lands, how its name is judged, and what the disk
//! remembers when the agent itself remembers nothing.
//!
//! 🔴 THIS MODULE IS PURE: no `#[cfg]`, no file system access.
//! It **composes and validates** paths, it creates none; it **judges** two
//! presence booleans, it reads no directory. The root
//! (`%ProgramData%`) is a **parameter**, otherwise nothing would be judgeable
//! on the Linux host.
//!
//! ⚠️ WHY `%ProgramData%` (spec D7): **not `%TEMP%`**, which Windows purges
//! even **during** an installation — a self-extracting archive that
//! re-reads its own file would fail midway, with an opaque
//! exit code; **not the user profile**, which an elevated installer, under
//! another token, may not see. `%ProgramData%` is readable by all
//! accounts and survives reboots — which `Etat` needs.
//!
//! 🔴 REJECT, NEVER SANITISE SILENTLY. The `nom` comes from the browser; a
//! silent sanitisation would turn `..\..\evil.exe` into an acceptable name
//! **and would still write a file**, under a name nobody
//! asked for — the product would do something reasonable instead of saying
//! no. Every refusal therefore carries **its own** reason, and that of a dangerous name
//! is not that of a refused extension.

/// The suffix of the root, under `%ProgramData%`; then the two markers,
/// one written **before** `CreateProcessW`, the other **after** the exit.
pub const RACINE_RELATIVE: &str = r"Guacamole\installeurs";
pub const MARQUEUR_COMMENCE: &str = ".commence";
pub const MARQUEUR_TERMINE: &str = ".termine";

/// Beyond this, a forgotten installer directory is to be deleted.
///
/// ⚠️ **NOT CALIBRATED** — 24 h is the value the spec proposes, and it
/// joins the list this repository has kept since `BPP_MIN`.
///
/// 🔴 CONSEQUENCE NOT TO LOSE: **the markers go away with the
/// directory**, so the memory of `Etat` is **bounded to 24 h** — beyond which an
/// order re-sent for an installation already played would be replayed. The second
/// belt is on the platform side, which stops re-sending as soon as the row is no
/// longer `en_attente`.
#[cfg(test)]
pub const EXPIRATION_INSTALLEUR_MS: u64 = 24 * 60 * 60 * 1000;

/// The two length bounds, in bytes.
///
/// ⚠️ IT IS THEIR SUM THAT MATTERS: `C:\ProgramData\Guacamole\installeurs\`
/// is 36 characters, and `36 + 64 + 1 + 128 = 229`, below the 260 of
/// `MAX_PATH`. Raising them without redoing this addition would produce a path that
/// `CreateProcessW` refuses, very far from here.
pub const IDENTIFIANT_MAX_OCTETS: usize = 64;
pub const NOM_MAX_OCTETS: usize = 128;

/// The accepted extensions, and what is done with them.
const EXTENSIONS_RETENUES: &[(&str, Extension)] =
    &[("exe", Extension::Exe), ("msi", Extension::Msi)];

/// Recognised as scripts, to be refused **separately** — see `Refus::Script`.
const SCRIPTS: &[&str] = &["bat", "cmd", "com", "ps1", "vbs", "js", "wsf"];

/// What Windows forbids in a name, apart from the separators, which have their
/// own reason.
const CARACTERES_INTERDITS: &[char] = &['<', '>', ':', '"', '|', '?', '*'];

/// The devices inherited from MS-DOS. Windows reserves them **whatever the
/// extension**: `CON.exe` always designates the console.
const NOMS_RESERVES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Why a path component is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusNom {
    Vide,
    TropLong(usize),
    /// A slash: the name claims to designate a path, when it must only
    /// designate a file.
    Separateur(char),
    /// A `..`, wherever it is. ⚠️ **`..` IS SIGNIFICANT UNDER WINDOWS** — the
    /// sub-block D3 had introduced it into a path component and then caught it
    /// in the next round.
    Remontee,
    CaractereInterdit(char),
    /// A reserved device, folded to lowercase.
    Reserve(String),
    /// A trailing dot or space: Windows trims them silently, so the
    /// file opened is not the one that was named.
    FinInterdite(char),
}

/// What the agent will do with the file once in place: `.exe` is run as is,
/// `.msi` goes through `msiexec /i` — no silent mode is imposed (spec D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extension {
    Exe,
    Msi,
}

/// Why a drop is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// The installation identifier, which is also a path component.
    Identifiant(RefusNom),
    Nom(RefusNom),
    /// 🔴 A SCRIPT HAS ITS OWN REASON: a `.bat` has no implicit
    /// interpreter like an `.exe`, and launching it would require choosing a shell,
    /// a working directory and an execution policy — three decisions
    /// nobody has made. Confusing it with "unknown extension" would make one
    /// look for an exotic file where there is a decision to make.
    Script(String),
    /// Any other extension, folded to lowercase — empty if the name carries
    /// none.
    Extension(String),
}

/// The installation identifier, judged against an **allow list**.
///
/// ⚠️ THE ASYMMETRY WITH `valider_nom` IS DELIBERATE. An identifier is made
/// by the platform: an alphabet can be imposed on it, and an allow
/// list is the only one that says something about the characters one has not
/// seen. A file name is written by a human — accents, spaces and
/// parentheses are legitimate there —, and an allow list would refuse
/// `Firefox Setup 130.0.exe`.
pub fn valider_identifiant(id: &str) -> Result<(), RefusNom> {
    if id.is_empty() {
        return Err(RefusNom::Vide);
    }
    if id.len() > IDENTIFIANT_MAX_OCTETS {
        return Err(RefusNom::TropLong(id.len()));
    }
    match id
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_')
    {
        Some(c @ ('\\' | '/')) => Err(RefusNom::Separateur(c)),
        Some(c) => Err(RefusNom::CaractereInterdit(c)),
        None => Ok(()),
    }
}

/// The file name, judged against a deny list.
///
/// Path traversal is looked for **before** the separator: it is the more
/// dangerous of the two facts, hence the one a log must name.
pub fn valider_nom(nom: &str) -> Result<(), RefusNom> {
    if nom.is_empty() {
        return Err(RefusNom::Vide);
    }
    if nom.len() > NOM_MAX_OCTETS {
        return Err(RefusNom::TropLong(nom.len()));
    }
    if nom.contains("..") {
        return Err(RefusNom::Remontee);
    }
    if let Some(c) = nom.chars().find(|c| *c == '\\' || *c == '/') {
        return Err(RefusNom::Separateur(c));
    }
    if let Some(c) = nom
        .chars()
        .find(|c| CARACTERES_INTERDITS.contains(c) || c.is_control())
    {
        return Err(RefusNom::CaractereInterdit(c));
    }
    let tronc = nom.split('.').next().unwrap_or_default().to_lowercase();
    if NOMS_RESERVES.contains(&tronc.as_str()) {
        return Err(RefusNom::Reserve(tronc));
    }
    match nom.chars().next_back() {
        Some(c @ ('.' | ' ')) => Err(RefusNom::FinInterdite(c)),
        _ => Ok(()),
    }
}

/// The extension rule, once the name is judged safe.
pub fn extension_de(nom: &str) -> Result<Extension, Refus> {
    let ext = match nom.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    };
    if let Some((_, retenue)) = EXTENSIONS_RETENUES.iter().find(|(n, _)| *n == ext) {
        Ok(*retenue)
    } else if SCRIPTS.contains(&ext.as_str()) {
        Err(Refus::Script(ext))
    } else {
        Err(Refus::Extension(ext))
    }
}

/// The directory of an installation, under the given root.
pub fn repertoire(racine: &str, id: &str) -> Result<String, Refus> {
    valider_identifiant(id).map_err(Refus::Identifiant)?;
    Ok(format!(
        "{}\\{RACINE_RELATIVE}\\{id}",
        racine.trim_end_matches('\\')
    ))
}

/// The landing path, and what will be done with the file.
///
/// 🔴 BOTH COMPONENTS ARE JUDGED, NOT ONLY THE NAME: the identifier is
/// also interpolated into a path, and *never interpolate a raw value
/// into a path component* is a rule this repository has already paid for.
pub fn chemin(racine: &str, id: &str, nom: &str) -> Result<(String, Extension), Refus> {
    let dossier = repertoire(racine, id)?;
    valider_nom(nom).map_err(Refus::Nom)?;
    Ok((format!("{dossier}\\{nom}"), extension_de(nom)?))
}

/// What the two markers say about an installation already seen. `Neuf`: we
/// download and run. `Termine`: we do nothing, and re-send the
/// stored verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Neuf,
    /// 🔴 `.commence` ALONE: WE DO NOT RUN, and report an unknown
    /// outcome. The installer ran, its exit code is lost; replaying
    /// would mean replaying an installer on a machine in an **unknown** state,
    /// that is the only thing one cannot get out of.
    Commence,
    Termine,
}

/// The rule, from the mere presence of the two markers.
///
/// `.termine` wins: written **after** `.commence`, both coexist at
/// rest, and reading `Commence` on a finished installation would report it
/// unknown while its verdict is there.
pub fn etat(commence: bool, termine: bool) -> Etat {
    match (commence, termine) {
        (_, true) => Etat::Termine,
        (true, false) => Etat::Commence,
        (false, false) => Etat::Neuf,
    }
}

/// The directories to delete, given their age.
///
/// ⚠️ THE SWEEP IS OPPORTUNISTIC, NEVER A TIMER: it runs at every
/// reconciliation. A maintenance timer is an operations decision —
/// who observes it, what it does if the disk is full — outside this sub-block.
#[cfg(test)]
pub fn a_purger(entrees: &[(String, u64)]) -> Vec<&str> {
    entrees
        .iter()
        .filter(|(_, age_ms)| *age_ms > EXPIRATION_INSTALLEUR_MS)
        .map(|(id, _)| id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RACINE: &str = r"C:\ProgramData";

    /// 🔴 THE RED, FIRST HALF: the name is **refused**, not silently
    /// sanitised, and both the refusal **and its reason** are compared.
    #[test]
    fn une_remontee_de_chemin_est_refusee_et_le_motif_la_nomme() {
        let mechant = r"..\..\Windows\System32\evil.exe";
        assert_eq!(
            chemin(RACINE, "i-1", mechant),
            Err(Refus::Nom(RefusNom::Remontee))
        );
        // A separator without traversal has its own reason: the name claims
        // to designate a path, which is not the same fault.
        assert_eq!(
            valider_nom(r"sous\setup.exe"),
            Err(RefusNom::Separateur('\\'))
        );
        assert_eq!(
            valider_nom("sous/setup.exe"),
            Err(RefusNom::Separateur('/'))
        );
    }

    /// 🔴 THE RED, SECOND HALF: two distinct reasons, two assertions —
    /// a test that only looked at "refused" would let them be
    /// confused.
    #[test]
    fn un_script_est_refuse_par_son_extension_et_non_par_son_nom() {
        assert_eq!(
            chemin(RACINE, "i", "setup.bat"),
            Err(Refus::Script("bat".into()))
        );
        assert_eq!(valider_nom("setup.bat"), Ok(()));
        assert_eq!(
            chemin(RACINE, "i", "s.zip"),
            Err(Refus::Extension("zip".into()))
        );
        assert_eq!(
            chemin(RACINE, "i", "s"),
            Err(Refus::Extension(String::new()))
        );
    }

    #[test]
    fn le_chemin_nominal_se_compose_et_dit_ce_qu_on_fera_du_fichier() {
        let attendu = r"C:\ProgramData\Guacamole\installeurs\a1-b2_c3\VB_Setup.exe";
        assert_eq!(
            chemin(RACINE, "a1-b2_c3", "VB_Setup.exe"),
            Ok((attendu.into(), Extension::Exe))
        );
        assert_eq!(
            chemin(RACINE, "i", "Truc.MSI").map(|(_, e)| e),
            Ok(Extension::Msi)
        );
        // Un nom d'humain passe : accents, espaces, points internes.
        assert!(chemin(RACINE, "i", "Éditeur Pro 3.1 (x64).exe").is_ok());
        // The longest path the two bounds allow fits under
        // MAX_PATH: that is what their sum buys.
        let nom = format!("{}.exe", "b".repeat(NOM_MAX_OCTETS - 4));
        let long = chemin(RACINE, &"a".repeat(IDENTIFIANT_MAX_OCTETS), &nom).unwrap();
        assert!(long.0.len() < 260, "{} caractères", long.0.len());
    }

    #[test]
    fn l_identifiant_est_juge_lui_aussi_sur_une_liste_d_autorisation() {
        // An accent is legitimate in a NAME and refused in an IDENTIFIER:
        // it is the asymmetry of the two lists, and it is intended.
        for (id, motif) in [
            ("..", RefusNom::CaractereInterdit('.')),
            (r"a\b", RefusNom::Separateur('\\')),
            ("", RefusNom::Vide),
            ("é", RefusNom::CaractereInterdit('é')),
        ] {
            assert_eq!(
                chemin(RACINE, id, "s.exe"),
                Err(Refus::Identifiant(motif)),
                "sur {id:?}"
            );
        }
        let trop = "a".repeat(IDENTIFIANT_MAX_OCTETS + 1);
        assert_eq!(
            valider_identifiant(&trop),
            Err(RefusNom::TropLong(trop.len()))
        );
    }

    #[test]
    fn les_autres_refus_de_nom_portent_chacun_le_leur() {
        let trop = "a".repeat(NOM_MAX_OCTETS + 1);
        for (nom, motif) in [
            ("CON.exe", RefusNom::Reserve("con".into())),
            ("lpt9.exe", RefusNom::Reserve("lpt9".into())),
            ("s<1>.exe", RefusNom::CaractereInterdit('<')),
            ("s\u{7}.exe", RefusNom::CaractereInterdit('\u{7}')),
            ("setup.exe ", RefusNom::FinInterdite(' ')),
            ("setup.exe.", RefusNom::FinInterdite('.')),
            ("", RefusNom::Vide),
            (trop.as_str(), RefusNom::TropLong(trop.len())),
        ] {
            assert_eq!(valider_nom(nom), Err(motif), "sur {nom:?}");
        }
    }

    #[test]
    fn les_marqueurs_disent_les_trois_etats_et_termine_l_emporte() {
        assert_eq!(etat(false, false), Etat::Neuf);
        assert_eq!(etat(true, false), Etat::Commence);
        // `.termine` is written AFTER `.commence`: both coexist at rest.
        assert_eq!(etat(true, true), Etat::Termine);
        assert_eq!(etat(false, true), Etat::Termine);
    }

    /// The bound is besieged from both sides: an age equal to the expiry is
    /// kept, one more millisecond goes.
    #[test]
    fn la_purge_prend_ce_qui_a_depasse_l_expiration_et_rien_d_autre() {
        let entrees = vec![
            ("neuf".to_string(), 0),
            ("pile".to_string(), EXPIRATION_INSTALLEUR_MS),
            ("juste-apres".to_string(), EXPIRATION_INSTALLEUR_MS + 1),
            ("vieux".to_string(), EXPIRATION_INSTALLEUR_MS * 3),
        ];
        assert_eq!(a_purger(&entrees), vec!["juste-apres", "vieux"]);
        assert!(a_purger(&[]).is_empty());
    }
}
