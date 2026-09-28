//! The child-side counterpart of the sensor's cadence counter
//! (`capteur/fenetre.rs`): how many access units the video track actually
//! writes, so that sub-block D4's acceptance criterion can
//! set two figures against each other — how many the sensor produces, how many the child
//! writes — rather than asserting just one.
//!
//! Extracted from `piste_video.rs` (task 8, sub-block D4): that file was at
//! 466 lines, margin 34, and adding this counter exceeded it.

use std::time::{Duration, Instant};

use super::Session;

/// Period of this track's cadence lines. **Same value as
/// `capteur/fenetre.rs::PERIODE_COMPTEURS`**: it is what makes the two
/// readings comparable. **Never a per-image trace**: the project already
/// lost a whole session to a per-packet trace (18,619 lines in
/// a few seconds, on a CIFS share) — see `CLAUDE.md`.
const PERIODE_COMPTEURS: Duration = Duration::from_secs(10);

impl Session {
    /// Sets the session identifier used by the periodic cadence line
    /// (`compter_la_cadence_video`). Called once by
    /// `demarrage.rs`, right after `Session::new` — never by the tests, which
    /// need no value to check `write_frame` or
    /// `next_frame_deadline`: the field stays empty (`session=""`) on those
    /// paths, without consequence since no test observes this field.
    pub fn set_session_id(&mut self, id: &str) {
        self.session_id = id.to_string();
    }

    /// See the module doc. Called from `piste_video::brancher_video` at
    /// each round where the video track passes its deadline, whether an image
    /// was written this round or not.
    ///
    /// Emitted even when nothing was written since the last reading — a
    /// window that no longer receives anything is precisely what this line
    /// must be able to show, and a counter falling silent at zero would be
    /// useless where it matters most. It is NOT a per-image trace:
    /// it only logs at the `PERIODE_COMPTEURS` step, never at each
    /// unit written (counted by `write_frame` in `unites_video_ecrites`).
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
