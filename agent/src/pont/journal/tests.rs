//! Tests du journal de reprise. **Purs, exécutés sur l'hôte.**

use super::*;

/// Rejoue un journal en repartant de ses lignes, comme le ferait un pont
/// relancé.
fn rejouer(lignes: &str) -> (Journal, usize) {
    Journal::relire(lignes)
}

/// 🔴 **UNE DERNIÈRE LIGNE TRONQUÉE NE FAIT PAS PERDRE LES PRÉCÉDENTES.**
///
/// C'est le seul dommage qu'un arrêt brutal puisse causer à un fichier en
/// ajout — le pont peut mourir au milieu d'un `write`. Lever, ou jeter le
/// fichier entier, ferait perdre des écritures **intactes** : c'est le rouge
/// que la spec §4.4 nomme pour ce module.
#[test]
fn une_derniere_ligne_tronquee_ne_fait_pas_perdre_les_precedentes() {
    let mut j = Journal::nouveau();
    let mut fichier = String::new();
    fichier.push_str(&j.inscrire("note.txt", 42));
    fichier.push_str(&j.inscrire("dossier/gros.bin", 12_582_912));
    // …et le processus meurt au milieu de la troisième ligne.
    fichier.push_str("+7 \"perd");

    let (relu, ignorees) = rejouer(&fichier);
    assert_eq!(ignorees, 1, "la ligne tronquée doit être COMPTÉE, pas tue");
    assert_eq!(
        relu.dues(),
        [("note.txt".to_string(), 42), ("dossier/gros.bin".to_string(), 12_582_912)],
        "les deux entrées ANTÉRIEURES doivent survivre"
    );
}

/// 🔴 **UN RETRAIT EFFACE SON INSCRIPTION, ET PAS UNE AUTRE.**
///
/// Retirer par préfixe ferait que `note.txt` effacerait `note.txt.bak`, et
/// qu'un dossier effacerait tout ce qu'il contient — c'est-à-dire une perte de
/// données silencieuse, produite par le module qui existe pour l'empêcher.
#[test]
fn un_retrait_efface_l_inscription_et_pas_une_autre() {
    let mut j = Journal::nouveau();
    let mut f = String::new();
    f.push_str(&j.inscrire("note.txt", 1));
    f.push_str(&j.inscrire("note.txt.bak", 2));
    f.push_str(&j.inscrire("dossier", 3));
    f.push_str(&j.inscrire("dossier/enfant.txt", 4));
    f.push_str(&j.retirer("note.txt"));
    f.push_str(&j.retirer("dossier"));

    for etat in [j.clone(), rejouer(&f).0] {
        assert_eq!(
            etat.dues(),
            [("note.txt.bak".to_string(), 2), ("dossier/enfant.txt".to_string(), 4)],
            "seuls les chemins EXACTS devaient partir"
        );
    }
}

/// L'ordre d'inscription est conservé — c'est ce qui rend la reprise
/// déterministe. Un `HashMap` rendrait un ordre différent à chaque exécution,
/// et la reprise d'un lot d'écritures deviendrait irreproductible.
#[test]
fn l_ordre_d_inscription_est_conserve() {
    let mut j = Journal::nouveau();
    let mut f = String::new();
    for i in 0..16u64 {
        f.push_str(&j.inscrire(&format!("f{i:02}.txt"), i));
    }
    let attendus: Vec<String> = (0..16).map(|i| format!("f{i:02}.txt")).collect();
    let vus: Vec<String> = rejouer(&f).0.dues().iter().map(|(c, _)| c.clone()).collect();
    assert_eq!(vus, attendus);
}

/// Un rejeu met à jour les octets **sans changer de place**.
///
/// Faire remonter l'entrée en queue ferait passer devant elle des écritures
/// plus jeunes, alors qu'elle attend depuis plus longtemps.
#[test]
fn un_rejeu_met_a_jour_les_octets_sans_changer_de_place() {
    let mut j = Journal::nouveau();
    let mut f = String::new();
    f.push_str(&j.inscrire("a.txt", 1));
    f.push_str(&j.inscrire("b.txt", 2));
    f.push_str(&j.inscrire("a.txt", 999));

    for etat in [j.clone(), rejouer(&f).0] {
        assert_eq!(etat.dues(), [("a.txt".to_string(), 999), ("b.txt".to_string(), 2)]);
    }
}

