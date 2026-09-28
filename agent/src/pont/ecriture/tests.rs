//! Tests of the due writes queue. **Pure, run on the host.**

use super::*;

fn modified(chemin: &str) -> Evenement {
    Evenement::Modified {
        chemin: chemin.to_string(),
    }
}
fn cree(chemin: &str) -> Evenement {
    Evenement::Cree {
        chemin: chemin.to_string(),
        repertoire: false,
    }
}
fn cree_dossier(chemin: &str) -> Evenement {
    Evenement::Cree {
        chemin: chemin.to_string(),
        repertoire: true,
    }
}

/// 🔴 **TWO NOTIFICATIONS OF THE SAME PATH MAKE ONLY ONE PUSH IN FLIGHT.**
///
/// Letting the second through would open two concurrent `createWritable()` streams
/// on the same file, which would overwrite each other.
#[test]
fn two_notifications_of_the_same_path_make_only_one_push_in_flight() {
    let mut f = File::new();
    assert_eq!(f.signaler(modified("a.txt")), Some(modified("a.txt")));
    assert_eq!(
        f.signaler(modified("a.txt")),
        None,
        "the second starts NOTHING"
    );
    assert_eq!(f.en_vol(), Some("a.txt"));
    assert_eq!(f.en_attente(), 0);
}

/// 🔴 **A NOTIFICATION DURING A PUSH IS REPLAYED AFTERWARDS.**
///
/// Throwing it away would lose **the last bytes written by the user**, without
/// any trace saying so: it is the silent loss this whole
/// sub-project exists to forbid.
#[test]
fn a_notification_during_a_push_is_replayed_afterwards() {
    let mut f = File::new();
    f.signaler(modified("a.txt"));
    f.signaler(modified("a.txt")); // arrives during the push
    assert_eq!(
        f.terminee("a.txt"),
        Some(modified("a.txt")),
        "the replay must restart, and the file be read again FROM THE START"
    );
    assert_eq!(f.en_vol(), Some("a.txt"));
    // …and once replayed, nothing is waiting any more.
    assert_eq!(f.terminee("a.txt"), None);
    assert_eq!(f.en_vol(), None);
}

/// The replay goes **ahead** of the queue: its bytes are the most recent
/// anyone is waiting for.
#[test]
fn a_replay_goes_ahead_of_the_queue() {
    let mut f = File::new();
    f.signaler(modified("a.txt"));
    f.signaler(modified("b.txt")); // en attente
    f.signaler(modified("a.txt")); // replay of the push in progress
    assert_eq!(f.terminee("a.txt"), Some(modified("a.txt")));
    assert_eq!(f.terminee("a.txt"), Some(modified("b.txt")));
}

/// The order between distinct paths is the registration order. A `HashSet`
/// would give a different one at each run.
#[test]
fn the_order_between_distinct_paths_is_the_registration_order() {
    let mut f = File::new();
    let noms: Vec<String> = (0..8).map(|i| format!("f{i}.txt")).collect();
    for n in &noms {
        f.signaler(modified(n));
    }
    let mut vus = vec![f.en_vol().expect("the first has left").to_string()];
    while let Some(next) = f.terminee(vus.last().expect("not empty")) {
        vus.push(next.chemin().to_string());
    }
    assert_eq!(vus, noms);
}

/// A path already WAITING keeps its rank: moving it back to the tail would
/// let younger entries pass ahead of it.
#[test]
fn a_path_already_pending_keeps_its_rank() {
    let mut f = File::new();
    f.signaler(modified("en-vol.txt"));
    f.signaler(modified("a.txt"));
    f.signaler(modified("b.txt"));
    assert_eq!(
        f.signaler(modified("a.txt")),
        None,
        "a.txt was already waiting"
    );
    assert_eq!(f.en_attente(), 2, "no duplicate enters the queue");
    assert_eq!(
        f.terminee("en-vol.txt").as_ref().map(Evenement::chemin),
        Some("a.txt")
    );
}

