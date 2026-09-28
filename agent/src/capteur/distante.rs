//! `SourceDistante` — a child's video source, fed by the sensor.
//!
//! **No `#[cfg(windows)]`**: the real pipe is gated (`capteur/tube.rs`),
//! but the decision whether or not to close a session lives here, and it is the
//! piece that costs most to get wrong. It is therefore written against an injected
//! `Canal` and a `Receiver`, both trivial to simulate on the host.

use std::sync::mpsc::Receiver;

use anyhow::{bail, Result};

use crate::capteur::protocole::{DepuisCapteur, VersCapteur};
use crate::capteur::reprise::FenetreCanal;
use crate::h264::AccessUnit;

/// What the child can ask of the sensor, and the means to re-attach to it
/// when the channel breaks.
pub trait Canal {
    fn commander(&mut self, message: VersCapteur) -> Result<DepuisCapteur>;
    /// Reopens a channel to the sensor and re-attaches to it. The implementation
    /// replaces its own internal write state; it returns the new frame
    /// queue and the dimensions announced at attach time.
    fn rattacher(&mut self) -> Result<Rattachee>;
}

/// The fruit of a successful re-attachment.
pub struct Rattachee {
    pub images: Receiver<Recu>,
    pub largeur: u32,
    pub hauteur: u32,
}

/// What the sensor pushes, unsolicited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    Image(AccessUnit),
    Etat {
        vivante: bool,
        epuisee: bool,
        largeur: u32,
        hauteur: u32,
    },
    /// Sleep change pushed by the sensor, unsolicited. Held by
    /// `SourceDistante::sommeil` until `sommeil_a_annoncer`
    /// consumes it.
    Sommeil {
        endormie: bool,
        raison: String,
    },
    /// Share of the bitrate budget granted by the sensor, pushed
    /// unsolicited. Held by `SourceDistante::part` until
    /// `part_a_appliquer` consumes it.
    Part {
        bps: u32,
    },
    /// Audio order pushed by the sensor, unsolicited. Held by
    /// `SourceDistante::audio` until `audio_a_appliquer` consumes it.
    Audio {
        actif: bool,
    },
    /// Fullscreen change pushed by the sensor, unsolicited. Held
    /// by `SourceDistante::plein_ecran` until
    /// `plein_ecran_a_annoncer` consumes it.
    PleinEcran {
        actif: bool,
    },
    /// The VM's clipboard changed (sub-block P1). Pushed
    /// unsolicited, **on change only**: it is the sensor that holds
    /// the clipboard and polls its sequence number.
    ///
    /// `texte` is `None` on a size REFUSAL (beyond
    /// `presse_papier::PRESSE_PAPIER_MAX`) — the content is refused, never
    /// truncated —, and `octets` then carries the refused size, so that the
    /// browser's banner can state it. Held by
    /// `SourceDistante::presse_papier` until
    /// `presse_papier_a_annoncer` consumes it.
    PressePapier {
        texte: Option<String>,
        octets: u32,
    },
    /// The Windows window's accent colour, pushed by the sensor on
    /// change — **first reading included**. Held in
    /// `SourceDistante::accent` until `accent_a_annoncer` consumes it.
    Accent {
        couleur: String,
    },
}

