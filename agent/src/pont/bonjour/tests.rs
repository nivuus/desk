use super::*;

#[test]
fn le_premier_montage_pousse_et_memorise() {
    assert_eq!(decider(None, "Documents", false), Decision::Pousser);
    assert_eq!(a_memoriser(Decision::Pousser, "Documents"), Some("Documents"));
}

#[test]
fn le_meme_repertoire_pousse() {
    assert_eq!(decider(Some("Documents"), "Documents", false), Decision::Pousser);
}

/// 🔴 Le cas pour lequel tout ce module existe.
#[test]
fn un_repertoire_different_sans_confirmation_RETIENT() {
    assert_eq!(decider(Some("Documents"), "Téléchargements", false), Decision::Retenir);
}

#[test]
fn un_repertoire_different_AVEC_confirmation_pousse_et_memorise_le_neuf() {
    let d = decider(Some("Documents"), "Téléchargements", true);
    assert_eq!(d, Decision::Pousser);
    assert_eq!(a_memoriser(d, "Téléchargements"), Some("Téléchargements"));
}

/// 🔴 **RETENIR NE MÉMORISE RIEN**, et c'est la moitié qui compte.
///
/// **Sa rouge** : faire rendre `Some(annonce)` à `a_memoriser` sur `Retenir`.
/// Ce test tombe, et le défaut serait qu'un simple rechargement de page ferait
/// pousser ce que le premier `Bonjour` avait refusé — *la retenue ne durerait
/// qu'une visite*.
#[test]
fn retenir_ne_memorise_rien() {
    assert_eq!(a_memoriser(Decision::Retenir, "Téléchargements"), None);
}

/// La comparaison est EXACTE : ni casse repliée, ni espaces rognés. Un nom de
/// répertoire est ce que le système de fichiers en dit, et « Documents » n'est
/// pas « documents » sur tous les systèmes.
#[test]
fn la_comparaison_de_nom_est_exacte() {
    assert_eq!(decider(Some("Documents"), "documents", false), Decision::Retenir);
    assert_eq!(decider(Some("Documents"), "Documents ", false), Decision::Retenir);
}

/// Un nom VIDE est un nom comme un autre : il ne vaut pas « absent ». Les
/// confondre ferait qu'une racine sans nom pousserait toujours.
#[test]
fn un_nom_memorise_vide_n_est_pas_un_nom_absent() {
    assert_eq!(decider(Some(""), "Documents", false), Decision::Retenir);
    assert_eq!(decider(Some(""), "", false), Decision::Pousser);
}

/// `forcer` sur un premier montage ou sur le même répertoire ne change rien :
/// on poussait déjà.
#[test]
fn forcer_ne_change_rien_quand_on_poussait_deja() {
    assert_eq!(decider(None, "Documents", true), Decision::Pousser);
    assert_eq!(decider(Some("Documents"), "Documents", true), Decision::Pousser);
}