/// 🔴 **A DIRECTORY CREATION IS NEVER REPLACED BY A
/// MODIFICATION.**
///
/// Letting it become a `Modified` would read a directory like a file,
/// and the local workstation would return `IsADirectory` on the most mundane path there
/// is.
#[test]
fn a_directory_creation_survives_a_modification() {
    // En vol, puis rejeu.
    let mut f = File::new();
    f.signaler(cree_dossier("dossier"));
    f.signaler(modified("dossier"));
    assert_eq!(f.terminee("dossier"), Some(cree_dossier("dossier")));

    // En attente, puis coalescence.
    let mut g = File::new();
    g.signaler(modified("autre.txt"));
    g.signaler(cree_dossier("dossier"));
    g.signaler(modified("dossier"));
    assert_eq!(g.terminee("autre.txt"), Some(cree_dossier("dossier")));
}

/// A FILE creation, for its part, is indeed replaced: creating then writing has
/// only one effect, writing — and the browser's writer creates along the way.
#[test]
fn a_file_creation_is_replaced_by_the_following_modification() {
    let mut f = File::new();
    f.signaler(cree("neuf.txt"));
    f.signaler(modified("neuf.txt"));
    assert_eq!(f.terminee("neuf.txt"), Some(modified("neuf.txt")));
}

/// 🔴 **A PUSH THAT FAILS FREES THE FLIGHT.**
///
/// `terminee` is called whatever the outcome. Freeing it only on success
/// would mean a SINGLE failure would block all following writes — and the
/// journal keeps the due entry anyway.
#[test]
fn a_failing_push_frees_the_flight() {
    let mut f = File::new();
    f.signaler(modified("echoue.txt"));
    f.signaler(modified("suivant.txt"));
    assert_eq!(f.terminee("echoue.txt"), Some(modified("suivant.txt")));
}

/// An end that does not match the flight in progress disturbs nothing — it is the case
/// of a late `Fait`, arrived after an expiry.
#[test]
fn an_end_not_matching_the_flight_disturbs_nothing() {
    let mut f = File::new();
    f.signaler(modified("a.txt"));
    assert_eq!(f.terminee("inconnu.txt"), None);
    assert_eq!(
        f.en_vol(),
        Some("a.txt"),
        "the flight in progress is INTACT"
    );
}

/// ⚠️ **WHAT F3 WILL READ.** Without `attend`, its renaming would not know that a
/// write is due on the source, and would push it AFTER the renaming — on a
/// path that no longer exists. The browser would then recreate the temporary
/// file, and the save would be lost.
#[test]
fn waits_sees_the_flight_and_the_queue() {
    let mut f = File::new();
    f.signaler(modified("en-vol.txt"));
    f.signaler(modified("en-attente.txt"));
    assert!(f.attend("en-vol.txt"));
    assert!(f.attend("en-attente.txt"));
    assert!(!f.attend("jamais-vu.txt"));
}

/// ⚠️ **WHAT F3 WILL CALL on a deletion.** Pushing a due write on
/// a deleted path **would recreate what the user erases**.
///
/// 🔴 **And `oublier` DOES NOT TOUCH THE FLIGHT IN PROGRESS**: its frames have already
/// gone, and its `Fait` must still find its recipient.
#[test]
fn forgetting_removes_from_the_queue_and_the_replay_but_not_the_flight() {
    let mut f = File::new();
    f.signaler(modified("en-vol.txt"));
    f.signaler(modified("en-vol.txt")); // pose un rejeu
    f.signaler(modified("condamne.txt"));

    f.oublier("condamne.txt");
    assert!(!f.attend("condamne.txt"));
    assert_eq!(f.en_attente(), 0);

    f.oublier("en-vol.txt");
    assert_eq!(
        f.en_vol(),
        Some("en-vol.txt"),
        "the flight in progress remains"
    );
    assert_eq!(
        f.terminee("en-vol.txt"),
        None,
        "but its replay was indeed forgotten"
    );
}