pub struct SourceDistante {
    canal: Box<dyn Canal + Send>,
    images: Receiver<Recu>,
    /// 🔴 **THE FRAME SIZE, AND IT IS SHARED — see
    /// [`crate::entrees::FrameSize`].** It was only a pair of `u32`
    /// until batch 32T; the input injector needed it, and the only
    /// way NOT to have two descriptions of the same rectangle is to
    /// have only one storage. `dimensions()` re-reads it, the injector re-reads it.
    ///
    /// ⚠️ **The three writes are those of `video_source.rs`** (attach,
    /// `Etat`, `Size`), plus the initial value. Adding a fourth
    /// elsewhere, without going through here, would reintroduce exactly the defect of
    /// batch 32M.
    size: std::sync::Arc<crate::entrees::FrameSize>,
    vivante: bool,
    epuisee: bool,
    /// Channel break in progress. A break does not exhaust the source as long as
    /// this window has not expired: that is what makes sessions survive
    /// a sensor restart.
    fenetre: FenetreCanal,
    /// Last sleep change received from the sensor, waiting to be
    /// announced to the browser. Consumed by `sommeil_a_annoncer`.
    ///
    /// **Current state, not a history**: two `Sommeil` received before
    /// a read happens overwrite each other, only the last survives — same
    /// regime as `Etat` just above, whose fields also overwrite each other
    /// without accumulating.
    sommeil: Option<(bool, String)>,
    /// Last bitrate budget share received from the sensor, waiting to be
    /// applied. Consumed by `part_a_appliquer`. Same overwrite
    /// regime as `sommeil`.
    part: Option<u32>,
    /// Last audio order received from the sensor, waiting to be applied. Consumed
    /// by `audio_a_appliquer`. Same overwrite regime as `part`.
    ///
    /// ⚠️ **`None` at birth, and it does not mean "no order" but "nothing
    /// to change"**: the child is born SILENT (see `demarrage.rs`), and the sensor
    /// sends it its first order at attach time. Starting from an implicit `Some(true)`
    /// would make both windows of the same process carry the sound
    /// during the milliseconds before the first arbitration.
    audio: Option<bool>,
    /// Last fullscreen change received from the sensor, waiting to be
    /// announced to the browser. Consumed by `plein_ecran_a_annoncer`.
    ///
    /// **Same overwrite regime as `sommeil`**: two changes arriving
    /// between two reads overwrite each other, only the last survives. There is no
    /// twin current state here, unlike `sommeil`/`endormie`: nothing
    /// in the child needs to re-read the fullscreen state outside the announcement.
    plein_ecran: Option<bool>,
    /// Last clipboard received from the sensor, waiting to be announced to the
    /// browser. Consumed by `presse_papier_a_annoncer`.
    ///
    /// **Same overwrite regime as `plein_ecran`**: two copies arriving
    /// between two reads overwrite each other, only the last survives. That is correct
    /// — the clipboard IS a state, not a history, and the browser
    /// would have nothing to do with a copy the user has already replaced.
    presse_papier: Option<(Option<String>, u32)>,
    /// Last accent colour received from the sensor, waiting to be announced
    /// to the browser. Consumed by `accent_a_annoncer`.
    ///
    /// **Same overwrite regime as `presse_papier`**: two changes
    /// arriving between two reads overwrite each other, only the last survives. That is
    /// correct — the accent IS a state, not a history, and the browser
    /// would have nothing to do with a tint the icon has already replaced.
    accent: Option<String>,
    /// CURRENT sleep state, as the sensor describes it.
    ///
    /// **Distinct from `sommeil` just above, and not redundant with it**:
    /// that one is the announcement to make to the browser, returned only once;
    /// this one is the state, re-read at every share applied by
    /// `Session::appliquer_part` — which must not propagate a sleeping window's
    /// floor to the congestion controller. Both are set at the same
    /// place, on the same message; only their lifetime differs.
    ///
    /// ⚠️ **True at birth, and it is not a cautious choice but a
    /// fact**: since sub-block D5 a window is born ASLEEP on the sensor side
    /// (`Fenetre::ouvrir` no longer builds a `WindowsSource`, see its doc),
    /// and no `Sommeil { endormie: true }` is ever pushed for this
    /// birth — there is no transition to announce. The very first
    /// share received, sent by `sommeil::inscrire` at attach time, is therefore the
    /// `PART_DORMANTE_BPS` floor. Starting from `false` would make it apply
    /// as an encoding ceiling, precisely the defect this field exists
    /// to avoid.
    endormie: bool,
    /// True only once, right after a successful re-attachment. Consumed by
    /// `rattachement_survenu`, on the same regime as `sommeil`/`part`/
    /// `audio`/`plein_ecran`: it is what lets `Session` reset
    /// `audio_mort_signale` — a restarted sensor has lost the memory of
    /// any earlier report.
    rattache: bool,
}

impl SourceDistante {
    pub fn new(
        canal: Box<dyn Canal + Send>,
        images: Receiver<Recu>,
        largeur: u32,
        hauteur: u32,
    ) -> Self {
        Self {
            canal,
            images,
            size: std::sync::Arc::new(crate::entrees::FrameSize::new(largeur, hauteur)),
            vivante: true,
            epuisee: false,
            fenetre: FenetreCanal::new(),
            sommeil: None,
            part: None,
            audio: None,
            plein_ecran: None,
            presse_papier: None,
            accent: None,
            endormie: true,
            rattache: false,
        }
    }

    /// The size cell, to hand to the input injector.
    ///
    /// 🔴 **An `Arc` CLONE, never a copy of the value**: it is the
    /// difference between "the same rectangle" and "two rectangles that looked
    /// alike at start-up".
    pub fn shared_size(&self) -> std::sync::Arc<crate::entrees::FrameSize> {
        std::sync::Arc::clone(&self.size)
    }

    /// Sends a command and accepts only `Fait` as success.
    fn commander_simple(&mut self, message: VersCapteur) -> Result<()> {
        match self.canal.commander(message)? {
            DepuisCapteur::Fait => Ok(()),
            DepuisCapteur::Error { motif } => bail!("the sensor refused: {motif}"),
            autre => bail!("unexpected answer from the sensor: {autre:?}"),
        }
    }

    /// Ages the resumption window, for tests only: without it,
    /// testing the expiry would require actually waiting 15 seconds.
    #[cfg(test)]
    pub fn vieillir_pour_test(&mut self, ecart: std::time::Duration) {
        self.fenetre.vieillir_pour_test(ecart);
    }
}

// The `VideoSource` implementation lives in a sibling file: this file
// was at 472 lines for a project cap of 500, and sub-block A1 had to
// graft the `PressePapier` pattern onto it, which weighs some thirty lines.
// The extraction is done BEFORE the addition that makes it necessary, never after
// — and never by compression. See the header of `distante/video_source.rs`.
#[path = "distante/video_source.rs"]
mod video_source;

// The tests live in a sibling file: this file was at 487 lines
// for a project cap of 500. See the header of `distante/tests.rs`.
#[cfg(test)]
mod tests;

// The tests of the STATES pushed by the sensor (visibility, sleep, share,
// audio, fullscreen, clipboard) live in a THIRD file:
// `distante/tests.rs` was at 474 lines for the same cap of 500, and the
// clipboard test (sub-block P1, task 10) would have eaten into the margin.
// See the header of `distante/tests_etats.rs`.
#[cfg(test)]
#[path = "distante/tests_etats.rs"]
mod tests_etats;
