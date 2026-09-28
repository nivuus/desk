use super::*;

fn e(nom: &str) -> Entree {
    Entree {
        nom: nom.to_string(),
        repertoire: false,
        size: 1,
        modified_ms: 0,
    }
}

fn t0() -> Instant {
    Instant::now()
}

#[test]
fn un_chemin_jamais_pose_n_est_pas_memorise() {
    let mut c = CacheEnumeration::new();
    assert!(c.lire("dossier", t0()).is_none());
    assert_eq!(c.size(), 0);
}

#[test]
fn ce_qui_est_pose_est_relu_a_l_identique() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt"), e("b.txt")], t);
    let lues = c.lire("dossier", t).expect("mémorisé");
    assert_eq!(lues.len(), 2);
    assert_eq!(lues[0].nom, "a.txt");
    assert_eq!(lues[1].nom, "b.txt");
}

/// 🔴 **EXPIRY WITHOUT SLEEPING** — it is what the injected clock buys.
/// The bound is besieged from BOTH sides, otherwise a `>` put in place of a `>=`
/// would go unnoticed.
#[test]
fn a_memory_expires_at_the_exact_term_and_not_earlier() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    assert!(c
        .lire("dossier", t + TTL_ENUMERATION - Duration::from_millis(1))
        .is_some());
    assert!(c.lire("dossier", t + TTL_ENUMERATION).is_none());
}

/// An expired memory is **REMOVED**, not merely ignored: without that the
/// cache would grow endlessly on a tree traversed once, and this
/// module has **no** eviction policy (spec §10 R4).
#[test]
fn an_expired_memory_is_removed_not_merely_ignored() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    assert_eq!(c.size(), 1);
    let _ = c.lire("dossier", t + TTL_ENUMERATION);
    assert_eq!(
        c.size(),
        0,
        "l'entrée expirée doit être retirée, pas gardée"
    );
}

#[test]
fn poser_deux_fois_ecrase_et_rearme_l_horloge() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("vieux.txt")], t);
    let tard = t + TTL_ENUMERATION - Duration::from_millis(1);
    c.poser("dossier".into(), vec![e("neuf.txt")], tard);
    assert_eq!(c.size(), 1, "la seconde pose écrase, elle n'ajoute pas");
    let lues = c
        .lire("dossier", tard + Duration::from_millis(1))
        .expect("réarmée");
    assert_eq!(lues[0].nom, "neuf.txt");
}

/// 🔴 **`invalider` TAKES THE PARENT, NEVER THE PATH ITSELF.**
///
/// **Its RED**: make [`CacheEnumeration::invalider`] take the path
/// itself instead of `parent_de(chemin)`. This test then fails — and the error
/// it locks is **silent** in production: the parent's listing
/// would keep being served from memory, and the created file
/// would never appear.
#[test]
fn invalider_oublie_le_repertoire_parent_du_chemin_mute() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    c.invalider("dossier/neuf.txt");
    assert!(c.lire("dossier", t).is_none(), "le PARENT doit être oublié");
}

/// ⚠️ **The parent of a path without a separator is the ROOT, `""`** — the key
/// a `Get-ChildItem` on the mounted drive solicits. Forgetting it would make
/// a creation at the root invisible: it is exactly the gesture of
/// criterion ① of F5's acceptance run.
#[test]
fn le_parent_d_un_chemin_de_premier_niveau_est_la_racine() {
    assert_eq!(parent_de("note.txt"), "");
    assert_eq!(parent_de("dossier/note.txt"), "dossier");
    assert_eq!(parent_de("a/b/c.txt"), "a/b");
    assert_eq!(parent_de(""), "");
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser(String::new(), vec![e("a.txt")], t);
    c.invalider("neuf.txt");
    assert!(
        c.lire("", t).is_none(),
        "la racine doit être oubliée sur une création de premier niveau"
    );
}

/// Invalidating a directory touches **no other**: a cache that would
/// empty itself entirely at each write would not be a cache.
#[test]
fn invalider_ne_touche_pas_les_repertoires_voisins() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.poser("b".into(), vec![e("y.txt")], t);
    c.invalider("a/neuf.txt");
    assert!(c.lire("a", t).is_none());
    assert!(c.lire("b", t).is_some(), "le voisin doit survivre");
}

/// Invalidating a path never memorised must do **nothing**, and above all not
/// panic: ProjFS notifications arrive for paths the bridge
/// has never listed.
#[test]
fn invalider_un_chemin_inconnu_est_inoffensif() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.invalider("jamais/vu.txt");
    assert_eq!(c.size(), 1);
    assert!(c.lire("a", t).is_some());
}

/// `drain` is what the `Rafraichir` announcement does: everything, at once.
#[test]
fn drain_forgets_everything() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.poser("b".into(), vec![e("y.txt")], t);
    c.poser(String::new(), vec![e("z.txt")], t);
    assert_eq!(c.size(), 3);
    c.drain();
    assert_eq!(c.size(), 0);
    assert!(c.lire("a", t).is_none());
    assert!(c.lire("", t).is_none());
}

/// A memorised **empty** directory is not the same thing as a directory
/// never memorised: confusing them would make each listing of an empty folder
/// pay a round trip again.
#[test]
fn a_remembered_empty_directory_is_served_as_such() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("vide".into(), Vec::new(), t);
    let lues = c.lire("vide", t).expect("mémorisé, quoique vide");
    assert!(lues.is_empty());
}
