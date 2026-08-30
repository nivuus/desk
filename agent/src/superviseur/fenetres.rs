//! Le critère « Alt-Tab-able » : quelles fenêtres Windows méritent une fenêtre
//! navigateur.
//!
//! Logique pure, délibérément séparée de la glue Windows de `hook.rs` : c'est
//! la règle produit, celle qui décide de ce que l'utilisateur voit, et elle
//! doit être éprouvable sans Windows.

/// Ce qu'on a relevé d'une fenêtre. Aucun appel système ici : `hook.rs`
/// remplit cette structure, ce module la juge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionFenetre {
    /// `WS_VISIBLE` et `IsWindowVisible`.
    pub visible: bool,
    /// `GetWindow(hwnd, GW_OWNER)` non nul.
    pub a_un_proprietaire: bool,
    /// `WS_EX_TOOLWINDOW`.
    pub tool_window: bool,
    /// `WS_EX_APPWINDOW`.
    pub app_window: bool,
    /// `DwmGetWindowAttribute` / `DWMWA_CLOAKED` non nul.
    pub masquee_dwm: bool,
    /// `GetWindowTextW`.
    pub titre: String,
}

/// Vrai si cette fenêtre mérite sa propre fenêtre navigateur.
///
/// Le critère est celui du cadrage (`specs/2026-07-28-support-jeux-design.md`
/// §4, « critère de filtrage retenu ») : tout ce qui n'est pas ici — menus
/// déroulants, infobulles, dialogues modaux, écrans de démarrage — reste
/// composé dans sa fenêtre parente et arrive donc par la capture de celle-ci.
pub fn merite_une_fenetre(d: &DescriptionFenetre) -> bool {
    if !d.visible || d.masquee_dwm || d.a_un_proprietaire {
        return false;
    }
    // Une fenêtre sans titre n'est présentable ni dans Alt-Tab ni dans la
    // page-shell : rien ne permettrait à l'utilisateur de la désigner.
    if d.titre.is_empty() {
        return false;
    }
    // `WS_EX_APPWINDOW` est la dérogation explicite : elle force la présence
    // dans Alt-Tab malgré `WS_EX_TOOLWINDOW`.
    !d.tool_window || d.app_window
}

/// Une fenêtre doit-elle être ÉCARTÉE parce qu'elle n'est pas à nous ?
///
/// 🔴 **RÈGLE D'APPARTENANCE, tranchée par le propriétaire le 30 août 2026 :
/// `desk` n'adopte que les fenêtres des applications qu'il a lui-même lancées,
/// et de leur descendance.** Le mécanisme est un job object sans aucune limite
/// — voir `crate::appartenance`, qui porte les mesures.
///
/// **Séparée de `merite_une_fenetre` À DESSEIN** : les deux refus n'ont pas la
/// même cause, et l'appelant doit pouvoir le DIRE dans son journal. Une
/// fenêtre écartée parce qu'elle n'est pas à nous ne se diagnostique pas
/// comme une fenêtre-outil.
///
/// | `appartient` | sens | verdict |
/// | --- | --- | --- |
/// | `Some(true)` | elle est à nous | gardée |
/// | `Some(false)` | elle est à un autre | **écartée** |
/// | `None` | la question a ÉCHOUÉ | **gardée** |
///
/// 🔴 **`None` GARDE, ET C'EST DÉLIBÉRÉ.** Écarter faute d'avoir su demander
/// transformerait une panne de mesure en disparition silencieuse de toutes les
/// fenêtres — la panne muette que ce dépôt paie plus cher qu'un défaut
/// bruyant. Même raisonnement que `installation::execution::dans_un_job`, qui
/// répond `false` quand la question échoue.
pub fn ecartee_pour_non_appartenance(appartient: Option<bool>, regle_armee: bool) -> bool {
    regle_armee && appartient == Some(false)
}

#[cfg(test)]
mod tests_appartenance {
    use super::*;

    #[test]
    fn une_fenetre_a_nous_est_gardee() {
        assert!(!ecartee_pour_non_appartenance(Some(true), true));
    }

    /// 🔴 LA ROUGE UTILE : Steam, Apollo, `cmd.exe` — tout ce que `desk` n'a
    /// pas lancé.
    #[test]
    fn une_fenetre_qui_n_est_pas_a_nous_est_ecartee() {
        assert!(ecartee_pour_non_appartenance(Some(false), true));
    }

    /// 🔴 Un échec de la QUESTION n'est pas une réponse : on garde.
    #[test]
    fn un_echec_de_la_question_garde_la_fenetre() {
        assert!(!ecartee_pour_non_appartenance(None, true));
    }

    /// Le bras de banc `APPARTENANCE=0` : le produit d'avant la règle,
    /// exactement. C'est le TÉMOIN qui rend la règle discriminante.
    #[test]
    fn desarmee_la_regle_n_ecarte_plus_rien() {
        for a in [Some(true), Some(false), None] {
            assert!(!ecartee_pour_non_appartenance(a, false), "{a:?} ne doit plus être écartée");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une fenêtre ordinaire d'application : tout ce qu'il faut pour mériter
    /// une fenêtre navigateur.
    fn ordinaire() -> DescriptionFenetre {
        DescriptionFenetre {
            visible: true,
            a_un_proprietaire: false,
            tool_window: false,
            app_window: false,
            masquee_dwm: false,
            titre: "Bloc-notes".into(),
        }
    }

    #[test]
    fn une_fenetre_ordinaire_merite_une_fenetre_navigateur() {
        assert!(merite_une_fenetre(&ordinaire()));
    }

    #[test]
    fn une_fenetre_invisible_est_ecartee() {
        let d = DescriptionFenetre { visible: false, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_possedee_est_ecartee() {
        // Dialogues modaux, palettes : elles restent composées dans leur
        // parente, qui a déjà sa fenêtre navigateur.
        let d = DescriptionFenetre { a_un_proprietaire: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_tool_window_est_ecartee() {
        let d = DescriptionFenetre { tool_window: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_tool_window_qui_est_aussi_app_window_est_gardee() {
        // WS_EX_APPWINDOW force la présence dans Alt-Tab : c'est la
        // dérogation exacte que le critère du cadrage prévoit.
        let d = DescriptionFenetre { tool_window: true, app_window: true, ..ordinaire() };
        assert!(merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_masquee_par_dwm_est_ecartee() {
        // Sans ce filtre on capte les fenêtres UWP fantômes, qui existent
        // sans jamais s'afficher.
        let d = DescriptionFenetre { masquee_dwm: true, ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn une_fenetre_sans_titre_est_ecartee() {
        let d = DescriptionFenetre { titre: String::new(), ..ordinaire() };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn le_masquage_dwm_prime_sur_app_window() {
        // Une fenêtre fantôme qui porterait WS_EX_APPWINDOW ne doit pas
        // ressortir par la dérogation : l'ordre des tests compte ici.
        let d = DescriptionFenetre {
            tool_window: true,
            app_window: true,
            masquee_dwm: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }
}
