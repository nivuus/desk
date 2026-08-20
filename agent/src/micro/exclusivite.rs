//! **PUR — aucun `cfg`, aucun objet Windows.** La politique d'exclusivité du
//! câble : qui a le droit d'écrire, quand on réessaie, et ce qui se journalise.
//!
//! Un seul processus enfant peut écrire sur CABLE Input à la fois. À deux
//! écrivains, Windows mélangerait deux copies décalées de la même voix — un
//! filtre en peigne. Le premier arrivé gagne (Décision 2 du plan E2).
//!
//! ⚠️ **Ce module ne tient AUCUN verrou.** Il arbitre, et il dit ce qu'il faut
//! journaliser. Le verrou lui-même est un mutex nommé Windows, posé par
//! `windows_micro.rs` derrière le trait `Verrou` ; les tests en posent un faux
//! qui **compte ses appels**.

/// Ce qu'un verrou inter-processus doit savoir faire.
pub trait Verrou {
    /// **NON BLOQUANT.** Rend `true` si ce processus tient le verrou après
    /// l'appel — y compris s'il le tenait déjà.
    ///
    /// ⚠️ **Il est appelé À CHAQUE dépôt, donc environ 50 fois par seconde, y
    /// compris quand le verrou est DÉJÀ tenu.** L'implémentation doit donc
    /// être bon marché *et* idempotente : un `WaitForSingleObject` sur un
    /// mutex déjà possédé en incrémenterait le compte de récursion, et il en
    /// faudrait autant de `ReleaseMutex`. C'est à la moitié Windows de court-
    /// circuiter ce cas, pas à `Exclusivite`.
    fn tenter(&mut self) -> bool;
}

/// Ce que l'arbitrage commande, journal compris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// Le dépôt est accepté, et rien n'est à journaliser.
    Accepte,
    /// Le dépôt est accepté, et c'est la PREMIÈRE fois après un refus : une
    /// ligne, sans quoi le `warn!` du refus resterait vrai pour toujours dans
    /// les yeux du lecteur.
    AccepteApresRefus,
    /// Refusé, et c'est la première fois : une ligne.
    RefusePremierement,
    /// Refusé, et c'est déjà dit. Silence.
    RefuseDejaDit,
}

/// L'arbitre. **La tentative est refaite à chaque dépôt ; seul le JOURNAL est
/// unique.**
pub struct Exclusivite<V: Verrou> {
    verrou: V,
    tenue: bool,
    refus_dit: bool,
}

impl<V: Verrou> Exclusivite<V> {
    pub fn new(verrou: V) -> Self {
        Self { verrou, tenue: false, refus_dit: false }
    }

    /// ⚠️ **TENTE À CHAQUE APPEL — Décision 2 du plan E2.** Seul le JOURNAL
    /// est unique, jamais la tentative.
    ///
    /// E1 prescrivait un refus *collant* : on journalise une fois et l'on
    /// n'insiste plus. Cela condamnait le cas suivant — les fenêtres A et B
    /// vivent, A tient le câble, B est refusée ; **A meurt**, Windows abandonne
    /// le mutex, et **B n'essaie plus jamais**, sans qu'aucune ligne ne le
    /// dise. Un `WaitForSingleObject(handle, 0)` coûte quelques microsecondes
    /// contre 50 dépôts par seconde : la tentative se refait.
    ///
    /// Un second épisode de refus rouvre son propre journal : perdre le câble
    /// une seconde fois est un fait neuf, pas la répétition du premier.
    pub fn arbitrer(&mut self) -> Issue {
        if self.verrou.tenter() {
            self.tenue = true;
            if std::mem::take(&mut self.refus_dit) {
                Issue::AccepteApresRefus
            } else {
                Issue::Accepte
            }
        } else {
            self.tenue = false;
            if self.refus_dit {
                Issue::RefuseDejaDit
            } else {
                self.refus_dit = true;
                Issue::RefusePremierement
            }
        }
    }

    /// Vrai si le dernier arbitrage a laissé ce processus propriétaire.
    pub fn tenue(&self) -> bool {
        self.tenue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un faux verrou qui **compte ses appels**. Un verrou qui ne rendrait
    /// qu'un booléen laisserait passer une implémentation collante : le test
    /// serait vacueux. C'est le patron du champ `journaux_micro` de E1.
    struct VerrouFactice {
        /// Les réponses à rendre, dans l'ordre ; la dernière se répète.
        reponses: Vec<bool>,
        appels: usize,
    }

    impl VerrouFactice {
        fn nouveau(reponses: &[bool]) -> Self {
            Self { reponses: reponses.to_vec(), appels: 0 }
        }
    }

    impl Verrou for VerrouFactice {
        fn tenter(&mut self) -> bool {
            let i = self.appels.min(self.reponses.len() - 1);
            self.appels += 1;
            self.reponses[i]
        }
    }

    #[test]
    fn un_verrou_libre_accepte_sans_rien_journaliser() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[true]));
        assert_eq!(e.arbitrer(), Issue::Accepte);
        assert!(e.tenue());
    }

    #[test]
    fn un_refus_ne_se_journalise_qu_une_fois() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false]));
        assert_eq!(e.arbitrer(), Issue::RefusePremierement);
        for _ in 0..10 {
            assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        }
        assert!(!e.tenue());
    }

    /// 🔴 **LE test du défaut latent de la Décision 4 de E1.** Un refus
    /// collant ne rappellerait jamais `tenter`, et la fenêtre B resterait sans
    /// micro pour la vie de son processus après la mort de la fenêtre A.
    #[test]
    #[allow(non_snake_case)]
    fn la_tentative_est_REFAITE_apres_un_refus() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false]));
        for _ in 0..20 {
            e.arbitrer();
        }
        assert_eq!(
            e.verrou.appels, 20,
            "la tentative doit être refaite à CHAQUE dépôt, refus compris"
        );
    }

    /// 🔴 Sans cette annonce, le `warn!` du refus resterait vrai à jamais.
    #[test]
    #[allow(non_snake_case)]
    fn une_acquisition_tardive_est_ANNONCEE() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[false, false, false, true]));
        assert_eq!(e.arbitrer(), Issue::RefusePremierement);
        assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        assert_eq!(e.arbitrer(), Issue::RefuseDejaDit);
        assert_eq!(e.arbitrer(), Issue::AccepteApresRefus);
        assert!(e.tenue());
        for _ in 0..5 {
            assert_eq!(e.arbitrer(), Issue::Accepte);
        }
    }

    #[test]
    fn une_acquisition_deja_tenue_ne_reannonce_rien() {
        let mut e = Exclusivite::new(VerrouFactice::nouveau(&[true]));
        for _ in 0..10 {
            assert_eq!(e.arbitrer(), Issue::Accepte);
        }
        assert!(e.tenue());
    }
}
