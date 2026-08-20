//! Découverte et lancement des applications de la VM.
//!
//! ⚠️ CE MODULE EST DÉCLARÉ SANS `cfg` dans `main.rs`, et ce sont ses enfants
//! Windows qui portent le leur. C'est ce qui fait exister `apps::raccourci` et
//! `apps::reconciliation` sur l'hôte Linux, où leurs tests courent — la
//! « Convention de module enfant » de `CLAUDE.md` n'est donc pas mobilisée :
//! aucun module ne franchit ici de frontière `#[cfg(windows)]`.

use std::time::Duration;

pub mod raccourci;
pub mod reconciliation;
pub mod sha256;

#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lancement;
#[cfg(windows)]
pub mod lecture;

/// Entre deux lectures complètes des quatre racines.
///
/// **NON CALIBRÉE.** Ce qui l'autorise est mesuré : sur cette VM, énumérer les
/// quatre racines coûte 31 ms et résoudre les 218 raccourcis 82 ms, soit
/// 113 ms — 0,38 % d'une période. Ce qui n'est PAS mesuré est le délai que
/// l'utilisateur ressent entre l'installation d'une application et son
/// apparition : il vaut jusqu'à trente secondes, et c'est le vrai arbitrage.
///
/// ⚠️ LA RÉCONCILIATION PÉRIODIQUE EST LA SOURCE DE VÉRITÉ, et le restera : la
/// notification par `ReadDirectoryChangesW` d'un sous-bloc ultérieur ne sera
/// qu'un ACCÉLÉRATEUR. Une notification manquée ne doit jamais pouvoir figer
/// un catalogue.
pub const PERIODE_RECONCILIATION: Duration = Duration::from_secs(30);

/// 🔴 `APPS=0` DÉSARME ; UNE SIMPLE PRÉSENCE N'ACTIVE PAS.
///
/// Tester `is_ok()` — ou `is_some()` ici — ACTIVERAIT le mécanisme en écrivant
/// `APPS=0` POUR LE COUPER. C'est la convention de `PLEIN_ECRAN`, `AUDIO` et
/// `PART_SONDAGE`, et `CLAUDE.md` l'écrit pour cette raison exacte.
///
/// La décision est isolée ici PARCE QUE `brancher` est `#[cfg(windows)]` et
/// qu'aucun test d'hôte ne peut donc l'atteindre : ce prédicat, lui, est pur,
/// et c'est la seule part de la garde qu'on puisse voir rougir sur l'hôte.
pub fn desarme(valeur: Option<&str>) -> bool {
    valeur == Some("0")
}

/// Branche la découverte sur le canal, ou rend `None` en disant pourquoi.
///
/// ⚠️ RIEN N'EST BRANCHÉ EN MODE CAPTEUR, et ce n'est pas un oubli : le mode
/// `CAPTEUR` retourne AVANT l'enrôlement dans `main.rs`, donc un capteur n'a
/// ni canal, ni jeton, ni préfixe, et ne peut porter aucun catalogue.
#[cfg(windows)]
pub fn brancher(canal: &mut crate::plateforme::Canal) -> Option<std::thread::JoinHandle<()>> {
    if desarme(std::env::var("APPS").ok().as_deref()) {
        tracing::warn!("decouverte d'applications DESARMEE (APPS=0)");
        return None;
    }
    let ordres = canal.ordres()?;
    let identite = canal.veille_identite();
    let emetteur = canal.emetteur();
    // 🔴 UN VRAI FIL, PAS UN `spawn_blocking`. L'appartement COM appartient à
    // SON fil : `IShellLinkW` et `ShellExecuteExW` doivent tous deux courir
    // sur celui qui a appelé `CoInitializeEx`, et un fil de pool tokio peut
    // servir d'autres tâches entre deux tours.
    std::thread::Builder::new()
        .name("decouverte-apps".into())
        .spawn(move || {
            boucle::tourner(
                move |message| emetteur.emettre(message),
                ordres,
                identite,
                PERIODE_RECONCILIATION,
            )
        })
        .map_err(|erreur| {
            tracing::error!(%erreur, "fil de decouverte d'applications non demarre");
        })
        .ok()
}

/// La variante hors Windows : il n'y a pas de raccourci à lire.
///
/// Elle existe pour que `main.rs` n'ait pas à porter un `cfg` de plus, et
/// **elle ne journalise rien** : un agent Linux n'a pas à se plaindre de ne
/// pas découvrir d'applications Windows, et la confondre avec `APPS=0` — qui,
/// lui, DIT qu'il est désarmé — brouillerait deux états distincts.
#[cfg(not(windows))]
pub fn brancher(_canal: &mut crate::plateforme::Canal) -> Option<std::thread::JoinHandle<()>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_valeur_zero_desarme_la_decouverte() {
        // 🔴 LA ROUGE : un `valeur.is_some()`. Il rendrait `true` pour `"1"`,
        // pour `""` et pour n'importe quoi — c'est-à-dire qu'écrire `APPS=0`
        // POUR COUPER la découverte l'activerait, et qu'écrire `APPS=1` POUR
        // L'ACTIVER la couperait. Les deux erreurs se compensent au point que
        // personne ne les verrait sans ce test.
        assert!(desarme(Some("0")));
        for valeur in [None, Some(""), Some("1"), Some("00"), Some("0 "), Some("false")] {
            assert!(!desarme(valeur), "{valeur:?} ne doit PAS désarmer");
        }
    }

    #[test]
    fn la_periode_de_reconciliation_laisse_la_place_a_son_propre_cout() {
        // Les 113 ms mesurés sur cette VM (31 ms d'énumération + 82 ms de
        // résolution COM) doivent rester une fraction négligeable de la
        // période, sans quoi la boucle passerait son temps à se lire elle-même.
        assert!(PERIODE_RECONCILIATION >= Duration::from_secs(5));
    }
}
