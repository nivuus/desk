//! Tests de `capteur::sommeil::parts` — fichier voisin plutôt que module en
//! ligne.
//!
//! **Extraction jouée dans une tâche DÉDIÉE, AVANT celle qui ajoute** (round
//! de correction 2, 25 août 2026) : `parts.rs` était à 488 lignes pour un
//! plafond de projet à 500, et le round 2 y porte à la fois un test réécrit
//! et une correction de doc. Extraire, jamais comprimer — et « la marge
//! regagnée se reperd si on la traite comme acquise », payé six fois.
//!
//! ⚠️ **`#[path]` chez le parent, et ce n'est PAS la convention
//! `<parent>_<enfant>`** : celle-ci ne vise que les modules extraits d'un
//! parent `#[cfg(windows)]` pour compiler sur l'hôte. Ici la raison est
//! autre — scinder un module de TESTS trop long dans un fichier par ailleurs
//! portable —, cas que `CLAUDE.md` met explicitement hors de sa portée.
//! Précédents du dépôt : `superviseur/table.rs`, et `file/tests.rs` extrait
//! au round 1 pour la même raison.
//!
//! **Le chemin de module reste `parts::tests`** : seul l'emplacement physique
//! du fichier change, aucune visibilité n'est touchée.


use crate::capteur::sommeil::tests::{premier_ordre, verrouiller_pour_le_test};
use crate::capteur::sommeil::file::{ReceveurSession, PROFONDEUR_MAX};
use crate::capteur::sommeil::{etat, inscrire, retirer, signaler, Message};
use crate::capteur::vivier::Ordre;

/// Dernière part reçue sur un canal, en vidant ce qui s'y trouve.
fn derniere_part(canal: &ReceveurSession) -> Option<u32> {
    canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(bps),
            _ => None,
        })
        .last()
}

/// 🔴 CE TEST A ÉTÉ RÉÉCRIT LE 25 AOÛT 2026, PARCE QUE LA BORNE PAR
/// COALESCENCE (`file.rs`) A RENDU SON ASSERTION D'ORIGINE FAUSSE — et
/// c'est la seule rouge qu'elle a produite sur les 1 022 tests du dépôt.
///
/// **Ce qu'il éprouvait**, sous le canal `mpsc` NON BORNÉ : que la part
/// d'éveillée arrive APRÈS l'ordre de réveil (`position_reveil <
/// position_part`), son nom étant
/// `une_session_qui_s_eveille_recoit_une_part_apres_son_ordre_de_reveil`.
/// Il tenait parce que la file gardait les DEUX parts : celle du plancher
/// endormi émise par `inscrire`, puis celle d'éveillée émise après le
/// `Reveiller`.
///
/// **Ce que la coalescence en place fait, et c'est voulu** : la seconde
/// part REMPLACE la première À SA PLACE, c'est-à-dire AVANT le
/// `Reveiller`. Il n'existe donc plus de part après l'ordre — non parce
/// qu'elle manque, mais parce qu'il n'y en a plus qu'UNE, et qu'elle porte
/// déjà la valeur d'éveillée.
///
/// 🔵 **CONSÉQUENCE ASSUMÉE, ET IL FAUT LA DIRE : L'ORDRE ENTRE UNE `Part`
/// ET UN `Sommeil` N'EST PLUS GARANTI** quand la fenêtre n'a pas lu entre
/// les deux. La coalescence en place conserve la position du CRÉNEAU, pas
/// l'ordre d'arrivée des VALEURS : la part peut précéder d'un message
/// l'ordre qui la motive.
///
/// ❌ ~~C'est le moindre des deux maux : coalescer en QUEUE livrerait une
/// part après un ordre qui la rend caduque, et `dernieres_parts` filtrant
/// les répétitions, cette part périmée ne serait jamais corrigée.~~
/// **CET ARGUMENT ÉTAIT FAUX, et le round de correction 1 l'a réfuté** :
/// coalescer en queue placerait toujours la valeur la plus RÉCENTE en
/// queue, donc `[Reveiller, Part(éveillée)]` — chronologiquement juste ET
/// portant la bonne valeur. Le mode de défaillance décrit n'existe pas.
/// ⚠️ Cette phrase contredisait de surcroît l'en-tête de `file.rs`, écrit
/// dans le MÊME commit, qui affirmait que remplacer en place « préserve
/// l'ordre » : les deux disent désormais la même chose, et la vraie.
///
/// 🔵 **CE QUI JUSTIFIE RÉELLEMENT LE CHOIX** : le désordre ne porte que
/// sur des variantes que le capteur RELAIE sans les appliquer —
/// `fenetre/transitions.rs` écrit « rien à faire localement » pour `Part`
/// comme pour `Audio`, seul `Sommeil` ayant un effet local. Les deux
/// politiques convergent vers le même état final, et la valeur livrée est
/// dans les deux cas la dernière calculée. La borne du désordre est d'UN
/// message, sur une fenêtre qui n'encode pas encore (elle attend son
/// réveil).
#[test]
fn une_session_qui_s_eveille_recoit_la_part_d_une_eveillee_et_une_seule() {
    let _verrou = verrouiller_pour_le_test();
    let (messages, generation) = inscrire("t6-a", 6001);
    signaler("t6-a", true, true);

    let recus: Vec<Message> = messages.vider();
    assert!(
        recus.iter().any(|m| matches!(m, Message::Sommeil(Ordre::Reveiller))),
        "l'ordre de réveil doit être présent : {recus:?}"
    );
    let parts: Vec<u32> = recus
        .iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(*bps),
            _ => None,
        })
        .collect();
    // **UNE seule**, et c'est la preuve directe de la coalescence : sans
    // elle il y en aurait deux, le plancher endormi de `inscrire` puis
    // celle d'éveillée.
    assert_eq!(parts.len(), 1, "les deux parts doivent s'être coalescées : {recus:?}");
    // **La valeur qui survit est la DERNIÈRE calculée**, celle d'une
    // éveillée — pas le plancher qu'elle a remplacé. C'est ce qui rend le
    // désordre d'un message inoffensif, et sans cette assertion le test
    // passerait aussi si la coalescence avait gardé la PREMIÈRE valeur.
    assert!(
        parts[0] > crate::capteur::repartiteur::PART_DORMANTE_BPS,
        "la part qui survit est celle d'une ÉVEILLÉE, pas le plancher : {parts:?}"
    );
    retirer("t6-a", generation);
}

