//! Table tests: writes, ProjFS commands, age and budgets, apart from
//! `tests.rs` to stay under 500 lines.

use super::tests::{attributs, maintenant};
use super::*;

/// 🔴 **UNE ÉCRITURE ET UNE LECTURE NE PARTAGENT JAMAIS UNE CORRÉLATION.**
///
/// C'est la raison pour laquelle `inscrire_sans_commande` passe par CETTE
/// table, et non par un second compteur : deux compteurs indépendants sur le
/// même canal se collisionneraient, et la collision serait **silencieuse** —
/// une réponse appliquée à la mauvaise commande.
#[test]
fn une_ecriture_et_une_lecture_ne_partagent_jamais_une_correlation() {
    let e = maintenant() + DELAI_LIRE;
    let mut t = Table::nouvelle();
    let mut vues = std::collections::HashSet::new();
    for i in 0..64 {
        // Les deux sortes s'entrelacent, comme sur le chemin réel : un fil
        // d'écriture pousse pendant qu'une application lit.
        let lecture = t.inscrire(i, attributs(&format!("l{i}")), e);
        let ecriture = t.inscrire_sans_commande(
            Attendue::Ecrire {
                chemin: format!("e{i}"),
                dernier: false,
            },
            e,
        );
        assert!(
            vues.insert(lecture),
            "corrélation {lecture} distribuée deux fois"
        );
        assert!(
            vues.insert(ecriture),
            "corrélation {ecriture} distribuée deux fois"
        );
    }
    assert_eq!(t.en_vol(), 128);
}

/// Une inscription sans commande reçoit une corrélation **et aucun
/// `command_id`** : `verbes::completer` doit pouvoir la distinguer.
#[test]
fn une_inscription_sans_commande_n_a_pas_de_command_id() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let c = t.inscrire_sans_commande(
        Attendue::Creer {
            chemin: "neuf.txt".into(),
        },
        e,
    );
    let (commande, quoi, _) = t.resoudre(c, Instant::now()).expect("inscrite à l'instant");
    assert_eq!(
        commande, None,
        "une écriture ne complète AUCUN rappel ProjFS"
    );
    assert_eq!(
        quoi,
        Attendue::Creer {
            chemin: "neuf.txt".into()
        }
    );
}

/// 🔴 **L'ÂGE RENDU PAR `resoudre` EST UNE VRAIE SOUSTRACTION, PAS UN ZÉRO.**
///
/// C'est la seule chose que le pont sache mesurer d'une traversée (F4, §0.5),
/// et un `Duration::ZERO` constant rendrait tout l'histogramme de
/// `pont::latence` muet **sans qu'aucune ligne de recensement ne manque** :
/// `n:` monterait, `moy_us:` resterait à 0. Le temps étant un paramètre, le
/// test l'éprouve **sans dormir**.
#[test]
fn resoudre_rend_l_age_de_la_commande_et_non_zero() {
    let depart = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire(
        7,
        Attendue::Attributs {
            chemin: "a.txt".into(),
        },
        depart + DELAI_ATTRIBUTS,
    );

    // `inscrire` lit `Instant::now()` pour l'inscription ; on mesure donc un
    // âge PLANCHER en prenant un « maintenant » décalé de 250 ms.
    let (_, _, age) = t
        .resoudre(c, Instant::now() + Duration::from_millis(250))
        .expect("inscrite à l'instant");
    assert!(age >= Duration::from_millis(250), "âge rendu : {age:?}");
    assert!(
        age < Duration::from_millis(2_000),
        "l'âge n'est pas le budget : {age:?}"
    );
}

