//! Tests d'hôte de [`super`], dont ceux qui portent sur le corpus réel.

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

/// Le prédicat injecté le plus simple : tout existe.
fn tout_existe(_: &str) -> bool {
    true
}

#[test]
fn ecarte_une_cible_vide() {
    // 7 des 218 raccourcis de la VM sont dans ce cas : des cibles de l'espace
    // de noms Shell, sans chemin de fichier. Ce n'est pas une erreur.
    assert_eq!(retenir(&brut("", "", ""), &tout_existe), Err(Ecart::CibleVide));
    assert_eq!(
        retenir(&brut("   ", "", ""), &tout_existe),
        Err(Ecart::CibleVide)
    );
}

#[test]
fn ecarte_toute_extension_qui_n_est_pas_exe() {
    // 🔴 `.msi` EST ÉCARTÉ, et ce n'est pas un oubli : un installeur
    // s'installe, il ne se lance pas. L'accepter « puisqu'il s'exécute »
    // ferait entrer au catalogue des lignes que l'utilisateur ne peut que
    // regretter d'avoir cliquées.
    for ext in ["msc", "url", "html", "pdf", "chm", "txt", "bat", "msi", "2"] {
        let cible = format!(r"C:\x\y.{ext}");
        assert_eq!(
            retenir(&brut(&cible, "", ""), &tout_existe),
            Err(Ecart::Extension(ext.into())),
            "extension {ext}"
        );
    }
    // Sans extension du tout — un cas RÉEL du corpus, et que la table du §3.1
    // de la spec n'énumère pas.
    assert_eq!(
        retenir(&brut(r"C:\x\sans_extension", "", ""), &tout_existe),
        Err(Ecart::Extension(String::new()))
    );
    // La casse de l'extension ne décide pas.
    assert!(retenir(&brut(r"C:\x\Y.EXE", "", ""), &tout_existe).is_ok());
}

#[test]
fn ecarte_une_cible_absente_et_c_est_le_predicat_injecte_qui_le_dit() {
    // 🔴 LA ROUGE SE JOUE EN CÂBLANT `Path::exists()` EN DUR : aucune cible du
    // corpus n'existe sur l'hôte, donc `retenir` rendrait `CibleAbsente` pour
    // les 167 et le test des chiffres tomberait. C'est la raison d'être de
    // l'injection, et ce test en est la moitié positive.
    let jamais = |_: &str| false;
    assert_eq!(
        retenir(&brut(r"C:\x\y.exe", "", ""), &jamais),
        Err(Ecart::CibleAbsente)
    );
    // Le prédicat reçoit bien la cible, pas autre chose.
    let seulement_notepad = |c: &str| c == r"C:\Windows\notepad.exe";
    assert!(retenir(&brut(r"C:\Windows\notepad.exe", "", ""), &seulement_notepad).is_ok());
    assert_eq!(
        retenir(&brut(r"C:\Windows\autre.exe", "", ""), &seulement_notepad),
        Err(Ecart::CibleAbsente)
    );
}

#[test]
fn n_ecarte_rien_par_le_nom() {
    // Un motif « Uninstall » dépend de la langue et écarterait en silence des
    // applications légitimes. `maintenancetool.exe` est le désinstalleur de Qt
    // et ne porte aucun de ces mots.
    for cible in [
        r"C:\Qt\maintenancetool.exe",
        r"C:\App\Uninstall.exe",
        r"C:\App\unins000.exe",
        r"C:\App\Désinstaller.exe",
    ] {
        assert!(
            retenir(&brut(cible, "", ""), &tout_existe).is_ok(),
            "{cible} doit être RETENU"
        );
    }
}

#[test]
fn n_ecarte_rien_par_le_chemin() {
    // 63 des cibles de la VM vivent sous C:\Windows, Bloc-notes et Paint
    // compris : un filtre système les perdrait.
    for cible in [
        r"C:\Windows\system32\notepad.exe",
        r"C:\Windows\system32\mspaint.exe",
        r"C:\Windows\SysWOW64\x.exe",
    ] {
        assert!(retenir(&brut(cible, "", ""), &tout_existe).is_ok(), "{cible}");
    }
}

