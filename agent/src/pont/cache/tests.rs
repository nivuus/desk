use super::*;

fn e(nom: &str) -> Entree {
    Entree { nom: nom.to_string(), repertoire: false, taille: 1, modifie_ms: 0 }
}

fn t0() -> Instant {
    Instant::now()
}

#[test]
fn un_chemin_jamais_pose_n_est_pas_memorise() {
    let mut c = CacheEnumeration::nouveau();
    assert!(c.lire("dossier", t0()).is_none());
    assert_eq!(c.taille(), 0);
}

#[test]
fn ce_qui_est_pose_est_relu_a_l_identique() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt"), e("b.txt")], t);
    let lues = c.lire("dossier", t).expect("mémorisé");
    assert_eq!(lues.len(), 2);
    assert_eq!(lues[0].nom, "a.txt");
    assert_eq!(lues[1].nom, "b.txt");
}

/// 🔴 **L'EXPIRATION SANS DORMIR** — c'est ce que l'horloge injectée achète.
/// La borne est assiégée des DEUX côtés, sans quoi un `>` mis pour un `>=`
/// passerait inaperçu.
#[test]
fn une_memoire_expire_au_terme_exact_et_pas_avant() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    assert!(c.lire("dossier", t + TTL_ENUMERATION - Duration::from_millis(1)).is_some());
    assert!(c.lire("dossier", t + TTL_ENUMERATION).is_none());
}

/// Une mémoire expirée est **RETIRÉE**, pas seulement ignorée : sans cela le
/// cache croîtrait sans terme sur une arborescence parcourue une fois, et ce
/// module n'a **aucune** politique d'éviction (spec §10 R4).
#[test]
fn une_memoire_expiree_est_retiree_et_pas_seulement_ignoree() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    assert_eq!(c.taille(), 1);
    let _ = c.lire("dossier", t + TTL_ENUMERATION);
    assert_eq!(c.taille(), 0, "l'entrée expirée doit être retirée, pas gardée");
}

#[test]
fn poser_deux_fois_ecrase_et_rearme_l_horloge() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("dossier".into(), vec![e("vieux.txt")], t);
    let tard = t + TTL_ENUMERATION - Duration::from_millis(1);
    c.poser("dossier".into(), vec![e("neuf.txt")], tard);
    assert_eq!(c.taille(), 1, "la seconde pose écrase, elle n'ajoute pas");
    let lues = c.lire("dossier", tard + Duration::from_millis(1)).expect("réarmée");
    assert_eq!(lues[0].nom, "neuf.txt");
}

/// 🔴 **`invalider` PREND LE PARENT, JAMAIS LE CHEMIN LUI-MÊME.**
///
/// **Sa ROUGE** : faire prendre à [`CacheEnumeration::invalider`] le chemin
/// lui-même au lieu de `parent_de(chemin)`. Ce test tombe alors — et l'erreur
/// qu'il verrouille est **silencieuse** en production : le listage du parent
/// continuerait d'être servi depuis la mémoire, et le fichier créé
/// n'apparaîtrait jamais.
#[test]
fn invalider_oublie_le_repertoire_parent_du_chemin_mute() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("dossier".into(), vec![e("a.txt")], t);
    c.invalider("dossier/neuf.txt");
    assert!(c.lire("dossier", t).is_none(), "le PARENT doit être oublié");
}

/// ⚠️ **Le parent d'un chemin sans séparateur est la RACINE, `""`** — la clé
/// qu'un `Get-ChildItem` sur le lecteur monté sollicite. L'oublier ferait
/// qu'une création à la racine serait invisible : c'est exactement le geste du
/// critère ① de la recette de F5.
#[test]
fn le_parent_d_un_chemin_de_premier_niveau_est_la_racine() {
    assert_eq!(parent_de("note.txt"), "");
    assert_eq!(parent_de("dossier/note.txt"), "dossier");
    assert_eq!(parent_de("a/b/c.txt"), "a/b");
    assert_eq!(parent_de(""), "");
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser(String::new(), vec![e("a.txt")], t);
    c.invalider("neuf.txt");
    assert!(c.lire("", t).is_none(), "la racine doit être oubliée sur une création de premier niveau");
}

/// Invalider un répertoire n'en touche **aucun autre** : un cache qui se
/// viderait entièrement à chaque écriture ne serait pas un cache.
#[test]
fn invalider_ne_touche_pas_les_repertoires_voisins() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.poser("b".into(), vec![e("y.txt")], t);
    c.invalider("a/neuf.txt");
    assert!(c.lire("a", t).is_none());
    assert!(c.lire("b", t).is_some(), "le voisin doit survivre");
}

/// Invalider un chemin jamais mémorisé ne doit **rien** faire, et surtout pas
/// paniquer : les notifications ProjFS arrivent pour des chemins que le pont
/// n'a jamais listés.
#[test]
fn invalider_un_chemin_inconnu_est_inoffensif() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.invalider("jamais/vu.txt");
    assert_eq!(c.taille(), 1);
    assert!(c.lire("a", t).is_some());
}

/// `vider` est ce que fait l'annonce `Rafraichir` : tout, d'un coup.
#[test]
fn vider_oublie_tout() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("a".into(), vec![e("x.txt")], t);
    c.poser("b".into(), vec![e("y.txt")], t);
    c.poser(String::new(), vec![e("z.txt")], t);
    assert_eq!(c.taille(), 3);
    c.vider();
    assert_eq!(c.taille(), 0);
    assert!(c.lire("a", t).is_none());
    assert!(c.lire("", t).is_none());
}

/// Un répertoire **vide** mémorisé n'est pas la même chose qu'un répertoire
/// jamais mémorisé : les confondre ferait repayer un aller-retour à chaque
/// listage d'un dossier vide.
#[test]
fn un_repertoire_vide_memorise_est_servi_comme_tel() {
    let mut c = CacheEnumeration::nouveau();
    let t = t0();
    c.poser("vide".into(), Vec::new(), t);
    let lues = c.lire("vide", t).expect("mémorisé, quoique vide");
    assert!(lues.is_empty());
}
