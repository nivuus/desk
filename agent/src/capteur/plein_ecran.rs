//! Detecting that an application has gone fullscreen, through its style.
//!
//! **Pure, without any `cfg`** — like `capteur/audio.rs` and
//! `capteur/repartiteur.rs` before it: it receives a style `u32` and returns
//! booleans. The only Win32 read lives in the `win` module at the bottom of the file,
//! behind a `#[cfg(windows)]`.
//!
//! ❌ **The criterion from the games framing §4.1 — comparing the window's rect to
//! the monitor's — is DEAD in this architecture, and it must not be
//! revisited.** Since sub-block D1, `superviseur::placement::poser` gives
//! the window the size kept for its virtual output, and
//! `controler_le_placement` periodically re-imposes it. The window's rect
//! is therefore **imposed by the supervisor, never by the application** —
//! and that alone kills the criterion: it measures a decision of our
//! own code, not a gesture of the observed application.
//!
//! ⚠️ **The original wording — "window rect == monitor rect is the
//! NOMINAL state, this criterion is ALWAYS TRUE" — is no longer accurate since
//! sub-block D10** (family ①, tasks 4 to 9). A virtual output can be born
//! larger than the requested size; the supervisor then accepts it and places
//! the window at the **kept size**, strictly smaller than the output.
//! The criterion would therefore be ALWAYS FALSE there instead of always true — a
//! faithful implementation would never announce fullscreen on those
//! windows, and would announce it permanently on the others. **The
//! conclusion does not move an inch: the criterion is dead in both
//! regimes, and it is now dead in two opposite ways depending on the state of the
//! registry — which is worse, not better.**
//!
//! `SHQueryUserNotificationState` was ruled out for another reason: it is
//! global to the interactive session, so with N windows it does not say WHICH one,
//! and it does not see the "borderless fullscreen" games use.
//!
//! **The "resolution follows" direction does not exist, and it is a measurement
//! decision, not an oversight.** Sub-block D8 had written a mode change of
//! the virtual output, shipped disarmed for never having run. Sub-block
//! D9 measured it and removed it:
//!
//! - the change **does not survive** — the output returns to its creation size
//!   as soon as one more virtual output is created, that is at every
//!   window opening. `n = 4` clean runs, targets all distinct
//!   from the creation size, under `CDS_UPDATEREGISTRY` as under `flags = 0`;
//! - `CDS_UPDATEREGISTRY` **pollutes the registry**, confirmed and attributable by
//!   GUID over 3 conclusive transitions: an output created afterwards is born at the
//!   polluted size, ~~which blocks the product~~ — the preparation of the
//!   D8 acceptance run had to lift that block by hand.
//!
//! ✅ **"Which blocks the product" IS NO LONGER TRUE since sub-block D10**
//! (family ①, tasks 4 to 9, acceptance run ① of task 10). The supervisor
//! **accepts** an output born too large instead of returning it to the driver, and
//! the capture crops the kept size inside it. Read on a registry left
//! dirty at 3840×2160: the `main` binary attaches **3** windows and rejects
//! **32** times, the branch attaches **10** with **0** rejections, over **two**
//! runs. ⚠️ **The POLLUTION itself remains: it is the product that has
//! become indifferent to it, not the registry that was cleaned** — nothing in this
//! repository cleans up behind. The measured fact above therefore remains whole;
//! only its product consequence has disappeared.
//!
//! Logs: `docs/superpowers/plans/journaux-multifenetres-d9/p-persistance-*`
//! and `p2-*`. **The mechanism is NOT explained**: it is separated into two
//! observable regimes, and a confounder covaries with their boundary.

/// `WS_CAPTION` — the window has a title bar.
pub const WS_CAPTION_BIT: u32 = 0x00C0_0000;
/// `WS_THICKFRAME` — the window has a resizable frame.
pub const WS_THICKFRAME_BIT: u32 = 0x0004_0000;

/// True if the style carries neither title bar nor resizable frame.
pub fn est_sans_bordure(style: u32) -> bool {
    style & (WS_CAPTION_BIT | WS_THICKFRAME_BIT) == 0
}

/// Tracks a window's border state and announces only CHANGES.
///
/// **The state read at attach time is the reference** (§5.2 of the spec): an application
/// born borderless announces nothing, and therefore does not put its browser
/// window into fullscreen for no reason.
pub struct SuiviBordure {
    sans_bordure: bool,
}

impl SuiviBordure {
    pub fn new(style_initial: u32) -> Self {
        Self {
            sans_bordure: est_sans_bordure(style_initial),
        }
    }

    /// Returns `Some(actif)` on change, `None` otherwise.
    pub fn observer(&mut self, style: u32) -> Option<bool> {
        let current = est_sans_bordure(style);
        if current == self.sans_bordure {
            return None;
        }
        self.sans_bordure = current;
        Some(current)
    }
}

use std::sync::OnceLock;
use std::time::Duration;

