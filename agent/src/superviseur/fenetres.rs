//! The "Alt-Tab-able" criterion: which Windows windows deserve a browser
//! window.
//!
//! Pure logic, deliberately separated from the Windows glue of `hook.rs`: it is
//! the product rule, the one deciding what the user sees, and it
//! must be exercisable without Windows.

/// What we recorded of a window. No system call here: `hook.rs`
/// fills this structure, this module judges it.
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

/// True if this window deserves its own browser window.
///
/// The criterion is the framing's (`specs/2026-07-28-support-jeux-design.md`
/// §4, "filtering criterion retained"): everything not here — drop-down
/// menus, tooltips, modal dialogs, splash screens — stays
/// composed in its parent window and therefore arrives through that one's capture.
pub fn merite_une_fenetre(d: &DescriptionFenetre) -> bool {
    if !d.visible || d.masquee_dwm || d.a_un_proprietaire {
        return false;
    }
    // A window without a title is presentable neither in Alt-Tab nor in the
    // shell page: nothing would allow the user to designate it.
    if d.titre.is_empty() {
        return false;
    }
    // `WS_EX_APPWINDOW` is the explicit exemption: it forces presence
    // in Alt-Tab despite `WS_EX_TOOLWINDOW`.
    !d.tool_window || d.app_window
}

/// Must a window be SET ASIDE because it is not ours?
///
/// 🔴 **OWNERSHIP RULE, decided by the owner on August 30th, 2026:
/// `desk` only adopts the windows of applications it launched itself,
/// and of their descendants.** The mechanism is a job object without any limit
/// — see `crate::appartenance`, which carries the measurements.
///
/// **Separated from `merite_une_fenetre` ON PURPOSE**: the two refusals do not have the
/// same cause, and the caller must be able to SAY so in its log. A
/// window set aside because it is not ours is not diagnosed
/// like a tool window.
///
/// | `appartient` | meaning | verdict |
/// | --- | --- | --- |
/// | `Some(true)` | it is ours | kept |
/// | `Some(false)` | it belongs to another | **set aside** |
/// | `None` | the question FAILED | **kept** |
///
/// 🔴 **`None` KEEPS, AND IT IS DELIBERATE.** Setting aside for failing to ask
/// would turn a measurement failure into the silent disappearance of all
/// windows — the mute failure this repository pays more dearly for than a loud
/// defect. Same reasoning as `installation::execution::in_a_job`, which
/// answers `false` when the question fails.
pub fn ecartee_pour_non_appartenance(appartient: Option<bool>, regle_armee: bool) -> bool {
    regle_armee && appartient == Some(false)
}

#[cfg(test)]
mod tests_appartenance {
    use super::*;

    #[test]
    fn a_window_of_ours_is_kept() {
        assert!(!ecartee_pour_non_appartenance(Some(true), true));
    }

    /// 🔴 THE USEFUL RED: Steam, Apollo, `cmd.exe` — everything `desk` did not
    /// launch.
    #[test]
    fn a_window_not_ours_is_discarded() {
        assert!(ecartee_pour_non_appartenance(Some(false), true));
    }

    /// 🔴 A failure of the QUESTION is not an answer: we keep.
    #[test]
    fn a_failure_of_the_query_keeps_the_window() {
        assert!(!ecartee_pour_non_appartenance(None, true));
    }

    /// The `APPARTENANCE=0` bench arm: the product from before the rule,
    /// exactly. It is the WITNESS that makes the rule discriminating.
    #[test]
    fn disarmed_the_rule_discards_nothing() {
        for a in [Some(true), Some(false), None] {
            assert!(
                !ecartee_pour_non_appartenance(a, false),
                "{a:?} must no longer be discarded"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An ordinary application window: everything needed to deserve
    /// a browser window.
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
    fn an_ordinary_window_deserves_a_browser_window() {
        assert!(merite_une_fenetre(&ordinaire()));
    }

    #[test]
    fn an_invisible_window_is_discarded() {
        let d = DescriptionFenetre {
            visible: false,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn an_owned_window_is_discarded() {
        // Modal dialogs, palettes: they stay composed in their
        // parent, which already has its browser window.
        let d = DescriptionFenetre {
            a_un_proprietaire: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn a_tool_window_is_discarded() {
        let d = DescriptionFenetre {
            tool_window: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn a_tool_window_that_is_also_app_window_is_kept() {
        // WS_EX_APPWINDOW forces presence in Alt-Tab: it is the
        // exact exemption the framing's criterion provides.
        let d = DescriptionFenetre {
            tool_window: true,
            app_window: true,
            ..ordinaire()
        };
        assert!(merite_une_fenetre(&d));
    }

    #[test]
    fn a_dwm_cloaked_window_is_discarded() {
        // Without this filter we pick up ghost UWP windows, which exist
        // without ever showing.
        let d = DescriptionFenetre {
            masquee_dwm: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn an_untitled_window_is_discarded() {
        let d = DescriptionFenetre {
            titre: String::new(),
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }

    #[test]
    fn dwm_cloaking_takes_precedence_over_app_window() {
        // A ghost window carrying WS_EX_APPWINDOW must not
        // come back through the exemption: the order of tests matters here.
        let d = DescriptionFenetre {
            tool_window: true,
            app_window: true,
            masquee_dwm: true,
            ..ordinaire()
        };
        assert!(!merite_une_fenetre(&d));
    }
}
