use super::*;

#[test]
fn a_climb_up_leaves_the_root_and_is_refused() {
    // The test spec §4.4 names. `a\..\..\secret` goes up TWO levels
    // above the root: if it were resolved, it would designate a file outside
    // the directory the user shared.
    assert_eq!(normaliser(r"a\..\..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"a\b\.."), Err(CheminRefuse::Remontee));
}

#[test]
fn a_disguised_climb_up_is_refused() {
    // The interposed `.` is what breaks a naive detection by substring
    // `"\.."` or by prefix.
    assert_eq!(normaliser(r"a\.\..\..\x"), Err(CheminRefuse::Remontee));
    // …and the ordinary slash too: ProjFS delivers backslashes,
    // but an application can make up something else.
    assert_eq!(normaliser("a/../x"), Err(CheminRefuse::Remontee));
}

#[test]
fn an_alternate_stream_is_refused() {
    // An NTFS alternate data stream has no equivalent in the File
    // System Access API: serving it would make no sense, and ignoring it
    // silently would return the file's CONTENT for a request for zone
    // metadata.
    assert_eq!(
        normaliser("fichier.txt:Zone.Identifier"),
        Err(CheminRefuse::FluxAlternatif)
    );
    assert_eq!(
        normaliser(r"dossier\fichier.txt:$DATA"),
        Err(CheminRefuse::FluxAlternatif)
    );
}

#[test]
fn a_reserved_name_is_refused() {
    for nom in ["CON", "PRN", "NUL", "AUX", "COM1", "LPT1"] {
        assert_eq!(normaliser(nom), Err(CheminRefuse::NomReserve), "{nom}");
    }
    // With an extension, and at any depth: Windows resolves these
    // names BEFORE looking at the file system.
    assert_eq!(normaliser("CON.txt"), Err(CheminRefuse::NomReserve));
    assert_eq!(
        normaliser(r"dossier\NUL.log"),
        Err(CheminRefuse::NomReserve)
    );
    // Trailing dots and spaces: Win32 removes them before resolving.
    assert_eq!(normaliser("con. "), Err(CheminRefuse::NomReserve));

    // …and what is NOT reserved passes. Without this half, the test would be
    // satisfied by a module that refuses everything.
    assert_eq!(normaliser("CONTRAT.txt").unwrap(), "CONTRAT.txt");
    assert_eq!(normaliser("COM10").unwrap(), "COM10");
    assert_eq!(normaliser("console").unwrap(), "console");
}

#[test]
fn an_absolute_path_is_refused() {
    assert_eq!(normaliser(r"C:\x"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\\serveur\part"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\depuis-la-racine"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser("/depuis-la-racine"), Err(CheminRefuse::Absolu));
}

#[test]
fn the_root_itself_is_the_empty_string_and_is_valid() {
    // It is the path of the root's enumeration: refusing it would make the
    // drive empty, and nothing would say so.
    assert_eq!(normaliser("").unwrap(), "");
}

#[test]
fn backslashes_become_slashes() {
    assert_eq!(normaliser(r"a\b\c").unwrap(), "a/b/c");
    assert_eq!(normaliser("a").unwrap(), "a");
    // An interposed `.` gets dropped, it does not become a component.
    assert_eq!(normaliser(r"a\.\b").unwrap(), "a/b");
    // A doubled separator produces an empty component: refusal, not
    // silent squashing.
    assert_eq!(normaliser(r"a\\b"), Err(CheminRefuse::Vide));
}

#[test]
fn case_is_preserved_but_comparison_is_not() {
    // ⚠️ Case is PRESERVED: the File System Access API is case-
    // sensitive, and folding the path would make every opening fail. Two
    // paths differing only in case therefore stay distinct on
    // output — it is F1's known limit, documented at the head of the module.
    assert_eq!(normaliser("Rapport.TXT").unwrap(), "Rapport.TXT");
    assert_eq!(normaliser("rapport.txt").unwrap(), "rapport.txt");
    assert_ne!(
        normaliser("Rapport.TXT").unwrap(),
        normaliser("rapport.txt").unwrap()
    );

    // …whereas the COMPARISON of reserved names, for its part, does fold case,
    // because Windows folds it. All three designate the console.
    for nom in ["CON", "con", "CoN"] {
        assert_eq!(normaliser(nom), Err(CheminRefuse::NomReserve), "{nom}");
    }
}

#[test]
fn invalid_utf16_units_are_refused_before_any_normalisation() {
    // 0xD800 is an isolated half of a surrogate pair: ProjFS delivers
    // `PCWSTR`s, and nothing guarantees they form valid text.
    assert_eq!(
        normaliser_utf16(&[0xD800]),
        Err(CheminRefuse::NonUtf16Valide)
    );
    // …and a valid UTF-16 string does get through to normalisation,
    // refusal included. Without this half, the test would pass on a function that
    // refuses everything.
    let valide: Vec<u16> = "a\\..\\b".encode_utf16().collect();
    assert_eq!(normaliser_utf16(&valide), Err(CheminRefuse::Remontee));
    let simple: Vec<u16> = "dossier\\éléphant.txt".encode_utf16().collect();
    assert_eq!(normaliser_utf16(&simple).unwrap(), "dossier/éléphant.txt");
}

/// 🔴 **THE CANONICAL NAME REPLACES THE LAST COMPONENT, AND NOTHING ELSE.**
#[test]
fn with_last_component_only_touches_the_last() {
    assert_eq!(
        super::with_last_component("Dossier\\GROS.BIN", "gros.bin"),
        Some("Dossier\\gros.bin".to_string())
    );
    assert_eq!(
        super::with_last_component("A\\B\\C\\NOTE.TXT", "note.txt"),
        Some("A\\B\\C\\note.txt".to_string())
    );
    assert_eq!(
        super::with_last_component("GROS.BIN", "gros.bin"),
        Some("gros.bin".to_string())
    );
}

/// 🔴 **NOTHING TO CHANGE ⇒ `None`, AND THE CALLER KEEPS THE ORIGINAL BYTES.**
///
/// F1 had given itself the property of never reconverting a ProjFS path —
/// "a round trip where a case or a separator could be lost". F3 only
/// breaks it WHEN there is something to gain.
#[test]
fn with_last_component_returns_none_when_there_is_nothing_to_change() {
    assert_eq!(
        super::with_last_component("Dossier\\note.txt", "note.txt"),
        None
    );
    assert_eq!(super::with_last_component("note.txt", "note.txt"), None);
    assert_eq!(super::with_last_component("", "note.txt"), None);
    assert_eq!(super::with_last_component("note.txt", ""), None);
}

/// ⚠️ **ProjFS's separator is `\`, never `/`**: a `/` in the delivered path
/// is not a separator but a name character, and treating it as
/// such would truncate the path.
#[test]
fn with_last_component_ignores_the_slash() {
    assert_eq!(
        super::with_last_component("a/b", "z"),
        Some("z".to_string()),
        "there is no backslash: the whole path is the last component"
    );
}
