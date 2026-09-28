//! Host tests of [`super`]. **None sleeps**: the duration is a parameter.

use std::time::Duration;

use super::{nom, seau_de, Famille, Histogramme, COUNT, SEAUX, SEAUX_MS};
use crate::pont::table::Attendue;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn attributs(chemin: &str) -> Attendue {
    Attendue::Attributs {
        chemin: chemin.to_string(),
    }
}

/// ⚠️ **THIS TEST ONLY EXERCISES A `Default`, AND IT IS KEPT ONLY BECAUSE IT IS
/// THE STARTING WITNESS OF THE OTHERS.** It is D9's legacy no. 11
/// (`une_telemetrie_neuve_est_a_zero`, "unable to return the other value"):
/// without it, `les_familles_ne_se_melangent_pas` could not distinguish
/// "the neighbouring family did not move" from "it never carried anything".
#[test]
fn un_histogramme_neuf_rend_des_zeros() {
    let h = Histogramme::new();
    for f in Famille::ALL {
        assert_eq!(h.compte(f), 0, "{}", nom(f));
        assert_eq!(h.moyenne_us(f), 0, "{}", nom(f));
        assert_eq!(h.max_us(f), 0, "{}", nom(f));
        for i in 0..SEAUX {
            assert_eq!(h.seau(f, i), 0, "{} seau {}", nom(f), i);
        }
    }
}

/// 🔴 **The bound is INCLUSIVE at the top.** A `<` instead of a `<=` would shift
/// the whole distribution by one bucket, silently: each value EXACTLY equal
/// to a bound is exercised, plus one value strictly below and one
/// strictly above.
#[test]
fn a_crossing_falls_in_the_bucket_that_contains_it() {
    // Exactly on each bound: the bucket of the SAME rank.
    for (i, borne) in SEAUX_MS.iter().enumerate() {
        assert_eq!(seau_de(ms(*borne)), i, "borne {} ms", borne);
    }
    // Just below the first bound, and zero.
    assert_eq!(seau_de(Duration::from_micros(999)), 0);
    assert_eq!(seau_de(Duration::ZERO), 0);
    // Just above a bound: the NEXT bucket.
    assert_eq!(seau_de(Duration::from_micros(1_001)), 1, "1,001 ms");
    assert_eq!(
        seau_de(Duration::from_micros(20_001)),
        5,
        "20,001 ms -> seau 50"
    );
    // Beyond the last bound: `inf`, the thirteenth.
    assert_eq!(seau_de(ms(5_001)), SEAUX_MS.len());
    assert_eq!(seau_de(ms(30_000)), SEAUX_MS.len());

    // And the bucket is indeed the one the histogram increments.
    let h = Histogramme::new();
    h.observer(Famille::Lire, ms(5));
    assert_eq!(h.seau(Famille::Lire, 2), 1, "5 ms est dans le seau '5'");
    assert_eq!(
        h.seau(Famille::Lire, 3),
        0,
        "et surtout PAS dans le seau '10'"
    );
}

/// The twin of `pont::compteurs`' guard: a wrong `rang()` would count
/// one family on the back of another.
#[test]
fn les_familles_ne_se_melangent_pas() {
    let h = Histogramme::new();
    h.observer(Famille::Lire, ms(7));
    h.observer(Famille::Lire, ms(7));
    h.observer(Famille::Lister, ms(300));

    assert_eq!(h.compte(Famille::Lire), 2);
    assert_eq!(h.compte(Famille::Lister), 1);
    for f in [Famille::Attributs, Famille::Write, Famille::Mutation] {
        assert_eq!(h.compte(f), 0, "{} n'a rien reçu", nom(f));
        assert_eq!(h.max_us(f), 0, "{}", nom(f));
    }
    // The buckets do not mix either.
    assert_eq!(h.seau(Famille::Lire, 3), 2, "7 ms -> seau '10'");
    assert_eq!(h.seau(Famille::Lister, 3), 0);
    assert_eq!(h.seau(Famille::Lister, 8), 1, "300 ms -> seau '500'");
}

