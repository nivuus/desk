//! Tests de `serveur.rs` — extraits ici plutôt qu'en ligne : `serveur.rs`
//! avait franchi 500 lignes en accueillant ce module (D10, tâche 16), et
//! `CLAUDE.md` interdit la compression au profit de l'extraction. Même
//! montage que `capteur/distante/tests.rs` et `capteur/sommeil/tests.rs`.
//!
//! **Ces tests ne s'exécutent PAS sur cet hôte** : `serveur.rs` porte
//! `#![cfg(windows)]` sur tout le fichier, et `capteur.rs` regate
//! `pub mod serveur;` avec `#[cfg(windows)]` en plus. Sur la cible par
//! défaut (Linux), `capteur::serveur` ne compile pas du tout — `cargo test -p
//! agent capteur::serveur` rend donc `0 passed; ... filtered out`, jamais une
//! exécution. Seul `cargo check --target x86_64-pc-windows-gnu --tests`
//! vérifie ce module sur cet hôte (types, emprunts, arité — jamais
//! l'exécution, faute de VM ou d'émulateur ici).

use super::*;

/// La course F5 (D7), sur le SECOND registre. Le sous-bloc D9 l'a fermée
/// sur le registre de sommeil seulement (`capteur/sommeil/registre.rs`) —
/// le brief de sa tâche 10 ne nommait que `sommeil`, et aucune revue par
/// tâche ne pouvait voir ce jumeau (voir le commentaire d'`oublier`
/// ci-dessus).
///
/// ⚠️ **La génération se prend à l'ATTACHE** (dans `attendre_le_media`,
/// appelée depuis `ouvrir_les_commandes`), **pas au lancement du
/// processus** : la première version de ce leg, en D9, frappait la
/// génération au lancement, et `retirer_est_perime` (son équivalent sur
/// l'autre registre) ne pouvait alors structurellement pas rendre `true`
/// en production — le défaut était dans la conception, pas dans
/// l'exécution. Ce test exerce donc la séquence réelle : inscrire,
/// réinscrire (réattache sous le même nom), oublier l'ANCIENNE
/// génération.
///
/// Les aides `inscrire_attente`/`canal_d_essai`/`attente_existe`
/// esquissées par le brief n'existent pas sous ces noms : ce fichier n'a
/// pas de couture dédiée aux tests. `attendre_le_media` EST la fonction
/// de production qui inscrit une attente (celle qu'appelle
/// `ouvrir_les_commandes`), et un canal de test s'obtient directement par
/// `channel::<std::fs::File>()`, exactement comme le fait le code réel —
/// c'est ce que ce test emploie à la place.
#[test]
fn un_oubli_perime_ne_retire_pas_l_attente_neuve() {
    // Nom propre à ce test : `EN_ATTENTE_DE_MEDIA` est un état GLOBAL du
    // processus de test, partagé avec les autres tests de ce module.
    let session = "test-course-f5-second-registre";
    registre_verrouille().remove(session);

    let (media1, _recepteur1) = channel::<std::fs::File>();
    let g1 = attendre_le_media(session, media1);
    let (media2, _recepteur2) = channel::<std::fs::File>();
    let g2 = attendre_le_media(session, media2); // réattache
    assert!(g2 > g1);

    oublier(session, g1); // l'ancien fil se réveille trop tard
    assert!(
        registre_verrouille().contains_key(session),
        "l'oubli de la génération 1 ne doit pas emporter la génération 2"
    );

    oublier(session, g2);
    assert!(!registre_verrouille().contains_key(session));
}
