//! Tests of the resumption journal. **Pure, run on the host.**

use super::*;

/// Replays a journal starting again from its lines, as a restarted bridge
/// would.
fn rejouer(lignes: &str) -> (Journal, usize) {
    Journal::relire(lignes)
}

/// 🔴 **A TRUNCATED LAST LINE DOES NOT LOSE THE PREVIOUS ONES.**
///
/// It is the only damage an abrupt stop can cause to an append-only
/// file — the bridge can die in the middle of a `write`. Raising, or throwing away the
/// whole file, would lose **intact** writes: it is the red
/// spec §4.4 names for this module.
#[test]
fn une_derniere_ligne_tronquee_ne_fait_pas_perdre_les_precedentes() {
    let mut j = Journal::nouveau();
    let mut fichier = String::new();
    fichier.push_str(&j.inscrire("note.txt", 42));
    fichier.push_str(&j.inscrire("dossier/gros.bin", 12_582_912));
    // …and the process dies in the middle of the third line.
    fichier.push_str("+7 \"perd");

    let (relu, ignorees) = rejouer(&fichier);
    assert_eq!(ignorees, 1, "la ligne tronquée doit être COMPTÉE, pas tue");
    assert_eq!(
        relu.dues(),
        [
            ("note.txt".to_string(), 42),
            ("dossier/gros.bin".to_string(), 12_582_912)
        ],
        "les deux entrées ANTÉRIEURES doivent survivre"
    );
}

/// 🔴 **A REMOVAL ERASES ITS REGISTRATION, AND NOT ANOTHER.**
///
/// Removing by prefix would mean that `note.txt` would erase `note.txt.bak`, and
/// that a folder would erase everything it contains — that is, a silent
/// data loss, produced by the module that exists to prevent it.
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
            [
                ("note.txt.bak".to_string(), 2),
                ("dossier/enfant.txt".to_string(), 4)
            ],
            "seuls les chemins EXACTS devaient partir"
        );
    }
}

/// The registration order is kept — it is what makes resumption
/// deterministic. A `HashMap` would give a different order at each run,
/// and the resumption of a batch of writes would become irreproducible.
#[test]
fn l_ordre_d_inscription_est_conserve() {
    let mut j = Journal::nouveau();
    let mut f = String::new();
    for i in 0..16u64 {
        f.push_str(&j.inscrire(&format!("f{i:02}.txt"), i));
    }
    let attendus: Vec<String> = (0..16).map(|i| format!("f{i:02}.txt")).collect();
    let vus: Vec<String> = rejouer(&f)
        .0
        .dues()
        .iter()
        .map(|(c, _)| c.clone())
        .collect();
    assert_eq!(vus, attendus);
}

/// A replay updates the bytes **without changing place**.
///
/// Moving the entry back to the tail would let younger writes pass
/// ahead of it, whereas it has been waiting longer.
#[test]
fn un_rejeu_met_a_jour_les_octets_sans_changer_de_place() {
    let mut j = Journal::nouveau();
    let mut f = String::new();
    f.push_str(&j.inscrire("a.txt", 1));
    f.push_str(&j.inscrire("b.txt", 2));
    f.push_str(&j.inscrire("a.txt", 999));

    for etat in [j.clone(), rejouer(&f).0] {
        assert_eq!(
            etat.dues(),
            [("a.txt".to_string(), 999), ("b.txt".to_string(), 2)]
        );
    }
}

/// 🔴 **A PATH WITH A LINE BREAK SURVIVES A ROUND TRIP.**
///
/// ⚠️ **It is not an affectation.** The File System Access API runs in
/// a browser that can be on macOS or Linux, where `\n` is a **legal**
/// file-name character. Encoding the raw path would cut the entry into
/// two lines: the first would be unreadable, the second would be interpreted
/// as a record of another kind.
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
    // …and the removal finds the same path.
    let mut relu = relu;
    f.push_str(&relu.retirer(tordus[0]));
    assert_eq!(rejouer(&f).0.compte(), tordus.len() - 1);
}

/// 🔴 **AN EMPTY JOURNAL IS COMPACTED, A NON-EMPTY JOURNAL NEVER.**
///
/// Compacting unconditionally would lose a due entry **exactly when it
/// matters**: a large journal is a journal where many writes failed.
#[test]
fn un_journal_vide_se_compacte_et_un_journal_non_vide_jamais() {
    let mut j = Journal::nouveau();
    assert!(
        !j.compactable(TAILLE_JOURNAL_COMPACTAGE),
        "au seuil exact : pas encore"
    );
    assert!(j.compactable(TAILLE_JOURNAL_COMPACTAGE + 1));

    j.inscrire("une seule due.txt", 1);
    assert!(
        !j.compactable(u64::MAX),
        "un journal PORTANT une due ne se compacte JAMAIS, si gros soit-il"
    );

    j.retirer("une seule due.txt");
    assert!(
        j.compactable(TAILLE_JOURNAL_COMPACTAGE + 1),
        "vidé, il redevient compactable"
    );
}

/// A line of an unknown kind is counted and thrown away, never fatal.
#[test]
fn une_ligne_de_genre_inconnu_est_comptee_et_jetee() {
    let (relu, ignorees) = rejouer("+1 \"a.txt\"\n?que suis-je\n+2 \"b.txt\"\n");
    assert_eq!(ignorees, 1);
    assert_eq!(relu.compte(), 2, "les deux lignes licites restent");
}

/// An empty journal, or one reduced to removals, is reread without inventing anything.
#[test]
fn un_journal_vide_ou_de_retraits_seuls_se_relit_sans_rien_inventer() {
    assert_eq!(rejouer("").0.compte(), 0);
    assert_eq!(rejouer("\n\n").0.compte(), 0);
    let (relu, ignorees) = rejouer("-\"jamais inscrit.txt\"\n");
    assert_eq!(
        ignorees, 0,
        "un retrait d'un chemin absent est LICITE, pas illisible"
    );
    assert_eq!(relu.compte(), 0);
}

/// An unreadable byte count does not make the entry pass for another.
#[test]
fn un_nombre_d_octets_illisible_rend_la_ligne_illisible() {
    let (relu, ignorees) = rejouer("+beaucoup \"a.txt\"\n");
    assert_eq!(ignorees, 1);
    assert_eq!(relu.compte(), 0);
}