#[test]
fn une_part_inchangee_n_est_pas_reemise() {
    let _verrou = verrouiller_pour_le_test();
    let (messages, generation) = inscrire("t6-b", 6002);
    signaler("t6-b", true, true);
    let _ = messages.vider();

    // Même signal, donc même état, donc même part : rien ne doit partir.
    signaler("t6-b", true, true);
    let parts: Vec<Message> = messages
        .vider()
        .into_iter()
        .filter(|m| matches!(m, Message::Part { .. }))
        .collect();
    assert!(parts.is_empty(), "une part inchangée ne se réémet pas : {parts:?}");
    retirer("t6-b", generation);
}

#[test]
fn l_arrivee_d_une_seconde_fenetre_reduit_la_part_de_la_premiere() {
    let _verrou = verrouiller_pour_le_test();
    let (a, generation_a) = inscrire("t6-c", 6003);
    signaler("t6-c", true, true);
    let premiere = derniere_part(&a).expect("la première doit avoir une part");

    let (b, generation_b) = inscrire("t6-d", 6004);
    signaler("t6-d", true, false);
    let apres = derniere_part(&a).expect("la première doit être ré-servie");
    assert!(
        apres < premiere,
        "part de la première : {premiere} puis {apres} — elle doit baisser"
    );
    assert!(derniere_part(&b).is_some(), "la seconde doit recevoir une part");

    retirer("t6-c", generation_a);
    retirer("t6-d", generation_b);
}

