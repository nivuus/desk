//! Host tests of the provenance. Pure: no disk, no COM, no VM.
//!
//! ⚠️ **DIVERGENCE FROM THE PLAN, NOTED RATHER THAN HIDDEN.** It prescribes
//! cases "taken from the checked-in corpus (`agent/testdata/gapps-corpus-vm.json`)" —
//! yet **that corpus carries NO `IconLocation` field**: its six fields are
//! `nom`, `chemin`, `cible`, `arguments`, `repertoire`, `existe`, and its own
//! provenance notice says so ("the FIRST FIVE fields are extracted
//! verbatim from the `WScript.Shell` probe"). The TARGETS below do come
//! from it; the `IconLocation` values, however, are the **verbatim lines of measurement
//! M1** transcribed in the G2 plan. None is invented, and the source of
//! each one is named.

use super::*;

/// 🔴 THE CASE OF THE 92 OUT OF 153: empty path, the icon is the TARGET's.
#[test]
fn an_empty_path_points_back_to_the_target() {
    // The exact shape recorded by M1 on this VM.
    assert_eq!(
        provenance(",0", r"c:\windows\system32\notepad.exe"),
        Provenance::Module(r"c:\windows\system32\notepad.exe".into())
    );
    // The entirely empty string, which is the other shape of the same state.
    assert_eq!(
        provenance("", r"c:\windows\system32\notepad.exe"),
        Provenance::Module(r"c:\windows\system32\notepad.exe".into())
    );
    // 🔴 THE RED: treating an empty path as `Absent`. That state would
    // lose the icon of more than one application in two, SILENTLY.
    assert_ne!(
        provenance(",0", r"c:\windows\system32\notepad.exe"),
        Provenance::Absent
    );
}

/// A standalone `.ico` — the verbatim line of M1.
#[test]
fn a_standalone_ico_is_read_as_an_ico() {
    assert_eq!(
        provenance(
            r"C:\Program Files\GSmartControl\gsmartcontrol.ico,0",
            r"c:\x\y.exe"
        ),
        Provenance::Ico(r"C:\Program Files\GSmartControl\gsmartcontrol.ico".into())
    );
    // Without an index: the format allows it.
    assert_eq!(
        provenance(
            r"C:\Program Files\GSmartControl\gsmartcontrol.ico",
            r"c:\x\y.exe"
        ),
        Provenance::Ico(r"C:\Program Files\GSmartControl\gsmartcontrol.ico".into())
    );
}

/// A PE module named explicitly — the verbatim line of M1.
#[test]
fn a_pe_module_is_read_as_a_module() {
    assert_eq!(
        provenance(r"%windir%\system32\notepad.exe,0", r"c:\autre\chose.exe"),
        Provenance::Module(r"%windir%\system32\notepad.exe".into())
    );
    for ext in ["dll", "mun", "cpl", "scr", "ocx"] {
        let l = format!(r"C:\Windows\System32\imageres.{ext},-5301");
        assert!(matches!(provenance(&l, ""), Provenance::Module(_)), "{ext}");
    }
}

/// 🔴 WITHOUT AN EXTENSION, WE CANNOT READ — and `Absent` SAYS so, where guessing
/// would produce a false measurement. The verbatim line of M1.
#[test]
fn un_chemin_sans_extension_ne_se_lit_pas() {
    assert_eq!(
        provenance(
            r"C:\Windows\Installer\{1BEA6F9E-0000-0000-0000-000000000000}\ProductIcon,0",
            ""
        ),
        Provenance::Absent
    );
    // Nor can an unknown extension.
    assert_eq!(provenance(r"C:\x\y.png,0", ""), Provenance::Absent);
    // And an empty target with an empty IconLocation: nothing at all.
    assert_eq!(provenance("", ""), Provenance::Absent);
}

/// 🔴 THE LAST COMMA, NEVER THE FIRST — a Windows path may
/// carry one, and splitting on the first would cut the path in two.
#[test]
fn le_decoupage_se_fait_sur_la_derniere_virgule() {
    let l = r"C:\Program Files\Machin, Inc\outil.exe,3";
    assert_eq!(
        provenance(l, ""),
        Provenance::Module(r"C:\Program Files\Machin, Inc\outil.exe".into())
    );
    assert_eq!(index(l), 3);
    // On the FIRST comma, the path would be `C:\Program Files\Machin`,
    // which has no extension — hence `Absent`. The red is visible.
    assert_ne!(provenance(l, ""), Provenance::Absent);
}

#[test]
fn l_index_vaut_zero_a_defaut_et_accepte_le_negatif() {
    assert_eq!(index(""), 0);
    assert_eq!(index(r"c:\x\y.exe"), 0);
    assert_eq!(index(r"c:\x\y.exe,0"), 0);
    assert_eq!(index(r"c:\x\y.exe,7"), 7);
    // 🔴 A NEGATIVE INDEX DESIGNATES A RESOURCE BY ITS IDENTIFIER, and it is
    // a common usage of `imageres.dll`. A `u32` would lose it.
    assert_eq!(index(r"C:\Windows\System32\imageres.dll,-5301"), -5301);
    // An unreadable index is worth 0 rather than making the read fail.
    assert_eq!(index(r"c:\x\y.exe,gros"), 0);
}

/// A DIRECTORY extension must not be taken for the file's.
#[test]
fn the_extension_is_that_of_the_last_segment() {
    assert_eq!(
        provenance(r"C:\dossier.exe\fichier,0", ""),
        Provenance::Absent
    );
    assert_eq!(
        provenance(r"C:\dossier.ico\fichier.exe,0", ""),
        Provenance::Module(r"C:\dossier.ico\fichier.exe".into())
    );
    // A trailing dot is not an extension.
    assert_eq!(provenance(r"C:\x\y.,0", ""), Provenance::Absent);
}

/// The case of the extension decides nothing.
#[test]
fn la_casse_de_l_extension_est_indifferente() {
    assert!(matches!(
        provenance(r"C:\X\Y.EXE,0", ""),
        Provenance::Module(_)
    ));
    assert!(matches!(
        provenance(r"C:\X\Y.Ico,0", ""),
        Provenance::Ico(_)
    ));
}
