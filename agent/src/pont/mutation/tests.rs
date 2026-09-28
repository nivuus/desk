//! The scheduling of mutations. **Pure, run on the host.**

use super::*;

fn renommer(de: &str, vers: &str) -> Mutation {
    Mutation::Renommer {
        de: de.into(),
        vers: vers.into(),
        repertoire: false,
    }
}
fn renommer_dossier(de: &str, vers: &str) -> Mutation {
    Mutation::Renommer {
        de: de.into(),
        vers: vers.into(),
        repertoire: true,
    }
}
fn remove(chemin: &str) -> Mutation {
    Mutation::Delete {
        chemin: chemin.into(),
        repertoire: false,
    }
}
fn remove_folder(chemin: &str) -> Mutation {
    Mutation::Delete {
        chemin: chemin.into(),
        repertoire: true,
    }
}
fn dues(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// 🔴 **THE TEST THAT PREVENTS LOSING LIBREOFFICE'S SAVE.**
///
/// Red: return `Pousser`. The due write would go out AFTER the renaming, on
/// a path that no longer exists, and `getFileHandle(…, { create: true })`
/// **would recreate the temporary file** — the save would be lost, and a
/// swap file would remain on the local workstation.
#[test]
fn un_renommage_attend_les_ecritures_dues_sur_la_source() {
    assert_eq!(
        ordonnancer(&dues(&["doc.tmp"]), &renommer("doc.tmp", "doc.odt")),
        Ordonnancement::AttendreEcrituresDues {
            chemins: dues(&["doc.tmp"])
        }
    );
}

/// 🔴 **THE TEST THAT PREVENTS AN ERASED FILE FROM REAPPEARING.**
///
/// Red: return `Pousser`. The due write would recreate on the local workstation what
/// the user has just erased.
#[test]
fn une_suppression_abandonne_les_ecritures_dues_sur_le_chemin() {
    assert_eq!(
        ordonnancer(&dues(&["vieux.odt"]), &remove("vieux.odt")),
        Ordonnancement::AbandonnerEcrituresDues {
            chemins: dues(&["vieux.odt"])
        }
    );
}

/// 🔴 **THE TWO SCHEDULINGS ARE NOT INTERCHANGEABLE.**
///
/// Abandoning the writes of a RENAMING would lose the save; waiting for
/// those of a DELETION would push the file before erasing it, which would
/// resurrect it if the deletion then fails.
#[test]
fn un_renommage_attend_la_ou_une_suppression_abandonne() {
    let r = ordonnancer(&dues(&["a.txt"]), &renommer("a.txt", "b.txt"));
    let s = ordonnancer(&dues(&["a.txt"]), &remove("a.txt"));
    assert_ne!(r, s);
    assert!(matches!(r, Ordonnancement::AttendreEcrituresDues { .. }));
    assert!(matches!(s, Ordonnancement::AbandonnerEcrituresDues { .. }));
}

/// Red: compare by PREFIX instead of equality — every write would
/// then block every renaming, and the bridge would freeze on the first large file.
#[test]
fn une_ecriture_due_sur_un_autre_chemin_ne_retarde_rien() {
    assert_eq!(
        ordonnancer(
            &dues(&["autre.txt", "dossier/x.bin"]),
            &renommer("doc.tmp", "doc.odt")
        ),
        Ordonnancement::Pousser
    );
}

/// Red: compare by EQUALITY alone — the directory case would slip
/// through, and the child would be recreated under the OLD path, outside the renamed
/// directory.
///
/// ⚠️ **This test and the previous one would contradict each other if file and
/// directory were confused**: it is `repertoire` that decides, and that is why it is
/// carried from the callback.
#[test]
fn une_ecriture_due_sur_un_enfant_du_repertoire_renomme_retarde() {
    assert_eq!(
        ordonnancer(
            &dues(&["projet/note.txt"]),
            &renommer_dossier("projet", "archives/projet")
        ),
        Ordonnancement::AttendreEcrituresDues {
            chemins: dues(&["projet/note.txt"])
        }
    );
    // …and the same path does NOT delay the renaming of a FILE of the same name.
    assert_eq!(
        ordonnancer(&dues(&["projet/note.txt"]), &renommer("projet", "autre")),
        Ordonnancement::Pousser
    );
}

/// 🔴 **THE PREFIX'S `/` IS NOT DECORATIVE.**
///
/// Red: `due.starts_with(cible)` without the separator. Renaming `a`
/// would then hold back a write due on `ab/x`, which is unrelated — and the
/// renaming would wait for a write that does not concern it, indefinitely if
/// it fails.
#[test]
fn un_prefixe_qui_n_est_pas_un_composant_ne_retarde_rien() {
    assert_eq!(
        ordonnancer(&dues(&["ab/x.txt", "a2.txt"]), &renommer_dossier("a", "z")),
        Ordonnancement::Pousser
    );
    assert_eq!(
        ordonnancer(&dues(&["a/x.txt"]), &renommer_dossier("a", "z")),
        Ordonnancement::AttendreEcrituresDues {
            chemins: dues(&["a/x.txt"])
        }
    );
}

/// The deleted directory takes its due children with it, too.
#[test]
fn une_suppression_de_repertoire_abandonne_les_ecritures_de_ses_enfants() {
    assert_eq!(
        ordonnancer(
            &dues(&["d/a.txt", "d/sous/b.txt", "hors.txt"]),
            &remove_folder("d")
        ),
        Ordonnancement::AbandonnerEcrituresDues {
            chemins: dues(&["d/a.txt", "d/sous/b.txt"])
        }
    );
}

/// ⚠️ **The root is never renamed nor deleted**, and the case is refused
/// rather than treated as "everything is a child" — which would hold back every
/// write forever, on a bridge that would appear to work.
#[test]
fn une_cible_vide_ne_retient_rien() {
    assert_eq!(
        ordonnancer(&dues(&["a.txt", "b/c.txt"]), &remove_folder("")),
        Ordonnancement::Pousser
    );
}

/// Without any due write, there is nothing to schedule.
#[test]
fn sans_ecriture_due_on_pousse() {
    assert_eq!(
        ordonnancer(&[], &renommer("a", "b")),
        Ordonnancement::Pousser
    );
    assert_eq!(ordonnancer(&[], &remove("a")), Ordonnancement::Pousser);
}

/// 🔴 **ONE MUTATION AT A TIME.**
///
/// Red: allow two in flight. Two renamings of the same path would
/// cross, and the order of their `Fait`s would decide the final name — that
/// is, the network's chance.
#[test]
fn une_seule_mutation_en_vol_a_la_fois() {
    let mut f = FileMutations::new();
    assert_eq!(f.signaler(renommer("a", "b")), Some(renommer("a", "b")));
    assert_eq!(f.signaler(renommer("b", "c")), None, "la seconde attend");
    assert_eq!(f.en_attente(), 1);
    assert_eq!(f.terminee(), Some(renommer("b", "c")));
    assert_eq!(f.terminee(), None, "plus rien");
}

/// 🔴 **NO COALESCING — the order of mutations IS their meaning.**
///
/// Red: merge two mutations of the same path as the write queue
/// does with two writes. `a`→`b` then `b`→`c` would leave `b` on the local
/// workstation, or would lose the file depending on the merge chosen.
#[test]
fn two_mutations_of_the_same_path_are_both_played_in_order() {
    let mut f = FileMutations::new();
    f.signaler(renommer("a", "b"));
    f.signaler(remove("a"));
    f.signaler(renommer("a", "c"));
    assert_eq!(f.en_attente(), 2);
    assert_eq!(f.terminee(), Some(remove("a")));
    assert_eq!(f.terminee(), Some(renommer("a", "c")));
    assert_eq!(f.terminee(), None);
}

/// A deferred mutation goes back **AHEAD** of those that followed it.
///
/// Red: put it back at the TAIL. The order of the user's gestures would be
/// reversed — they would see the second renaming take effect before the first.
#[test]
fn une_mutation_differee_repasse_devant() {
    let mut f = FileMutations::new();
    assert_eq!(f.signaler(renommer("a", "b")), Some(renommer("a", "b")));
    f.signaler(renommer("x", "y"));
    f.differer();
    assert!(f.en_vol().is_none(), "différer libère le vol");
    assert_eq!(
        f.terminee(),
        Some(renommer("a", "b")),
        "la différée repasse la PREMIÈRE"
    );
}

/// `source()` returns the SOURCE of a renaming, never the destination: it is on
/// it that bytes can be due.
#[test]
fn source_rend_le_chemin_qui_existe_encore() {
    assert_eq!(renommer("de.txt", "vers.txt").source(), "de.txt");
    assert_eq!(remove("c.txt").source(), "c.txt");
    assert!(renommer_dossier("d", "e").repertoire());
    assert!(!renommer("d", "e").repertoire());
}
