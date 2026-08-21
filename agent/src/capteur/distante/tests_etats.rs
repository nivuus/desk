//! Tests des ÉTATS que le capteur pousse à `SourceDistante` — visibilité,
//! sommeil, part de budget, ordre audio, plein écran, presse-papier.
//!
//! **Extrait de `distante/tests.rs` VERBATIM le 20 août 2026**, sous-bloc P1
//! du presse-papier, tâche 10 : le fichier voisin était à **474 lignes** pour
//! un plafond de projet à 500, soit une marge de 26 que le test de
//! `Recu::PressePapier` aurait entamée — et le §7.2 de la spécification ne
//! listait pas ce fichier du tout (divergence E1 du plan). La règle du dépôt
//! est d'extraire AVANT d'ajouter, jamais de comprimer.
//!
//! Le partage suit ce que les tests exercent : ce qui ARRIVE par la file
//! `Recu` et se relit par une méthode `…_a_annoncer` / `…_a_appliquer` vit
//! ici ; les images, le rattachement et les commandes restent chez le voisin,
//! avec les fabriques (`source_avec`, `source_rattachable`) et le canal
//! factice, que ce fichier réemprunte plutôt que de les dupliquer.

use super::tests::{source_avec, source_rattachable};
use super::*;
// `VideoSource` est importé ICI depuis que l'implémentation du trait a été
// extraite vers `distante/video_source.rs` (sous-bloc A1) : le parent ne s'en
// sert plus, et un trait doit être en portée pour que ses méthodes soient
// appelables.
use crate::source::VideoSource;

/// `set_awake` relaie la visibilité telle quelle au capteur : c'est lui qui
/// arbitre globalement (tâche 7). `source_avec` sert ici de canal espion, par
/// son troisième élément (`recus`), pour vérifier le message ÉMIS.
#[test]
fn set_awake_transmet_la_visibilite_au_capteur() {
    let (mut source, _tx, recus) = source_avec(4);
    source.set_awake(false, false).expect("le capteur accepte");
    assert_eq!(
        recus.lock().unwrap().as_slice(),
        &[VersCapteur::Visibilite { visible: false, focalisee: false }]
    );
}

/// `ecrire_le_presse_papier` relaie le texte tel quel au capteur, qui en est
/// le seul propriétaire (D1). `source_avec` sert de canal espion pour
/// vérifier le message ÉMIS, pas seulement l'effet.
///
/// ROUGE si `commander_simple` n'est pas appelé, ou si le texte est altéré en
/// route — l'enfant l'a déjà normalisé, borné et dénormalisé, et le capteur
/// n'a rien à en décider.
#[test]
fn ecrire_le_presse_papier_transmet_le_texte_au_capteur() {
    let (mut source, _tx, recus) = source_avec(4);
    source.ecrire_le_presse_papier("une\r\ndeux").expect("le capteur accepte");
    assert_eq!(
        recus.lock().unwrap().as_slice(),
        &[VersCapteur::PressePapierEcrire { texte: "une\r\ndeux".to_string() }]
    );
}

/// 🔴 **Un refus du capteur doit remonter en `Err`, et c'est ce qui empêche
/// l'injection de `Ctrl+V`** : sans lui, la touche partirait sur un
/// presse-papier inchangé et collerait le contenu PRÉCÉDENT.
///
/// ROUGE si l'implémentation employait `commander` nu au lieu de
/// `commander_simple` : elle accepterait alors n'importe quelle réponse, y
/// compris une `Erreur`. Ce test ne vérifie donc pas `commander_simple`
/// lui-même — il vérifie qu'on l'a bien employé, LUI.
#[test]
fn un_refus_du_capteur_empeche_le_collage() {
    let (mut source, _tx, _recus) = super::tests::source_avec_reponses(vec![Ok(
        DepuisCapteur::Erreur { motif: "OpenClipboard".into() },
    )]);
    let erreur = source
        .ecrire_le_presse_papier("colle")
        .expect_err("un refus du capteur doit remonter");
    assert!(
        erreur.to_string().contains("OpenClipboard"),
        "le motif du capteur doit survivre : {erreur}"
    );
}