/// 🔴 **UN CHEMIN À SAUT DE LIGNE SURVIT À UN ALLER-RETOUR.**
///
/// ⚠️ **Ce n'est pas une coquetterie.** La File System Access API tourne dans
/// un navigateur qui peut être sur macOS ou sur Linux, où `\n` est un caractère
/// de nom de fichier **licite**. Encoder le chemin brut couperait l'entrée en
/// deux lignes : la première serait illisible, la seconde serait interprétée
/// comme un enregistrement d'un autre genre.
#[test]
fn un_chemin_a_saut_de_ligne_survit_a_un_aller_retour() {
    let tordus = [
        "dossier/nom\navec saut.txt",
        "éphémère été.txt",
        "guillemet\"et\\antislash.txt",
        "espaces    multiples.txt",
        "+trompeur -aussi.txt",
    ];
    let mut j = Journal::nouveau();
    let mut f = String::new();
    for (i, chemin) in tordus.iter().enumerate() {
        f.push_str(&j.inscrire(chemin, i as u64));
    }
    let (relu, ignorees) = rejouer(&f);
    assert_eq!(ignorees, 0, "aucune ligne ne devait être illisible");
    let vus: Vec<&str> = relu.dues().iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(vus, tordus);
    // …et le retrait retrouve le même chemin.
    let mut relu = relu;
    f.push_str(&relu.retirer(tordus[0]));
    assert_eq!(rejouer(&f).0.compte(), tordus.len() - 1);
}

/// 🔴 **UN JOURNAL VIDE SE COMPACTE, UN JOURNAL NON VIDE JAMAIS.**
///
/// Compacter inconditionnellement perdrait une due **exactement quand elle
/// sert** : un journal gros est un journal où beaucoup d'écritures ont échoué.
#[test]
fn un_journal_vide_se_compacte_et_un_journal_non_vide_jamais() {
    let mut j = Journal::nouveau();
    assert!(!j.compactable(TAILLE_JOURNAL_COMPACTAGE), "au seuil exact : pas encore");
    assert!(j.compactable(TAILLE_JOURNAL_COMPACTAGE + 1));

    j.inscrire("une seule due.txt", 1);
    assert!(
        !j.compactable(u64::MAX),
        "un journal PORTANT une due ne se compacte JAMAIS, si gros soit-il"
    );

    j.retirer("une seule due.txt");
    assert!(j.compactable(TAILLE_JOURNAL_COMPACTAGE + 1), "vidé, il redevient compactable");
}

/// Une ligne d'un genre inconnu est comptée et jetée, jamais fatale.
#[test]
fn une_ligne_de_genre_inconnu_est_comptee_et_jetee() {
    let (relu, ignorees) = rejouer("+1 \"a.txt\"\n?que suis-je\n+2 \"b.txt\"\n");
    assert_eq!(ignorees, 1);
    assert_eq!(relu.compte(), 2, "les deux lignes licites restent");
}

/// Un journal vide, ou réduit à des retraits, se relit sans rien inventer.
#[test]
fn un_journal_vide_ou_de_retraits_seuls_se_relit_sans_rien_inventer() {
    assert_eq!(rejouer("").0.compte(), 0);
    assert_eq!(rejouer("\n\n").0.compte(), 0);
    let (relu, ignorees) = rejouer("-\"jamais inscrit.txt\"\n");
    assert_eq!(ignorees, 0, "un retrait d'un chemin absent est LICITE, pas illisible");
    assert_eq!(relu.compte(), 0);
}

/// Un nombre d'octets illisible ne fait pas passer l'entrée pour une autre.
#[test]
fn un_nombre_d_octets_illisible_rend_la_ligne_illisible() {
    let (relu, ignorees) = rejouer("+beaucoup \"a.txt\"\n");
    assert_eq!(ignorees, 1);
    assert_eq!(relu.compte(), 0);
}
