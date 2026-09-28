//! The counter of the twelve causes. **Pure, run on the host.**

use super::*;

/// 🔴 **EACH VARIANT HAS ITS COUNTER, AND ITS NAME.**
///
/// Red: adding a variant to [`Erreur`] without adding it to [`nom`] first
/// makes compilation fail (the `match` is exhaustive); forgetting it in
/// `TOUTES` makes that of `erreurs.rs` fail (`NOMBRE` types the array). This
/// test covers what the two do not: that the name is **distinct**
/// and **non-empty**.
#[test]
fn chaque_variante_a_son_compteur_et_un_nom_distinct() {
    let mut noms: Vec<&str> = Erreur::TOUTES.iter().map(|e| nom(*e)).collect();
    assert_eq!(
        noms.len(),
        NOMBRE,
        "TOUTES doit porter les {NOMBRE} variantes"
    );
    assert!(
        noms.iter().all(|n| !n.is_empty()),
        "un nom vide ne se grep pas"
    );
    noms.sort_unstable();
    let avant = noms.len();
    noms.dedup();
    assert_eq!(noms.len(), avant, "deux causes partagent un nom : {noms:?}");
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
fn aucun_nom_n_est_prefixe_d_un_autre() {
    for a in Erreur::TOUTES {
        for b in Erreur::TOUTES {
            if a == b {
                continue;
            }
            assert!(
                !nom(b).starts_with(nom(a)),
                "« {} » est préfixe de « {} » : le recensement deviendrait ambigu",
                nom(a),
                nom(b)
            );
        }
    }
}

#[test]
fn un_recensement_neuf_est_a_zero_partout() {
    let c = Compteurs::nouveaux();
    assert_eq!(c.total(), 0);
    for e in Erreur::TOUTES {
        assert_eq!(c.compte(e), 0, "{} devrait naître à zéro", nom(e));
    }
}

/// 🔴 **`rendre` INCREMENTS THE RIGHT SLOT, AND IT ALONE.**
///
/// Red: incrementing `rang(e) + 1` makes this test fail by naming BOTH
/// wrong counters — the one that did not rise and the one that wrongly rose.
#[test]
fn rendre_incremente_la_bonne_case_et_elle_seule() {
    for cible in Erreur::TOUTES {
        let c = Compteurs::nouveaux();
        let code = c.rendre(cible);
        assert_eq!(
            code,
            crate::pont::erreurs::hresult(cible),
            "le code rendu doit être celui de la table"
        );
        for e in Erreur::TOUTES {
            let attendu = u64::from(e == cible);
            assert_eq!(
                c.compte(e),
                attendu,
                "après rendre({}), le compteur « {} » vaut {} au lieu de {}",
                nom(cible),
                nom(e),
                c.compte(e),
                attendu
            );
        }
    }
}

#[test]
fn rendre_cumule() {
    let c = Compteurs::nouveaux();
    for _ in 0..3 {
        c.rendre(Erreur::Introuvable);
    }
    c.rendre(Erreur::CanalFerme);
    assert_eq!(c.compte(Erreur::Introuvable), 3);
    assert_eq!(c.compte(Erreur::CanalFerme), 1);
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
fn manquants_rend_exactement_les_causes_a_zero() {
    let c = Compteurs::nouveaux();
    assert_eq!(
        c.manquants().len(),
        NOMBRE,
        "tout manque sur un compteur neuf"
    );

    c.rendre(Erreur::Introuvable);
    c.rendre(Erreur::DisquePlein);
    let manquants = c.manquants();
    assert_eq!(manquants.len(), NOMBRE - 2);
    assert!(!manquants.contains(&Erreur::Introuvable));
    assert!(!manquants.contains(&Erreur::DisquePlein));
    assert!(manquants.contains(&Erreur::Abandonnee));

    for e in Erreur::TOUTES {
        c.rendre(e);
    }
    assert!(c.manquants().is_empty(), "le critère (4) est alors TENU");
}

/// 🔴 **THE CENSUS ORDER IS THAT OF `TOUTES`**, and the whole string
/// is compared: swapping two names makes it fail.
#[test]
#[allow(non_snake_case)]
fn l_ordre_du_recensement_est_celui_de_TOUTES() {
    let c = Compteurs::nouveaux();
    c.rendre(Erreur::Introuvable);
    c.rendre(Erreur::Introuvable);
    c.rendre(Erreur::ProtegeEnEcriture);
    assert_eq!(
        c.recensement(),
        "total=3 introuvable=2 chemin-introuvable=0 acces-refuse=0 canal-ferme=0 \
delai-depasse=0 abandonnee=0 disque-plein=0 non-supporte=0 repertoire-non-vide=0 \
deja-present=0 protege-en-ecriture=1 inattendue=0"
    );
}

/// The census carries **exactly** one field per variant, plus the total.
///
/// ⚠️ Without this test, adding a variant to `TOUTES` without touching `recensement`
/// would be caught — but removing a loop and writing the twelve by hand
/// would pass, and the thirteenth field would be silently missing.
#[test]
fn le_recensement_porte_un_champ_par_variante() {
    let ligne = Compteurs::nouveaux().recensement();
    assert_eq!(ligne.split(' ').count(), NOMBRE + 1);
    for e in Erreur::TOUTES {
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
fn le_compteur_est_juste_sous_concurrence() {
    let c = std::sync::Arc::new(Compteurs::nouveaux());
    let fils: Vec<_> = (0..8)
        .map(|_| {
            let c = std::sync::Arc::clone(&c);
            std::thread::spawn(move || {
                for _ in 0..250 {
                    c.rendre(Erreur::DelaiDepasse);
                }
            })
        })
        .collect();
    for f in fils {
        f.join().expect("aucun fil ne panique");
    }
    assert_eq!(c.compte(Erreur::DelaiDepasse), 2_000);
}
