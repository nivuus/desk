//! Host tests of [`super`], including those on the real corpus.

use super::*;
use serde::Deserialize;
use std::collections::HashSet;

fn brut(cible: &str, arguments: &str, repertoire: &str) -> Brut {
    Brut {
        nom: "peu importe".into(),
        chemin: r"C:\Users\u\Desktop\peu importe.lnk".into(),
        cible: cible.into(),
        arguments: arguments.into(),
        repertoire: repertoire.into(),
    }
}

/// The simplest injected predicate: everything exists.
fn tout_existe(_: &str) -> bool {
    true
}

#[test]
fn ecarte_une_cible_vide() {
    // 7 of the VM's 218 shortcuts are in that case: Shell namespace
    // targets, without a file path. It is not an error.
    assert_eq!(
        retenir(&brut("", "", ""), &tout_existe),
        Err(Ecart::CibleVide)
    );
    assert_eq!(
        retenir(&brut("   ", "", ""), &tout_existe),
        Err(Ecart::CibleVide)
    );
}

#[test]
fn ecarte_toute_extension_qui_n_est_pas_exe() {
    // 🔴 `.msi` IS DISCARDED, and it is not an oversight: an installer
    // is installed, it is not launched. Accepting it "since it runs"
    // would bring into the catalogue lines the user can only
    // regret having clicked.
    for ext in ["msc", "url", "html", "pdf", "chm", "txt", "bat", "msi", "2"] {
        let cible = format!(r"C:\x\y.{ext}");
        assert_eq!(
            retenir(&brut(&cible, "", ""), &tout_existe),
            Err(Ecart::Extension(ext.into())),
            "extension {ext}"
        );
    }
    // With no extension at all — a REAL case from the corpus, and one the table in §3.1
    // of the spec does not list.
    assert_eq!(
        retenir(&brut(r"C:\x\sans_extension", "", ""), &tout_existe),
        Err(Ecart::Extension(String::new()))
    );
    // The extension's case does not decide.
    assert!(retenir(&brut(r"C:\x\Y.EXE", "", ""), &tout_existe).is_ok());
}

#[test]
fn ecarte_une_cible_absente_et_c_est_le_predicat_injecte_qui_le_dit() {
    // 🔴 THE RED IS PLAYED BY HARDWIRING `Path::exists()`: no target of the
    // corpus exists on the host, so `retenir` would return `CibleAbsente` for
    // all 167 and the figures test would fail. It is the reason for the
    // injection, and this test is its positive half.
    let jamais = |_: &str| false;
    assert_eq!(
        retenir(&brut(r"C:\x\y.exe", "", ""), &jamais),
        Err(Ecart::CibleAbsente)
    );
    // The predicate does receive the target, not something else.
    let notepad_only = |c: &str| c == r"C:\Windows\notepad.exe";
    assert!(retenir(&brut(r"C:\Windows\notepad.exe", "", ""), &notepad_only).is_ok());
    assert_eq!(
        retenir(&brut(r"C:\Windows\autre.exe", "", ""), &notepad_only),
        Err(Ecart::CibleAbsente)
    );
}

#[test]
fn n_ecarte_rien_par_le_nom() {
    // An "Uninstall" pattern depends on the language and would silently discard
    // legitimate applications. `maintenancetool.exe` is Qt's uninstaller
    // and carries none of these words.
    for cible in [
        r"C:\Qt\maintenancetool.exe",
        r"C:\App\Uninstall.exe",
        r"C:\App\unins000.exe",
        r"C:\App\Désinstaller.exe",
    ] {
        assert!(
            retenir(&brut(cible, "", ""), &tout_existe).is_ok(),
            "{cible} must be RETAINED"
        );
    }
}

#[test]
fn n_ecarte_rien_par_le_chemin() {
    // 63 of the VM's targets live under C:\Windows, Notepad and Paint
    // included: a system filter would lose them.
    for cible in [
        r"C:\Windows\system32\notepad.exe",
        r"C:\Windows\system32\mspaint.exe",
        r"C:\Windows\SysWOW64\x.exe",
    ] {
        assert!(
            retenir(&brut(cible, "", ""), &tout_existe).is_ok(),
            "{cible}"
        );
    }
}

#[test]
fn la_cle_replie_la_casse_de_la_cible_et_du_repertoire_mais_pas_des_arguments() {
    // 🔴 Replier la casse des arguments fondrait deux invocations distinctes.
    assert_eq!(
        cle(r"C:\A\B.EXE", "-x", r"C:\A"),
        cle(r"c:\a\b.exe", "-x", r"c:\a")
    );
    assert_ne!(
        cle(r"C:\a\b.exe", "-X", r"C:\a"),
        cle(r"C:\a\b.exe", "-x", r"C:\a")
    );
}

#[test]
fn la_cle_ne_confond_pas_deux_decoupages_du_meme_texte() {
    // Without the nul separator, these two triples would have the same fingerprint.
    assert_ne!(cle("ab", "", "c"), cle("a", "b", "c"));
}

