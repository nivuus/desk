//! Tests d'hôte de [`super`]. **Aucun ne dort** : la durée est un paramètre.

use std::time::Duration;

use super::{nom, seau_de, Famille, Histogramme, NOMBRE, SEAUX, SEAUX_MS};
use crate::pont::table::Attendue;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn attributs(chemin: &str) -> Attendue {
    Attendue::Attributs { chemin: chemin.to_string() }
}

/// ⚠️ **CE TEST N'ÉPROUVE QU'UN `Default`, ET IL N'EST GARDÉ QUE PARCE QU'IL EST
/// LE TÉMOIN DE DÉPART DES AUTRES.** C'est le legs n°11 de D9
/// (`une_telemetrie_neuve_est_a_zero`, « incapable de rendre l'autre valeur ») :
/// sans lui, `les_familles_ne_se_melangent_pas` ne pourrait pas distinguer
/// « la famille voisine n'a pas bougé » de « elle n'a jamais rien porté ».
#[test]
fn un_histogramme_neuf_rend_des_zeros() {
    let h = Histogramme::nouveau();
    for f in Famille::TOUTES {
        assert_eq!(h.compte(f), 0, "{}", nom(f));
        assert_eq!(h.moyenne_us(f), 0, "{}", nom(f));
        assert_eq!(h.max_us(f), 0, "{}", nom(f));
        for i in 0..SEAUX {
            assert_eq!(h.seau(f, i), 0, "{} seau {}", nom(f), i);
        }
    }
}

/// 🔴 **La borne est INCLUSIVE en haut.** Un `<` au lieu d'un `<=` décalerait
/// toute la distribution d'un seau, en silence : chaque valeur EXACTEMENT égale
/// à une borne est éprouvée, plus une valeur strictement en dessous et une
/// strictement au-dessus.
#[test]
fn une_traversee_tombe_dans_le_seau_qui_la_contient() {
    // Exactement sur chaque borne : le seau de MÊME rang.
    for (i, borne) in SEAUX_MS.iter().enumerate() {
        assert_eq!(seau_de(ms(*borne)), i, "borne {} ms", borne);
    }
    // Juste en dessous de la première borne, et zéro.
    assert_eq!(seau_de(Duration::from_micros(999)), 0);
    assert_eq!(seau_de(Duration::ZERO), 0);
    // Juste au-dessus d'une borne : le seau SUIVANT.
    assert_eq!(seau_de(Duration::from_micros(1_001)), 1, "1,001 ms");
    assert_eq!(seau_de(Duration::from_micros(20_001)), 5, "20,001 ms -> seau 50");
    // Au-delà de la dernière borne : `inf`, le treizième.
    assert_eq!(seau_de(ms(5_001)), SEAUX_MS.len());
    assert_eq!(seau_de(ms(30_000)), SEAUX_MS.len());

    // Et le seau est bien celui que l'histogramme incrémente.
    let h = Histogramme::nouveau();
    h.observer(Famille::Lire, ms(5));
    assert_eq!(h.seau(Famille::Lire, 2), 1, "5 ms est dans le seau '5'");
    assert_eq!(h.seau(Famille::Lire, 3), 0, "et surtout PAS dans le seau '10'");
}

/// Le jumeau du garde de `pont::compteurs` : un `rang()` faux ferait compter
/// une famille sur le dos d'une autre.
#[test]
fn les_familles_ne_se_melangent_pas() {
    let h = Histogramme::nouveau();
    h.observer(Famille::Lire, ms(7));
    h.observer(Famille::Lire, ms(7));
    h.observer(Famille::Lister, ms(300));

    assert_eq!(h.compte(Famille::Lire), 2);
    assert_eq!(h.compte(Famille::Lister), 1);
    for f in [Famille::Attributs, Famille::Ecrire, Famille::Mutation] {
        assert_eq!(h.compte(f), 0, "{} n'a rien reçu", nom(f));
        assert_eq!(h.max_us(f), 0, "{}", nom(f));
    }
    // Les seaux non plus ne se mélangent pas.
    assert_eq!(h.seau(Famille::Lire, 3), 2, "7 ms -> seau '10'");
    assert_eq!(h.seau(Famille::Lister, 3), 0);
    assert_eq!(h.seau(Famille::Lister, 8), 1, "300 ms -> seau '500'");
}

