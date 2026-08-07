//! Tests d'`attentes.rs` — voisins de la logique qu'ils exercent (revue de
//! la tâche 16, D10 : la première extraction les avait laissés dans
//! `serveur/tests.rs`, séparés du registre qu'ils testent).
//!
//! **Ces tests ne s'exécutent PAS sur cet hôte** : `serveur.rs`, qui déclare
//! `mod attentes;`, porte `#![cfg(windows)]` sur tout le fichier, et
//! `capteur.rs` regate `pub mod serveur;` avec `#[cfg(windows)]` en plus. Sur
//! la cible par défaut (Linux), `capteur::serveur` — donc
//! `capteur::serveur::attentes` — ne compile pas du tout : `cargo test -p
//! agent capteur::serveur` rend `0 passed; ... filtered out`, jamais une
//! exécution. Seul `cargo check --target x86_64-pc-windows-gnu --tests`
//! vérifie ce module sur cet hôte (types, emprunts, arité — jamais
//! l'exécution, faute de VM ou d'émulateur ici).

use super::*;
use std::sync::mpsc::channel;

/// La course F5 (D7), sur le SECOND registre. Le sous-bloc D9 l'a fermée sur
/// le registre de sommeil seulement (`capteur/sommeil/registre.rs`) — le
/// brief de sa tâche 10 ne nommait que `sommeil`, et aucune revue par tâche
/// ne pouvait voir ce jumeau (voir le commentaire d'`oublier`).
///
/// ⚠️ **La génération se prend à l'ATTACHE** (dans `attendre_le_media`,
/// appelée depuis `ouvrir_les_commandes` dans `serveur.rs`), **pas au
/// lancement du processus** : la première version de ce leg, en D9, frappait
/// la génération au lancement, et `retirer_est_perime` (son équivalent sur
/// l'autre registre) ne pouvait alors structurellement pas rendre `true` en
/// production — le défaut était dans la conception, pas dans l'exécution. Ce
/// test exerce donc la séquence réelle : inscrire, réinscrire (réattache sous
/// le même nom), oublier l'ANCIENNE génération.
///
/// ⚠️ **Ce que ce test NE COUVRE PAS : le CÂBLAGE.** Il appelle
/// `attendre_le_media` et `oublier` directement, exactement comme le ferait
/// un appelant correct — mais aussi exactement comme le ferait un appelant
/// qui transmettrait la MAUVAISE génération à `oublier` (par exemple une
/// génération capturée trop tôt, ou celle d'une autre session). Les quatre
/// sites d'appel réels (`serveur.rs`, dans `ouvrir_les_commandes` et
/// `tenir_la_fenetre`) portent chacun la génération qu'ils ont reçue de LEUR
/// propre `attendre_le_media` — vérifié par LECTURE, pas par ce test. Rien de
/// bon marché ne le couvrirait (il faudrait de vrais tubes nommés, donc la
/// VM) : un successeur ne doit pas lire « ce test passe » comme une preuve
/// que le câblage de production est correct.
#[test]
fn un_oubli_perime_ne_retire_pas_l_attente_neuve() {
    // Nom propre à ce test : `ETAT` est un état GLOBAL du processus de test,
    // partagé avec les autres tests de ce module.
    let session = "test-course-f5-second-registre";
    etat().attentes.remove(session);

    let (media1, _recepteur1) = channel::<std::fs::File>();
    let g1 = attendre_le_media(session, media1);
    let (media2, _recepteur2) = channel::<std::fs::File>();
    let g2 = attendre_le_media(session, media2); // réattache
    assert!(g2 > g1);

    oublier(session, g1); // l'ancien fil se réveille trop tard
    assert!(
        etat().attentes.contains_key(session),
        "l'oubli de la génération 1 ne doit pas emporter la génération 2"
    );

    oublier(session, g2);
    assert!(!etat().attentes.contains_key(session));
}
