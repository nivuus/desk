//! Le pendant, côté enfant, du compteur de cadence du capteur
//! (`capteur/fenetre.rs`) : combien d'unités d'accès la piste vidéo écrit
//! réellement, pour que le critère de réception du sous-bloc D4 puisse
//! opposer deux chiffres — combien le capteur produit, combien l'enfant en
//! écrit — plutôt que d'en affirmer un seul.
//!
//! Extrait de `piste_video.rs` (tâche 8, sous-bloc D4) : ce fichier était à
//! 466 lignes, marge 34, et l'ajout de ce compteur la dépassait.

use std::time::{Duration, Instant};

use super::Session;

/// Période des lignes de cadence de cette piste. **Même valeur que
/// `capteur/fenetre.rs::PERIODE_COMPTEURS`** : c'est ce qui rend les deux
/// relevés comparables. **Jamais de trace par image** : le projet a déjà
/// perdu une session entière à une trace par paquet (18 619 lignes en
/// quelques secondes, sur un partage CIFS) — voir `CLAUDE.md`.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

impl Session {
    /// Pose l'identifiant de session utilisé par la ligne de cadence
    /// périodique (`compter_la_cadence_video`). Appelé une fois par
    /// `demarrage.rs`, juste après `Session::new` — jamais par les tests, qui
    /// n'ont besoin d'aucune valeur pour vérifier `write_frame` ou
    /// `next_frame_deadline` : le champ reste vide (`session=""`) sur ces
    /// chemins, sans conséquence puisqu'aucun test n'observe ce champ.
    pub fn set_session_id(&mut self, id: &str) {
        self.session_id = id.to_string();
    }

    /// Voir la doc de module. Appelée depuis `piste_video::brancher_video` à
    /// chaque tour où la piste vidéo dépasse son échéance, qu'une image ait
    /// été écrite ce tour-ci ou non.
    ///
    /// Émise même quand rien n'a été écrit depuis le dernier relevé — une
    /// fenêtre qui ne reçoit plus rien est précisément ce que cette ligne
    /// doit pouvoir montrer, et un compteur qui se tait à zéro serait
    /// inutile là où il sert le plus. Ce n'est PAS une trace par image :
    /// elle ne journalise qu'au pas de `PERIODE_COMPTEURS`, jamais à chaque
    /// unité écrite (comptée par `write_frame` dans `unites_video_ecrites`).
    pub(super) fn compter_la_cadence_video(&mut self) {
        if self.dernier_compte_video.elapsed() < PERIODE_COMPTEURS {
            return;
        }
        let ecoule = self.dernier_compte_video.elapsed().as_secs_f64();
        tracing::info!(
            session = %self.session_id,
            unites = self.unites_video_ecrites,
            cadence = format!("{:.1}", self.unites_video_ecrites as f64 / ecoule),
            "cadence de la piste vidéo (côté enfant)"
        );
        self.unites_video_ecrites = 0;
        self.dernier_compte_video = Instant::now();
    }
}