#[test]
fn la_cle_replie_la_casse_de_la_cible_et_du_repertoire_mais_pas_des_arguments() {
    // 🔴 Replier la casse des arguments fondrait deux invocations distinctes.
    assert_eq!(
        cle(r"C:\A\B.EXE", "-x", r"C:\A"),
        cle(r"c:\a\b.exe", "-x", r"c:\a")
    );
    assert_ne!(cle(r"C:\a\b.exe", "-X", r"C:\a"), cle(r"C:\a\b.exe", "-x", r"C:\a"));
}

#[test]
fn la_cle_ne_confond_pas_deux_decoupages_du_meme_texte() {
    // Sans le séparateur nul, ces deux triplets auraient la même empreinte.
    assert_ne!(cle("ab", "", "c"), cle("a", "b", "c"));
}

#[test]
fn deux_raccourcis_au_meme_triplet_rendent_une_seule_application() {
    // Un raccourci qui se déplace du Bureau vers le menu Démarrer reste la
    // même application : l'identité n'est ni le chemin du `.lnk`, ni le nom.
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
    // C'est le défaut de `src/app.js:67`, qui identifie par le nom.
    let a = brut(r"C:\A\jeu.exe", "", r"C:\A");
    let b = brut(r"C:\B\jeu.exe", "", r"C:\B");
    assert_ne!(depuis_brut(a).cle, depuis_brut(b).cle);
}

// ---------------------------------------------------------------------------
// Le corpus réel de la VM — 218 raccourcis, versés.
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
    let doc: Corpus = serde_json::from_str(raw).expect("corpus lisible");
    doc.raccourcis
}

/// 🔴 LES CHIFFRES SONT ÉCRITS EN DUR ICI, JAMAIS LUS DU CORPUS. Le corpus
/// porte bien un bloc `attendus`, mais s'en servir ferait du test une
/// tautologie : il vérifierait que le fichier est d'accord avec lui-même.
#[test]
fn le_corpus_reel_rend_218_lus_167_retenus_154_cles_et_104_par_la_cible_seule() {
    let entrees = corpus();
    assert_eq!(entrees.len(), 218, "218 raccourcis lus sur les quatre racines");

    // Le prédicat injecté relit le booléen MESURÉ sur la VM. C'est très
    // exactement ce que D7 achète : sur l'hôte, aucune de ces cibles n'existe.
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

    assert_eq!(vides, 7, "cibles de l'espace de noms Shell");
    assert_eq!(absentes, 3, "cibles .exe nommées mais absentes du disque");
    assert_eq!(retenus.len(), 167, "raccourcis retenus par les trois règles");
    // Contrôle croisé de la spec §3.1, le seul de ce paragraphe :
    // 170 cibles .exe moins 3 absentes font 167.
    assert_eq!(retenus.len() + absentes, 170, "cibles .exe non vides");
    assert_eq!(vides + par_extension + absentes + retenus.len(), 218);

    let par_triplet: HashSet<String> = retenus
        .iter()
        .map(|b| cle(&b.cible, &b.arguments, &b.repertoire))
        .collect();
    assert_eq!(par_triplet.len(), 154, "applications distinctes sur cette VM");

    // 🔴 C'EST LA ROUGE GRATUITE, ET ELLE EST ICI, SUR L'HÔTE, SANS VM. Un
    // `cle()` qui ignorerait les arguments ferait tomber l'assertion
    // précédente en rendant 104 : les 26 raccourcis de `smartmontools` visent
    // tous `runcmdu.exe` avec des arguments différents, et l'écart total entre
    // les deux comptes est de 50 applications.
    let par_cible_seule: HashSet<String> =
        retenus.iter().map(|b| normaliser_chemin(&b.cible)).collect();
    assert_eq!(par_cible_seule.len(), 104, "clés par la cible seule");
    assert_eq!(par_triplet.len() - par_cible_seule.len(), 50);
}

#[test]
fn le_corpus_ne_porte_aucune_cible_hors_exe_parmi_les_retenus() {
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
                "retenu à tort : {}",
                e.cible
            );
        }
    }
}
