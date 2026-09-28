use super::{commande_vise, executable_de_commande, normaliser_extension, ranger};

// ── `executable_de_commande` ────────────────────────────────────────────────

#[test]
fn une_ligne_citee_garde_les_espaces_du_chemin() {
    // 🔴 THE CASE THAT IMPOSES THE RULE: without the quote handling, we
    // would return `"C:\Program` and matching would fail for every application
    // installed under `Program Files` — that is, almost all of them.
    assert_eq!(
        executable_de_commande("\"C:\\Program Files\\App\\a.exe\" \"%1\""),
        Some("C:\\Program Files\\App\\a.exe".to_string()),
    );
}

#[test]
fn une_ligne_non_citee_court_jusqu_au_premier_espace() {
    assert_eq!(
        executable_de_commande("C:\\Windows\\notepad.exe %1"),
        Some("C:\\Windows\\notepad.exe".to_string()),
    );
}

#[test]
fn une_ligne_sans_argument_rend_le_chemin_entier() {
    assert_eq!(
        executable_de_commande("C:\\Windows\\notepad.exe"),
        Some("C:\\Windows\\notepad.exe".to_string()),
    );
}

#[test]
fn les_espaces_de_tete_ne_trompent_pas_la_lecture() {
    assert_eq!(
        executable_de_commande("   \"C:\\a b\\x.exe\" %1"),
        Some("C:\\a b\\x.exe".to_string()),
    );
}

#[test]
fn une_ligne_vide_ne_rend_rien() {
    assert_eq!(executable_de_commande(""), None);
    assert_eq!(executable_de_commande("   "), None);
}

#[test]
fn un_guillemet_ouvrant_jamais_ferme_est_refuse() {
    // ⚠️ Refusing rather than taking all the rest: a malformed line
    // is not a path, and making one up would attribute the association to
    // anything.
    // RED: `reste.split_once('"').map(...).unwrap_or(reste)` ⟹ returns
    // `C:\a b\x.exe %1`, which matches nothing but is not a
    // refusal either — the defect would be SILENT.
    assert_eq!(executable_de_commande("\"C:\\a b\\x.exe %1"), None);
}

#[test]
fn un_guillemet_ferme_immediatement_ne_rend_rien() {
    assert_eq!(executable_de_commande("\"\" %1"), None);
}

// ── `commande_vise` — MATCHING BY IDENTITY ──────────────────────────────────

#[test]
fn une_commande_vise_sa_cible_a_la_casse_pres() {
    // `normaliser_chemin` folds case: it is the rule ALREADY written, and
    // it is REUSED rather than copied.
    assert!(commande_vise(
        "\"C:\\Program Files\\App\\a.exe\" \"%1\"",
        "c:\\program files\\app\\A.EXE",
    ));
}

#[test]
fn une_commande_ne_vise_pas_une_autre_version_du_meme_produit() {
    // 🔴 IT IS THE REASON FOR DECISION D12, AND THE TEST THAT IMPOSES IT.
    // The model the design cites (`src/app.js:15-37`) matches by
    // NAME SUBSTRING after stripping digits: "Nsight 2020.3"
    // and "Nsight 2024.6" produce the same key there, and the association
    // would land on the wrong version without anyone seeing it.
    // RED: compare with `contains` on the name ⟹ the next two
    // assertions fail TOGETHER.
    let commande = "\"C:\\Nsight 2020.3\\nsight.exe\" \"%1\"";
    assert!(
        commande_vise(commande, "C:\\Nsight 2020.3\\nsight.exe"),
        "la bonne"
    );
    assert!(
        !commande_vise(commande, "C:\\Nsight 2024.6\\nsight.exe"),
        "l'autre version"
    );
}

#[test]
fn une_commande_illisible_ne_vise_rien() {
    assert!(!commande_vise("", "C:\\x.exe"));
    assert!(!commande_vise("\"C:\\x.exe", "C:\\x.exe"));
}

#[test]
fn one_executable_under_two_spellings_is_the_same() {
    // A trailing slash and a different case do not make two
    // applications — that is what `normaliser_chemin` guarantees, and
    // reusing it gives it to us for free.
    assert!(commande_vise(
        "C:\\WINDOWS\\Notepad.exe %1",
        "c:\\windows\\notepad.exe"
    ));
}

// ── `normaliser_extension` ──────────────────────────────────────────────────

#[test]
fn une_extension_prend_son_point_et_ses_minuscules() {
    // The registry writes `.txt` under `HKCR` and `txt` under `FileExts`: both
    // forms arrive, and only one must travel.
    assert_eq!(normaliser_extension("TXT"), Some(".txt".to_string()));
    assert_eq!(normaliser_extension(".TxT"), Some(".txt".to_string()));
    assert_eq!(normaliser_extension("  .md  "), Some(".md".to_string()));
}

