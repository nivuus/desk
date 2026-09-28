//! The tests of [`super::identifiant_capte`]: **the rule that must return the
//! same verdict as `wasapi::rendu::resoudre`**.
//!
//! ⚠️ Extracted here rather than added to the parent's `mod tests`, under the
//! 500-line rule and on the pattern of `peripherique/tests_cable.rs`.
//!
//! ⚠️ **All four branches are tested, and it is the substance of this file.**
//! A single one of them diverging from `resoudre` would make the
//! loop guard compare a device that is not the one captured — hence
//! letting a real loop through, or cutting a healthy microphone.

use super::identifiant_capte;
use crate::wasapi_peripherique::Peripherique;

fn p(nom: &str, id: &str) -> Peripherique {
    Peripherique {
        nom: nom.to_string(),
        identifiant: id.to_string(),
    }
}

/// The inventory surveyed on the VM on 20 August 2026, name for name.
fn vm() -> Vec<Peripherique> {
    vec![
        p(
            "Haut-parleurs (Steam Streaming Speakers)",
            "{0.0.0.00000000}.{8695a111}",
        ),
        p(
            "HDP-V104 (NVIDIA High Definition Audio)",
            "{0.0.0.00000000}.{8bf867bf}",
        ),
        p(
            "Haut-parleurs (VB-Audio Virtual Cable)",
            "{0.0.0.00000000}.{deec1914}",
        ),
    ]
}

/// No request: it is Windows' default that will be captured, and only a
/// COM call can name it.
#[test]
fn sans_demande_le_capte_est_le_defaut_de_windows() {
    assert_eq!(identifiant_capte(&vm(), None), None);
    assert_eq!(identifiant_capte(&vm(), Some("   ")), None);
}

/// A request that elects: it is the elected one's identifier.
#[test]
fn une_demande_qui_elit_rend_son_identifiant() {
    assert_eq!(
        identifiant_capte(&vm(), Some("Steam")),
        Some("{0.0.0.00000000}.{8695a111}".to_string())
    );
    assert_eq!(
        identifiant_capte(&vm(), Some("{0.0.0.00000000}.{deec1914}")),
        Some("{0.0.0.00000000}.{deec1914}".to_string())
    );
}

/// 🔴 A request NOT FOUND: `resoudre` falls back on Windows' default
/// with a `warn!`. Returning here the requested identifier — or nothing at all in the sense of
/// "no capture" — would make the guard diverge from reality.
#[test]
fn a_request_not_found_falls_back_to_the_default_like_resolve() {
    assert_eq!(identifiant_capte(&vm(), Some("Realtek")), None);
}

/// 🔴 Same thing for AMBIGUITY: "Haut-parleurs" designates two of the three
/// render devices of this VM, `resoudre` refuses to decide and falls back.
#[test]
fn an_ambiguous_request_falls_back_to_the_default_like_resolve() {
    assert_eq!(identifiant_capte(&vm(), Some("Haut-parleurs")), None);
}
