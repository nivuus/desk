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
//! vivier libère la place alors que la fenêtre encode encore, donc
//! ~~le plafond de huit encodeurs se sur-souscrit~~. **SUR-AFFIRMÉ ICI
//! AUSSI, ET LAISSÉ NON BARRÉ ALORS QUE LE MÊME FICHIER LE CORRIGE PLUS BAS**
//! (vers la ligne 112, et `vivier.rs` le réécrit à son tour) : la
//! sur-souscription a bien lieu, mais ce n'est pas ce que le remède empêche
//! — il la rend TRANSITOIRE au lieu de permanente. Il existe un
//! `echec_de_reveil` pour le premier sens ; il n'existe **aucun**
//! `echec_de_sommeil`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

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

/// 🔴 SENS 2 — UN `Dormir` REFUSÉ NE DOIT PAS LAISSER LE VIVIER COMPTER
/// ENDORMIE UNE FENÊTRE QUI ENCODE ENCORE. **C'est la moitié la plus
/// coûteuse**, et celle que le premier diagnostic avait manquée : sans le
/// remède, `eveillee` passe à `false` définitivement — la fenêtre n'a jamais
/// reçu l'ordre, tient toujours son encodeur, et **plus aucun arbitrage ne la
/// réordonnera**, puisque le vivier la croit déjà endormie.
///
/// ⚠️ ~~Le plafond de huit encodeurs se sur-souscrit.~~ **CE N'EST PAS CE QUE
/// LE REMÈDE EMPÊCHE, et l'écrire ainsi était sur-affirmé** (round 3) :
/// `arbitrer` libère le créneau et élit la remplaçante DANS LA MÊME PASSE,
/// étapes 1, 4 et 5, **avant que le dépôt ne soit seulement tenté** —
/// l'annulation ne court qu'après. La sur-souscription a donc bien lieu ; ce
/// que le remède obtient est qu'elle soit **TRANSITOIRE** au lieu de
/// permanente. Voir
/// `une_sur_souscription_par_un_dormir_non_depose_est_resorbee_au_tour_suivant`,
/// juste en dessous, qui la mesure dans les deux temps.
///
/// **Rougit sur sa PREMIÈRE assertion** — `un Dormir non déposé ne doit pas
/// laisser le vivier compter endormie une fenêtre qui encode encore` —,
/// `eveillee` valant alors `Some(false)`.
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
        "un Dormir non déposé ne doit pas laisser le vivier compter endormie une \
         fenêtre qui encode encore"
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

/// Compte les événements `tracing` émis par `sommeil::registre` sur ce fil.
///
/// 🔴 **C'EST CE QUI REND LA CADENCE MESURABLE PLUTÔT QU'AFFIRMÉE.** Sans lui,
/// on ne pourrait éprouver que le prédicat arithmétique — vrai quel que soit
/// le code qui l'emploie, donc un contrôle incapable d'échouer.
///
/// Le filtre est le `target`, que `tracing` remplit avec le chemin du module
/// d'émission : seules les lignes de `registre.rs` sont comptées, jamais
/// celles de `file.rs` qui sortent au même palier.
struct CompteurDeTraces(Arc<AtomicUsize>);