/// Une somme et un compte échangés rendraient la moyenne égale au compte.
#[test]
fn le_max_est_le_max_et_la_moyenne_est_la_moyenne() {
    let h = Histogramme::nouveau();
    h.observer(Famille::Attributs, ms(2));
    h.observer(Famille::Attributs, ms(8));
    h.observer(Famille::Attributs, ms(2));

    assert_eq!(h.compte(Famille::Attributs), 3);
    assert_eq!(h.moyenne_us(Famille::Attributs), 4_000, "(2+8+2)/3 = 4 ms");
    assert_eq!(h.max_us(Famille::Attributs), 8_000);

    // Le maximum ne RECULE pas quand une traversée plus courte suit.
    h.observer(Famille::Attributs, ms(1));
    assert_eq!(h.max_us(Famille::Attributs), 8_000, "fetch_max, pas store");
}

/// 🔴 **Une dérive d'ordre ferait LIRE UN COMPTEUR POUR UN AUTRE.** La ligne
/// est épinglée en toutes lettres, valeurs comprises.
#[test]
fn l_ordre_du_recensement_est_epingle() {
    let h = Histogramme::nouveau();
    h.observer(Famille::Lire, ms(3));

    let ligne = h.recensement();
    let (tetes, seaux) = ligne.split_once(" | ").expect("la ligne a deux moitiés");

    assert_eq!(
        tetes,
        "traversees attributs=n:0 moy_us:0 max_us:0 lister=n:0 moy_us:0 max_us:0 \
         lire=n:1 moy_us:3000 max_us:3000 ecrire=n:0 moy_us:0 max_us:0 \
         mutation=n:0 moy_us:0 max_us:0"
    );
    // Les CINQ familles portent leurs seaux, et `lire` porte le sien au bon rang.
    assert!(seaux.starts_with("seaux_ms attributs="), "{seaux}");
    for f in Famille::TOUTES {
        assert!(seaux.contains(&format!(" {}=1:", nom(f))), "{} absent : {seaux}", nom(f));
    }
    assert!(seaux.contains("lire=1:0,2:0,5:1,10:0,"), "3 ms est dans le seau '5' : {seaux}");
    assert!(seaux.ends_with("inf:0"), "{seaux}");
    // Une seule occurrence de chaque nom de famille dans chaque moitié.
    for f in Famille::TOUTES {
        assert_eq!(tetes.matches(&format!(" {}=", nom(f))).count(), 1, "{}", nom(f));
        assert_eq!(seaux.matches(&format!(" {}=", nom(f))).count(), 1, "{}", nom(f));
    }
}

/// Le troisième étage du garde structurel : sans un nom propre, une famille
/// neuve n'apparaîtrait pas au recensement, ou pire, s'y confondrait avec une
/// autre.
#[test]
fn une_famille_neuve_ne_peut_pas_heriter_du_nom_d_une_autre() {
    let mut noms: Vec<&str> = Famille::TOUTES.iter().map(|f| nom(*f)).collect();
    let avant = noms.len();
    noms.sort_unstable();
    noms.dedup();
    assert_eq!(noms.len(), avant, "deux familles partagent un nom : {noms:?}");
    assert_eq!(avant, NOMBRE, "TOUTES doit porter les NOMBRE familles");
    // ⚠️ Aucun nom n'est le PRÉFIXE d'un autre : deux messages qui partagent une
    // sous-chaîne font un instrument faux (piège maison, payé par F1).
    for a in Famille::TOUTES {
        for b in Famille::TOUTES {
            if a != b {
                assert!(!nom(a).starts_with(nom(b)), "{} préfixe {}", nom(b), nom(a));
            }
        }
    }
}

/// La famille est le BUDGET, pas le verbe : `Creer` et `Ecrire` partagent
/// `DELAI_ECRIRE`, donc la famille `ecrire`.
#[test]
fn la_famille_suit_le_budget_et_creer_est_de_la_famille_ecrire() {
    assert_eq!(Famille::de(&attributs("a")), Famille::Attributs);
    assert_eq!(
        Famille::de(&Attendue::Lister { chemin: "d".into(), enumeration: [0; 16] }),
        Famille::Lister
    );
    assert_eq!(
        Famille::de(&Attendue::Lire { chemin: "f".into(), position: 0, longueur: 1 }),
        Famille::Lire
    );
    assert_eq!(
        Famille::de(&Attendue::Ecrire { chemin: "f".into(), dernier: true }),
        Famille::Ecrire
    );
    assert_eq!(
        Famille::de(&Attendue::Creer { chemin: "f".into() }),
        Famille::Ecrire,
        "Creer est inscrite par ecriture::fil sous DELAI_ECRIRE"
    );
    assert_eq!(
        Famille::de(&Attendue::Muter { chemin: "f".into(), renommage: true, destination: Some("g".into()) }),
        Famille::Mutation
    );
}
