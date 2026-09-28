use super::*;
use std::cmp::Ordering;

fn e(nom: &str) -> Entree {
    Entree {
        nom: nom.to_string(),
        repertoire: false,
        size: 0,
        modified_ms: 0,
    }
}

/// A comparator that sorts **in reverse** of lexicographic order.
///
/// It is what makes the test able to fail: with a lexicographic comparator,
/// a forgotten `sort()` would give the same result as the call to the injected
/// comparator, and the test would pass on code that ignores ProjFS.
fn a_l_envers(a: &str, b: &str) -> Ordering {
    b.cmp(a)
}

/// 🔴 **The order is IMPOSED by `PrjFileNameCompare`**, which is neither the
/// lexicographic order of `OsStr` nor `Ordering::cmp` (spec §7.2). `dir.values()` of
/// the File System Access API guarantees **no** order. A provider that
/// fills in the wrong order sees its enumeration **silently**
/// truncated or scrambled by ProjFS.
#[test]
fn l_ordre_suit_le_comparateur_injecte_et_non_l_ordre_lexicographique() {
    let entrees = vec![e("alpha"), e("charlie"), e("bravo")];
    let prepare = preparer(entrees, None, |_, _| true, a_l_envers);
    let noms: Vec<&str> = prepare.iter().map(|e| e.nom.as_str()).collect();
    assert_eq!(noms, ["charlie", "bravo", "alpha"]);
}

/// 🔴 **The `searchExpression` filter is OPTIONAL and it is PROVIDED.**
/// Ignoring it is a silent fault: a `dir /b *.txt` would return everything.
#[test]
fn the_search_expression_is_applied_when_provided() {
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

/// Without an expression, the matcher is **never** consulted: ProjFS does not
/// always provide one, and inventing one (`*`) would make the result depend on
/// `PrjFileNameMatch`'s behaviour on a pattern we forged.
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
    assert!(
        !consulte,
        "l'apparieur a été consulté alors qu'aucune expression n'est fournie"
    );
}

#[test]
fn une_session_neuve_n_est_pas_chargee() {
    let session = Session::new();
    assert!(!session.chargee());
    assert!(session.prochaine().is_none());
}

#[test]
fn a_loaded_session_returns_its_entries_in_order_then_runs_out() {
    let mut session = Session::new();
    session.poser(vec![e("un"), e("deux")]);
    assert!(session.chargee());
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("un"));
    session.avancer();
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("deux"));
    session.avancer();
    assert!(
        session.prochaine().is_none(),
        "la session doit être épuisée"
    );
}

/// 🔴 `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (mod.rs:177) **must be honoured**:
/// it restarts the enumeration in progress. Not doing so would return an empty
/// directory to any application that asks again from the start — silently.
#[test]
fn un_redemarrage_ramene_le_curseur_au_debut_sans_perdre_les_entrees() {
    let mut session = Session::new();
    session.poser(vec![e("un"), e("deux")]);
    session.avancer();
    session.avancer();
    assert!(session.prochaine().is_none());
    session.redemarrer();
    assert!(
        session.chargee(),
        "un redémarrage ne doit PAS jeter les entrées déjà obtenues"
    );
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("un"));
}

/// Reloading an already loaded session also resets the cursor to zero: without
/// it, a second `Entrees` response would leave the cursor beyond the
/// new list, and the enumeration would return empty.
#[test]
fn reposer_des_entrees_remet_le_curseur_a_zero() {
    let mut session = Session::new();
    session.poser(vec![e("un"), e("deux")]);
    session.avancer();
    session.poser(vec![e("trois")]);
    assert_eq!(session.prochaine().map(|e| e.nom.as_str()), Some("trois"));
}