impl<S: tracing::Subscriber> Layer<S> for CompteurDeTraces {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() == "agent::capteur::sommeil::registre" {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Le compte CUMULÉ de dépôts refusés d'une session.
fn refus_de(session: &str) -> u64 {
    etat().canaux.get(session).expect("la session est inscrite").refuses()
}

/// 🔴 LE DÉFAUT QUE LE REMÈDE DU ROUND 2 A LUI-MÊME CRÉÉ, ET SA CADENCE.
///
/// Tant que l'ordre était PERDU, `distribuer` n'émettait un ordre que sur un
/// CHANGEMENT d'arbitrage — jamais à chaque tour. C'était le motif écrit pour
/// ne pas cadencer sa trace, et **il était vrai**.
///
/// 🔴 **L'ANNULATION L'A RENVERSÉ.** L'état étant désormais RENDU, le
/// ré-arbitrage suivant revoit la même divergence, réémet le même ordre, et il
/// est refusé à nouveau : **+1 par tour, strictement, sans borne**. À
/// `PERIODE_REARBITRAGE` (250 ms), cela ferait **quatre lignes par seconde et
/// par fenêtre bloquée, indéfiniment** — le piège que `file.rs` évite dix
/// lignes plus loin, et que `CLAUDE.md` nomme (18 619 lignes en quelques
/// secondes ont déjà empêché une session de s'établir).
///
/// 🔴 **CE TEST COMPTE LES TRACES RÉELLEMENT ÉMISES**, par un abonné `tracing`
/// posé sur ce fil — et non le prédicat arithmétique de la cadence. Un premier
/// jet le faisait, et cette assertion-là était **structurellement incapable
/// d'échouer** : elle aurait été vraie quel que soit le code de `distribuer`.
///
/// **Rougit sur sa SECONDE assertion, DES DEUX CÔTÉS** — et c'est une
/// égalité (`assert_eq!`), pas une borne haute : une borne `<= 4` ne
/// dénonce pas une trace SUPPRIMÉE (`0 <= 4` est vrai). Sans la trace
/// (`if refuses.is_power_of_two()` → `if false` dans `registre.rs`) :
/// `0` au lieu de `3`. Sans la cadence (→ `if true`) : `10` traces pour dix
/// tours, au lieu des `3` paliers réellement franchis (2, 4, 8).
#[test]
fn un_ordre_refuse_a_chaque_tour_est_trace_a_cadence_logarithmique() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("r3-cadence", 6600);
    signaler("r3-cadence", true, true);
    let _ = canal.vider();
    boucher_la_file("r3-cadence");
    // Le masquage engendre un `Dormir` — refusé, et annulé.
    signaler("r3-cadence", false, false);
    let depart = refus_de("r3-cadence");

    const TOURS: u64 = 10;
    let compte = Arc::new(AtomicUsize::new(0));
    let abonne = tracing_subscriber::registry().with(CompteurDeTraces(Arc::clone(&compte)));
    let comptes: Vec<u64> = tracing::subscriber::with_default(abonne, || {
        (0..TOURS)
            .map(|_| {
                un_tour_de_roue();
                refus_de("r3-cadence")
            })
            .collect()
    });

    // 🔴 LE PHÉNOMÈNE : chaque tour de roue réémet l'ordre, et chaque
    // réémission est refusée. C'est ce que l'annulation du round 2 obtient —
    // et c'est ce qui rend une trace non cadencée non bornée.
    let attendus: Vec<u64> = (1..=TOURS).map(|i| depart + i).collect();
    assert_eq!(comptes, attendus, "l'ordre doit être réémis à CHAQUE tour");

    // 🔴 LA CADENCE, MESURÉE SUR LES LIGNES ÉMISES ET ÉGALÉE, PAS SEULEMENT
    // BORNÉE PAR LE HAUT.
    //
    // ⚠️ **UNE BORNE `<= 4` NE ROUGIT PAS SI LA TRACE DISPARAÎT ENTIÈREMENT**
    // — mesuré : `if refuses.is_power_of_two()` remplacé par `if false` dans
    // `registre.rs` (la trace supprimée) laisse `cargo test --workspace`
    // entièrement VERT, `0 <= 4` étant vrai. Ce que ce test doit tenir n'est
    // pas seulement « pas trop de lignes », mais « exactement les lignes
    // attendues ».
    //
    // Sur les dix tours de cette boucle, partant d'un compte de refus
    // CUMULÉ non nul (`depart`), les paliers de puissance de deux réellement
    // franchis sont **2, 4 et 8** — trois lignes, ni plus ni moins.
    // `assert_eq!` rougit donc des DEUX côtés : la trace supprimée (`0`,
    // au lieu de `3`) ET la trace non cadencée (`10`, au lieu de `3`).
    let traces = compte.load(Ordering::Relaxed);
    assert_eq!(
        traces, 3,
        "la trace de l'ordre non déposé doit être CADENCÉE aux puissances de \
         deux, ni supprimée ni émise à chaque refus : {traces} lignes pour \
         {TOURS} tours, 3 attendues (paliers 2, 4, 8)"
    );