/// 🔴 **LE DÉFAUT DU TRAIT, ET C'EST LE PIÈGE QUE D10 A PAYÉ.** Les sources
/// factices de ce dépôt implémentent leurs effets de bord en NO-OP, et 456
/// tests sont restés verts sur un produit muet. Ce test-ci porte donc sur une
/// source qui **NE REDÉFINIT PAS** la méthode — `FileSource`, la source de
/// test du dépôt —, c'est-à-dire sur le défaut lui-même.
///
/// ROUGE si le défaut est un `Ok(())` inerte, comme ses quatre voisines de
/// `source.rs`. Ce serait le mode MONO-FENÊTRE collant silencieusement le
/// contenu PRÉCÉDENT à chaque `Ctrl+V`.
#[test]
fn le_defaut_du_trait_refuse_d_ecrire_plutot_que_de_faire_semblant() {
    let source_path =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
    let mut source = crate::source::FileSource::from_path(source_path, 1280, 720, 60)
        .expect("chargement du flux de test");
    let erreur = source
        .ecrire_le_presse_papier("colle")
        .expect_err("le défaut du trait DOIT rendre Err, jamais Ok(())");
    assert!(
        erreur.to_string().contains("aucun capteur"),
        "le motif doit nommer la cause : {erreur}"
    );
}

/// Un `Sommeil` poussé par le capteur est retenu, pas ignoré : c'est
/// `sommeil_a_annoncer` qui le rend disponible à la boucle de transport, et
/// une seule fois — la réémettre à chaque tour inonderait le canal de
/// contrôle vers le navigateur. `source_avec` sert ici de file injectable par
/// son deuxième élément (`tx`).
#[test]
fn un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Sommeil { endormie: true, raison: "evincee".into() }).expect("dépôt");
    // Le sommeil est consommé par le tour de boucle qui cherche une image.
    assert!(source.next_frame().is_none());
    assert_eq!(source.sommeil_a_annoncer(), Some((true, "evincee".to_string())));
    assert_eq!(source.sommeil_a_annoncer(), None, "une annonce ne se répète pas");
}

/// `sommeil` est un état COURANT, pas un historique : deux `Sommeil` reçus
/// avant toute lecture s'écrasent, et seul le dernier doit survivre — sans
/// quoi la boucle de transport annoncerait au navigateur un état déjà
/// périmé, ou pire, une file d'annonces grandirait sans jamais se vider.
#[test]
fn deux_sommeils_consecutifs_ne_retiennent_que_le_dernier() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Sommeil { endormie: true, raison: "masquee".into() }).expect("dépôt");
    tx.send(Recu::Sommeil { endormie: true, raison: "evincee".into() }).expect("dépôt");
    assert!(source.next_frame().is_none());
    assert_eq!(
        source.sommeil_a_annoncer(),
        Some((true, "evincee".to_string())),
        "seul le dernier sommeil reçu doit survivre"
    );
    assert_eq!(source.sommeil_a_annoncer(), None);
}

/// Une part reçue est retenue jusqu'à ce que la boucle de transport la
/// consomme, et ne se rend qu'une fois — même patron que
/// `un_sommeil_pousse_par_le_capteur_est_retenu_pour_le_client` plus haut.
#[test]
fn une_part_recue_est_rendue_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("dépôt");
    // `next_frame` est ce qui draine le canal : sans lui, rien n'est lu.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(4_000_000));
    assert_eq!(source.part_a_appliquer(), None, "une part ne se réapplique pas");
}

/// Deux parts arrivées entre deux lectures s'écrasent : c'est un état
/// courant, pas un historique — même régime que `Etat` et `Sommeil`.
#[test]
fn deux_parts_arrivees_avant_lecture_s_ecrasent() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Part { bps: 4_000_000 }).expect("dépôt");
    tx.send(Recu::Part { bps: 2_000_000 }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.part_a_appliquer(), Some(2_000_000), "seule la dernière survit");
}

/// Un ordre audio reçu est retenu jusqu'à ce que la boucle de transport le
/// consomme, et ne se rend qu'une fois — même patron que
/// `une_part_recue_est_rendue_une_seule_fois` plus haut.
#[test]
fn un_ordre_audio_recu_est_rendu_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    // `next_frame` est ce qui draine le canal : sans lui, rien n'est lu.
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));
    assert_eq!(source.audio_a_appliquer(), None, "un ordre audio ne se réapplique pas");
}

