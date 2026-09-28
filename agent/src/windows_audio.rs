//! Windows audio source: loopback capture, framing, Opus encoding.
//!
//! Everything happens on a dedicated thread, which deposits into a bounded ring buffer.
//! The transport loop only does a non-blocking pop per round there: it
//! must never wait for WASAPI or the encoder.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::audio::{
    temporisation_de_reprise, AudioPacket, AudioSource, PacketRing, LECTURES_ECHOUEES_MAX,
};
use crate::frames::FrameAssembler;
use crate::opus::OpusEncoder;
use crate::wasapi::process_loopback::CaptureProcessus;
use crate::wasapi::LoopbackCapture;

// The body of the capture thread (task 3 of sub-block D10): extracted on the
// production side, to stay under the project's 500-line ceiling and before
// the addition of task 13 (`AUDIO_FAUTE_LECTURE`) that would otherwise have
// made it cross. Same scheme as `superviseur/boucle/creation_sortie.rs` and
// `capteur/serveur/instances.rs`.
mod fil;
use fil::tourner;

/// Depth of the shared buffer, in 10 ms packets. 10 packets = 100 ms:
/// enough to absorb a late loop round, too little for
/// latency to settle in.
const RING_CAPACITY: usize = 10;

/// Polling interval of the capture thread. Twice as fast as the
/// frame cadence: capture must never be the limiting factor.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Interval between two aggregated counter logs.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// What the capture thread reads, depending on the mode.
///
/// **Two paths, a single thread.** Global mode (`Session`) is the one from before
/// D7 and serves the single-window case — an agent launched by hand, without
/// `FENETRE_HWND`. `Processus` mode is the multi-window one.
enum Capture {
    Session(LoopbackCapture),
    Processus(CaptureProcessus),
}

impl Capture {
    fn read(&mut self) -> Result<Option<Vec<i16>>> {
        match self {
            Capture::Session(c) => c.read(),
            Capture::Processus(c) => c.read(),
        }
    }

    fn description(&self) -> String {
        match self {
            Capture::Session(c) => c.description().to_string(),
            Capture::Processus(c) => c.description().to_string(),
        }
    }

    /// Starts or stops the stream. **No effect in session mode** on the
    /// WASAPI stream itself: the global loopback is never started/stopped by
    /// this method — a single-window agent always carries its sound.
    ///
    /// ⚠️ This immunity does NOT extend to the calling thread: it is the thread that gates
    /// reading/encoding/depositing on the value of `emettait` (see `start`),
    /// not this method. A `set_actif(false)` reaching a source in
    /// `Session` mode would therefore make it just as silent as a source in
    /// `Processus` mode — ~~only `new()` (which never exposes
    /// the `Arc<AtomicBool>` to an external order) makes this case unreachable
    /// today~~.
    ///
    /// ❌ **THIS CASE BECAME REACHABLE IN SUB-BLOCK D10** (cross-cutting
    /// review): `Session::reconstruire_ou_signaler` applies
    /// `set_actif(self.audio_porteuse)` **unconditionally** to any rebuilt
    /// source, and the rebuilder of `demarrage/audio.rs` goes through
    /// `WindowsAudioSource::new` — hence through `Capture::Session` — when
    /// `config.fenetre_hwnd` is `None`. An external order therefore does reach
    /// a source in session mode.
    ///
    /// ✅ **"And it silences it" was written here, and it has been WRONG since
    /// sub-block D11** (cross-cutting review): `audio_porteuse` is now
    /// `true` in single-window mode, set by `set_audio_porteuse` at wiring
    /// (`demarrage/audio.rs`), and the re-arming RE-ENABLES this source instead
    /// of silencing it. **Reachability stays true, its consequence no
    /// longer does.** See `Session::reconstruire_ou_signaler`.
    fn emettre(&mut self, actif: bool) -> Result<()> {
        match self {
            Capture::Session(_) => Ok(()),
            Capture::Processus(c) => {
                if actif {
                    c.start()
                } else {
                    c.arreter()
                }
            }
        }
    }
}

pub struct WindowsAudioSource {
    ring: PacketRing,
    arret: Arc<AtomicBool>,
    description: String,
    /// Loss rate wanted by the congestion controller, as a percentage.
    /// Read by the capture thread before each encoding — see its comment
    /// in `new` for the reason for this indirection: the `OpusEncoder`
    /// itself is moved into that thread and is therefore not accessible here.
    perte_desiree: Arc<AtomicI32>,
    /// Emission order wanted by the sensor's arbitration, read by the capture
    /// thread before each round. Same indirection pattern as
    /// `perte_desiree`: the capture lives on the thread, not here.
    ///
    /// **False at birth.** The child is born SILENT and only emits on the
    /// sensor's order — same doctrine as `SourceDistante::endormie`, which is born at
    /// `true`. That is what avoids two windows of the same process being
    /// both audible during the milliseconds preceding the first
    /// arbitration.
    ///
    /// ⚠️ **"At birth" means at EACH call of `start` — hence
    /// also at each REBUILD**, not only at the initial opening
    /// (defect found during VM acceptance, sub-block D10: "audio capture
    /// rebuilt" = 2, `compteurs_audio_actif_true` = 0 in both
    /// runs). `pour_processus` never enables its own emission, unlike
    /// `new()` (session mode), which enables itself
    /// right after construction.
    ///
    /// ❌ **"The only path a rebuilder takes" was written here,
    /// and it is wrong: `demarrage/audio.rs` sets one in BOTH modes.**
    /// The consequence — in single-window mode the re-arming REMOVED the sound
    /// `new()` had just given, for lack of a sensor order to set
    /// `audio_porteuse` — is documented next to
    /// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`).
    /// ✅ **"And left as legacy": no longer since sub-block D11** (legacy 4, measured in
    /// VM acceptance — 441 Hz received against the sentinel on the red arm).
    /// `demarrage/audio.rs::brancher` sets `audio_porteuse = true` in its
    /// single-window branch only. It is
    /// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`) that
    /// now re-arms a rebuilt source, on `audio_porteuse` — otherwise
    /// a capture rebuilt for a carrier window stayed
    /// silent forever (no packet → no proof → no
    /// re-election → silent, an ABSORBING state).
    emet: Arc<AtomicBool>,
    /// Captured PID. `None` in session mode. Exposed by `pid()`, in the opening
    /// log (`demarrage/audio.rs`) — it is `pid_fil`, a local copy
    /// taken before this parameter is moved into the capture thread, that
    /// feeds the periodic "audio counters" trace.
    pid: Option<u32>,
    /// Set by the capture thread just before it ends for good,
    /// and never cleared — its death is final. Read by `appliquer_audio`,
    /// which would otherwise announce `actif=true` for a window that will never
    /// produce a packet again (F3, final branch review of sub-block D7; see
    /// `AudioSource::capture_morte`).
    capture_morte: Arc<AtomicBool>,
}