    retirer("r3-cadence", generation);
}

/// 🔴 CE QUE LE REMÈDE OBTIENT RÉELLEMENT, MESURÉ DANS LES DEUX TEMPS — et ce
/// qu'il n'obtient PAS.
///
/// ⚠️ **Les deux tests ci-dessus ne peuvent pas voir ce défaut** : ils n'ont
/// qu'UNE session, donc ils mesurent `eveillee(s)` et jamais
/// `eveillees().len()`. C'est ce trou qui a laissé passer l'affirmation
/// « la place n'est pas libérée », mesurée fausse au round 3.
///
/// **Le mécanisme** : `arbitrer` pose `eveillee = false` à l'étape 1, exclut
/// donc la session des épinglées à l'étape 2, remplit le créneau libéré à
/// l'étape 4, et émet le `Reveiller` de la neuvième à l'étape 5 — **tout dans
/// la même passe, avant que le dépôt du `Dormir` ne soit seulement tenté**.
/// L'annulation ne court qu'après : elle ne peut pas l'empêcher.
///
/// **Ce qui est vrai, et que ce test tient** : la sur-souscription est
/// TRANSITOIRE. Au ré-arbitrage suivant, le vivier voit 9 > 8 et rendort
/// quelqu'un. Sans le remède, il ne verrait jamais 9 — il compterait 8 en
/// croyant la fenêtre bloquée endormie, et la dérive serait PERMANENTE.
#[test]
fn une_sur_souscription_par_un_dormir_non_depose_est_resorbee_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let plafond = crate::capteur::vivier::PLAFOND_EVEIL;

    // Sature les places, en gardant les receveurs vivants.
    let mut occupantes = Vec::new();
    for i in 0..plafond {
        let nom = format!("r3-plein-{i}");
        let (canal, generation) = inscrire(&nom, 6700 + i as u32);
        signaler(&nom, true, true);
        occupantes.push((nom, canal, generation));
    }
    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond,
        "précondition : les places sont toutes prises"
    );

    // Une candidate de plus, qui attend qu'une place se libère.
    let (attente, generation_attente) = inscrire("r3-attente", 6799);
    signaler("r3-attente", true, true);
    assert_eq!(etat().vivier.eveillees().len(), plafond, "précondition : elle attend");

    // La file de la PREMIÈRE occupante se bouche, puis elle est masquée : son
    // `Dormir` est refusé, et annulé — mais `arbitrer` a déjà élu la neuvième
    // dans la même passe.
    let (ref nom_bloquee, ref canal_bloquee, _) = occupantes[0];
    let _ = canal_bloquee.vider();
    boucher_la_file(nom_bloquee);
    signaler(nom_bloquee, false, false);

    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond + 1,
        "la sur-souscription a bien lieu : l'annulation ne court qu'APRÈS l'élection"
    );

    // La fenêtre bloquée reprend sa lecture ; le tour suivant résorbe.
    let _ = canal_bloquee.vider();
    un_tour_de_roue();
    assert_eq!(
        etat().vivier.eveillees().len(),
        plafond,
        "la sur-souscription doit être TRANSITOIRE : résorbée au ré-arbitrage suivant"
    );

    retirer("r3-attente", generation_attente);
    drop(attente);
    for (nom, canal, generation) in occupantes {
        retirer(&nom, generation);
        drop(canal);
    }
}
