//! Détecter qu'une application est passée en plein écran, par son style.
//!
//! **Pur, sans aucun `cfg`** — comme `capteur/audio.rs` et
//! `capteur/repartiteur.rs` avant lui : il reçoit un `u32` de style et rend des
//! booléens. La seule lecture Win32 vit dans le module `win` en bas de fichier,
//! derrière un `#[cfg(windows)]`.
//!
//! ❌ **Le critère du cadrage jeux §4.1 — comparer le rect de la fenêtre à
//! celui du moniteur — est MORT dans cette architecture, et il ne faut pas y
//! revenir.** Depuis le sous-bloc D1, `superviseur::placement::poser` donne à
//! la fenêtre exactement la taille de sa sortie virtuelle, et
//! `controler_le_placement` la lui réimpose périodiquement : « rect fenêtre ==
//! rect moniteur » est l'état NOMINAL. Ce critère n'est pas inopérant, il est
//! TOUJOURS VRAI — une implémentation fidèle annoncerait le plein écran en
//! permanence, pour toutes les fenêtres.
//!
//! `SHQueryUserNotificationState` a été écarté pour une autre raison : il est
//! global à la session interactive, donc à N fenêtres il ne dit pas LAQUELLE,
//! et il ne voit pas le « borderless fullscreen » que les jeux emploient.

/// `WS_CAPTION` — la fenêtre a une barre de titre.
pub const WS_CAPTION_BIT: u32 = 0x00C0_0000;
/// `WS_THICKFRAME` — la fenêtre a un cadre redimensionnable.
pub const WS_THICKFRAME_BIT: u32 = 0x0004_0000;

/// Vrai si le style ne porte ni barre de titre ni cadre redimensionnable.
pub fn est_sans_bordure(style: u32) -> bool {
    style & (WS_CAPTION_BIT | WS_THICKFRAME_BIT) == 0
}

/// Suit l'état de bordure d'une fenêtre et n'annonce que les CHANGEMENTS.
///
/// **L'état lu à l'attache fait référence** (§5.2 de la spec) : une application
/// née sans bordure n'annonce rien, et ne fait donc pas entrer sa fenêtre
/// navigateur en plein écran sans raison.
pub struct SuiviBordure {
    sans_bordure: bool,
}

impl SuiviBordure {
    pub fn nouveau(style_initial: u32) -> Self {
        Self { sans_bordure: est_sans_bordure(style_initial) }
    }

    /// Rend `Some(actif)` au changement, `None` sinon.
    pub fn observer(&mut self, style: u32) -> Option<bool> {
        let courant = est_sans_bordure(style);
        if courant == self.sans_bordure {
            return None;
        }
        self.sans_bordure = courant;
        Some(courant)
    }
}

use std::sync::OnceLock;
use std::time::Duration;

/// `PLEIN_ECRAN=0` désarme le mécanisme ENTIER — la relecture du style
/// (`capteur/fenetre.rs`) comme le changement de mode de la sortie
/// (`windows_source/redimensionnement.rs`).
///
/// **`=0` DÉSACTIVE, une simple présence n'active pas**, exactement comme
/// `AUDIO`, `SUPERVISEUR` et `CAPTEUR` (`main.rs`) : tester `is_ok()`
/// activerait le plein écran en écrivant `PLEIN_ECRAN=0` pour le couper.
///
/// ⚠️ **Lue dans le processus CAPTEUR**, où vivent les deux moitiés du
/// mécanisme depuis D4 — le fil de fenêtre qui lit le style, et la
/// `WindowsSource` que `resize` retaille. L'enfant ne la consulte pas : il ne
/// fait que relayer.
///
/// `OnceLock` et non une lecture par appel : la relecture du style court à
/// 4 Hz par fenêtre, et l'environnement ne change pas en cours de processus.
/// C'est le même montage que `BUDGET_BPS` (`capteur/sommeil/parts.rs`).
pub fn actif() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("PLEIN_ECRAN").as_deref() != Ok("0");
        if !actif {
            tracing::warn!(
                "plein ecran DESARME (PLEIN_ECRAN=0) : ni detection de style, \
                 ni changement de mode de sortie"
            );
        }
        actif
    })
}

/// Période de relecture du style de la fenêtre.
///
/// ⚠️ **Ce n'est PAS le tour de roue de `PERIODE_REARBITRAGE`**, qui vit sur le
/// fil de sommeil (`capteur/sommeil.rs`) et n'a pas les `HWND`. La valeur est du
/// même ordre, délibérément, mais la constante est propre à ce module : les
/// faire suivre l'une l'autre coupleraient deux mécanismes que rien ne lie.
///
/// **Jamais à l'image** : un `GetWindowLongPtrW` est bon marché, pas gratuit à
/// N × 90 i/s.
pub const PERIODE_STYLE: Duration = Duration::from_millis(250);

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_STYLE};

    /// Style de la fenêtre, ou `None` si la fenêtre a disparu.
    ///
    /// `GetWindowLongPtrW` rend `0` sur erreur, ce qui est aussi un style
    /// valide en théorie — mais une fenêtre applicative réelle en a toujours au
    /// moins un bit. On traite donc `0` comme une disparition : le seul risque
    /// est de ne rien annoncer, jamais d'annoncer à tort.
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

    /// Style d'une fenêtre applicative ordinaire : titre + cadre.
    const ORDINAIRE: u32 = WS_CAPTION_BIT | WS_THICKFRAME_BIT | 0x1000_0000;
    /// Style d'une fenêtre « borderless fullscreen » : ni l'un ni l'autre.
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
        // Une fenêtre non redimensionnable garde sa barre de titre : elle
        // n'est pas en plein écran.
        assert!(!est_sans_bordure(WS_CAPTION_BIT));
    }

    #[test]
    fn le_cadre_seul_suffit_a_faire_une_bordure() {
        assert!(!est_sans_bordure(WS_THICKFRAME_BIT));
    }

    #[test]
    fn le_premier_observer_sur_l_etat_initial_n_annonce_rien() {
        // La garde du §5.2 : l'état lu à l'attache fait référence, et l'on
        // n'annonce que les CHANGEMENTS.
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn une_fenetre_nee_sans_bordure_n_annonce_rien() {
        // Sans cette garde, une application déjà sans bordure au démarrage
        // ferait entrer sa fenêtre navigateur en plein écran sans raison.
        let mut suivi = SuiviBordure::nouveau(SANS_BORDURE);
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn la_perte_de_la_bordure_annonce_le_plein_ecran_une_seule_fois() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        // Deuxième lecture identique : plus rien à annoncer.
        assert_eq!(suivi.observer(SANS_BORDURE), None);
    }

    #[test]
    fn le_retour_de_la_bordure_annonce_la_sortie_du_plein_ecran() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        assert_eq!(suivi.observer(SANS_BORDURE), Some(true));
        assert_eq!(suivi.observer(ORDINAIRE), Some(false));
        assert_eq!(suivi.observer(ORDINAIRE), None);
    }

    #[test]
    fn un_aller_retour_complet_annonce_deux_fois_et_pas_davantage() {
        let mut suivi = SuiviBordure::nouveau(ORDINAIRE);
        let annonces: Vec<Option<bool>> = [SANS_BORDURE, SANS_BORDURE, ORDINAIRE, ORDINAIRE]
            .into_iter()
            .map(|s| suivi.observer(s))
            .collect();
        assert_eq!(annonces, vec![Some(true), None, Some(false), None]);
    }
}
