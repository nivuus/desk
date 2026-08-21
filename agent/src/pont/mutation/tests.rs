//! L'ordonnancement des mutations. **Purs, exécutés sur l'hôte.**

use super::*;

fn renommer(de: &str, vers: &str) -> Mutation {
    Mutation::Renommer { de: de.into(), vers: vers.into(), repertoire: false }
}
fn renommer_dossier(de: &str, vers: &str) -> Mutation {
    Mutation::Renommer { de: de.into(), vers: vers.into(), repertoire: true }
}
fn supprimer(chemin: &str) -> Mutation {
    Mutation::Supprimer { chemin: chemin.into(), repertoire: false }
}
fn supprimer_dossier(chemin: &str) -> Mutation {
    Mutation::Supprimer { chemin: chemin.into(), repertoire: true }
}
fn dues(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// 🔴 **LE TEST QUI EMPÊCHE LA PERTE DE L'ENREGISTREMENT DE LIBREOFFICE.**
///
/// Rouge : rendre `Pousser`. L'écriture due partirait APRÈS le renommage, sur
/// un chemin qui n'existe plus, et `getFileHandle(…, { create: true })`
/// **recréerait le fichier temporaire** — l'enregistrement serait perdu, et un
/// fichier d'échange resterait sur le poste local.
#[test]
fn un_renommage_attend_les_ecritures_dues_sur_la_source() {
    assert_eq!(
        ordonnancer(&dues(&["doc.tmp"]), &renommer("doc.tmp", "doc.odt")),
        Ordonnancement::AttendreEcrituresDues { chemins: dues(&["doc.tmp"]) }
    );
}

/// 🔴 **LE TEST QUI EMPÊCHE UN FICHIER EFFACÉ DE RÉAPPARAÎTRE.**
///
/// Rouge : rendre `Pousser`. L'écriture due recréerait sur le poste local ce
/// que l'utilisateur vient d'effacer.
#[test]
fn une_suppression_abandonne_les_ecritures_dues_sur_le_chemin() {
    assert_eq!(
        ordonnancer(&dues(&["vieux.odt"]), &supprimer("vieux.odt")),
        Ordonnancement::AbandonnerEcrituresDues { chemins: dues(&["vieux.odt"]) }
    );
}

/// 🔴 **LES DEUX ORDONNANCEMENTS NE SONT PAS INTERCHANGEABLES.**
///
/// Abandonner les écritures d'un RENOMMAGE perdrait l'enregistrement ; attendre
/// celles d'une SUPPRESSION pousserait le fichier avant de l'effacer, ce qui le
/// ressusciterait si la suppression échoue ensuite.
#[test]
fn un_renommage_attend_la_ou_une_suppression_abandonne() {
    let r = ordonnancer(&dues(&["a.txt"]), &renommer("a.txt", "b.txt"));
    let s = ordonnancer(&dues(&["a.txt"]), &supprimer("a.txt"));
    assert_ne!(r, s);
    assert!(matches!(r, Ordonnancement::AttendreEcrituresDues { .. }));
    assert!(matches!(s, Ordonnancement::AbandonnerEcrituresDues { .. }));
}

/// Rouge : comparer par PRÉFIXE au lieu d'égalité — toute écriture bloquerait
/// alors tout renommage, et le pont se figerait sur le premier gros fichier.
#[test]
fn une_ecriture_due_sur_un_AUTRE_chemin_ne_retarde_rien() {
    assert_eq!(
        ordonnancer(&dues(&["autre.txt", "dossier/x.bin"]), &renommer("doc.tmp", "doc.odt")),
        Ordonnancement::Pousser
    );
}

/// Rouge : comparer par ÉGALITÉ seule — le cas du répertoire passerait à
/// travers, et l'enfant serait recréé sous l'ANCIEN chemin, hors du répertoire
/// renommé.
///
/// ⚠️ **Ce test et le précédent se contrediraient si l'on confondait fichier et
/// répertoire** : c'est `repertoire` qui tranche, et c'est pour cela qu'il est
/// transporté depuis le rappel.
#[test]
fn une_ecriture_due_sur_un_ENFANT_du_repertoire_renomme_retarde() {
    assert_eq!(
        ordonnancer(&dues(&["projet/note.txt"]), &renommer_dossier("projet", "archives/projet")),
        Ordonnancement::AttendreEcrituresDues { chemins: dues(&["projet/note.txt"]) }
    );
    // …et le même chemin ne retarde PAS le renommage d'un FICHIER homonyme.
    assert_eq!(
        ordonnancer(&dues(&["projet/note.txt"]), &renommer("projet", "autre")),
        Ordonnancement::Pousser
    );
}

/// 🔴 **LE `/` DU PRÉFIXE N'EST PAS DÉCORATIF.**
///
/// Rouge : `due.starts_with(cible)` sans le séparateur. Renommer `a`
/// retiendrait alors une écriture due sur `ab/x`, qui n'a rien à voir — et le
/// renommage attendrait une écriture qui ne le concerne pas, indéfiniment si
/// elle échoue.
#[test]
fn un_prefixe_qui_n_est_pas_un_composant_ne_retarde_rien() {
    assert_eq!(
        ordonnancer(&dues(&["ab/x.txt", "a2.txt"]), &renommer_dossier("a", "z")),
        Ordonnancement::Pousser
    );
    assert_eq!(
        ordonnancer(&dues(&["a/x.txt"]), &renommer_dossier("a", "z")),
        Ordonnancement::AttendreEcrituresDues { chemins: dues(&["a/x.txt"]) }
    );
}

/// Le répertoire supprimé emporte ses enfants dus, eux aussi.
#[test]
fn une_suppression_de_repertoire_abandonne_les_ecritures_de_ses_enfants() {
    assert_eq!(
        ordonnancer(&dues(&["d/a.txt", "d/sous/b.txt", "hors.txt"]), &supprimer_dossier("d")),
        Ordonnancement::AbandonnerEcrituresDues { chemins: dues(&["d/a.txt", "d/sous/b.txt"]) }
    );
}

/// ⚠️ **La racine n'est jamais renommée ni supprimée**, et le cas est refusé
/// plutôt que traité comme « tout est enfant » — ce qui retiendrait toute
/// écriture pour toujours, sur un pont qui aurait l'air de fonctionner.
#[test]
fn une_cible_vide_ne_retient_rien() {
    assert_eq!(
        ordonnancer(&dues(&["a.txt", "b/c.txt"]), &supprimer_dossier("")),
        Ordonnancement::Pousser
    );
}

/// Sans aucune écriture due, il n'y a rien à ordonnancer.
#[test]
fn sans_ecriture_due_on_pousse() {
    assert_eq!(ordonnancer(&[], &renommer("a", "b")), Ordonnancement::Pousser);
    assert_eq!(ordonnancer(&[], &supprimer("a")), Ordonnancement::Pousser);
}

/// 🔴 **UNE MUTATION À LA FOIS.**
///
/// Rouge : autoriser deux en vol. Deux renommages du même chemin se
/// croiseraient, et l'ordre de leurs `Fait` déciderait du nom final — c'est-à-
/// dire le hasard du réseau.
#[test]
fn une_seule_mutation_en_vol_a_la_fois() {
    let mut f = FileMutations::nouvelle();
    assert_eq!(f.signaler(renommer("a", "b")), Some(renommer("a", "b")));
    assert_eq!(f.signaler(renommer("b", "c")), None, "la seconde attend");
    assert_eq!(f.en_attente(), 1);
    assert_eq!(f.terminee(), Some(renommer("b", "c")));
    assert_eq!(f.terminee(), None, "plus rien");
}

/// 🔴 **AUCUNE COALESCENCE — l'ordre des mutations EST leur sens.**
///
/// Rouge : fusionner deux mutations du même chemin comme la file d'écriture le
/// fait de deux écritures. `a`→`b` puis `b`→`c` laisserait `b` sur le poste
/// local, ou perdrait le fichier selon la fusion retenue.
#[test]
fn deux_mutations_du_meme_chemin_sont_toutes_deux_jouees_dans_l_ordre() {
    let mut f = FileMutations::nouvelle();
    f.signaler(renommer("a", "b"));
    f.signaler(supprimer("a"));
    f.signaler(renommer("a", "c"));
    assert_eq!(f.en_attente(), 2);
    assert_eq!(f.terminee(), Some(supprimer("a")));
    assert_eq!(f.terminee(), Some(renommer("a", "c")));
    assert_eq!(f.terminee(), None);
}

/// Une mutation différée repasse **DEVANT** celles qui l'ont suivie.
///
/// Rouge : la remettre en QUEUE. L'ordre des gestes de l'utilisateur serait
/// inversé — il verrait le second renommage prendre effet avant le premier.
#[test]
fn une_mutation_differee_repasse_devant() {
    let mut f = FileMutations::nouvelle();
    assert_eq!(f.signaler(renommer("a", "b")), Some(renommer("a", "b")));
    f.signaler(renommer("x", "y"));
    f.differer();
    assert!(f.en_vol().is_none(), "différer libère le vol");
    assert_eq!(f.terminee(), Some(renommer("a", "b")), "la différée repasse la PREMIÈRE");
}

/// `source()` rend la SOURCE d'un renommage, jamais la destination : c'est sur
/// elle que des octets peuvent être dus.
#[test]
fn source_rend_le_chemin_qui_existe_encore() {
    assert_eq!(renommer("de.txt", "vers.txt").source(), "de.txt");
    assert_eq!(supprimer("c.txt").source(), "c.txt");
    assert!(renommer_dossier("d", "e").repertoire());
    assert!(!renommer("d", "e").repertoire());
}
