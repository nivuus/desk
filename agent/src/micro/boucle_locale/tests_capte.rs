//! Les tests d'[`super::identifiant_capte`] : **la règle qui doit rendre le
//! même verdict que `wasapi::rendu::resoudre`**.
//!
//! ⚠️ Extraits ici plutôt qu'ajoutés au `mod tests` du parent, au titre de la
//! règle des 500 lignes et sur le patron de `peripherique/tests_cable.rs`.
//!
//! ⚠️ **Les quatre branches sont éprouvées, et c'est le fond de ce fichier.**
//! Une seule d'entre elles qui divergerait de `resoudre` ferait comparer à la
//! garde de boucle un périphérique qui n'est pas celui qu'on capte — donc
//! laisser passer une boucle réelle, ou couper un micro sain.

use super::identifiant_capte;
use crate::wasapi_peripherique::Peripherique;

fn p(nom: &str, id: &str) -> Peripherique {
    Peripherique { nom: nom.to_string(), identifiant: id.to_string() }
}

/// L'inventaire relevé sur la VM le 20 août 2026, nom pour nom.
fn vm() -> Vec<Peripherique> {
    vec![
        p("Haut-parleurs (Steam Streaming Speakers)", "{0.0.0.00000000}.{8695a111}"),
        p("HDP-V104 (NVIDIA High Definition Audio)", "{0.0.0.00000000}.{8bf867bf}"),
        p("Haut-parleurs (VB-Audio Virtual Cable)", "{0.0.0.00000000}.{deec1914}"),
    ]
}

/// Aucune demande : c'est le défaut de Windows qui sera capté, et seul un
/// appel COM peut le nommer.
#[test]
fn sans_demande_le_capte_est_le_defaut_de_windows() {
    assert_eq!(identifiant_capte(&vm(), None), None);
    assert_eq!(identifiant_capte(&vm(), Some("   ")), None);
}

/// Une demande qui élit : c'est l'identifiant de l'élu.
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

/// 🔴 Une demande INTROUVABLE : `resoudre` se replie sur le défaut de Windows
/// en `warn!`. Rendre ici l'identifiant demandé — ou rien du tout au sens
/// « aucune capture » — ferait diverger la garde de la réalité.
#[test]
fn une_demande_introuvable_retombe_sur_le_defaut_comme_resoudre() {
    assert_eq!(identifiant_capte(&vm(), Some("Realtek")), None);
}

/// 🔴 Même chose pour l'AMBIGUÏTÉ : « Haut-parleurs » désigne deux des trois
/// rendus de cette VM, `resoudre` refuse de trancher et se replie.
#[test]
fn une_demande_ambigue_retombe_sur_le_defaut_comme_resoudre() {
    assert_eq!(identifiant_capte(&vm(), Some("Haut-parleurs")), None);
}
