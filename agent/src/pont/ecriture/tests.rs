//! Tests de la file des écritures dues. **Purs, exécutés sur l'hôte.**

use super::*;

fn modifie(chemin: &str) -> Evenement {
    Evenement::Modifie { chemin: chemin.to_string() }
}
fn cree(chemin: &str) -> Evenement {
    Evenement::Cree { chemin: chemin.to_string(), repertoire: false }
}
fn cree_dossier(chemin: &str) -> Evenement {
    Evenement::Cree { chemin: chemin.to_string(), repertoire: true }
}

/// 🔴 **DEUX NOTIFICATIONS DU MÊME CHEMIN NE FONT QU'UNE POUSSÉE EN VOL.**
///
/// Laisser passer la seconde ouvrirait deux flux `createWritable()` concurrents
/// sur le même fichier, qui s'écraseraient l'un l'autre.
#[test]
fn deux_notifications_du_meme_chemin_ne_font_qu_une_poussee_en_vol() {
    let mut f = File::nouvelle();
    assert_eq!(f.signaler(modifie("a.txt")), Some(modifie("a.txt")));
    assert_eq!(f.signaler(modifie("a.txt")), None, "la seconde ne démarre RIEN");
    assert_eq!(f.en_vol(), Some("a.txt"));
    assert_eq!(f.en_attente(), 0);
}

/// 🔴 **UNE NOTIFICATION PENDANT UNE POUSSÉE EST REJOUÉE APRÈS.**
///
/// La jeter perdrait **les derniers octets écrits par l'utilisateur**, sans
/// qu'aucune trace ne le dise : c'est la perte silencieuse que tout ce
/// sous-projet existe pour interdire.
#[test]
fn une_notification_pendant_une_poussee_est_rejouee_apres() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("a.txt")); // arrive pendant la poussée
    assert_eq!(
        f.terminee("a.txt"),
        Some(modifie("a.txt")),
        "le rejeu doit repartir, et le fichier être relu DEPUIS LE DÉBUT"
    );
    assert_eq!(f.en_vol(), Some("a.txt"));
    // …et une fois rejoué, plus rien n'attend.
    assert_eq!(f.terminee("a.txt"), None);
    assert_eq!(f.en_vol(), None);
}

/// Le rejeu passe **devant** la file : ses octets sont les plus récents que
/// quiconque attende.
#[test]
fn un_rejeu_passe_devant_la_file() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("b.txt")); // en attente
    f.signaler(modifie("a.txt")); // rejeu de la poussée en cours
    assert_eq!(f.terminee("a.txt"), Some(modifie("a.txt")));
    assert_eq!(f.terminee("a.txt"), Some(modifie("b.txt")));
}

/// L'ordre entre chemins distincts est celui d'inscription. Un `HashSet` en
/// rendrait un différent à chaque exécution.
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

/// Un chemin déjà EN ATTENTE garde son rang : le faire remonter en queue ferait
/// passer devant lui des entrées plus jeunes.
#[test]
fn un_chemin_deja_en_attente_garde_son_rang() {
    let mut f = File::nouvelle();
    f.signaler(modifie("en-vol.txt"));
    f.signaler(modifie("a.txt"));
    f.signaler(modifie("b.txt"));
    assert_eq!(f.signaler(modifie("a.txt")), None, "a.txt attendait déjà");
    assert_eq!(f.en_attente(), 2, "aucun doublon n'entre dans la file");
    assert_eq!(f.terminee("en-vol.txt").as_ref().map(Evenement::chemin), Some("a.txt"));
}

/// 🔴 **UNE CRÉATION DE RÉPERTOIRE N'EST JAMAIS REMPLACÉE PAR UNE
/// MODIFICATION.**
///
/// Le laisser devenir un `Modifie` ferait lire un répertoire comme un fichier,
/// et le poste local rendrait `IsADirectory` sur le chemin le plus banal qui
/// soit.
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

/// Une création de FICHIER, elle, est bien remplacée : créer puis écrire n'a
/// qu'un effet, écrire — et l'écrivain du navigateur crée au passage.
#[test]
fn une_creation_de_fichier_est_remplacee_par_la_modification_qui_suit() {
    let mut f = File::nouvelle();
    f.signaler(cree("neuf.txt"));
    f.signaler(modifie("neuf.txt"));
    assert_eq!(f.terminee("neuf.txt"), Some(modifie("neuf.txt")));
}

/// 🔴 **UNE POUSSÉE QUI ÉCHOUE LIBÈRE LE VOL.**
///
/// `terminee` est appelée quelle que soit l'issue. Ne la libérer qu'au succès
/// ferait qu'un SEUL échec bloquerait toutes les écritures suivantes — et le
/// journal, lui, garde l'entrée due de toute façon.
#[test]
fn une_poussee_qui_echoue_libere_le_vol() {
    let mut f = File::nouvelle();
    f.signaler(modifie("echoue.txt"));
    f.signaler(modifie("suivant.txt"));
    assert_eq!(f.terminee("echoue.txt"), Some(modifie("suivant.txt")));
}

/// Une fin qui ne correspond pas au vol en cours ne dérange rien — c'est le cas
/// d'un `Fait` tardif, arrivé après une expiration.
#[test]
fn une_fin_qui_ne_correspond_pas_au_vol_ne_derange_rien() {
    let mut f = File::nouvelle();
    f.signaler(modifie("a.txt"));
    assert_eq!(f.terminee("inconnu.txt"), None);
    assert_eq!(f.en_vol(), Some("a.txt"), "le vol en cours est INTACT");
}

/// ⚠️ **CE QUE F3 LIRA.** Sans `attend`, son renommage ne saurait pas qu'une
/// écriture est due sur la source, et la pousserait APRÈS le renommage — sur un
/// chemin qui n'existe plus. Le navigateur recréerait alors le fichier
/// temporaire, et l'enregistrement serait perdu.
#[test]
fn attend_voit_le_vol_et_la_file() {
    let mut f = File::nouvelle();
    f.signaler(modifie("en-vol.txt"));
    f.signaler(modifie("en-attente.txt"));
    assert!(f.attend("en-vol.txt"));
    assert!(f.attend("en-attente.txt"));
    assert!(!f.attend("jamais-vu.txt"));
}

/// ⚠️ **CE QUE F3 APPELLERA sur une suppression.** Pousser une écriture due sur
/// un chemin supprimé **recréerait ce que l'utilisateur efface**.
///
/// 🔴 **Et `oublier` NE TOUCHE PAS AU VOL EN COURS** : ses trames sont déjà
/// parties, et son `Fait` doit encore trouver son destinataire.
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
    assert_eq!(f.terminee("en-vol.txt"), None, "mais son rejeu a bien été oublié");
}
