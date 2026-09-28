//! The counter of the twelve causes. **Pure, run on the host.**

use super::*;

/// 🔴 **EACH VARIANT HAS ITS COUNTER, AND ITS NAME.**
///
/// Red: adding a variant to [`Error`] without adding it to [`nom`] first
/// makes compilation fail (the `match` is exhaustive); forgetting it in
/// `ALL` makes that of `errors.rs` fail (`COUNT` types the array). This
/// test covers what the two do not: that the name is **distinct**
/// and **non-empty**.
#[test]
fn each_variant_has_its_counter_and_a_distinct_name() {
    let mut noms: Vec<&str> = Error::ALL.iter().map(|e| nom(*e)).collect();
    assert_eq!(noms.len(), COUNT, "ALL must carry the {COUNT} variants");
    assert!(
        noms.iter().all(|n| !n.is_empty()),
        "an empty name cannot be grepped"
    );
    noms.sort_unstable();
    let before = noms.len();
    noms.dedup();
    assert_eq!(noms.len(), before, "two causes share a name: {noms:?}");
}

/// 🔴 **NO NAME IS A PREFIX OF ANOTHER.**
///
/// The census is written `name=value`, separated by spaces, and it is read
/// with `grep`. If `introuvable` were a prefix of `introuvable-bis`, a
/// `grep -o 'introuvable=[0-9]*'` would catch the wrong field — it is
/// exactly the "Drive … **mount**ed" versus "could not be
/// **mount**ed" trap that F1 paid nine minutes of measurement for, transposed to counter
/// names.
///
/// ⚠️ **`chemin-introuvable` CONTAINS `introuvable`, and this test passes
/// anyway**: the relation it forbids is the PREFIX, because the separator
/// that follows a name is always `=`. `grep 'introuvable='` would indeed catch
/// `chemin-introuvable=` — hence the acceptance run reads ` introuvable=`,
/// space included. Saying it here avoids rediscovering it at `grep` time.
#[test]
fn no_name_is_a_prefix_of_another() {
    for a in Error::ALL {
        for b in Error::ALL {
            if a == b {
                continue;
            }
            assert!(
                !nom(b).starts_with(nom(a)),
                "« {} » is a prefix of « {} »: the census would become ambiguous",
                nom(a),
                nom(b)
            );
        }
    }
}

#[test]
fn a_fresh_census_is_zero_everywhere() {
    let c = Compteurs::nouveaux();
    assert_eq!(c.total(), 0);
    for e in Error::ALL {
        assert_eq!(c.compte(e), 0, "{} should start at zero", nom(e));
    }
}

/// 🔴 **`rendre` INCREMENTS THE RIGHT SLOT, AND IT ALONE.**
///
/// Red: incrementing `rang(e) + 1` makes this test fail by naming BOTH
/// wrong counters — the one that did not rise and the one that wrongly rose.
#[test]
fn giving_back_increments_the_right_slot_and_only_it() {
    for cible in Error::ALL {
        let c = Compteurs::nouveaux();
        let code = c.rendre(cible);
        assert_eq!(
            code,
            crate::pont::errors::hresult(cible),
            "the returned code must be the table's"
        );
        for e in Error::ALL {
            let attendu = u64::from(e == cible);
            assert_eq!(
                c.compte(e),
                attendu,
                "after rendre({}), counter « {} » is {} instead of {}",
                nom(cible),
                nom(e),
                c.compte(e),
                attendu
            );
        }
    }
}

#[test]
fn giving_back_accumulates() {
    let c = Compteurs::nouveaux();
    for _ in 0..3 {
        c.rendre(Error::Introuvable);
    }
    c.rendre(Error::CanalFerme);
    assert_eq!(c.compte(Error::Introuvable), 3);
    assert_eq!(c.compte(Error::CanalFerme), 1);
    assert_eq!(c.total(), 4);
}

/// 🔴 **THE MOST IMPORTANT RED OF THIS MODULE.**
///
/// A `manquants()` that unconditionally returned `Vec::new()` would make
/// F3's criterion (4) be declared **HELD on a run where nothing was
/// exercised** — that is a check that cannot fail, applied to the
/// criterion that exists precisely to prevent the §5 table from being
/// decorative.
#[test]
fn missing_returns_exactly_the_causes_at_zero() {
    let c = Compteurs::nouveaux();
    assert_eq!(
        c.manquants().len(),
        COUNT,
        "everything is missing on a fresh counter"
    );

    c.rendre(Error::Introuvable);
    c.rendre(Error::DisquePlein);
    let manquants = c.manquants();
    assert_eq!(manquants.len(), COUNT - 2);
    assert!(!manquants.contains(&Error::Introuvable));
    assert!(!manquants.contains(&Error::DisquePlein));
    assert!(manquants.contains(&Error::Abandonnee));

    for e in Error::ALL {
        c.rendre(e);
    }
    assert!(c.manquants().is_empty(), "criterion (4) is then MET");
}

/// 🔴 **THE CENSUS ORDER IS THAT OF `ALL`**, and the whole string
/// is compared: swapping two names makes it fail.
#[test]
#[allow(non_snake_case)]
fn the_census_order_is_that_of_all() {
    let c = Compteurs::nouveaux();
    c.rendre(Error::Introuvable);
    c.rendre(Error::Introuvable);
    c.rendre(Error::ProtegeEnEcriture);
    assert_eq!(
        c.recensement(),
        "total=3 introuvable=2 chemin-introuvable=0 acces-refuse=0 canal-ferme=0 \
delai-depasse=0 abandonnee=0 disque-plein=0 non-supporte=0 repertoire-non-vide=0 \
deja-present=0 protege-en-ecriture=1 inattendue=0"
    );
}

/// The census carries **exactly** one field per variant, plus the total.
///
/// ⚠️ Without this test, adding a variant to `ALL` without touching `recensement`
/// would be caught — but removing a loop and writing the twelve by hand
/// would pass, and the thirteenth field would be silently missing.
#[test]
fn the_census_carries_one_field_per_variant() {
    let ligne = Compteurs::nouveaux().recensement();
    assert_eq!(ligne.split(' ').count(), COUNT + 1);
    for e in Error::ALL {
        assert!(
            ligne.contains(&format!(" {}=", nom(e))),
            "« {} » absent",
            nom(e)
        );
    }
}

/// The counter is shared between the callback threads: it must count right
/// under concurrency, otherwise criterion (4) would lie in the most
/// flattering direction.
#[test]
fn the_counter_is_correct_under_concurrency() {
    let c = std::sync::Arc::new(Compteurs::nouveaux());
    let fils: Vec<_> = (0..8)
        .map(|_| {
            let c = std::sync::Arc::clone(&c);
            std::thread::spawn(move || {
                for _ in 0..250 {
                    c.rendre(Error::DelaiDepasse);
                }
            })
        })
        .collect();
    for f in fils {
        f.join().expect("no thread panics");
    }
    assert_eq!(c.compte(Error::DelaiDepasse), 2_000);
}