/// A swapped sum and count would make the mean equal to the count.
#[test]
fn le_max_est_le_max_et_la_moyenne_est_la_moyenne() {
    let h = Histogramme::new();
    h.observer(Famille::Attributs, ms(2));
    h.observer(Famille::Attributs, ms(8));
    h.observer(Famille::Attributs, ms(2));

    assert_eq!(h.compte(Famille::Attributs), 3);
    assert_eq!(h.moyenne_us(Famille::Attributs), 4_000, "(2+8+2)/3 = 4 ms");
    assert_eq!(h.max_us(Famille::Attributs), 8_000);

    // The maximum does NOT go back when a shorter traversal follows.
    h.observer(Famille::Attributs, ms(1));
    assert_eq!(h.max_us(Famille::Attributs), 8_000, "fetch_max, pas store");
}

/// 🔴 **An order drift would READ ONE COUNTER FOR ANOTHER.** The line
/// is pinned in full, values included.
#[test]
fn l_ordre_du_recensement_est_epingle() {
    let h = Histogramme::new();
    h.observer(Famille::Lire, ms(3));

    let ligne = h.recensement();
    let (tetes, seaux) = ligne.split_once(" | ").expect("la ligne a deux moitiés");

    assert_eq!(
        tetes,
        "traversees attributs=n:0 moy_us:0 max_us:0 lister=n:0 moy_us:0 max_us:0 \
         lire=n:1 moy_us:3000 max_us:3000 ecrire=n:0 moy_us:0 max_us:0 \
         mutation=n:0 moy_us:0 max_us:0"
    );
    // The FIVE families carry their buckets, and `lire` carries its own at the right rank.
    assert!(seaux.starts_with("seaux_ms attributs="), "{seaux}");
    for f in Famille::ALL {
        assert!(
            seaux.contains(&format!(" {}=1:", nom(f))),
            "{} absent : {seaux}",
            nom(f)
        );
    }
    assert!(
        seaux.contains("lire=1:0,2:0,5:1,10:0,"),
        "3 ms est dans le seau '5' : {seaux}"
    );
    assert!(seaux.ends_with("inf:0"), "{seaux}");
    // A single occurrence of each family name in each half.
    for f in Famille::ALL {
        assert_eq!(
            tetes.matches(&format!(" {}=", nom(f))).count(),
            1,
            "{}",
            nom(f)
        );
        assert_eq!(
            seaux.matches(&format!(" {}=", nom(f))).count(),
            1,
            "{}",
            nom(f)
        );
    }
}

/// The third stage of the structural guard: without its own name, a new
/// family would not appear in the census, or worse, would be confused there with
/// another.
#[test]
fn a_new_family_cannot_inherit_another_name() {
    let mut noms: Vec<&str> = Famille::ALL.iter().map(|f| nom(*f)).collect();
    let before = noms.len();
    noms.sort_unstable();
    noms.dedup();
    assert_eq!(
        noms.len(),
        before,
        "deux familles partagent un nom : {noms:?}"
    );
    assert_eq!(before, COUNT, "TOUTES doit porter les NOMBRE familles");
    // ⚠️ No name is the PREFIX of another: two messages sharing a
    // substring make a false instrument (house trap, paid for by F1).
    for a in Famille::ALL {
        for b in Famille::ALL {
            if a != b {
                assert!(!nom(a).starts_with(nom(b)), "{} préfixe {}", nom(b), nom(a));
            }
        }
    }
}

/// The family is the BUDGET, not the verb: `Create` and `Write` share
/// `WRITE_TIMEOUT`, hence the `write` family.
#[test]
fn the_family_follows_the_budget_and_create_belongs_to_the_write_family() {
    assert_eq!(Famille::de(&attributs("a")), Famille::Attributs);
    assert_eq!(
        Famille::de(&Attendue::Lister {
            chemin: "d".into(),
            enumeration: [0; 16]
        }),
        Famille::Lister
    );
    assert_eq!(
        Famille::de(&Attendue::Lire {
            chemin: "f".into(),
            position: 0,
            length: 1
        }),
        Famille::Lire
    );
    assert_eq!(
        Famille::de(&Attendue::Write {
            chemin: "f".into(),
            last: true
        }),
        Famille::Write
    );
    assert_eq!(
        Famille::de(&Attendue::Create { chemin: "f".into() }),
        Famille::Write,
        "Creer est inscrite par ecriture::fil sous DELAI_ECRIRE"
    );
    assert_eq!(
        Famille::de(&Attendue::Muter {
            chemin: "f".into(),
            renommage: true,
            destination: Some("g".into())
        }),
        Famille::Mutation
    );
}