#[test]
fn une_extension_vide_ou_absurde_est_refusee() {
    // ⚠️ Without these refusals, a messed-up registry key would send `"."` or
    // a path fragment down the wire, which the platform would put as is into a
    // manifest.
    assert_eq!(normaliser_extension(""), None);
    assert_eq!(normaliser_extension("."), None);
    assert_eq!(normaliser_extension("a b"), None);
    assert_eq!(normaliser_extension("c:\\x"), None);
    assert_eq!(normaliser_extension("a/b"), None);
}

// ── `ranger` ────────────────────────────────────────────────────────────────

#[test]
fn ranger_trie_et_dedoublonne() {
    // 🔴 THE ORDER IS NOT AN ORNAMENT: the platform compares the received
    // catalogue with the one it knows. Two identical lists in a different
    // order would make it write on every tick and log a change
    // that did not happen.
    // RED: remove the `sort()` ⟹ this assertion fails.
    assert_eq!(
        ranger(vec![
            ".TXT".into(),
            "md".into(),
            ".txt".into(),
            ".MD".into(),
            ".c".into()
        ]),
        vec![".c".to_string(), ".md".to_string(), ".txt".to_string()],
    );
}

#[test]
fn ranger_ecarte_ce_qui_n_est_pas_une_extension_sans_tout_perdre() {
    assert_eq!(
        ranger(vec!["".into(), ".doc".into(), "a b".into(), ".".into()]),
        vec![".doc".to_string()],
    );
}

#[test]
fn sort_returns_an_empty_list_rather_than_nothing() {
    // An application with no association is a NORMAL STATE, not a failure: the
    // field stays present on the wire, and it is empty.
    assert_eq!(ranger(vec![]), Vec::<String>::new());
}

// ── `table` and `pour_cible` — THE PATH THE PRODUCT TAKES ───────────────────

use super::{pour_cible, table};

#[test]
fn la_table_groupe_les_extensions_par_executable() {
    let t = table(vec![
        (".txt".into(), "C:\\Windows\\notepad.exe %1".into()),
        (".log".into(), "\"C:\\Windows\\notepad.exe\" \"%1\"".into()),
        (
            ".png".into(),
            "\"C:\\Program Files\\Vue\\vue.exe\" \"%1\"".into(),
        ),
    ]);
    assert_eq!(
        t.get("c:\\windows\\notepad.exe"),
        Some(&vec![".log".to_string(), ".txt".to_string()]),
        "les DEUX extensions du bloc-notes, rangées",
    );
    assert_eq!(
        t.get("c:\\program files\\vue\\vue.exe"),
        Some(&vec![".png".to_string()]),
        "et le chemin à espaces n'est pas tronqué",
    );
}

#[test]
fn la_table_ecarte_ce_qui_ne_se_lit_pas_sans_perdre_le_reste() {
    // ⚠️ A ProgID whose command cannot be read designates NO
    // application; attributing it at random would be worse than discarding it.
    let t = table(vec![
        (".a".into(), "".into()),
        (".b".into(), "\"C:\\x.exe".into()),
        ("".into(), "C:\\y.exe %1".into()),
        (".c".into(), "C:\\y.exe %1".into()),
    ]);
    assert_eq!(t.len(), 1, "seul `y.exe` survit");
    assert_eq!(t.get("c:\\y.exe"), Some(&vec![".c".to_string()]));
}

#[test]
fn pour_cible_normalise_la_cible_aussi() {
    // 🔴 THE DEFECT THIS TEST PREVENTS IS SILENT: the table is built on
    // normalised paths; querying it with a raw path would
    // NEVER find anything, and every list would simply be empty — a product
    // that looks like it works and associates nothing.
    // RED: `table.get(cible)` instead of `table.get(&normaliser_chemin(cible))`
    // ⟹ this assertion fails, the next one stays green.
    let t = table(vec![(".txt".into(), "C:\\Windows\\Notepad.exe %1".into())]);
    assert_eq!(
        pour_cible(&t, "C:\\WINDOWS\\NOTEPAD.EXE"),
        vec![".txt".to_string()]
    );
    assert_eq!(
        pour_cible(&t, "c:\\windows\\notepad.exe"),
        vec![".txt".to_string()]
    );
}

#[test]
fn for_target_returns_an_empty_list_when_the_application_opens_nothing() {
    // That is the state of the vast majority of applications, and it is
    // not a failure.
    let t = table(vec![(".txt".into(), "C:\\Windows\\notepad.exe %1".into())]);
    assert_eq!(pour_cible(&t, "C:\\autre\\chose.exe"), Vec::<String>::new());
}
