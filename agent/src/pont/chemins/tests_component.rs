//! Tests of the validation of peer-supplied names, apart from `tests.rs`
//! which covers the normalisation of ProjFS paths.

use super::*;

/// A stand-in for `PrjFileNameCompare` on the host: Unicode case folding.
/// Its exact folding is not what is tested here — the caller injects the
/// file system's own.
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[test]
fn ordinary_names_in_any_script_are_valid() {
    for name in [
        "note.txt",
        ".bashrc",
        "Résumé final.pdf",
        "отчёт.docx",
        "データ.csv",
        "تقرير.txt",
        "a..b",
    ] {
        assert_eq!(valid_component(name), Ok(()), "{name}");
    }
}

#[test]
fn a_name_that_is_not_one_component_is_refused() {
    assert_eq!(valid_component(r"a\b"), Err(CheminRefuse::Separator));
    assert_eq!(valid_component("a/b"), Err(CheminRefuse::Separator));
    assert_eq!(valid_component(".."), Err(CheminRefuse::Remontee));
    assert_eq!(valid_component("."), Err(CheminRefuse::CurrentDirectory));
    assert_eq!(valid_component(""), Err(CheminRefuse::Vide));
}

#[test]
fn a_name_win32_cannot_hold_is_refused() {
    assert_eq!(valid_component("x:y"), Err(CheminRefuse::FluxAlternatif));
    for name in [
        "a<b", "a>b", "a\"b", "a|b", "a?b", "a*b", "a\u{0}b", "a\u{1f}b",
    ] {
        assert_eq!(
            valid_component(name),
            Err(CheminRefuse::ForbiddenCharacter),
            "{name:?}"
        );
    }
    assert_eq!(valid_component("note."), Err(CheminRefuse::ForbiddenEnding));
    assert_eq!(valid_component("note "), Err(CheminRefuse::ForbiddenEnding));
}

#[test]
fn a_reserved_device_name_is_refused_whatever_its_case_or_extension() {
    for name in [
        "CON",
        "con",
        "Nul.txt",
        "COM1.tar.gz",
        "lpt9",
        "COM¹",
        "LPT³.log",
    ] {
        assert_eq!(
            valid_component(name),
            Err(CheminRefuse::NomReserve),
            "{name}"
        );
    }
}

#[test]
fn a_canonical_name_differing_only_by_case_replaces_the_requested_one() {
    assert_eq!(
        canonical_name(r"Dossier\GROS.BIN", "gros.bin", same_name),
        Ok(Some(r"Dossier\gros.bin".to_string()))
    );
    assert_eq!(
        canonical_name(r"Dossier\gros.bin", "gros.bin", same_name),
        Ok(None),
        "an identical name changes nothing"
    );
}

#[test]
fn a_canonical_name_designating_another_file_is_refused() {
    assert_eq!(
        canonical_name(r"Dossier\note.txt", "autre.txt", same_name),
        Err(CanonicalRefused::OtherName)
    );
}

#[test]
fn an_invalid_canonical_name_is_refused_before_anything_else() {
    assert_eq!(
        canonical_name(r"Dossier\note.txt", r"..\note.txt", same_name),
        Err(CanonicalRefused::Invalid(CheminRefuse::Separator))
    );
    assert_eq!(
        canonical_name("con", "CON", same_name),
        Err(CanonicalRefused::Invalid(CheminRefuse::NomReserve))
    );
}
