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

/// L'état lu au PREMIER tour fait référence, et n'est pas annoncé (D-P1-4,
/// patron de `SuiviBordure`).
///
/// ❌ **CE COMMENTAIRE AJOUTAIT « une fenêtre qui s'attache ne reçoit pas le
/// contenu déjà présent, elle reçoit la première copie QUI SUIT », ET LE
/// SOUS-BLOC P3 L'A RÉFUTÉ** — c'était le legs n°3 de P1, désormais fermé par
/// `Etat::dernier_presse_papier` et son émission à l'inscription. La propriété
/// que CE test éprouve, elle, est intacte : le `Sondeur` n'annonce rien à son
/// premier tour.
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

// ---------------------------------------------------------------------------
// Sous-bloc P2 — le sens navigateur → VM : le garde n°1 de D5, la réciproque
// de `normaliser`, et la borne du texte ENTRANT.
// ---------------------------------------------------------------------------

/// 🔴 **C'est le garde n°1 de D5, et rien d'autre ne le mesure.**
///
/// Le témoin n'est pas que `observer` rende `None` — le garde n°2 le rendrait
/// aussi. Le témoin est que la fermeture de lecture **ne soit pas appelée du
/// tout** : le presse-papier Windows n'est même pas rouvert. D'où une
/// fermeture qui PANIQUE.
///
/// ROUGE si `apres_notre_ecriture` ne pose pas `reference` : `observer` lit,
/// et le test explose.
#[test]
fn apres_notre_ecriture_le_tour_suivant_n_ouvre_pas_le_presse_papier() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(
        sondeur.observer(7, || panic!("le garde n°1 a laissé rouvrir le presse-papier")),
        None
    );
}

/// 🔴 **C'est le garde n°2 ARMÉ SUR NOTRE PROPRE ÉCRITURE**, c'est-à-dire le
/// cas que D5 donne pour raison d'être du n°2 : une écriture TIERCE s'est
/// intercalée entre notre `SetClipboardData` et notre relecture du compteur,
/// si bien que le numéro que nous avons relu n'est déjà plus le courant.
///
/// ROUGE si `apres_notre_ecriture` ne pose que `reference` : le compteur ayant
/// bougé, `observer` lit, trouve notre propre texte, et le renvoie au
/// navigateur — un aller-retour pour rien.
#[test]
fn apres_notre_ecriture_un_compteur_qui_a_bouge_ne_renvoie_pas_notre_texte() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(sondeur.observer(8, || Some(String::from("colle"))), None);
}

/// Le pendant du précédent : le garde n°2 ne doit pas absorber TOUT ce qui
/// suit une écriture. Une copie tierce d'un AUTRE texte est bien annoncée.
///
/// ROUGE si `apres_notre_ecriture` posait un état « on se tait désormais ».
/// Sans ce test, un garde trop large passerait les deux précédents.
#[test]
fn apres_notre_ecriture_une_copie_tierce_est_quand_meme_annoncee() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    sondeur.apres_notre_ecriture(7, "colle");
    assert_eq!(
        sondeur.observer(8, || Some(String::from("autre chose"))),
        Some(Annonce::Texte(String::from("autre chose")))
    );
}

/// `apres_notre_ecriture` normalise le texte qu'elle mémorise, comme
/// `observer` normalise celui qu'il lit — sans quoi le garde n°2 comparerait
/// un texte à `\r\n` (ce que Windows nous rendra) à un texte à `\n`, et ne
/// reconnaîtrait jamais notre propre écriture.
///
/// ROUGE si l'on mémorise le texte brut.
#[test]
fn apres_notre_ecriture_memorise_le_texte_normalise() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    // Ce que l'on a REMIS à Windows porte des `\r\n` (c'est `denormaliser` qui
    // les y met) ; ce que l'on relira en portera donc aussi.
    sondeur.apres_notre_ecriture(7, "une\r\ndeux");
    assert_eq!(sondeur.observer(8, || Some(String::from("une\r\ndeux"))), None);
}

/// 🔴 **La première chose qu'un test doit voir rouge** (spec §7.1) :
/// l'aller-retour ne doit rien changer.
///
/// ROUGE si `denormaliser` double les `\r` — `normaliser` rendrait alors deux
/// lignes là où il y en avait une.
#[test]
fn l_aller_retour_normaliser_denormaliser_est_l_identite() {
    let normalise = "une\ndeux\ntrois";
    assert_eq!(normaliser(&denormaliser(normalise)), normalise);
}

/// ROUGE si `denormaliser` ajoutait un `\r\n` là où il n'y a pas de saut.
#[test]
fn denormaliser_laisse_un_texte_sans_saut_de_ligne_intact() {
    assert_eq!(denormaliser("abc"), "abc");
}

