use super::*;
use std::cell::Cell;

/// Un sondeur qui a déjà pris sa référence : c'est l'état nominal après
/// le premier tour, et celui dans lequel toutes les propriétés ci-dessous
/// se jugent.
fn amorce(sondeur: &mut Sondeur) {
    assert_eq!(sondeur.observer(1, || Some(String::from("etat-initial"))), None);
}

/// ROUGE si `normaliser` laisse passer `\r\n` : l'aller-retour de P2
/// doublerait alors les lignes à chaque tour.
#[test]
fn normaliser_ramene_crlf_a_lf() {
    assert_eq!(normaliser("a\r\nb"), "a\nb");
}

/// ROUGE si l'on ne traite que `\r\n` : les fins de ligne Mac classiques
/// passeraient telles quelles.
#[test]
fn normaliser_ramene_un_cr_seul_a_lf() {
    assert_eq!(normaliser("a\rb"), "a\nb");
}

/// ROUGE si `normaliser` remplaçait `\n` par `\r\n` : la fonction ne
/// serait plus idempotente et l'aller-retour de P2 divergerait.
#[test]
fn normaliser_est_idempotente() {
    let une = normaliser("a\r\nb\rc\nd");
    assert_eq!(une, "a\nb\nc\nd");
    assert_eq!(normaliser(&une), une);
}

/// 🔴 « On n'ouvre pas le presse-papier pour rien », et c'est vérifiable
/// SANS Windows : le témoin est un `Cell<bool>`.
///
/// ROUGE si `observer` appelle `lire` inconditionnellement.
#[test]
fn un_numero_inchange_n_ouvre_pas_le_presse_papier() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let appele = Cell::new(false);
    let annonce = sondeur.observer(1, || {
        appele.set(true);
        Some(String::from("bonjour"))
    });
    assert_eq!(annonce, None);
    assert!(!appele.get(), "lire() ne doit pas être appelée à numéro inchangé");
}

#[test]
fn un_numero_neuf_annonce_le_texte() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    assert_eq!(
        sondeur.observer(2, || Some(String::from("bonjour"))),
        Some(Annonce::Texte(String::from("bonjour")))
    );
}

/// 🔴 Le garde n°2 de D5, et la rouge du critère ② de la recette.
///
/// Le compteur BOUGE sur une réécriture identique — mesuré par la sonde
/// P0 (`q2="bouge"`, deux exécutions). Sans la comparaison de contenu, ce
/// geste pousserait un message pour rien.
///
/// ROUGE si l'on retire la comparaison : `Some` serait rendu deux fois.
#[test]
fn un_meme_texte_a_un_numero_different_n_est_annonce_qu_une_fois() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    assert_eq!(
        sondeur.observer(2, || Some(String::from("bonjour"))),
        Some(Annonce::Texte(String::from("bonjour")))
    );
    assert_eq!(sondeur.observer(3, || Some(String::from("bonjour"))), None);
}

/// 🔴 Au-delà de la borne on REFUSE, on ne tronque JAMAIS : un collage
/// silencieusement amputé est le pire résultat possible.
///
/// ROUGE si l'implémentation tronque — l'assertion sur la variante tombe.
#[test]
fn un_texte_trop_grand_est_refuse_jamais_tronque() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
    let annonce = sondeur.observer(2, || Some(gros));
    assert_eq!(annonce, Some(Annonce::Refus { octets: (PRESSE_PAPIER_MAX + 1) as u32 }));
}

/// ROUGE si la borne est écrite `>=` au lieu de `>`.
#[test]
fn un_texte_de_la_taille_exacte_de_la_borne_passe() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let pile = "a".repeat(PRESSE_PAPIER_MAX);
    assert_eq!(sondeur.observer(2, || Some(pile.clone())), Some(Annonce::Texte(pile)));
}