/// **Au rattachement, l'enfant REDEVIENT MUET** (conception §4.4 ; F2, revue
/// finale de branche).
///
/// Ce que ce test attrape, et que rien n'attrapait : une porteuse dont le
/// canal casse gardait son drapeau `emet` d'avant la rupture, parce que le
/// rattachement ne remettait à zéro que l'ordre EN ATTENTE (`None`, « rien à
/// changer ») et jamais l'état de la source. Deux fenêtres d'un même PID
/// jouaient alors le même mix, désynchronisées, jusqu'à ce que l'ordre
/// d'extinction arrive — un écho audible.
#[test]
fn un_rattachement_remet_l_enfant_au_silence() {
    let (mut source, tx, _recus, _rattachements, _essais) = source_rattachable(vec![Some(1600)]);
    // La fenêtre porte le son, et la boucle de transport a consommé l'ordre :
    // il ne reste plus rien en attente, seul l'état réel de la source le sait.
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(true));

    // Le capteur meurt, l'enfant se rattache.
    drop(tx);
    assert_eq!(source.next_frame(), None, "le tour de la rupture ne rend pas d'image");

    assert_eq!(
        source.audio_a_appliquer(),
        Some(false),
        "un rattachement doit ORDONNER le silence, pas se taire sur la question"
    );
}

/// Deux ordres audio arrivés entre deux lectures s'écrasent : même régime
/// que `deux_parts_arrivees_avant_lecture_s_ecrasent` juste au-dessus.
#[test]
fn deux_ordres_audio_arrives_avant_lecture_s_ecrasent() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::Audio { actif: true }).expect("dépôt");
    tx.send(Recu::Audio { actif: false }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.audio_a_appliquer(), Some(false), "seul le dernier ordre survit");
}

/// Même régime que `sommeil_a_annoncer` : l'annonce est un CHANGEMENT, elle
/// se consomme. Sans quoi la branche de transport qui l'interroge à ~100 Hz
/// inonderait le canal de contrôle.
#[test]
fn un_plein_ecran_pousse_est_annonce_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::PleinEcran { actif: true }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(source.plein_ecran_a_annoncer(), Some(true));
    assert_eq!(source.plein_ecran_a_annoncer(), None, "une annonce ne se répète pas");
}

/// Même régime que `plein_ecran_a_annoncer` juste au-dessus : l'annonce est un
/// ÉTAT COURANT, et elle se CONSOMME. Sans quoi la branche `a1septies` de
/// `transport/tick.rs`, qui interroge la source à ~100 Hz, réémettrait le même
/// `AgentControl::Clipboard` cent fois par seconde et inonderait le canal de
/// contrôle — le texte pouvant peser jusqu'à `PRESSE_PAPIER_MAX`.
#[test]
fn un_presse_papier_pousse_est_annonce_une_seule_fois() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::PressePapier { texte: Some("bonjour".into()), octets: 7 }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.presse_papier_a_annoncer(),
        Some((Some("bonjour".to_string()), 7))
    );
    assert_eq!(source.presse_papier_a_annoncer(), None, "une annonce ne se répète pas");
}

/// Le REFUS de taille (D-P1-1) voyage par la même variante, `texte` à `None`
/// et `octets` portant la taille refusée : c'est ce qui permet au bandeau du
/// navigateur de la dire. Deux annonces arrivées entre deux lectures
/// s'écrasent — le presse-papier EST un état, pas un historique.
#[test]
fn deux_presse_papiers_arrives_avant_lecture_s_ecrasent_et_le_refus_passe() {
    let (mut source, tx, _recus) = source_avec(4);
    tx.send(Recu::PressePapier { texte: Some("premier".into()), octets: 7 }).expect("dépôt");
    tx.send(Recu::PressePapier { texte: None, octets: 100_000 }).expect("dépôt");
    assert_eq!(source.next_frame(), None);
    assert_eq!(
        source.presse_papier_a_annoncer(),
        Some((None, 100_000)),
        "seule la dernière annonce survit, refus compris"
    );
}