impl WindowsAudioSource {
    /// Opens the session's GLOBAL loopback and starts the production thread.
    ///
    /// Single-window mode: an agent launched by hand, without `FENETRE_HWND`. The
    /// sound is carried without arbitration — there is nobody to share it with.
    ///
    /// `origin` is the **session's** clock origin, shared with the
    /// video source: it is what makes both timelines
    /// comparable, hence A/V sync exact.
    pub fn new(origin: Instant) -> Result<Self> {
        let capture = LoopbackCapture::open().context("opening the audio loopback")?;
        let source = Self::start(Capture::Session(capture), origin, None)?;
        // No sensor will ever send an order to this agent: it emits straight away.
        source.emettre(true);
        Ok(source)
    }

    /// Opens the loopback of PROCESS `pid` and its tree, and starts the production
    /// thread.
    ///
    /// **The source is born SILENT**: it is the sensor that decides who carries the
    /// sound, and its first order arrives right at attach time. See the `emet` field.
    pub fn pour_processus(pid: u32, origin: Instant) -> Result<Self> {
        let capture = CaptureProcessus::ouvrir(pid)
            .with_context(|| format!("opening the process loopback of PID {pid}"))?;
        Self::start(Capture::Processus(capture), origin, Some(pid))
    }

    /// Body common to both constructors: starts the production thread from
    /// an already opened capture, whatever its mode.
    fn start(capture: Capture, origin: Instant, pid: Option<u32>) -> Result<Self> {
        let description = capture.description();
        let encodeur = OpusEncoder::new().context("creating the Opus encoder")?;

        let ring = PacketRing::new(RING_CAPACITY);
        let arret = Arc::new(AtomicBool::new(false));
        // Bridge between `AudioSource::set_packet_loss_perc` (called from the
        // transport loop) and the Opus encoder, which lives on the capture
        // thread and is therefore only accessible from it. A `Mutex` around
        // the encoder would be taken at each 10 ms frame on this hot
        // path; an atomic integer read once per frame costs nothing.
        let perte_desiree = Arc::new(AtomicI32::new(0));
        // Same pattern: see the doc of the `emet` field.
        let emet = Arc::new(AtomicBool::new(false));
        // Same pattern again: see the doc of the `capture_morte` field.
        let capture_morte = Arc::new(AtomicBool::new(false));

        let ring_fil = ring.clone();
        let arret_fil = Arc::clone(&arret);
        let perte_desiree_fil = Arc::clone(&perte_desiree);
        let emet_fil = Arc::clone(&emet);
        let capture_morte_fil = Arc::clone(&capture_morte);
        // `Option<u32>` is `Copy`: this local copy is the one the thread
        // takes, independently of the `pid` field of `Self` built below.
        let pid_fil = pid;
        std::thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                tourner(
                    capture,
                    encodeur,
                    origin,
                    fil::PartageFil {
                        ring_fil,
                        arret_fil,
                        perte_desiree_fil,
                        emet_fil,
                        capture_morte_fil,
                    },
                    pid_fil,
                )
            })
            .context("starting the audio capture thread")?;

        tracing::info!(format = %description, "audio source started");
        Ok(Self {
            ring,
            arret,
            description,
            perte_desiree,
            emet,
            pid,
            capture_morte,
        })
    }

    /// Mix format obtained, for the log.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Captured PID, for the opening log. `None` in session mode.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Carries the sound, or stays silent. Called from the transport loop, which
    /// consumes the sensor's order.
    pub fn emettre(&self, actif: bool) {
        self.emet.store(actif, Ordering::Relaxed);
    }
}

impl AudioSource for WindowsAudioSource {
    fn next_packet(&mut self) -> Option<AudioPacket> {
        self.ring.pop()
    }

    fn set_packet_loss_perc(&mut self, perc: i32) -> Result<()> {
        // Only writes: it is the capture thread that reads this value and
        // relays to `OpusEncoder::set_packet_loss_perc`, sole holder of
        // the encoder (see the comment of the `perte_desiree` field).
        self.perte_desiree.store(perc, Ordering::Relaxed);
        Ok(())
    }

    fn set_actif(&mut self, actif: bool) {
        self.emettre(actif);
    }

    fn capture_morte(&self) -> bool {
        self.capture_morte.load(Ordering::Relaxed)
    }
}

impl Drop for WindowsAudioSource {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}