/// Le test qui couvre le défaut trouvé en revue : un canal rompu détecté
/// PENDANT la distribution des PARTS (pas pendant celle des ordres) doit
/// libérer sa place au vivier, pas seulement dans `canaux`. Avant le
/// remède, l'entrée y survivait pour toute la vie du processus dès qu'un
/// fil de fenêtre mourait sans passer par `retirer` — le cas nominal
/// d'une panique, court-circuitant le point de passage unique de
/// `Fenetre::servir`.
#[test]
fn un_canal_rompu_detecte_par_les_parts_est_retire_du_vivier() {
    let _verrou = verrouiller_pour_le_test();
    // Sature les PLAFOND_EVEIL (8) places.
    let mut recepteurs_pleins = Vec::new();
    for i in 0..8 {
        let nom = format!("t7-plein-{i}");
        let (ordres, generation) = inscrire(&nom, 6100 + i as u32);
        signaler(&nom, true, false);
        assert_eq!(
            premier_ordre(&ordres),
            Some(Ordre::Reveiller),
            "{nom} devrait s'eveiller"
        );
        recepteurs_pleins.push((nom, ordres, generation));
    }

    // Le fil de la premiere "meurt" : son recepteur est jete SANS passer
    // par `retirer`, exactement ce qui arrive quand un fil de fenetre
    // panique avant d'atteindre son point de retrait unique. `canaux`
    // garde donc une entree dont plus personne ne lit.
    let (session_morte, recepteur_mort, generation_morte) = recepteurs_pleins.remove(0);
    drop(recepteur_mort);

    // "t7-attend" arrive. La simple INSCRIPTION d'une session neuve
    // (endormie) fait deja varier le calcul des parts des huit eveillees
    // existantes : chaque endormie retranche son `PART_DORMANTE_BPS` du
    // budget partage AVANT que le reste ne soit divise (voir `repartir`),
    // donc `reste` change, donc la part de CHAQUE eveillee change — y
    // compris celle de la session morte. La tentative d'envoi qui en
    // resulte sur son canal rompu declenche le remede : elle est retiree
    // du VIVIER (et pas seulement de `canaux`), ce qui libere sa place.
    let (ordres_attend, generation_attend) = inscrire("t7-attend", 6200);

    // Se signaler visible suffit desormais : la place est deja libre.
    // Sans le remede (retrait du vivier en plus de `canaux`), la session
    // morte y resterait comptee comme eveillee pour toujours, la place ne
    // se libererait jamais, et "t7-attend" resterait endormie ici.
    signaler("t7-attend", true, false);
    assert_eq!(
        premier_ordre(&ordres_attend),
        Some(Ordre::Reveiller),
        "le retrait de la session morte, detecte par la distribution des parts, \
         doit liberer sa place au vivier"
    );

    // Nettoyage. `retirer` sur la session deja retiree par le remede est
    // un no-op sur une cle deja absente, aussi bien pour `Vivier::retirer`
    // (HashMap::remove) que pour `canaux` — pas un double retrait.
    retirer("t7-attend", generation_attend);
    retirer(&session_morte, generation_morte);
    for (nom, _, generation) in recepteurs_pleins {
        retirer(&nom, generation);
    }
}