/// `PLEIN_ECRAN=0` disarms detection — re-reading the style
/// (`capteur/fenetre.rs`) and the resulting `PleinEcran` → `Fullscreen`
/// announcement. **That is all this mechanism does now**: sub-block
/// D9 removed the other half, the virtual output's mode change
/// (see the measurement finding at the head of the file) — there is therefore nothing
/// else left to disarm.
///
/// **`=0` DISABLES, mere presence does not enable**, exactly like
/// `AUDIO`, `SUPERVISEUR` and `CAPTEUR` (`main.rs`): testing `is_ok()`
/// would enable fullscreen when writing `PLEIN_ECRAN=0` to turn it off.
///
/// ⚠️ **Read in the SENSOR process**, where this mechanism has lived since D4 — the window
/// thread that reads the style. The child does not consult it: it only
/// relays.
///
/// `OnceLock` and not a read per call: re-reading the style runs at
/// 4 Hz per window, and the environment does not change during the process.
/// It is the same set-up as `BUDGET_BPS` (`capteur/sommeil/parts.rs`).
pub fn actif() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("PLEIN_ECRAN").as_deref() != Ok("0");
        if !actif {
            tracing::warn!("fullscreen DISARMED (PLEIN_ECRAN=0): style detection disabled");
        }
        actif
    })
}

/// Period for re-reading the window's style.
///
/// ⚠️ **This is NOT the `PERIODE_REARBITRAGE` wheel round**, which lives on the
/// sleep thread (`capteur/sommeil.rs`) and does not have the `HWND`s. The value is of the
/// same order, deliberately, but the constant is specific to this module: making
/// them follow each other would couple two mechanisms nothing links.
///
/// **Never per frame**: a `GetWindowLongPtrW` is cheap, not free at
/// N × 90 fps.
pub const PERIODE_STYLE: Duration = Duration::from_millis(250);

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_STYLE};

    /// Style of the window, or `None` if the window has disappeared.
    ///
    /// `GetWindowLongPtrW` returns `0` on error, which is also a valid style
    /// in theory — but a real application window always has at
    /// least one bit. We therefore treat `0` as a disappearance: the only risk
    /// is to announce nothing, never to announce wrongly.
    pub fn lire_style(hwnd: HWND) -> Option<u32> {
        let brut = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
        if brut == 0 {
            return None;
        }
        Some(brut as u32)
    }
}

#[cfg(windows)]
pub use win::lire_style;

#[cfg(test)]
mod tests {
    use super::*;

    /// Style of an ordinary application window: title + frame.
    const ORDINAIRE: u32 = WS_CAPTION_BIT | WS_THICKFRAME_BIT | 0x1000_0000;
    /// Style of a "borderless fullscreen" window: neither one nor the other.
    const SANS_BORDURE: u32 = 0x1000_0000;

    #[test]
    fn une_fenetre_a_titre_et_cadre_a_une_bordure() {
        assert!(!est_sans_bordure(ORDINAIRE));
    }

    #[test]
    fn une_fenetre_sans_titre_ni_cadre_n_a_pas_de_bordure() {
        assert!(est_sans_bordure(SANS_BORDURE));
    }

    #[test]
    fn le_titre_seul_suffit_a_faire_une_bordure() {
        // A non-resizable window keeps its title bar: it
        // is not fullscreen.
        assert!(!est_sans_bordure(WS_CAPTION_BIT));
    }

    #[test]
    fn le_cadre_seul_suffit_a_faire_une_bordure() {
        assert!(!est_sans_bordure(WS_THICKFRAME_BIT));
    }

    #[test]
    fn le_premier_observer_sur_l_etat_initial_n_annonce_rien() {
        // The §5.2 guard: the state read at attach time is the reference, and we
        // only announce CHANGES.
        let mut suivi = SuiviBordure::new(ORDINAIRE);
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn une_fenetre_nee_sans_bordure_n_annonce_rien() {
        // Without this guard, an application already borderless at start-up
        // would put its browser window into fullscreen for no reason.
        let mut suivi = SuiviBordure::new(SANS_BORDURE);
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn la_perte_de_la_bordure_annonce_le_plein_ecran_une_seule_fois() {
        let mut suivi = SuiviBordure::new(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        // Second identical reading: nothing more to announce.
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn le_retour_de_la_bordure_annonce_la_sortie_du_plein_ecran() {
        let mut suivi = SuiviBordure::new(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        assert_eq!(suivi.observer(ORDINAIRE), Some(false));
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn un_aller_retour_complet_annonce_deux_fois_et_pas_davantage() {
        let mut suivi = SuiviBordure::new(ORDINAIRE);
        let annonces: Vec<Option<bool>> = [SANS_BORDURE, SANS_BORDURE, ORDINAIRE, ORDINAIRE]
            .into_iter()
            .map(|s| suivi.observer(s))
            .collect();
        assert_eq!(annonces, vec![Some(true), None, Some(false), None]);
    }
}
