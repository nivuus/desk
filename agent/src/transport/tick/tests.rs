//! Transport tick tests, split by branch family.
//!
//! Split in task 2 of sub-block D10: the file was at 489 lines
//! (margin 11) and task 11 adds audio rebuild coverage
//! to it. Extraction precedes addition — see `CLAUDE.md`.
//!
//! The shared helpers stay here; the two children's `use super::*;`
//! reach them.

mod audio;
mod reste;

use super::*;
use crate::audio::{AudioPacket, AudioSource};
use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Minimal session for tests that only exercise an isolated subsystem
/// (here, audio rebuild): test video, local IP, arbitrary clock and
/// ceiling — none of these choices is inspected by the tests that
/// use it.
pub(super) fn session_d_essai() -> Session {
    let source = Box::new(crate::transport::fixtures::video_test_source());
    Session::new(
        source,
        crate::transport::fixtures::local_ip(),
        Instant::now(),
        12_000_000,
    )
    .expect("session")
}

/// Minimal Opus packet, timestamped at the present instant — enough for
/// tests that only concern the PATH a packet takes, never
/// its content.
pub(super) fn paquet_d_essai() -> AudioPacket {
    AudioPacket {
        data: vec![0xAA],
        pts_48k: 0,
        captured_at: Instant::now(),
    }
}

/// Fake source that records calls to `set_awake` and returns a
/// prepared sleep announcement only once — as
/// `SourceDistante` really does (`Option` consumed by `take()`), without depending on the
/// sensor: this test checks the wiring of branches a1bis/a1ter
/// of `act_on_timeout`, not the logic of `SourceDistante` itself
/// (covered by `capteur/distante/tests.rs`).
struct SourceAvecSommeil {
    inner: crate::source::FileSource,
    awake_recus: std::sync::Arc<std::sync::Mutex<Vec<(bool, bool)>>>,
    sommeil_prepare: Option<(bool, String)>,
}

impl VideoSource for SourceAvecSommeil {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn set_awake(&mut self, visible: bool, focalisee: bool) -> anyhow::Result<()> {
        self.awake_recus.lock().unwrap().push((visible, focalisee));
        Ok(())
    }
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil_prepare.take()
    }
}

/// Fake source that returns a prepared budget share only once —
/// as `SourceDistante` really does (`Option` consumed by
/// `take()`), without depending on the sensor: this test checks the WIRING of
/// branch a1quater of `act_on_timeout`, not the logic of `SourceDistante`
/// itself (covered by `capteur/distante/tests.rs`) nor that of
/// `Session::appliquer_part` (covered by `transport/part.rs`).
struct SourceAvecPart {
    inner: crate::source::FileSource,
    part_preparee: Option<u32>,
}

impl VideoSource for SourceAvecPart {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part_preparee.take()
    }
}

/// Fake audio source driven from outside (`Arc<AtomicBool>`): returns
/// `capture_morte()` on command, without ever producing a packet — this test
/// only exercises DETECTION (branch a1sexies), not audio emission.
struct AudioSourceMortelle {
    morte: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AudioSource for AudioSourceMortelle {
    fn next_packet(&mut self) -> Option<AudioPacket> {
        None
    }
    fn capture_morte(&self) -> bool {
        self.morte.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Fake video source that counts calls to `signaler_audio_mort`, returns
/// a reattachment driven from outside, and records calls to
/// `signaler_audio_vivant` (sub-block D10) — same pattern as
/// `SourceAvecSommeil`/`SourceAvecPart` above: this test checks the
/// WIRING of branch a1sexies of `act_on_timeout` (and its reset
/// by a1sexies itself), not the logic of `SourceDistante`, covered by
/// `capteur/distante/tests.rs`.
struct SourceAvecAudioMort {
    inner: crate::source::FileSource,
    signalements: std::sync::Arc<std::sync::Mutex<u32>>,
    rattachement_prepare: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set to `true` by `signaler_audio_vivant`: the only way to observe
    /// a call on a `Box<dyn VideoSource>` without a downcast (D9's legacy 6).
    annonces_audio_vivant: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl VideoSource for SourceAvecAudioMort {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn signaler_audio_mort(&mut self) {
        *self.signalements.lock().unwrap() += 1;
    }
    fn signaler_audio_vivant(&mut self) {
        self.annonces_audio_vivant
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    fn rattachement_survenu(&mut self) -> bool {
        // `swap`, not a mere read: CONSUMED, on the same regime as
        // `Option::take()` in `SourceAvecSommeil`/`SourceAvecPart` — otherwise
        // this flag would stay true indefinitely and would mask the
        // defect this test exists to catch (a latch that never
        // reset would be, identically, invisible).
        self.rattachement_prepare
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}

/// Fake source that returns a prepared clipboard announcement only
/// once — as `SourceDistante` really does (`Option` consumed by
/// `take()`), without depending on the sensor: this test checks the WIRING of
/// branch a1septies of `act_on_timeout`, not the logic of `SourceDistante`,
/// covered by `capteur/distante/tests_etats.rs`.
///
/// **This test exists because the repository has already paid for its absence**: the review of
/// task 8 of sub-block D6 found that removing the whole a1quater block
/// left the tests green, the tests of `transport/part.rs` calling
/// `Session::appliquer_part` directly. P1's plan prescribed none
/// for a1septies ("`cargo test -p agent` → unchanged"); it is an
/// assumed divergence, and in the direction this file already documents.
struct SourceAvecPressePapier {
    inner: crate::source::FileSource,
    presse_papier_prepare: Option<(Option<String>, u32)>,
}

impl VideoSource for SourceAvecPressePapier {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn presse_papier_a_annoncer(&mut self) -> Option<(Option<String>, u32)> {
        self.presse_papier_prepare.take()
    }
}

/// Fake source that returns a prepared accent colour only once — as
/// `SourceDistante` really does (`Option` consumed by `take()`), without
/// depending on the sensor: these tests check the WIRING of branch a1nonies
/// of `act_on_timeout`, not the logic of `SourceDistante`.
///
/// **It exists for the reason the neighbour above documents**: the review
/// of task 8 of sub-block D6 found that removing the whole a1quater block
/// left the tests green. Link 6 of A1's plan is explicitly
/// "NOT guarded — a forgotten `if let` compiles", and it is that hole these
/// two tests close.
struct SourceAvecAccent {
    inner: crate::source::FileSource,
    accent_prepare: Option<String>,
}

impl VideoSource for SourceAvecAccent {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn accent_a_annoncer(&mut self) -> Option<String> {
        self.accent_prepare.take()
    }
}
