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
//! la fenêtre la taille retenue pour sa sortie virtuelle, et
//! `controler_le_placement` la lui réimpose périodiquement. Le rect de la
//! fenêtre est donc **imposé par le superviseur, jamais par l'application** —
//! et c'est cela seul qui tue le critère : il mesure une décision de notre
//! propre code, pas un geste de l'application observée.
//!
//! ⚠️ **La formulation d'origine — « rect fenêtre == rect moniteur est l'état
//! NOMINAL, ce critère est TOUJOURS VRAI » — n'est plus exacte depuis le
//! sous-bloc D10** (famille ①, tâches 4 à 9). Une sortie virtuelle peut naître
//! plus grande que la taille demandée ; le superviseur l'accepte alors et pose
//! la fenêtre à la **taille retenue**, strictement plus petite que la sortie.
//! Le critère y serait donc TOUJOURS FAUX au lieu de toujours vrai — une
//! implémentation fidèle n'annoncerait jamais le plein écran sur ces
//! fenêtres-là, et l'annoncerait en permanence sur les autres. **La
//! conclusion ne bouge pas d'un pouce : le critère est mort dans les deux
//! régimes, et il est désormais mort de deux façons opposées selon l'état du
//! registre — ce qui est pire, pas mieux.**
//!
//! `SHQueryUserNotificationState` a été écarté pour une autre raison : il est
//! global à la session interactive, donc à N fenêtres il ne dit pas LAQUELLE,
//! et il ne voit pas le « borderless fullscreen » que les jeux emploient.
//!
//! **Le sens « la résolution suit » n'existe pas, et c'est une décision de
//! mesure, pas un oubli.** Le sous-bloc D8 avait écrit un changement de mode de
//! la sortie virtuelle, livré désarmé faute d'avoir jamais tourné. Le sous-bloc
//! D9 l'a mesuré et l'a retiré :
//!
//! - le changement **ne survit pas** — la sortie revient à sa taille de création
//!   dès qu'une sortie virtuelle de plus est créée, c'est-à-dire à chaque
//!   ouverture de fenêtre. `n = 4` exécutions propres, cibles toutes distinctes
//!   de la taille de création, sous `CDS_UPDATEREGISTRY` comme sous `flags = 0` ;
//! - `CDS_UPDATEREGISTRY` **pollue le registre**, confirmé et attribuable par
//!   GUID sur 3 transitions probantes : une sortie créée ensuite naît à la
//!   taille polluée, ~~ce qui bloque le produit~~ — la préparation de la
//!   recette D8 avait dû lever ce blocage à la main.
//!
//! ✅ **« Ce qui bloque le produit » N'EST PLUS VRAI depuis le sous-bloc D10**
//! (famille ①, tâches 4 à 9, recette ① de la tâche 10). Le superviseur
//! **accepte** une sortie née trop grande au lieu de la rendre au pilote, et
//! la capture recadre la taille retenue dedans. Relevé sur un registre laissé
//! sale à 3840×2160 : le binaire de `main` attache **3** fenêtres et rejette
//! **32** fois, la branche en attache **10** avec **0** rejet, à **deux**
//! exécutions. ⚠️ **La POLLUTION, elle, subsiste : c'est le produit qui y est
//! devenu indifférent, pas le registre qui a été nettoyé** — rien, dans ce
//! dépôt, ne nettoie derrière. Le fait mesuré ci-dessus reste donc entier ;
//! seule sa conséquence produit a disparu.
//!
//! Journaux : `docs/superpowers/plans/journaux-multifenetres-d9/p-persistance-*`
//! et `p2-*`. **Le mécanisme n'est PAS expliqué** : il est séparé en deux
//! régimes observables, et un confondeur covarie avec leur frontière.

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

/// `PLEIN_ECRAN=0` désarme la détection — la relecture du style
/// (`capteur/fenetre.rs`) et l'annonce `PleinEcran` → `Fullscreen` qui en
/// découle. **C'est tout ce que ce mécanisme fait désormais** : le sous-bloc
/// D9 a retiré l'autre moitié, le changement de mode de la sortie virtuelle
/// (voir le constat de mesure en tête de fichier) — il n'y a donc plus rien
/// d'autre à désarmer.
///
/// **`=0` DÉSACTIVE, une simple présence n'active pas**, exactement comme
/// `AUDIO`, `SUPERVISEUR` et `CAPTEUR` (`main.rs`) : tester `is_ok()`
/// activerait le plein écran en écrivant `PLEIN_ECRAN=0` pour le couper.
///
/// ⚠️ **Lue dans le processus CAPTEUR**, où vit ce mécanisme depuis D4 — le fil
/// de fenêtre qui lit le style. L'enfant ne la consulte pas : il ne fait que
/// relayer.
///
/// `OnceLock` et non une lecture par appel : la relecture du style court à
/// 4 Hz par fenêtre, et l'environnement ne change pas en cours de processus.
/// C'est le même montage que `BUDGET_BPS` (`capteur/sommeil/parts.rs`).
pub fn actif() -> bool {
    static ACTIF: OnceLock<bool> = OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("PLEIN_ECRAN").as_deref() != Ok("0");
        if !actif {
            tracing::warn!("plein ecran DESARME (PLEIN_ECRAN=0) : detection de style desactivee");
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