/// 🔴 **`vider` REND LES ÉCRITURES AVEC UN `command_id` ABSENT.**
///
/// Rendre `Some(0)` ferait appeler `PrjCompleteCommand(0)` à l'arrêt du pont,
/// c'est-à-dire compléter une commande qui appartient à quelqu'un d'autre.
#[test]
fn vider_rend_les_ecritures_avec_un_command_id_absent() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let lecture = t.inscrire(42, attributs("a"), e);
    let ecriture = t.inscrire_sans_commande(
        Attendue::Ecrire {
            chemin: "b".into(),
            dernier: true,
        },
        e,
    );
    let tout = t.vider();
    assert_eq!(tout.len(), 2);
    assert!(tout.contains(&(Some(42), lecture)));
    assert!(
        tout.contains(&(None, ecriture)),
        "l'écriture doit sortir SANS command_id"
    );
}

/// Une écriture expirée est retirée comme les autres.
///
/// L'exclure du balayage la laisserait en table **pour toujours** : rien
/// d'autre ne la retire, puisqu'aucun rappel ProjFS ne l'a inscrite et
/// qu'aucune annulation ne peut la viser.
#[test]
fn une_ecriture_expiree_est_retiree_comme_les_autres() {
    let debut = maintenant();
    let mut t = Table::nouvelle();
    let c = t.inscrire_sans_commande(
        Attendue::Ecrire {
            chemin: "gros.bin".into(),
            dernier: false,
        },
        debut + DELAI_ECRIRE,
    );
    assert!(t.expirees(debut).is_empty());
    assert_eq!(t.expirees(debut + DELAI_ECRIRE), vec![(None, c)]);
    assert_eq!(t.en_vol(), 0);
}

/// `annuler` ne peut pas viser une écriture — elle n'a pas de `command_id`.
///
/// ⚠️ **Sans cette assertion, `annuler(0)` pourrait apparier une écriture dont
/// le `command_id` est `None`** le jour où la comparaison serait écrite à
/// l'envers. Une application qui abandonne son E/S emporterait alors une
/// écriture due, qui ne serait jamais poussée ET jamais retirée du journal.
#[test]
fn annuler_ne_vise_jamais_une_ecriture() {
    let e = maintenant() + DELAI_ECRIRE;
    let mut t = Table::nouvelle();
    let ecriture = t.inscrire_sans_commande(
        Attendue::Ecrire {
            chemin: "a".into(),
            dernier: true,
        },
        e,
    );
    assert!(t.annuler(0).is_empty(), "aucune commande ProjFS 0 n'existe");
    assert_eq!(t.en_vol(), 1, "l'écriture est toujours là");
    assert!(t.resoudre(ecriture, Instant::now()).is_some());
}

/// 🔴 **LE RELEVÉ QUI REND LE LEGS N°4 DE F1 DIAGNOSTICABLE.**
///
/// F1 a mesuré des lectures qui CALENT sans jamais expirer — `commande expirée`
/// reste à 0 pendant 540 s — et déclare qu'on ne sait pas OÙ le blocage se
/// produit, « faute d'une trace à l'inscription en table ». C'est cette trace.
///
/// Rouge : rendre `None` inconditionnellement, ou rendre le MINIMUM au lieu du
/// maximum. Dans les deux cas le legs reste indiagnosticable, et c'est
/// exactement l'état d'aujourd'hui.
#[test]
fn plus_ancienne_rend_la_duree_de_la_plus_vieille_commande_en_vol() {
    let mut t = Table::nouvelle();
    let depart = Instant::now();
    // Rien en vol : `None`, et c'est la PREMIÈRE ligne du tableau de lecture —
    // « rien n'a jamais été inscrit, le blocage est dans le rappel ».
    assert_eq!(t.plus_ancienne(depart), None);

    t.inscrire(
        1,
        Attendue::Attributs { chemin: "a".into() },
        depart + Duration::from_secs(2),
    );
    std::thread::sleep(Duration::from_millis(20));
    t.inscrire(
        2,
        Attendue::Attributs { chemin: "b".into() },
        depart + Duration::from_secs(2),
    );

    let vue = t.plus_ancienne(Instant::now()).expect("deux en vol");
    assert!(
        vue >= Duration::from_millis(20),
        "la PLUS ANCIENNE, pas la plus jeune : {vue:?}"
    );
}

