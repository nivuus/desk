//! Le SIXIÈME site de mémorisation : le vivier écrivait `eveillee` avant que
//! l'ordre parte.
//!
//! **Fichier voisin dédié plutôt que des tests ajoutés à `sommeil/tests.rs`**
//! (round de correction 2, 25 août 2026) : cette suite-là est à 473 lignes
//! pour un plafond de projet à 500, et ces deux tests l'auraient fait
//! franchir. Extraire, jamais comprimer — et ici, ne pas faire grossir plutôt
//! que d'avoir à extraire ensuite. Même idiome et même précédent que
//! `superviseur/table.rs`, qui range de la même façon ses tests de relance
//! dans un second fichier.
//!
//! 🔴 CE QUE CES DEUX TESTS TIENNENT, ET QUE RIEN D'AUTRE NE TIENT.
//! `Vivier::arbitrer` écrit `eveillee` AUX ÉTAPES 1 ET 5, c'est-à-dire **avant
//! que l'ordre correspondant ait été déposé** dans la file de sa fenêtre. Si
//! ce dépôt est REFUSÉ (file pleine), le vivier ment sur l'état réel — et
//! `arbitrer` étant idempotent, **aucun ré-arbitrage futur ne réémet
//! l'ordre**. Le remède est `Vivier::annuler_ordre_non_livre`, appelé par
//! `registre::distribuer` : voir sa doc pour ce que chacun des deux sens
//! coûte.
//!
//! ⚠️ **LES DEUX SENS SONT ÉPROUVÉS, ET C'EST LE POINT** : le premier jet du
//! diagnostic ne nommait que `Reveiller` (« une fenêtre qui ne s'endort
//! plus »). `Dormir` est l'autre moitié, et c'est **la plus coûteuse** — le
//! vivier libère la place alors que la fenêtre encode encore, donc le plafond
//! de huit encodeurs se sur-souscrit. Il existe un `echec_de_reveil` pour le
//! premier sens ; il n'existe **aucun** `echec_de_sommeil`.

use std::time::Instant;

use super::file::PROFONDEUR_MAX;
use super::tests::verrouiller_pour_le_test;
use super::{distribuer, etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;

/// Bouche la file d'une session par des messages INCOALESCABLES — `Sommeil`
/// ne se coalesce jamais, c'est ce qui permet d'atteindre la borne.
fn boucher_la_file(session: &str) {
    let garde = etat();
    let emetteur = garde.canaux.get(session).expect("la session est inscrite");
    for _ in 0..PROFONDEUR_MAX {
        let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
    }
}

/// Un tour de roue, sans attendre les 250 ms qu'il prend en production.
fn un_tour_de_roue() {
    let mut garde = etat();
    let ordres = garde.vivier.rearbitrer(Instant::now());
    distribuer(&mut garde, ordres);
}

/// 🔴 SENS 1 — UN `Reveiller` REFUSÉ NE DOIT PAS LAISSER LE VIVIER CROIRE LA
/// FENÊTRE ÉVEILLÉE.
///
/// Sans le remède, `eveillee` reste `true` pour une fenêtre qui n'a jamais
/// reçu l'ordre : sa place au vivier est occupée sans qu'aucun encodeur réel
/// ne l'occupe, et dix `rearbitrer` de suite ne réémettent rien.
///
/// **Rougit sur sa PREMIÈRE assertion** — `un Reveiller non déposé ne doit pas
/// laisser le vivier croire la fenêtre éveillée` —, `eveillee` valant alors
/// `Some(true)`.
#[test]
fn un_reveil_non_depose_laisse_le_vivier_intact_et_repart_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r2-reveil", 6500);
    // Précondition : endormie, et le vivier le sait.
    assert_eq!(etat().vivier.eveillee("r2-reveil"), Some(false));
    boucher_la_file("r2-reveil");

    // La fenêtre devient visible et focalisée : le vivier l'élit et émet
    // `Reveiller` — qui est REFUSÉ, sa file étant pleine.
    signaler("r2-reveil", true, true);

    assert_eq!(
        etat().vivier.eveillee("r2-reveil"),
        Some(false),
        "un Reveiller non déposé ne doit pas laisser le vivier croire la fenêtre éveillée"
    );

    // La fenêtre reprend sa lecture, et le tour de roue suivant doit réémettre
    // l'ordre de lui-même — c'est tout l'intérêt de ne pas avoir menti.
    let recus = canal.vider();
    assert_eq!(recus.len(), PROFONDEUR_MAX, "précondition : la file était bien pleine");
    un_tour_de_roue();
    let ordres: Vec<Ordre> = canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Sommeil(o) => Some(o),
            _ => None,
        })
        .collect();
    assert!(
        ordres.contains(&Ordre::Reveiller),
        "le Reveiller non déposé doit repartir au tour suivant : {ordres:?}"
    );

    retirer("r2-reveil", generation);
}

/// 🔴 SENS 2 — UN `Dormir` REFUSÉ NE DOIT PAS LIBÉRER LA PLACE D'UNE FENÊTRE
/// QUI ENCODE ENCORE. **C'est la moitié la plus coûteuse**, et celle que le
/// premier diagnostic avait manquée : sans le remède, `eveillee` passe à
/// `false` et le plafond de huit encodeurs se sur-souscrit — la fenêtre n'a
/// jamais reçu l'ordre et tient toujours le sien.
///
/// **Rougit sur sa PREMIÈRE assertion** — `un Dormir non déposé ne doit pas
/// libérer la place d'une fenêtre qui encode encore` —, `eveillee` valant
/// alors `Some(false)`.
#[test]
fn un_sommeil_non_depose_laisse_le_vivier_intact_et_repart_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r2-sommeil", 6501);
    // Elle s'éveille pour de bon, file libre : l'ordre est livré.
    signaler("r2-sommeil", true, true);
    assert_eq!(
        etat().vivier.eveillee("r2-sommeil"),
        Some(true),
        "précondition : la fenêtre est bien éveillée"
    );
    let _ = canal.vider();
    boucher_la_file("r2-sommeil");

    // Elle devient invisible : le vivier émet `Dormir(Masquee)` — REFUSÉ.
    signaler("r2-sommeil", false, false);

    assert_eq!(
        etat().vivier.eveillee("r2-sommeil"),
        Some(true),
        "un Dormir non déposé ne doit pas libérer la place d'une fenêtre qui encode encore"
    );

    let recus = canal.vider();
    assert_eq!(recus.len(), PROFONDEUR_MAX, "précondition : la file était bien pleine");
    un_tour_de_roue();
    let ordres: Vec<Ordre> = canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Sommeil(o) => Some(o),
            _ => None,
        })
        .collect();
    assert!(
        ordres.iter().any(|o| matches!(o, Ordre::Dormir(_))),
        "le Dormir non déposé doit repartir au tour suivant : {ordres:?}"
    );

    retirer("r2-sommeil", generation);
}
