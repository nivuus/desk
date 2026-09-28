use super::*;

#[test]
fn the_first_mount_pushes_and_memorises() {
    assert_eq!(decider(None, "Documents", false), Decision::Pousser);
    assert_eq!(
        a_memoriser(Decision::Pousser, "Documents"),
        Some("Documents")
    );
}

#[test]
fn the_same_directory_pushes() {
    assert_eq!(
        decider(Some("Documents"), "Documents", false),
        Decision::Pousser
    );
}

/// 🔴 The case this whole module exists for.
#[test]
fn a_different_directory_without_confirmation_holds() {
    assert_eq!(
        decider(Some("Documents"), "Téléchargements", false),
        Decision::Retenir
    );
}

#[test]
fn a_different_directory_with_confirmation_pushes_and_remembers_the_new_one() {
    let d = decider(Some("Documents"), "Téléchargements", true);
    assert_eq!(d, Decision::Pousser);
    assert_eq!(a_memoriser(d, "Téléchargements"), Some("Téléchargements"));
}

/// 🔴 **HOLDING BACK REMEMBERS NOTHING**, and it is the half that matters.
///
/// **Its red**: make `a_memoriser` return `Some(annonce)` on `Retenir`.
/// This test fails, and the defect would be that a simple page reload would
/// push what the first `Bonjour` had refused — *holding back would only last
/// one visit*.
#[test]
fn retaining_memorises_nothing() {
    assert_eq!(a_memoriser(Decision::Retenir, "Téléchargements"), None);
}

/// The comparison is EXACT: no case folding, no trimmed spaces. A
/// directory name is what the file system says it is, and "Documents" is
/// not "documents" on every system.
#[test]
fn the_name_comparison_is_exact() {
    assert_eq!(
        decider(Some("Documents"), "documents", false),
        Decision::Retenir
    );
    assert_eq!(
        decider(Some("Documents"), "Documents ", false),
        Decision::Retenir
    );
}

/// An EMPTY name is a name like any other: it is not worth "absent". Confusing
/// them would mean a nameless root would always push.
#[test]
fn an_empty_memorised_name_is_not_a_missing_name() {
    assert_eq!(decider(Some(""), "Documents", false), Decision::Retenir);
    assert_eq!(decider(Some(""), "", false), Decision::Pousser);
}

/// `forcer` on a first mount or on the same directory changes nothing:
/// we were already pushing.
#[test]
fn forcing_changes_nothing_when_already_pushing() {
    assert_eq!(decider(None, "Documents", true), Decision::Pousser);
    assert_eq!(
        decider(Some("Documents"), "Documents", true),
        Decision::Pousser
    );
}
