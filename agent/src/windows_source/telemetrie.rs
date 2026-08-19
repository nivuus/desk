//! Compteurs de capture, **par session**.
//!
//! **Pourquoi ce fichier existe** : `windows_source.rs` portait trois statiques
//! de processus (`TICKS`, `CAPTURED`, `PRODUCED`). Depuis le sous-bloc D4, la
//! capture est mutualisée dans un processus unique qui tient N fenêtres : les
//! trois compteurs mélangeaient donc N sessions. Pire, leur unique lecteur
//! (`demarrage.rs`) vit dans l'ENFANT, qui n'écrit rien — `SOURCE_TRACE=1`
//! n'affichait que des zéros, et les compteurs du capteur n'étaient lus par
//! personne. Consignation n°1 du sous-bloc D6, due depuis le 3 août 2026.
//!
//! **Pur, aucun `cfg`** : c'est ce qui le rend éprouvable sur l'hôte.

use std::sync::atomic::{AtomicU64, Ordering};

/// Compteurs de capture d'UNE fenêtre.
#[derive(Debug, Default)]
pub struct Telemetrie {
    ticks: AtomicU64,
    captured: AtomicU64,
    produced: AtomicU64,
}

impl Telemetrie {
    /// Un tour de boucle de capture.
    pub fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
    }

    /// Une image acquise auprès de la duplication.
    pub fn capturee(&self) {
        self.captured.fetch_add(1, Ordering::Relaxed);
    }

    /// Une image délivrée en aval.
    pub fn produite(&self) {
        self.produced.fetch_add(1, Ordering::Relaxed);
    }

    /// `(ticks, capturées, produites)`.
    pub fn lire(&self) -> (u64, u64, u64) {
        (
            self.ticks.load(Ordering::Relaxed),
            self.captured.load(Ordering::Relaxed),
            self.produced.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_telemetries_ne_se_melangent_pas() {
        // C'est TOUT l'objet du leg : trois statiques de processus mélangeaient
        // les N fenêtres du capteur depuis D4, et `SOURCE_TRACE=1` n'affichait
        // que des zéros dans l'enfant, qui n'écrit rien.
        let a = Telemetrie::default();
        let b = Telemetrie::default();
        a.tick();
        a.tick();
        a.capturee();
        b.produite();
        assert_eq!(a.lire(), (2, 1, 0));
        assert_eq!(b.lire(), (0, 0, 1));
    }

    /// Une télémétrie remise à neuf repart de zéro — et ce test le PROUVE en
    /// l'ayant d'abord fait compter sur ses TROIS compteurs.
    ///
    /// ⚠️ **Il remplace `une_telemetrie_neuve_est_a_zero` (leg n°11 de D9),
    /// qui n'éprouvait que `#[derive(Default)]` et ne pouvait pas rendre
    /// l'autre valeur** : il passait quel que soit le corps de
    /// `tick`/`capturee`/`produite`. La faiblesse a été PROUVÉE, pas
    /// affirmée — `tick()` rendu no-op, l'ancien test reste VERT quand
    /// celui-ci vire au rouge sur `left: (0, 1, 1)`.
    ///
    /// La précondition compare le triplet EXACT et non « différent de
    /// zéro » : saboter un seul des trois compteurs laisserait un
    /// `assert_ne!(…, (0,0,0))` vert, les deux autres suffisant à le
    /// satisfaire. C'est la première rédaction de ce test, et elle a été
    /// mesurée verte sous le sabotage qu'elle devait dénoncer.
    #[test]
    fn une_telemetrie_remise_a_neuf_repart_de_zero() {
        let mut t = Telemetrie::default();
        t.tick();
        t.capturee();
        t.produite();
        assert_eq!(t.lire(), (1, 1, 1), "précondition : les TROIS compteurs ont compté");
        t = Telemetrie::default();
        assert_eq!(t.lire(), (0, 0, 0));
    }
}
