//! Les tests de la désignation du câble (tâche 5 du plan E2).
//!
//! ⚠️ **Extraits ici plutôt qu'ajoutés au `mod tests` de `peripherique.rs`, au
//! titre de la règle des 500 lignes et AVANT l'addition**, pas après : le
//! fichier parent valait 422 et ces tests l'auraient porté au-delà du plafond.
//! Le patron est `superviseur/table.rs`, qui déclare de la même façon ses
//! `#[path = "table/tests.rs"] mod tests;`.

use super::{choisir, demande_cable, Choix, Critere, Peripherique, DESIGNATION_CABLE};

fn p(nom: &str, id: &str) -> Peripherique {
    Peripherique { nom: nom.to_string(), identifiant: id.to_string() }
}

/// L'inventaire **relevé sur la VM le 20 août 2026** par `micro-format-e1.ps1`
/// — les trois points de terminaison de rendu ACTIFS, nom pour nom.
///
/// ⚠️ **Embarquer les noms exacts est délibéré, et ce n'est pas une
/// fragilité** : le jour où le nom changera, un test tombera **avant** la
/// recette, et non pendant.
fn inventaire_de_la_vm() -> Vec<Peripherique> {
    vec![
        p(
            "Haut-parleurs (Steam Streaming Speakers)",
            "{0.0.0.00000000}.{8695a111-abf2-4199-88c0-fe4a9176f3e9}",
        ),
        p(
            "HDP-V104 (NVIDIA High Definition Audio)",
            "{0.0.0.00000000}.{8bf867bf-5ebe-45ea-b488-3d70ce3e430c}",
        ),
        p(
            "Haut-parleurs (VB-Audio Virtual Cable)",
            "{0.0.0.00000000}.{deec1914-6490-47ee-9475-091b9a2ea537}",
        ),
    ]
}

/// 🔴 **La désignation que la spec prescrivait ne correspond à RIEN.**
/// Son §3 et son §6 disent « l'endpoint visé est *CABLE Input* » ; le
/// `PKEY_Device_FriendlyName` du rendu du câble vaut
/// `Haut-parleurs (VB-Audio Virtual Cable)`.
#[test]
#[allow(non_snake_case)]
fn la_designation_integree_elit_le_cable_de_la_VM() {
    let inv = inventaire_de_la_vm();
    match choisir(&inv, Some(DESIGNATION_CABLE)) {
        Choix::Elu { peripherique, critere } => {
            assert_eq!(peripherique.nom, "Haut-parleurs (VB-Audio Virtual Cable)");
            assert_eq!(critere, Critere::NomPartiel);
        }
        autre => panic!("le câble de la VM doit être élu, obtenu {autre:?}"),
    }

    // Et la preuve que le contrôle peut échouer : la désignation de la spec.
    assert!(
        matches!(choisir(&inv, Some("CABLE Input")), Choix::Introuvable { .. }),
        "« CABLE Input » n'est le nom d'aucun rendu de cette VM"
    );
}

/// 🔴 Sur une machine portant deux câbles VB-Audio, la règle **refuse** et
/// nomme les candidats. Trancher au premier serait le rang d'énumération par
/// la porte de derrière — payé en D1.
#[test]
#[allow(non_snake_case)]
fn deux_cables_VB_rendent_la_designation_AMBIGUE() {
    let mut inv = inventaire_de_la_vm();
    inv.push(p(
        "Haut-parleurs (VB-Audio Virtual Cable B)",
        "{0.0.0.00000000}.{00000000-0000-0000-0000-00000000000b}",
    ));
    match choisir(&inv, Some(DESIGNATION_CABLE)) {
        Choix::Ambigu { demande, candidats } => {
            assert_eq!(demande, DESIGNATION_CABLE);
            assert_eq!(candidats.len(), 2, "les deux câbles doivent être nommés");
            assert!(candidats.iter().any(|c| c.contains("Virtual Cable B")));
        }
        autre => panic!("deux câbles doivent rendre Ambigu, obtenu {autre:?}"),
    }
}

#[test]
fn une_demande_explicite_prime_sur_la_designation_integree() {
    let inv = inventaire_de_la_vm();
    assert_eq!(demande_cable(Some("Steam")), "Steam");
    match choisir(&inv, Some(demande_cable(Some("Steam")))) {
        Choix::Elu { peripherique, .. } => {
            assert_eq!(peripherique.nom, "Haut-parleurs (Steam Streaming Speakers)");
        }
        autre => panic!("la demande explicite doit primer, obtenu {autre:?}"),
    }
}

/// 🔴 **`Choix::Defaut` doit être INATTEIGNABLE par ce chemin.** Retomber sur
/// `GetDefaultAudioEndpoint` ferait sortir la voix de l'utilisateur par les
/// haut-parleurs de la machine le jour où le défaut n'est pas le câble.
#[test]
#[allow(non_snake_case)]
fn la_demande_du_cable_n_est_JAMAIS_None() {
    let inv = inventaire_de_la_vm();
    for variable in [None, Some(""), Some("   ")] {
        let demande = demande_cable(variable);
        assert_eq!(demande, DESIGNATION_CABLE, "variable {variable:?}");
        assert!(
            !matches!(choisir(&inv, Some(demande)), Choix::Defaut),
            "Choix::Defaut est inatteignable par le chemin du câble (variable {variable:?})"
        );
    }
}
