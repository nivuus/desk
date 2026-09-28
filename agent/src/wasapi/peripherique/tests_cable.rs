//! The tests of the cable designation (task 5 of plan E2).
//!
//! ⚠️ **Extracted here rather than added to the `mod tests` of `peripherique.rs`, under
//! the 500-line rule and BEFORE the addition**, not after: the
//! parent file was at 422 and these tests would have taken it beyond the ceiling.
//! The pattern is `superviseur/table.rs`, which declares its
//! `#[path = "table/tests.rs"] mod tests;` the same way.

use super::{choisir, demande_cable, Choix, Critere, Peripherique, DESIGNATION_CABLE};

fn p(nom: &str, id: &str) -> Peripherique {
    Peripherique {
        nom: nom.to_string(),
        identifiant: id.to_string(),
    }
}

/// The inventory **noted on the VM on August 20th, 2026** by `micro-format-e1.ps1`
/// — the three ACTIVE render endpoints, name for name.
///
/// ⚠️ **Embedding the exact names is deliberate, and it is not a
/// fragility**: the day the name changes, a test will fail **before** the
/// acceptance run, and not during it.
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

/// 🔴 **The designation the spec prescribed matches NOTHING.**
/// Its §3 and §6 say "the targeted endpoint is *CABLE Input*"; the
/// `PKEY_Device_FriendlyName` of the cable's render endpoint is
/// `Haut-parleurs (VB-Audio Virtual Cable)`.
#[test]
#[allow(non_snake_case)]
fn la_designation_integree_elit_le_cable_de_la_VM() {
    let inv = inventaire_de_la_vm();
    match choisir(&inv, Some(DESIGNATION_CABLE)) {
        Choix::Elu {
            peripherique,
            critere,
        } => {
            assert_eq!(peripherique.nom, "Haut-parleurs (VB-Audio Virtual Cable)");
            assert_eq!(critere, Critere::NomPartiel);
        }
        autre => panic!("le câble de la VM doit être élu, obtenu {autre:?}"),
    }

    // And the proof that the check can fail: the spec's designation.
    assert!(
        matches!(
            choisir(&inv, Some("CABLE Input")),
            Choix::Introuvable { .. }
        ),
        "« CABLE Input » n'est le nom d'aucun rendu de cette VM"
    );
}

/// 🔴 On a machine carrying two VB-Audio cables, the rule **refuses** and
/// names the candidates. Deciding on the first would be the enumeration rank through
/// the back door — paid for in D1.
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

/// 🔴 **`Choix::Defaut` must be UNREACHABLE through this path.** Falling back to
/// `GetDefaultAudioEndpoint` would send the user's voice out through the machine's
/// speakers the day the default is not the cable.
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
