//! Le compteur des douze causes. **Purs, exécutés sur l'hôte.**

use super::*;

/// 🔴 **CHAQUE VARIANTE A SON COMPTEUR, ET SON NOM.**
///
/// Rouge : ajouter une variante à [`Erreur`] sans l'ajouter à [`nom`] fait
/// d'abord échouer la compilation (le `match` est exhaustif) ; l'oublier dans
/// `TOUTES` fait échouer celle de `erreurs.rs` (`NOMBRE` typé le tableau). Ce
/// test-ci couvre ce que les deux ne couvrent pas : que le nom soit **distinct**
/// et **non vide**.
#[test]
fn chaque_variante_a_son_compteur_et_un_nom_distinct() {
    let mut noms: Vec<&str> = Erreur::TOUTES.iter().map(|e| nom(*e)).collect();
    assert_eq!(noms.len(), NOMBRE, "TOUTES doit porter les {NOMBRE} variantes");
    assert!(noms.iter().all(|n| !n.is_empty()), "un nom vide ne se grep pas");
    noms.sort_unstable();
    let avant = noms.len();
    noms.dedup();
    assert_eq!(noms.len(), avant, "deux causes partagent un nom : {noms:?}");
}

/// 🔴 **AUCUN NOM N'EST PRÉFIXE D'UN AUTRE.**
///
/// Le recensement s'écrit `nom=valeur`, séparés par des espaces, et il se lit
/// au `grep`. Si `introuvable` était préfixe de `introuvable-bis`, un
/// `grep -o 'introuvable=[0-9]*'` attraperait le mauvais champ — c'est
/// exactement le piège « Lecteur … **mont**é » contre « n'a pas pu être
/// **mont**é » que F1 a payé neuf minutes de mesure, transposé à des noms de
/// compteur.
///
/// ⚠️ **`chemin-introuvable` CONTIENT `introuvable`, et ce test passe quand
/// même** : la relation qu'il interdit est le PRÉFIXE, parce que le séparateur
/// qui suit un nom est toujours `=`. `grep 'introuvable='` attraperait bien
/// `chemin-introuvable=` — d'où le fait que la recette lise ` introuvable=`,
/// espace compris. Le dire ici évite qu'on le redécouvre au `grep`.
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

/// 🔴 **`rendre` INCRÉMENTE LA BONNE CASE, ET ELLE SEULE.**
///
/// Rouge : incrémenter `rang(e) + 1` fait échouer ce test en nommant les DEUX
/// compteurs faux — celui qui n'a pas monté et celui qui a monté à tort.
#[test]
fn rendre_incremente_la_bonne_case_et_elle_seule() {
    for cible in Erreur::TOUTES {
        let c = Compteurs::nouveaux();
        let code = c.rendre(cible);
        assert_eq!(code, crate::pont::erreurs::hresult(cible), "le code rendu doit être celui de la table");
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

/// 🔴 **LE ROUGE LE PLUS IMPORTANT DE CE MODULE.**
///
/// Un `manquants()` qui rendrait `Vec::new()` inconditionnellement ferait
/// déclarer le critère (4) de F3 **TENU sur une exécution où rien n'a été
/// exercé** — c'est-à-dire un contrôle qui ne peut pas échouer, appliqué au
/// critère qui existe précisément pour empêcher la table du §5 d'être
/// décorative.
#[test]
fn manquants_rend_exactement_les_causes_a_zero() {
    let c = Compteurs::nouveaux();
    assert_eq!(c.manquants().len(), NOMBRE, "tout manque sur un compteur neuf");

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

/// 🔴 **L'ORDRE DU RECENSEMENT EST CELUI DE `TOUTES`**, et la chaîne entière
/// est comparée : permuter deux noms le fait échouer.
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

/// Le recensement porte **exactement** un champ par variante, plus le total.
///
/// ⚠️ Sans ce test, ajouter une variante à `TOUTES` sans toucher `recensement`
/// serait rattrapé — mais retirer une boucle et écrire les douze à la main
/// passerait, et le treizième champ manquerait en silence.
#[test]
fn le_recensement_porte_un_champ_par_variante() {
    let ligne = Compteurs::nouveaux().recensement();
    assert_eq!(ligne.split(' ').count(), NOMBRE + 1);
    for e in Erreur::TOUTES {
        assert!(ligne.contains(&format!(" {}=", nom(e))), "« {} » absent", nom(e));
    }
}

/// Le compteur est partagé entre les fils de rappel : il doit compter juste
/// sous concurrence, sinon le critère (4) mentirait dans le sens le plus
/// flatteur.
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