/// Le défaut trouvé en revue de la tâche 6 : `sommeil::inscrire` remplace
/// le canal d'une session déjà connue (rattachement après rupture de
/// tube) sans purger `dernieres_parts`. Si la topologie n'a pas changé
/// entre les deux inscriptions, la part recalculée est identique à celle
/// déjà mémorisée, le filtre d'écrasement de `distribuer_les_parts` la
/// juge donc déjà livrée, et le canal NEUF ne reçoit jamais rien — le
/// plafond de débit de cet enfant reste périmé sans terme.
#[test]
fn un_rattachement_a_topologie_inchangee_renvoie_une_part_sur_le_canal_neuf() {
    let _verrou = verrouiller_pour_le_test();

    // Premier canal : inscription seule, aucune autre fenêtre, aucun
    // signal — la fenêtre naît endormie et reçoit tout de même la part
    // plancher à l'inscription (voir la doc de `inscrire`).
    let (premier_canal, _generation_initiale) = inscrire("t8-rattache", 6300);
    let premiere_part = derniere_part(&premier_canal)
        .expect("une première part doit partir à l'inscription initiale");

    // Le tube se rompt et l'enfant se rattache : MÊME session, rien
    // d'autre dans la topologie n'a bougé (aucune autre fenêtre, aucun
    // signal de visibilité entre-temps). `inscrire` détecte le
    // remplacement (elle journalise « canal d'ordres remplacé pour
    // cette session ») et rend un canal neuf, avec une génération neuve
    // (D9, F5 de D7).
    let (canal_neuf, generation_neuve) = inscrire("t8-rattache", 6300);

    // Sans le remède, la part recalculée est identique à `premiere_part`
    // : `dernieres_parts` la juge déjà livrée (elle l'était, mais sur
    // L'ANCIEN canal, disparu avec la rupture) et rien ne part sur le
    // canal neuf, qui reste muet pour toujours tant que la topologie ne
    // change pas.
    assert_eq!(
        derniere_part(&canal_neuf),
        Some(premiere_part),
        "le canal neuf doit recevoir sa part même si elle est identique à celle \
         déjà envoyée sur l'ancien canal : dernieres_parts doit être purgée pour \
         cette session au moment où son canal est remplacé"
    );

    retirer("t8-rattache", generation_neuve);
}
/// 🔴 LA ROUGE DU CRITIQUE DU ROUND 1 : UNE PART REFUSÉE ÉTAIT MÉMORISÉE
/// COMME ENVOYÉE, ET N'ÉTAIT PLUS JAMAIS RÉÉMISE.
///
/// Sous `mpsc`, `send(...).is_ok()` valait « livré ». Depuis la file
/// bornée, il ne vaut plus que « pas déconnecté » : `Ok(Depot::Refusee)`
/// est un refus de file pleine, et le lire comme une livraison faisait
/// écrire la valeur dans `dernieres_parts`. Le garde d'écrasement en tête
/// de boucle (`if dernieres_parts.get(&session) == Some(&bps) { continue }`)
/// supprimait alors **toute réémission future de cette valeur** : la
/// fenêtre restait à son débit précédent tant que sa part calculée ne
/// changeait pas — sans borne, et sans une ligne de journal.
///
/// **Ce test échoue sur sa DERNIÈRE assertion avant le correctif**
/// (`une part refusée doit être RÉÉMISE au tour suivant`), la part ayant
/// été mémorisée à tort. Les deux premières passent des deux côtés : elles
/// établissent la précondition (la file est bien pleine, la part n'est
/// bien pas livrée), sans quoi la troisième ne mesurerait rien.
#[test]
fn une_part_refusee_n_est_pas_memorisee_et_repart_au_tour_suivant() {
    let _verrou = verrouiller_pour_le_test();
    let (canal, generation) = inscrire("t17-refus", 6400);
    // On part d'une file vide et d'une mémoire déjà posée par
    // l'inscription : c'est l'état ordinaire d'une session vivante.
    let _ = canal.vider();

    // Sature la file par des messages INCOALESCABLES — `Sommeil` ne se
    // coalesce jamais, c'est ce qui permet d'atteindre la borne.
    {
        let garde = etat();
        let emetteur = garde.canaux.get("t17-refus").expect("la session est inscrite");
        for _ in 0..PROFONDEUR_MAX {
            let _ = emetteur.envoyer(Message::Sommeil(Ordre::Reveiller));
        }
    }

    // La session s'éveille : sa part passe du plancher au budget entier,
    // donc une part NEUVE est calculée — et REFUSÉE, la file étant pleine.
    signaler("t17-refus", true, true);

    // La fenêtre reprend sa lecture. Aucune part ne s'y trouve : elle n'a
    // jamais été déposée.
    let recus = canal.vider();
    assert_eq!(recus.len(), PROFONDEUR_MAX, "précondition : la file était bien pleine");
    assert!(
        !recus.iter().any(|m| matches!(m, Message::Part { .. })),
        "précondition : la part refusée n'a PAS été livrée : {recus:?}"
    );

    // Le tour suivant, sans que rien n'ait changé : la MÊME valeur doit
    // repartir, puisqu'elle n'a jamais atteint la fenêtre.
    {
        let mut garde = etat();
        super::distribuer_les_parts(&mut garde);
    }
    let parts: Vec<u32> = canal
        .vider()
        .into_iter()
        .filter_map(|m| match m {
            Message::Part { bps } => Some(bps),
            _ => None,
        })
        .collect();
    assert!(
        !parts.is_empty(),
        "une part refusée doit être RÉÉMISE au tour suivant : elle n'a jamais été livrée"
    );

    retirer("t17-refus", generation);
}