/// 🔴 D-P1-2 : on normalise D'ABORD, on borne ENSUITE.
///
/// Le texte pèse `PRESSE_PAPIER_MAX + 8` octets bruts et porte 12 `\r`
/// appariés à autant de `\n` : la normalisation lui en retire 12, donc il
/// tient. ROUGE si l'on borne avant de normaliser — il serait refusé.
#[test]
fn on_normalise_avant_de_borner() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let corps = "a".repeat(PRESSE_PAPIER_MAX + 8 - 24);
    let brut = format!("{corps}{}", "\r\n".repeat(12));
    assert_eq!(brut.len(), PRESSE_PAPIER_MAX + 8);
    let attendu = normaliser(&brut);
    assert_eq!(attendu.len(), PRESSE_PAPIER_MAX - 4);
    assert_eq!(sondeur.observer(2, || Some(brut)), Some(Annonce::Texte(attendu)));
}

/// 🔴 Le bornage compte des OCTETS d'UTF-8, pas des `char`.
///
/// ROUGE si l'on borne sur `.chars().count()` : ce texte fait
/// `PRESSE_PAPIER_MAX / 4 + 1` caractères, très en dessous de la borne
/// comptée ainsi, et passerait alors qu'il pèse plus de 64 KiB.
#[test]
fn le_bornage_compte_des_octets_pas_des_caracteres() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let emoji = "🙂".repeat(PRESSE_PAPIER_MAX / 4 + 1);
    assert_eq!(emoji.chars().count(), PRESSE_PAPIER_MAX / 4 + 1);
    assert!(emoji.len() > PRESSE_PAPIER_MAX);
    assert!(matches!(
        sondeur.observer(2, || Some(emoji)),
        Some(Annonce::Refus { .. })
    ));
}

/// 🔴 D-P1-5 : une lecture qui ÉCHOUE n'avance pas la référence.
///
/// Sinon le contenu correspondant serait perdu à jamais : le tour suivant
/// verrait un compteur « inchangé » et ne retenterait rien.
///
/// ROUGE si l'on mémorise le numéro avant la lecture — le second appel,
/// au MÊME numéro, n'appellerait plus `lire`.
#[test]
fn une_lecture_echouee_n_avance_pas_la_reference() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    assert_eq!(sondeur.observer(2, || None), None);
    let rappelee = Cell::new(false);
    let annonce = sondeur.observer(2, || {
        rappelee.set(true);
        Some(String::from("rattrape"))
    });
    assert!(rappelee.get(), "le même numéro doit être retenté après un échec");
    assert_eq!(annonce, Some(Annonce::Texte(String::from("rattrape"))));
}

/// ROUGE si le refus n'est pas mémorisé : le bandeau clignoterait à
/// chaque copie voisine tant que le contenu énorme reste en place.
#[test]
fn un_refus_repete_a_l_identique_n_est_annonce_qu_une_fois() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    let gros = "a".repeat(PRESSE_PAPIER_MAX + 1);
    assert!(sondeur.observer(2, || Some(gros.clone())).is_some());
    assert_eq!(sondeur.observer(3, || Some(gros)), None);
}

/// L'état lu au PREMIER tour fait référence, et n'est pas annoncé : une
/// fenêtre qui s'attache ne reçoit pas le contenu déjà présent, elle
/// reçoit la première copie QUI SUIT (D-P1-4, patron de `SuiviBordure`).
///
/// ROUGE si le premier tour annonce — ce qui ferait recevoir à chaque
/// attache un contenu que l'utilisateur n'a pas copié pour elle.
#[test]
fn le_premier_tour_prend_reference_et_n_annonce_rien() {
    let mut sondeur = Sondeur::nouveau();
    assert_eq!(sondeur.observer(7, || Some(String::from("deja-la"))), None);
    // Et ce contenu-là est bien retenu : le recopier ne relance rien.
    assert_eq!(sondeur.observer(8, || Some(String::from("deja-la"))), None);
    assert_eq!(
        sondeur.observer(9, || Some(String::from("neuf"))),
        Some(Annonce::Texte(String::from("neuf")))
    );
}
