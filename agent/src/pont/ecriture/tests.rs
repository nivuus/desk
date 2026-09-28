//! Tests of the due writes queue. **Pure, run on the host.**

use super::*;

fn modifie(chemin: &str) -> Evenement {
    Evenement::Modifie {
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
fn deux_notifications_du_meme_chemin_ne_font_qu_une_poussee_en_vol() {
    let mut f = File::nouvelle();
    assert_eq!(f.signaler(modifie("a.txt")), Some(modifie("a.txt")));
    assert_eq!(
        f.signaler(modifie("a.txt")),
        None,
        "la seconde ne démarre RIEN"
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
fn une_notification_pendant_une_poussee_est_rejouee_apres() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("a.txt")); // arrives during the push
    assert_eq!(
        f.terminee("a.txt"),
        Some(modifie("a.txt")),
        "le rejeu doit repartir, et le fichier être relu DEPUIS LE DÉBUT"
    );
    assert_eq!(f.en_vol(), Some("a.txt"));
    // …and once replayed, nothing is waiting any more.
    assert_eq!(f.terminee("a.txt"), None);
    assert_eq!(f.en_vol(), None);
}

/// The replay goes **ahead** of the queue: its bytes are the most recent
/// anyone is waiting for.
#[test]
fn un_rejeu_passe_devant_la_file() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("b.txt")); // en attente
    f.signaler(modifie("a.txt")); // replay of the push in progress
    assert_eq!(f.terminee("a.txt"), Some(modifie("a.txt")));
    assert_eq!(f.terminee("a.txt"), Some(modifie("b.txt")));
}

/// The order between distinct paths is the registration order. A `HashSet`
/// would give a different one at each run.
#[test]
fn l_ordre_entre_chemins_distincts_est_celui_d_inscription() {
    let mut f = File::nouvelle();
    let noms: Vec<String> = (0..8).map(|i| format!("f{i}.txt")).collect();
    for n in &noms {
        f.signaler(modifie(n));
    }
    let mut vus = vec![f.en_vol().expect("le premier est parti").to_string()];
    while let Some(suivant) = f.terminee(vus.last().expect("non vide")) {
        vus.push(suivant.chemin().to_string());
    }
    assert_eq!(vus, noms);
}

/// A path already WAITING keeps its rank: moving it back to the tail would
/// let younger entries pass ahead of it.
#[test]
fn un_chemin_deja_en_attente_garde_son_rang() {
    let mut f = File::nouvelle();
    f.signaler(modifie("en-vol.txt"));
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("b.txt"));
    assert_eq!(f.signaler(modifie("a.txt")), None, "a.txt attendait déjà");
    assert_eq!(f.en_attente(), 2, "aucun doublon n'entre dans la file");
    assert_eq!(
        f.terminee("en-vol.txt").as_ref().map(Evenement::chemin),
        Some("a.txt")
    );
}

/// 🔴 **A DIRECTORY CREATION IS NEVER REPLACED BY A
/// MODIFICATION.**
///
/// Letting it become a `Modifie` would read a directory like a file,
/// and the local workstation would return `IsADirectory` on the most mundane path there
/// is.
#[test]
fn une_creation_de_repertoire_survit_a_une_modification() {
    // En vol, puis rejeu.
    let mut f = File::nouvelle();
    f.signaler(cree_dossier("dossier"));
    f.signaler(modifie("dossier"));
    assert_eq!(f.terminee("dossier"), Some(cree_dossier("dossier")));

    // En attente, puis coalescence.
    let mut g = File::nouvelle();
    g.signaler(modifie("autre.txt"));
    g.signaler(cree_dossier("dossier"));
    g.signaler(modifie("dossier"));
    assert_eq!(g.terminee("autre.txt"), Some(cree_dossier("dossier")));
}

/// A FILE creation, for its part, is indeed replaced: creating then writing has
/// only one effect, writing — and the browser's writer creates along the way.
#[test]
fn une_creation_de_fichier_est_remplacee_par_la_modification_qui_suit() {
    let mut f = File::nouvelle();
    f.signaler(cree("neuf.txt"));
    f.signaler(modifie("neuf.txt"));
    assert_eq!(f.terminee("neuf.txt"), Some(modifie("neuf.txt")));
}

/// 🔴 **A PUSH THAT FAILS FREES THE FLIGHT.**
///
/// `terminee` is called whatever the outcome. Freeing it only on success
/// would mean a SINGLE failure would block all following writes — and the
/// journal keeps the due entry anyway.
#[test]
fn une_poussee_qui_echoue_libere_le_vol() {
    let mut f = File::nouvelle();
    f.signaler(modifie("echoue.txt"));
    f.signaler(modifie("suivant.txt"));
    assert_eq!(f.terminee("echoue.txt"), Some(modifie("suivant.txt")));
}

/// An end that does not match the flight in progress disturbs nothing — it is the case
/// of a late `Fait`, arrived after an expiry.
#[test]
fn une_fin_qui_ne_correspond_pas_au_vol_ne_derange_rien() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    assert_eq!(f.terminee("inconnu.txt"), None);
    assert_eq!(f.en_vol(), Some("a.txt"), "le vol en cours est INTACT");
}

/// ⚠️ **WHAT F3 WILL READ.** Without `attend`, its renaming would not know that a
/// write is due on the source, and would push it AFTER the renaming — on a
/// path that no longer exists. The browser would then recreate the temporary
/// file, and the save would be lost.
#[test]
fn attend_voit_le_vol_et_la_file() {
    let mut f = File::nouvelle();
    f.signaler(modifie("en-vol.txt"));
    f.signaler(modifie("en-attente.txt"));
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
fn oublier_retire_de_la_file_et_du_rejeu_mais_pas_du_vol() {
    let mut f = File::nouvelle();
    f.signaler(modifie("en-vol.txt"));
    f.signaler(modifie("en-vol.txt")); // pose un rejeu
    f.signaler(modifie("condamne.txt"));

    f.oublier("condamne.txt");
    assert!(!f.attend("condamne.txt"));
    assert_eq!(f.en_attente(), 0);

    f.oublier("en-vol.txt");
    assert_eq!(f.en_vol(), Some("en-vol.txt"), "le vol en cours reste");
    assert_eq!(
        f.terminee("en-vol.txt"),
        None,
        "mais son rejeu a bien été oublié"
    );
}