/// 🔴 **C'est le cas RÉEL, pas une curiosité** : le texte vient d'un
/// navigateur, et rien ne garantit qu'il n'a pas déjà des `\r\n` — un copier
/// depuis un éditeur Windows local en porte.
///
/// ROUGE si `denormaliser` est un `replace("\n", "\r\n")` naïf : il rendrait
/// `a\r\r\nb`, et le Bloc-notes afficherait une ligne vide de plus.
#[test]
fn denormaliser_ne_double_pas_des_crlf_deja_presents() {
    assert_eq!(denormaliser("a\r\nb"), "a\r\nb");
}

/// Un `\r` seul devient `\r\n` lui aussi : Windows n'affiche pas un `\r` nu
/// comme un saut de ligne dans le Bloc-notes.
#[test]
fn denormaliser_traite_aussi_un_cr_seul() {
    assert_eq!(denormaliser("a\rb"), "a\r\nb");
}

/// ROUGE si la comparaison est un `>=` au lieu d'un `>` : le cas limite exact
/// serait refusé alors qu'il tient.
#[test]
fn borner_entrant_accepte_exactement_la_borne_et_refuse_un_octet_de_plus() {
    let pile = "a".repeat(PRESSE_PAPIER_MAX);
    assert_eq!(borner_entrant(&pile), Some(pile.clone()));
    let un_de_trop = "a".repeat(PRESSE_PAPIER_MAX + 1);
    assert_eq!(borner_entrant(&un_de_trop), None);
}

/// 🔴 La borne compte des **octets d'UTF-8**, jamais des `char` — c'est la
/// même unité que celle du sens sortant, qui protège un canal.
///
/// ROUGE si l'implémentation est `texte.chars().count()` : ce texte a
/// `PRESSE_PAPIER_MAX / 4` caractères, donc passerait, pour exactement
/// `PRESSE_PAPIER_MAX` octets — puis un caractère de plus le ferait déborder
/// de quatre octets sans que le compte de `char` ne s'en aperçoive.
#[test]
fn borner_entrant_compte_des_octets_utf8_et_non_des_char() {
    let emojis = "😀".repeat(PRESSE_PAPIER_MAX / 4);
    assert_eq!(emojis.len(), PRESSE_PAPIER_MAX);
    assert_eq!(emojis.chars().count(), PRESSE_PAPIER_MAX / 4);
    assert_eq!(borner_entrant(&emojis), Some(emojis.clone()));

    let un_de_trop = format!("{emojis}😀");
    assert_eq!(un_de_trop.chars().count(), PRESSE_PAPIER_MAX / 4 + 1);
    assert_eq!(borner_entrant(&un_de_trop), None);
}

/// 🔴 **LE BRAS DÉSARMÉ DU CRITÈRE ④, ET IL DOIT DÉSARMER LES DEUX GARDES.**
///
/// L'observable est double, et les deux moitiés comptent :
/// - la fermeture de lecture **est appelée** ⟹ `reference` n'a pas été posée,
///   donc le garde n°1 est bien désarmé ;
/// - `observer` rend **`Some`** ⟹ `dernier_emis` n'a pas été posé non plus,
///   donc le garde n°2 l'est aussi.
///
/// ROUGE si `armer` ne désarme que `reference` : la lecture aurait bien lieu,
/// mais le garde n°2 absorberait l'annonce et le compte de la recette resterait
/// à ZÉRO — la rouge du critère ④ serait vacueuse une seconde fois.
#[test]
fn desarme_les_gardes_laisse_relire_et_annoncer_notre_propre_ecriture() {
    let mut sondeur = Sondeur::nouveau();
    amorce(&mut sondeur);
    sondeur.armer(false, 7, "colle");

    let lu = Cell::new(false);
    let annonce = sondeur.observer(7, || {
        lu.set(true);
        Some(String::from("colle"))
    });

    assert!(lu.get(), "désarmé, le presse-papier DOIT être rouvert (garde n°1)");
    assert_eq!(
        annonce,
        Some(Annonce::Texte(String::from("colle"))),
        "désarmé, notre propre texte DOIT être annoncé (garde n°2)"
    );
}

/// Le pendant : armé — l'état par défaut, sans la variable —, les deux gardes
/// mordent. C'est le test que `apres_notre_ecriture` porte déjà ; celui-ci
/// vérifie que `armer(true, …)` en est bien le même chemin, et non un second.
///
/// ROUGE si `apres_notre_ecriture` cessait de déléguer à `armer`.
#[test]
fn armer_a_vrai_est_le_meme_chemin_qu_apres_notre_ecriture() {
    let mut par_defaut = Sondeur::nouveau();
    amorce(&mut par_defaut);
    par_defaut.apres_notre_ecriture(7, "colle");

    let mut explicite = Sondeur::nouveau();
    amorce(&mut explicite);
    explicite.armer(true, 7, "colle");

    assert_eq!(
        par_defaut.observer(7, || panic!("garde n°1")),
        explicite.observer(7, || panic!("garde n°1"))
    );
    assert_eq!(
        par_defaut.observer(8, || Some(String::from("colle"))),
        explicite.observer(8, || Some(String::from("colle")))
    );
}

