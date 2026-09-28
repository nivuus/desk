use super::*;

#[test]
fn une_remontee_sort_de_la_racine_et_est_refusee() {
    // The test spec §4.4 names. `a\..\..\secret` goes up TWO levels
    // above the root: if it were resolved, it would designate a file outside
    // the directory the user shared.
    assert_eq!(normaliser(r"a\..\..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"..\secret"), Err(CheminRefuse::Remontee));
    assert_eq!(normaliser(r"a\b\.."), Err(CheminRefuse::Remontee));
}

#[test]
fn une_remontee_deguisee_est_refusee() {
    // The interposed `.` is what breaks a naive detection by substring
    // `"\.."` or by prefix.
    assert_eq!(normaliser(r"a\.\..\..\x"), Err(CheminRefuse::Remontee));
    // …and the ordinary slash too: ProjFS delivers backslashes,
    // but an application can make up something else.
    assert_eq!(normaliser("a/../x"), Err(CheminRefuse::Remontee));
}

#[test]
fn un_flux_alternatif_est_refuse() {
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
fn un_nom_reserve_est_refuse() {
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
fn un_chemin_absolu_est_refuse() {
    assert_eq!(normaliser(r"C:\x"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\\serveur\part"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser(r"\depuis-la-racine"), Err(CheminRefuse::Absolu));
    assert_eq!(normaliser("/depuis-la-racine"), Err(CheminRefuse::Absolu));
}

#[test]
fn la_racine_elle_meme_est_la_chaine_vide_et_est_licite() {
    // It is the path of the root's enumeration: refusing it would make the
    // drive empty, and nothing would say so.
    assert_eq!(normaliser("").unwrap(), "");
}

#[test]
fn les_contre_obliques_deviennent_des_barres() {
    assert_eq!(normaliser(r"a\b\c").unwrap(), "a/b/c");
    assert_eq!(normaliser("a").unwrap(), "a");
    // An interposed `.` gets dropped, it does not become a component.
    assert_eq!(normaliser(r"a\.\b").unwrap(), "a/b");
    // A doubled separator produces an empty component: refusal, not
    // silent squashing.
    assert_eq!(normaliser(r"a\\b"), Err(CheminRefuse::Vide));
}

#[test]
fn la_casse_est_conservee_mais_la_comparaison_ne_l_est_pas() {
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
fn des_unites_utf16_invalides_sont_refusees_avant_toute_normalisation() {
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
fn avec_dernier_composant_ne_touche_que_le_dernier() {
    assert_eq!(
        super::avec_dernier_composant("Dossier\\GROS.BIN", "gros.bin"),
        Some("Dossier\\gros.bin".to_string())
    );
    assert_eq!(
        super::avec_dernier_composant("A\\B\\C\\NOTE.TXT", "note.txt"),
        Some("A\\B\\C\\note.txt".to_string())
    );
    assert_eq!(
        super::avec_dernier_composant("GROS.BIN", "gros.bin"),
        Some("gros.bin".to_string())
    );
}

/// 🔴 **NOTHING TO CHANGE ⇒ `None`, AND THE CALLER KEEPS THE ORIGINAL BYTES.**
///
/// F1 had given itself the property of never reconverting a ProjFS path —
/// "a round trip where a case or a separator could be lost". F3 only
/// breaks it WHEN there is something to gain.
#[test]
fn avec_dernier_composant_rend_none_quand_il_n_y_a_rien_a_changer() {
    assert_eq!(
        super::avec_dernier_composant("Dossier\\note.txt", "note.txt"),
        None
    );
    assert_eq!(super::avec_dernier_composant("note.txt", "note.txt"), None);
    assert_eq!(super::avec_dernier_composant("", "note.txt"), None);
    assert_eq!(super::avec_dernier_composant("note.txt", ""), None);
}

/// ⚠️ **ProjFS's separator is `\`, never `/`**: a `/` in the delivered path
/// is not a separator but a name character, and treating it as
/// such would truncate the path.
#[test]
fn avec_dernier_composant_ignore_la_barre_oblique() {
    assert_eq!(
        super::avec_dernier_composant("a/b", "z"),
        Some("z".to_string()),
        "il n'y a aucun antislash : tout le chemin est le dernier composant"
    );
}
