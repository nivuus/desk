use super::{commande_vise, executable_de_commande, normaliser_extension, ranger};

// ── `executable_de_commande` ────────────────────────────────────────────────

#[test]
fn une_ligne_citee_garde_les_espaces_du_chemin() {
    // 🔴 LE CAS QUI IMPOSE LA RÈGLE : sans le traitement du guillemet, on
    // rendrait `"C:\Program` et l'appariement échouerait sur toute application
    // installée sous `Program Files` — c'est-à-dire sur la quasi-totalité.
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
fn un_guillemet_ouvrant_jamais_ferme_est_REFUSE() {
    // ⚠️ On refuse plutôt que de prendre tout le reste : une ligne mal formée
    // n'est pas un chemin, et en fabriquer un attribuerait l'association à
    // n'importe quoi.
    // ROUGE : `reste.split_once('"').map(...).unwrap_or(reste)` ⟹ rend
    // `C:\a b\x.exe %1`, qui n'apparie rien mais qui n'est pas non plus un
    // refus — le défaut serait SILENCIEUX.
    assert_eq!(executable_de_commande("\"C:\\a b\\x.exe %1"), None);
}

#[test]
fn un_guillemet_ferme_immediatement_ne_rend_rien() {
    assert_eq!(executable_de_commande("\"\" %1"), None);
}

// ── `commande_vise` — L'APPARIEMENT PAR IDENTITÉ ────────────────────────────

#[test]
fn une_commande_vise_sa_cible_a_la_casse_pres() {
    // `normaliser_chemin` replie la casse : c'est la règle DÉJÀ écrite, et
    // elle est RÉEMPLOYÉE plutôt que recopiée.
    assert!(commande_vise(
        "\"C:\\Program Files\\App\\a.exe\" \"%1\"",
        "c:\\program files\\app\\A.EXE",
    ));
}

#[test]
fn une_commande_ne_vise_PAS_une_autre_version_du_meme_produit() {
    // 🔴 C'EST LA RAISON D'ÊTRE DE LA DÉCISION D12, ET LE TEST QUI L'IMPOSE.
    // Le modèle que la conception cite (`src/app.js:15-37`) apparie par
    // SOUS-CHAÎNE DE NOM après avoir retiré les chiffres : « Nsight 2020.3 »
    // et « Nsight 2024.6 » y produisent la même clé, et l'association
    // atterrirait sur la mauvaise version sans que personne ne le voie.
    // ROUGE : comparer par `contains` sur le nom ⟹ les deux assertions
    // suivantes tombent ENSEMBLE.
    let commande = "\"C:\\Nsight 2020.3\\nsight.exe\" \"%1\"";
    assert!(commande_vise(commande, "C:\\Nsight 2020.3\\nsight.exe"), "la bonne");
    assert!(!commande_vise(commande, "C:\\Nsight 2024.6\\nsight.exe"), "l'autre version");
}

#[test]
fn une_commande_illisible_ne_vise_rien() {
    assert!(!commande_vise("", "C:\\x.exe"));
    assert!(!commande_vise("\"C:\\x.exe", "C:\\x.exe"));
}

#[test]
fn un_meme_executable_sous_deux_ecritures_est_le_meme() {
    // Une barre oblique finale et une casse différente ne font pas deux
    // applications — c'est ce que `normaliser_chemin` garantit, et le
    // réemployer nous le donne gratuitement.
    assert!(commande_vise("C:\\WINDOWS\\Notepad.exe %1", "c:\\windows\\notepad.exe"));
}

// ── `normaliser_extension` ──────────────────────────────────────────────────

#[test]
fn une_extension_prend_son_point_et_ses_minuscules() {
    // Le registre écrit `.txt` sous `HKCR` et `txt` sous `FileExts` : les deux
    // formes arrivent, et une seule doit voyager.
    assert_eq!(normaliser_extension("TXT"), Some(".txt".to_string()));
    assert_eq!(normaliser_extension(".TxT"), Some(".txt".to_string()));
    assert_eq!(normaliser_extension("  .md  "), Some(".md".to_string()));
}

#[test]
fn une_extension_vide_ou_absurde_est_REFUSEE() {
    // ⚠️ Sans ces refus, une clé de registre déréglée ferait voyager `"."` ou
    // un fragment de chemin, que la plateforme poserait tel quel dans un
    // manifeste.
    assert_eq!(normaliser_extension(""), None);
    assert_eq!(normaliser_extension("."), None);
    assert_eq!(normaliser_extension("a b"), None);
    assert_eq!(normaliser_extension("c:\\x"), None);
    assert_eq!(normaliser_extension("a/b"), None);
}

// ── `ranger` ────────────────────────────────────────────────────────────────

#[test]
fn ranger_trie_ET_dedoublonne() {
    // 🔴 L'ORDRE N'EST PAS UN ORNEMENT : la plateforme compare le catalogue
    // reçu à celui qu'elle connaît. Deux listes identiques dans un ordre
    // différent la feraient écrire à chaque tour et journaliser un changement
    // qui n'a pas eu lieu.
    // ROUGE : retirer le `sort()` ⟹ cette assertion tombe.
    assert_eq!(
        ranger(vec![".TXT".into(), "md".into(), ".txt".into(), ".MD".into(), ".c".into()]),
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
fn ranger_rend_une_liste_vide_plutot_que_rien() {
    // Une application sans association est un ÉTAT NORMAL, pas une panne : le
    // champ reste présent sur le fil, et il est vide.
    assert_eq!(ranger(vec![]), Vec::<String>::new());
}

// ── `table` et `pour_cible` — LA VOIE QUE LE PRODUIT EMPRUNTE ───────────────

use super::{pour_cible, table};

#[test]
fn la_table_groupe_les_extensions_par_executable() {
    let t = table(vec![
        (".txt".into(), "C:\\Windows\\notepad.exe %1".into()),
        (".log".into(), "\"C:\\Windows\\notepad.exe\" \"%1\"".into()),
        (".png".into(), "\"C:\\Program Files\\Vue\\vue.exe\" \"%1\"".into()),
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
    // ⚠️ Un ProgID dont la commande ne se lit pas ne désigne AUCUNE
    // application ; l'attribuer au hasard serait pire que de l'écarter.
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
fn pour_cible_normalise_LA_CIBLE_AUSSI() {
    // 🔴 LE DÉFAUT QUE CE TEST EMPÊCHE EST SILENCIEUX : la table est bâtie sur
    // des chemins normalisés ; l'interroger avec un chemin brut ne trouverait
    // JAMAIS rien, et toutes les listes seraient simplement vides — un produit
    // qui a l'air de fonctionner et n'associe rien.
    // ROUGE : `table.get(cible)` au lieu de `table.get(&normaliser_chemin(cible))`
    // ⟹ cette assertion tombe, la suivante reste verte.
    let t = table(vec![(".txt".into(), "C:\\Windows\\Notepad.exe %1".into())]);
    assert_eq!(pour_cible(&t, "C:\\WINDOWS\\NOTEPAD.EXE"), vec![".txt".to_string()]);
    assert_eq!(pour_cible(&t, "c:\\windows\\notepad.exe"), vec![".txt".to_string()]);
}

#[test]
fn pour_cible_rend_une_liste_VIDE_quand_l_application_n_ouvre_rien() {
    // C'est l'état de la très grande majorité des applications, et ce n'est
    // pas une panne.
    let t = table(vec![(".txt".into(), "C:\\Windows\\notepad.exe %1".into())]);
    assert_eq!(pour_cible(&t, "C:\\autre\\chose.exe"), Vec::<String>::new());
}
