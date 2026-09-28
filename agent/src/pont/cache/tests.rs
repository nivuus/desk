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
fn a_never_set_path_is_not_memorised() {
    let mut c = CacheEnumeration::new();
    assert!(c.lire("dossier", t0()).is_none());
    assert_eq!(c.size(), 0);
}

#[test]
fn what_is_set_is_read_back_identically() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt"), e("b.txt")], t);
    let lues = c.lire("dossier", t).expect("memorised");
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
    assert_eq!(c.size(), 0, "the expired entry must be removed, not kept");
}

#[test]
fn setting_twice_overwrites_and_re_arms_the_clock() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("vieux.txt")], t);
    let tard = t + TTL_ENUMERATION - Duration::from_millis(1);
    c.poser("dossier".into(), vec![e("neuf.txt")], tard);
    assert_eq!(c.size(), 1, "the second set overwrites, it does not add");
    let lues = c
        .lire("dossier", tard + Duration::from_millis(1))
        .expect("re-armed");
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
fn invalidating_forgets_the_parent_directory_of_the_mutated_path() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    c.invalider("dossier/neuf.txt");
    assert!(
        c.lire("dossier", t).is_none(),
        "the PARENT must be forgotten"
    );
}

/// ⚠️ **The parent of a path without a separator is the ROOT, `""`** — the key
/// a `Get-ChildItem` on the mounted drive solicits. Forgetting it would make
/// a creation at the root invisible: it is exactly the gesture of
/// criterion ① of F5's acceptance run.
#[test]
fn the_parent_of_a_first_level_path_is_the_root() {
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
        "the root must be forgotten on a first-level creation"
    );
}

/// Invalidating a directory touches **no other**: a cache that would
/// empty itself entirely at each write would not be a cache.
#[test]
fn invalidating_does_not_touch_neighbouring_directories() {
    let mut c = CacheEnumeration::new();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.poser("b".into(), vec![e("y.txt")], t);
    c.invalider("a/neuf.txt");
    assert!(c.lire("a", t).is_none());
    assert!(c.lire("b", t).is_some(), "the neighbour must survive");
}

/// Invalidating a path never memorised must do **nothing**, and above all not
/// panic: ProjFS notifications arrive for paths the bridge
/// has never listed.
#[test]
fn invalidating_an_unknown_path_is_harmless() {
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
    let lues = c.lire("vide", t).expect("memorised, although empty");
    assert!(lues.is_empty());
}