/// Les commandes **sans rappel ProjFS** sont comptées à part.
///
/// ⚠️ Une application figée avec `en vol=3` et `sans_commande=3` n'attend RIEN
/// du pont : les trois sont des poussées, et son blocage est ailleurs. Sans
/// cette distinction, le recensement ferait accuser le pont d'un blocage qui
/// ne le concerne pas.
#[test]
fn sans_commande_ne_compte_que_ce_qui_ne_complete_aucun_rappel() {
    let mut t = Table::nouvelle();
    let echeance = Instant::now() + Duration::from_secs(5);
    t.inscrire(1, Attendue::Attributs { chemin: "a".into() }, echeance);
    t.inscrire_sans_commande(
        Attendue::Ecrire {
            chemin: "b".into(),
            dernier: true,
        },
        echeance,
    );
    t.inscrire_sans_commande(
        Attendue::Muter {
            chemin: "c".into(),
            renommage: true,
            destination: Some("d".into()),
        },
        echeance,
    );
    assert_eq!(t.en_vol(), 3);
    assert_eq!(t.sans_commande(), 2);
}

/// ⚠️ **Les cinq budgets sont DISTINCTS, et le rester est le point.**
///
/// L'ancien pont en avait **un seul**, 10 s, pour tout (`src/file.js:89`),
/// d'où deux défauts symétriques : des lectures de gros blocs qui expiraient
/// avant d'aboutir, et des `getattr` qui figeaient l'Explorateur dix secondes
/// sur un chemin inexistant.
#[test]
fn les_cinq_budgets_sont_distincts() {
    let tous = [
        DELAI_ATTRIBUTS,
        DELAI_LIRE,
        DELAI_LISTER,
        DELAI_ECRIRE,
        DELAI_MUTATION,
    ];
    for (i, a) in tous.iter().enumerate() {
        for b in &tous[i + 1..] {
            assert_ne!(a, b, "deux budgets partagent la valeur {a:?}");
        }
    }
    // Le budget d'une mutation est entre celui d'une lecture et celui d'une
    // écriture : un seul aller-retour, mais dont le repli de copie est en
    // O(taille) côté navigateur.
    assert!(DELAI_MUTATION > DELAI_LIRE);
    assert!(DELAI_MUTATION < DELAI_ECRIRE);
}

/// 🔴 **`annuler` REND TOUTES LES CORRÉLATIONS D'UNE COMMANDE, ET C'EST LA
/// FENÊTRE DE LECTURE DE F3 QUI L'EXIGE.**
///
/// Rouge : n'en rendre qu'une, comme jusqu'à F2. Les *N−1* autres resteraient
/// en vol, expireraient au budget, et `service::balayer` appellerait alors
/// `PrjCompleteCommand` sur une commande **DÉJÀ COMPLÉTÉE** — un appel au
/// système sur un identifiant qui appartient désormais à quelqu'un d'autre.
/// *Muet, différé, et hors de notre processus.*
#[test]
fn annuler_retire_les_n_correlations_d_une_lecture_a_fenetre() {
    let mut t = Table::nouvelle();
    let e = maintenant() + DELAI_LIRE;
    let a = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 0,
            longueur: 4,
        },
        e,
    );
    let b = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 4,
            longueur: 4,
        },
        e,
    );
    let c = t.inscrire(
        7,
        Attendue::Lire {
            chemin: "g".into(),
            position: 8,
            longueur: 4,
        },
        e,
    );
    // Une commande VOISINE ne doit pas être emportée.
    let autre = t.inscrire(8, attributs("x"), e);
    assert_eq!(t.en_vol(), 4);

    let annulees = t.annuler(7);
    assert_eq!(
        annulees,
        vec![a, b, c],
        "les TROIS, dans un ordre déterministe"
    );
    assert_eq!(t.en_vol(), 1, "seule la commande 8 survit");
    assert!(t.resoudre(autre, Instant::now()).is_some());
}