#[test]
fn deux_raccourcis_au_meme_triplet_rendent_une_seule_application() {
    // A shortcut that moves from the Desktop to the Start menu remains the
    // same application: the identity is neither the `.lnk` path nor the name.
    let bureau = Brut {
        nom: "Bloc-notes".into(),
        chemin: r"C:\Users\u\Desktop\Bloc-notes.lnk".into(),
        cible: r"C:\Windows\notepad.exe".into(),
        arguments: String::new(),
        repertoire: r"C:\Windows".into(),
    };
    let menu = Brut {
        nom: "Bloc notes".into(),
        chemin: r"C:\ProgramData\...\Start Menu\Bloc notes.lnk".into(),
        ..bureau.clone()
    };
    assert_eq!(depuis_brut(bureau).cle, depuis_brut(menu).cle);
}

#[test]
fn deux_applications_de_meme_nom_a_triplets_differents_restent_deux() {
    // That is the defect of `src/app.js:67`, which identifies by name.
    let a = brut(r"C:\A\jeu.exe", "", r"C:\A");
    let b = brut(r"C:\B\jeu.exe", "", r"C:\B");
    assert_ne!(depuis_brut(a).cle, depuis_brut(b).cle);
}

// ---------------------------------------------------------------------------
// The VM's real corpus — 218 shortcuts, checked in.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Corpus {
    raccourcis: Vec<EntreeCorpus>,
}

#[derive(Deserialize)]
struct EntreeCorpus {
    nom: String,
    chemin: String,
    cible: String,
    arguments: String,
    repertoire: String,
    existe: bool,
}

fn corpus() -> Vec<EntreeCorpus> {
    let raw = include_str!("../../../testdata/gapps-corpus-vm.json");
    let doc: Corpus = serde_json::from_str(raw).expect("readable corpus");
    doc.raccourcis
}

/// 🔴 THE FIGURES ARE HARDCODED HERE, NEVER READ FROM THE CORPUS. The corpus
/// does carry an `attendus` block, but using it would make the test a
/// tautology: it would check that the file agrees with itself.
#[test]
fn le_corpus_reel_rend_218_lus_167_retenus_154_cles_et_104_par_la_cible_seule() {
    let entrees = corpus();
    assert_eq!(entrees.len(), 218, "218 shortcuts read over the four roots");

    // The injected predicate re-reads the boolean MEASURED on the VM. That is
    // exactly what D7 buys: on the host, none of these targets exists.
    let presents: HashSet<&str> = entrees
        .iter()
        .filter(|e| e.existe)
        .map(|e| e.cible.as_str())
        .collect();
    let existe = |c: &str| presents.contains(c);

    let mut vides = 0;
    let mut par_extension = 0;
    let mut absentes = 0;
    let mut retenus = Vec::new();
    for e in &entrees {
        let b = Brut {
            nom: e.nom.clone(),
            chemin: e.chemin.clone(),
            cible: e.cible.clone(),
            arguments: e.arguments.clone(),
            repertoire: e.repertoire.clone(),
        };
        match retenir(&b, &existe) {
            Ok(()) => retenus.push(b),
            Err(Ecart::CibleVide) => vides += 1,
            Err(Ecart::Extension(_)) => par_extension += 1,
            Err(Ecart::CibleAbsente) => absentes += 1,
        }
    }

    assert_eq!(vides, 7, "Shell namespace targets");
    assert_eq!(absentes, 3, ".exe targets named but absent from the disk");
    assert_eq!(retenus.len(), 167, "shortcuts retained by the three rules");
    // Cross-check of spec §3.1, the only one in this paragraph:
    // 170 .exe targets minus 3 missing make 167.
    assert_eq!(retenus.len() + absentes, 170, "non-empty .exe targets");
    assert_eq!(vides + par_extension + absentes + retenus.len(), 218);

    let par_triplet: HashSet<String> = retenus
        .iter()
        .map(|b| cle(&b.cible, &b.arguments, &b.repertoire))
        .collect();
    assert_eq!(par_triplet.len(), 154, "distinct applications on this VM");

    // 🔴 THIS IS THE FREE RED, AND IT IS HERE, ON THE HOST, WITHOUT A VM. A
    // `cle()` that ignored the arguments would make the previous assertion
    // fail by returning 104: the 26 `smartmontools` shortcuts all
    // target `runcmdu.exe` with different arguments, and the total gap between
    // the two counts is 50 applications.
    let par_cible_seule: HashSet<String> = retenus
        .iter()
        .map(|b| normaliser_chemin(&b.cible))
        .collect();
    assert_eq!(par_cible_seule.len(), 104, "keys by target alone");
    assert_eq!(par_triplet.len() - par_cible_seule.len(), 50);
}

#[test]
fn the_corpus_holds_no_non_exe_target_among_the_retained() {
    let entrees = corpus();
    let presents: HashSet<&str> = entrees
        .iter()
        .filter(|e| e.existe)
        .map(|e| e.cible.as_str())
        .collect();
    let existe = |c: &str| presents.contains(c);
    for e in &entrees {
        let b = Brut {
            nom: e.nom.clone(),
            chemin: e.chemin.clone(),
            cible: e.cible.clone(),
            arguments: e.arguments.clone(),
            repertoire: e.repertoire.clone(),
        };
        if retenir(&b, &existe).is_ok() {
            assert!(
                e.cible.to_lowercase().ends_with(".exe") && e.existe,
                "wrongly retained: {}",
                e.cible
            );
        }
    }
}
