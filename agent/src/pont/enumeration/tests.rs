use super::*;
use std::cmp::Ordering;

fn e(nom: &str) -> Entree {
    Entree { nom: nom.to_string(), repertoire: false, taille: 0, modifie_ms: 0 }
}

/// Un comparateur qui trie **à l'envers** de l'ordre lexicographique.
///
/// C'est ce qui rend le test capable d'échouer : à comparateur lexicographique,
/// un `sort()` oublié rendrait le même résultat que l'appel au comparateur
/// injecté, et le test passerait sur un code qui ignore ProjFS.
fn a_l_envers(a: &str, b: &str) -> Ordering {
    b.cmp(a)
}

/// 🔴 **L'ordre est IMPOSÉ par `PrjFileNameCompare`**, qui n'est ni l'ordre
/// lexicographique d'`OsStr` ni `Ordering::cmp` (spec §7.2). `dir.values()` de
/// la File System Access API ne garantit **aucun** ordre. Un fournisseur qui
/// remplit dans le mauvais ordre voit son énumération **silencieusement**
/// tronquée ou désordonnée par ProjFS.
#[test]
fn l_ordre_suit_le_comparateur_injecte_et_non_l_ordre_lexicographique() {
    let entrees = vec![e("alpha"), e("charlie"), e("bravo")];
    let prepare = preparer(entrees, None, |_, _| true, a_l_envers);
    let noms: Vec<&str> = prepare.iter().map(|e| e.nom.as_str()).collect();
    assert_eq!(noms, ["charlie", "bravo", "alpha"]);
}

/// 🔴 **Le filtre `searchExpression` est FACULTATIF et il est FOURNI.**
/// L'ignorer est une faute silencieuse : un `dir /b *.txt` rendrait tout.
#[test]
fn l_expression_de_recherche_est_appliquee_quand_elle_est_fournie() {
    let entrees = vec![e("note.txt"), e("image.png"), e("autre.txt")];
    let prepare = preparer(
        entrees,
        Some("*.txt"),
        |nom, motif| motif == "*.txt" && nom.ends_with(".txt"),
        |a, b| a.cmp(b),
    );
    let noms: Vec<&str> = prepare.iter().map(|e| e.nom.as_str()).collect();
    assert_eq!(noms, ["autre.txt", "note.txt"]);
}

/// Sans expression, l'apparieur n'est **jamais** consulté : ProjFS n'en fournit
/// pas toujours une, et en inventer une (`*`) ferait dépendre le résultat du
/// comportement de `PrjFileNameMatch` sur un motif qu'on aurait forgé.
#[test]
fn sans_expression_l_apparieur_n_est_pas_consulte() {
    let mut consulte = false;
    let prepare = preparer(
        vec![e("a"), e("b")],
        None,
        |_, _| {
            consulte = true;
            false
        },
        |a, b| a.cmp(b),
    );
    assert_eq!(prepare.len(), 2);
    assert!(!consulte, "l'apparieur a été consulté alors qu'aucune expression n'est fournie");
}

#[test]
fn une_session_neuve_n_est_pas_chargee() {
    let session = Session::nouvelle();
    assert!(!session.chargee());
    assert!(session.prochaine().is_none());
}

#[test]
fn une_session_chargee_rend_ses_entrees_dans_l_ordre_puis_s_epuise() {
    let mut session = Session::nouvelle();
    session.poser(vec![e("un"), e("deux")]);
    assert!(session.chargee());
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("un"));
    session.avancer();
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("deux"));
    session.avancer();
    assert!(session.prochaine().is_none(), "la session doit être épuisée");
}

/// 🔴 `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (mod.rs:177) **doit être honoré** :
/// il redémarre l'énumération en cours. Ne pas le faire rendrait un répertoire
/// vide à toute application qui redemande depuis le début — silencieusement.
#[test]
fn un_redemarrage_ramene_le_curseur_au_debut_sans_perdre_les_entrees() {
    let mut session = Session::nouvelle();
    session.poser(vec![e("un"), e("deux")]);
    session.avancer();
    session.avancer();
    assert!(session.prochaine().is_none());
    session.redemarrer();
    assert!(session.chargee(), "un redémarrage ne doit PAS jeter les entrées déjà obtenues");
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("un"));
}

/// Recharger une session déjà chargée remet aussi le curseur à zéro : sans
/// cela, une seconde réponse `Entrees` laisserait le curseur au-delà de la
/// nouvelle liste, et l'énumération rendrait vide.
#[test]
fn reposer_des_entrees_remet_le_curseur_a_zero() {
    let mut session = Session::nouvelle();
    session.poser(vec![e("un"), e("deux")]);
    session.avancer();
    session.poser(vec![e("trois")]);
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("trois"));
}