// ── LA SECONDE PRISE DE D-P3-6 (sous-bloc P3, tâche 5) ───────────────────
//
// 🔴 **UN DÉFAUT DU PLAN, SIGNALÉ ET CORRIGÉ ICI PLUTÔT QUE RECOPIÉ.** Son
// Step 1 prescrit un test de DEUX lignes — `armer(true, seqA, textA)` puis
// `observer(seqB, || Some(textB))` — « vu ROUGE sur l'arbre intact ». Il l'a
// été, et la pièce est versée
// (`journaux-presse-papier-p3/rouge-t5-d-p3-6-arbre-intact.log`) : la course
// est CONFIRMÉE, RP3-9 n'est pas réalisé.
//
// ⚠️ **Mais ces deux lignes seules ne peuvent JAMAIS devenir vertes**, et le
// plan ne l'avait pas vu : le remède qu'il tranche lui-même est un
// POST-FILTRE — `filtrer_nos_ecritures_tardives` court APRÈS `tour()`, sur son
// résultat. `observer` ne peut pas connaître une écriture qui n'est arrivée
// qu'après lui ; exiger qu'il rende `None` serait exiger qu'il devine.
//
// **La lettre du test est donc conservée, et une ligne lui est ajoutée** : la
// seconde prise, appliquée au résultat. Les deux premières lignes sont
// celles-là mêmes qui ont rougi.

/// ROUGE sur l'arbre intact : les deux premières lignes rendaient
/// `Some(Texte("textB"))` là où la troisième doit rendre `None`.
#[test]
fn une_ecriture_notre_survenue_apres_l_armement_n_est_pas_annoncee() {
    let mut s = Sondeur::nouveau();
    // Le tour de roue a armé sur la première écriture (fenêtre A).
    s.armer(true, 10, "textA");
    // La fenêtre B colle : le presse-papier porte `textB`, le compteur a
    // rebougé, et `tour()` produit donc une annonce que les deux gardes de D5
    // laissent passer.
    let annonce = s.observer(11, || Some(String::from("textB")));
    assert_eq!(annonce, Some(Annonce::Texte(String::from("textB"))));
    // La seconde prise consomme le couple de B et écarte SON PROPRE texte.
    assert_eq!(s.ecarter(true, Some((11, String::from("textB"))), annonce), None);
}

/// 🔴 LE GARDE-FOU DU CORRECTIF : filtrer trop large ferait taire une VRAIE
/// copie. ROUGE si le filtre porte sur le seul `seq` au lieu du texte — une
/// copie tierce survenue après notre écriture porte elle aussi un `seq`
/// postérieur, et le numéro seul ne les distingue pas.
#[test]
fn une_copie_tierce_survenue_apres_l_armement_est_toujours_annoncee() {
    let mut s = Sondeur::nouveau();
    s.armer(true, 10, "textA");
    // Nous avons écrit `textB` (seq 11), PUIS une application tierce a copié
    // `textC` : c'est `textC` que le presse-papier porte, et il doit partir.
    let annonce = s.observer(12, || Some(String::from("textC")));
    assert_eq!(
        s.ecarter(true, Some((11, String::from("textB"))), annonce),
        Some(Annonce::Texte(String::from("textC")))
    );
}

/// ROUGE si le filtre s'applique à `Annonce::Refus`, qui n'a pas de texte à
/// comparer : l'utilisateur perdrait le bandeau qui lui dit pourquoi rien
/// n'est arrivé.
#[test]
fn le_filtre_ne_touche_pas_un_refus_de_taille() {
    let mut s = Sondeur::nouveau();
    let refus = Some(Annonce::Refus { octets: 99_999 });
    assert_eq!(
        s.ecarter(true, Some((11, String::from("textB"))), refus.clone()),
        refus
    );
}

/// ROUGE si la seconde prise écarte quand même sous `PRESSE_PAPIER_GARDE=0` :
/// ce bras de banc existe pour rendre atteignable la rouge du critère ④ de P2,
/// qui compte les messages revenant vers la fenêtre après un collage, et une
/// prise qui mordrait quand même le viderait de son sens.
#[test]
fn la_seconde_prise_est_desarmee_par_le_bras_de_banc() {
    let mut s = Sondeur::nouveau();
    s.armer(true, 10, "textA");
    let annonce = s.observer(11, || Some(String::from("textB")));
    assert_eq!(
        s.ecarter(false, Some((11, String::from("textB"))), annonce.clone()),
        annonce
    );
}

/// Sans écriture de notre part, la seconde prise est transparente — et elle
/// n'arme rien : ROUGE si elle posait `reference` sur un `seq` inventé.
#[test]
fn sans_notre_ecriture_la_seconde_prise_ne_touche_a_rien() {
    let mut s = Sondeur::nouveau();
    amorce(&mut s);
    let annonce = s.observer(2, || Some(String::from("copie-tierce")));
    assert_eq!(
        s.ecarter(true, None, annonce.clone()),
        Some(Annonce::Texte(String::from("copie-tierce")))
    );
    assert_eq!(annonce, Some(Annonce::Texte(String::from("copie-tierce"))));
}
