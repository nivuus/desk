use super::*;

#[test]
fn le_premier_montage_pousse_et_memorise() {
    assert_eq!(decider(None, "Documents", false), Decision::Pousser);
    assert_eq!(
        a_memoriser(Decision::Pousser, "Documents"),
        Some("Documents")
    );
}

#[test]
fn le_meme_repertoire_pousse() {
    assert_eq!(
        decider(Some("Documents"), "Documents", false),
        Decision::Pousser
    );
}

/// 🔴 The case this whole module exists for.
#[test]
fn un_repertoire_different_sans_confirmation_retient() {
    assert_eq!(
        decider(Some("Documents"), "Téléchargements", false),
        Decision::Retenir
    );
}

#[test]
fn un_repertoire_different_avec_confirmation_pousse_et_memorise_le_neuf() {
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
fn retenir_ne_memorise_rien() {
    assert_eq!(a_memoriser(Decision::Retenir, "Téléchargements"), None);
}

/// The comparison is EXACT: no case folding, no trimmed spaces. A
/// directory name is what the file system says it is, and "Documents" is
/// not "documents" on every system.
#[test]
fn la_comparaison_de_nom_est_exacte() {
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
fn un_nom_memorise_vide_n_est_pas_un_nom_absent() {
    assert_eq!(decider(Some(""), "Documents", false), Decision::Retenir);
    assert_eq!(decider(Some(""), "", false), Decision::Pousser);
}

/// `forcer` on a first mount or on the same directory changes nothing:
/// we were already pushing.
#[test]
fn forcer_ne_change_rien_quand_on_poussait_deja() {
    assert_eq!(decider(None, "Documents", true), Decision::Pousser);
    assert_eq!(
        decider(Some("Documents"), "Documents", true),
        Decision::Pousser
    );
}
