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

    #[test]
    fn une_telemetrie_neuve_est_a_zero() {
        assert_eq!(Telemetrie::default().lire(), (0, 0, 0));
    }
}
