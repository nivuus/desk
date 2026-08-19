//! Le prédicat PUR de PERSISTANCE (tâche 2bis, sous-bloc D9, revue du
//! coordinateur, correction n°14) : deux mesures, potentiellement absentes
//! chacune, disent-elles la même taille ?
//!
//! Extrait de `diagnostics::multifenetre::mode_sortie::persistance`, qui
//! reste `#[cfg(windows)]` (par le `#[cfg(windows)]` posé sur `mod multifenetre`
//! dans `diagnostics.rs`, qui gate tout ce qui vit dessous — un prédicat pur
//! niché plus profond dans cet arbre ne compilerait donc jamais sur l'hôte,
//! quels que soient ses propres attributs). Ce prédicat-ci ne touche aucun
//! type Windows — seulement des `Option<(u32, u32)>` — et PEUT donc se
//! compiler et se tester sur l'hôte Linux : même précédent que
//! `geometry.rs` et `sortie_dxgi.rs`, extraits pour la même raison.
//!
//! **Racine nue, et non `#[path]` chez un parent** (convention tranchée
//! tâche 17, sous-bloc D10, voir `CLAUDE.md` §« Convention de module
//! enfant ») : un module dont le nom s'écrit `<parent>_<enfant>`
//! (`capture_reprise`, `windows_source_sortie`, `windows_source_telemetrie`)
//! reste physiquement chez ce parent et se hisse par
//! `#[path]` dans `main.rs`. Un module dont le nom se comprend SANS
//! préfixer un parent — c'est le cas ici, `survie_verdict` ne porte le nom
//! d'aucun module de premier niveau — vit à la racine nue, comme
//! `geometry.rs` et `sortie_dxgi.rs`. La profondeur d'où on l'extrait
//! (`diagnostics::multifenetre::mode_sortie::persistance`, quatre niveaux)
//! n'y change rien : il n'y a de toute façon aucun nom de parent court et
//! unique à préfixer.
//!
//! C'est le prédicat même qui portait l'Important I4 de la revue de la
//! tâche 2bis : l'ancienne version repliait une taille introuvable sur
//! `(0, 0)`, si bien qu'une sortie disparue AVANT et APRÈS rendait
//! `(0, 0) == (0, 0)` → `survit=true` — l'instrument annonçait « le
//! changement a survécu » exactement quand la sortie s'était volatilisée.
//! Isolé ici, ce défaut aurait été visible dans un test dès le premier
//! essai — doctrine du dépôt : un contrôle qu'on n'a jamais vu ROUGE n'en
//! est pas un.

/// Rend `"true"` ou `"false"` quand LES DEUX mesures existent et qu'on peut
/// donc les comparer, `"indetermine (sortie disparue)"` dès que l'UNE des
/// deux manque — jamais une comparaison sur une sentinelle numérique
/// confondable avec un succès.
pub(crate) fn verdict_persistance(
    avant: Option<(u32, u32)>,
    apres: Option<(u32, u32)>,
) -> &'static str {
    match (avant, apres) {
        (Some(a), Some(b)) if a == b => "true",
        (Some(_), Some(_)) => "false",
        _ => "indetermine (sortie disparue)",
    }
}

#[cfg(test)]
mod tests {
    use super::verdict_persistance;

    #[test]
    fn deux_mesures_egales_rendent_true() {
        assert_eq!(verdict_persistance(Some((1920, 1080)), Some((1920, 1080))), "true");
    }

    #[test]
    fn deux_mesures_differentes_rendent_false() {
        assert_eq!(verdict_persistance(Some((1920, 1080)), Some((1280, 720))), "false");
    }

    /// Le cas que l'ancienne version confondait avec `"true"` : les DEUX
    /// mesures absentes (sortie disparue avant ET après) ne doivent JAMAIS
    /// se lire comme une survie -- ni comme une comparaison sur `(0, 0)`.
    #[test]
    fn une_mesure_absente_de_chaque_cote_rend_indetermine() {
        assert_eq!(verdict_persistance(None, Some((1280, 720))), "indetermine (sortie disparue)");
        assert_eq!(verdict_persistance(Some((1280, 720)), None), "indetermine (sortie disparue)");
        assert_eq!(verdict_persistance(None, None), "indetermine (sortie disparue)");
    }
}
