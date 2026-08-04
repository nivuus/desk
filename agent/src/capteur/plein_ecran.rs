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
