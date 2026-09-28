//! Video source combining window capture and hardware encoding.

#![cfg(windows)]

use anyhow::Result;
use windows::Win32::Foundation::HWND;

use crate::capture::DesktopCapture;
use crate::encode::H264Encoder;
use crate::geometry::{crop_region, Rect};
use crate::h264::{AccessUnit, CLOCK_RATE_HZ};
use crate::source::VideoSource;
use crate::window;
use crate::windows_source_sortie::ModeCapture;
// `Telemetrie` (D9, task 11): per-SESSION capture counters, same
// SIBLING setup as `windows_source_sortie` above — see
// `windows_source/telemetrie.rs`.
use crate::windows_source_telemetrie::Telemetrie;
// `WindowsSource::sur_sortie` lives in `windows_source/sortie.rs` (SIBLING
// module, declared `#[path]` in `main.rs` under the name `windows_source_sortie`
// so that its pure region computation and its `ModeCapture` stay testable on
// the Linux host); `WindowsSource::resize` lives in the CHILD module
// `windows_source/redimensionnement.rs`, declared below. The difference
// is not cosmetic: a child module sees the private fields of this
// module, a sibling module does not — see the fields' comment.

/// Resizing the window and rebuilding the encoding chain.
///
/// **Child** module of `windows_source` (and not a sibling) precisely so that
/// `resize` keeps reading and writing `WindowsSource`'s private fields
/// without any having to be opened up. Extracted from here because this file is
/// in size debt (see `CLAUDE.md`) and fix C1 added
/// `depuis_pieces` and the `mode` field to it.
mod redimensionnement;

/// Changing only the encoding size, capture and window unchanged.
///
/// **Child** module for the same reason as `redimensionnement` above:
/// it reads and writes `WindowsSource`'s private fields. Extracted from here in
/// sub-block D5, the remedy for defect C2 (destroy the encoder before
/// building a new one) adding some fifteen lines to it that this file, in
/// frozen size debt, could not absorb.
mod encodage;

// `impl VideoSource` and the bounded retry of the encoder's very first
// start, split out to stay under 500 lines.
mod source_video;

pub struct WindowsSource {
    // PRIVATE fields, and it is a design invariant, not a detail:
    // several of them (`capture`, `fatal`, `width`/`height` against
    // `encoder.encode_size()`) are only correct taken together, and their
    // comments say how. The only writers are therefore this module and
    // its child `redimensionnement`; the sibling module
    // `windows_source_sortie`, for its part, goes through `depuis_pieces` (`pub(crate)`)
    // and touches none of them.
    hwnd: HWND,
    /// `None` only transiently, inside `resize` (see
    /// its comment): DXGI only allows a single live instance
    /// of `IDXGIOutputDuplication` per output and per process at a time, so
    /// the old capture must be explicitly released before
    /// `DesktopCapture::new()` calls `DuplicateOutput` again.
    ///
    /// **Fix round 1 (review):** a first version of this
    /// fix emptied this field then attempted the rebuild through `?` — if
    /// that failed (GPU transiently unavailable, window moved
    /// off screen during the gesture...), the field stayed `None` for good,
    /// and the next call to `next_frame` panicked on
    /// `capture.as_mut().expect(...)`. A panic crosses `spawn_blocking` and
    /// ends the whole agent process — exactly what the brief asks
    /// never to do for a resize failure that is supposed to be
    /// tolerated. `resize` now guarantees that a failure falls back on a
    /// backup capture (see `rebuild::rebuild_or_recover`) rather than
    /// leaving this field empty; the only case where it stays `None` after `resize`
    /// is the double failure (neither the full rebuild, nor the backup),
    /// in which case `fatal` becomes true and `next_frame` short-circuits BEFORE
    /// touching this field (see its guard at the head of the function) — never
    /// a panic, including in this worst case.
    capture: Option<DesktopCapture>,
    /// Screen region to crop, recomputed at each resize.
    region: Rect,
    /// `None` only transiently, inside
    /// `set_encode_size` (child module `encodage`), which must destroy
    /// the current encoder BEFORE building a new one — see its
    /// comment for the why and for the price. As for `capture`
    /// above, the only state where this field stays empty after returning is the one
    /// where `fatal` is true: readers therefore go through `encoder_mut`,
    /// which returns an ERROR and never a panic — two of them
    /// (`request_keyframe`, `set_bitrate`) are called from the transport
    /// loop without a `fatal` guard, and a panic there would cross
    /// `spawn_blocking`.
    encoder: Option<H264Encoder>,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    /// The **session's** clock origin, imposed by `demarrage.rs` and shared
    /// with the audio source. It is this common origin that makes both
    /// timelines comparable, hence A/V sync exact. Creating it here
    /// would shift it by the other source's initialisation duration.
    ///
    /// Never reset, including when `resize` rebuilds the encoding
    /// chain: the browser's decoder would reject a timestamp that
    /// goes backwards.
    clock_origin: std::time::Instant,
    /// Last timestamp assigned, to guarantee strict monotonicity even
    /// if two captures fell into the same 1/90000 s tick.
    last_pts_90k: Option<u64>,
    /// True after an unrecoverable error (capture or encoder): makes the
    /// source permanently exhausted (see `is_exhausted`), independently
    /// of the window's state. A vanished window (`!is_alive()`) is
    /// the other exhaustion case; this one covers failures that do not
    /// necessarily affect the window itself (lost GPU device...).
    fatal: bool,
    /// True as soon as `self.encoder` has returned its very first output. Used
    /// only to bound `SUBMIT_POLL_BUDGET` (see its doc) to the startup
    /// phase: reset to false by `resize`, which rebuilds a new encoder
    /// that has not produced anything yet either.
    encoder_warmed_up: bool,
    /// Access units already retrieved from the encoder but not yet returned to
    /// the caller: `VideoSource::next_frame` only returns one per round, whereas
    /// draining can bring out several (see `drain_ready_output`).
    /// Never purged by `resize`: those units are valid and already
    /// timestamped, throwing them away would only punch holes in the video.
    ready: std::collections::VecDeque<AccessUnit>,
    /// What this source captures — desktop cropped to the window, or a whole DXGI
    /// output. **Only `resize` consults it**, and that is its reason for being:
    /// see `ModeCapture` (`windows_source/sortie.rs`) for what its absence
    /// produced.
    mode: ModeCapture,
    /// Per-session capture counters (D9, task 11), replacing the
    /// `TICKS`/`CAPTURED`/`PRODUCED` statics. `pub(crate)`: read from
    /// `capteur/fenetre.rs`, a sibling and not a child module of this one.
    pub(crate) telemetrie: Telemetrie,
}

// SAFETY: the COM types wrapped here (`HWND`, `ID3D11Device`,
// `IMFTransform`...) are not `Send` by default in windows-rs, but
// `Session` (see `transport.rs`) requires `Box<dyn VideoSource + Send>` to
// end up on the dedicated thread of `Session::run` (`tokio::task::spawn_blocking`,
// see `demarrage.rs`). This transfer is NOT a single literal move:
// between its construction and this hand-over to `spawn_blocking`, the object is
// carried by a `#[tokio::main]` task (multi-threaded scheduler by default)
// which crosses several `.await`s (receiving the offer, sending the
// answer...), and can therefore be resumed on a different worker thread at
// each of them before reaching the dedicated blocking thread. What makes `Send`
// safe is not a count of moves, but the absence of CONCURRENT
// access: at any instant, a single thread at a time owns the object, whichever
// it is, and none touches it any more once handed to `spawn_blocking`.
// The D3D11 device is explicitly protected for multi-threaded access
// (`SetMultithreadProtected(TRUE)`, set in `DesktopCapture::new`, see its
// comment) precisely because it is also used by Media Foundation's internal
// threads; the MF objects themselves are documented as
// agile (usable from any thread).
unsafe impl Send for WindowsSource {}

impl WindowsSource {
    pub fn new(
        hwnd: HWND,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let window_rect = window::client_rect_on_screen(hwnd)?;

        let capture = DesktopCapture::new()?;
        let (dw, dh) = capture.desktop_size();
        // `crop_region` already aligns dimensions on even values,
        // required by the H.264 encoder.
        let region = crop_region(window_rect, dw, dh)
            .ok_or_else(|| anyhow::anyhow!("the window is off screen"))?;
        let (width, height) = (region.width, region.height);

        let mut encoder = H264Encoder::new(
            capture.device(),
            (width, height),
            (width, height),
            fps,
            bitrate,
        )?;
        encoder.request_keyframe()?;

        Ok(Self::depuis_pieces(
            hwnd,
            capture,
            encoder,
            region,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            ModeCapture::FenetreRecadree,
        ))
    }

    // `sur_sortie` (sub-block D1's mode: one window per virtual output;
    // ⚠️ "nothing left to crop" was written here and D10 refuted it — see
    // the header of `windows_source/sortie.rs`) is defined in a second
    // `impl WindowsSource`, located in
    // `windows_source/sortie.rs` under `#[cfg(windows)]`. Moved out of here during
    // review so that this file — already in size debt (see `CLAUDE.md`) —
    // only carries the wiring; it ends, like `new` above, with a
    // call to `Self::depuis_pieces`, resolved by inherent method lookup
    // across the whole crate, independently of the file that defines it.

    /// Final assembly, shared by both constructors (`new` above and
    /// `sur_sortie`, in `windows_source/sortie.rs`).
    ///
    /// Extracted so that "desktop capture + window cropping" and
    /// "capture of a whole output" do not diverge on initialising
    /// the fields — they only differ by the way of obtaining the capture,
    /// the encoder, the region, and by `mode`.
    ///
    /// **Stays HERE, in the module that declares `WindowsSource`**, and it is the
    /// fix from a review: having moved it into the sibling module
    /// `windows_source_sortie` forced opening the thirteen fields as
    /// `pub(crate)` for the `Self { … }` literal to compile there. `pub(crate)`
    /// on this single function is enough for the cross-module call and exposes
    /// no field.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn depuis_pieces(
        hwnd: HWND,
        capture: DesktopCapture,
        encoder: H264Encoder,
        region: Rect,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
        mode: ModeCapture,
    ) -> Self {
        Self {
            hwnd,
            capture: Some(capture),
            region,
            encoder: Some(encoder),
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            last_pts_90k: None,
            fatal: false,
            encoder_warmed_up: false,
            ready: std::collections::VecDeque::new(),
            mode,
            telemetrie: Telemetrie::default(),
        }
    }

    /// Current capture, mutable. The only caller (`next_frame`) keeps the
    /// `if self.fatal { return None; }` guard before any call: this field
    /// is only `None` during `resize`, and `resize` never gives control back
    /// with `capture` at `None` without also having set `fatal` to true (see the
    /// field's comment). So it only panics on a real bug in this
    /// invariant, never in normal use.
    fn capture_mut(&mut self) -> &mut DesktopCapture {
        self.capture
            .as_mut()
            .expect("capture always present when fatal is false")
    }

    /// Current encoder, mutable — or an **error**, never a panic.
    ///
    /// The difference with `capture_mut` above is deliberate: `capture`
    /// is only read by `next_frame`, behind its `if self.fatal` guard, which
    /// makes its `expect` unreachable. The encoder, for its part, is also read by
    /// `request_keyframe` and `set_bitrate`, which the transport loop calls
    /// on a browser event **without consulting `is_exhausted`**: between
    /// the failure of `set_encode_size` and the session closing, such a call
    /// is possible, and a panic there would take down the whole process — hence
    /// all the other windows. Both know what to do with an `Err`.
    fn encoder_mut(&mut self) -> Result<&mut H264Encoder> {
        self.encoder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("source exhausted: encoder destroyed"))
    }

    /// True as long as the captured window exists.
    pub fn is_alive(&self) -> bool {
        window::is_window_alive(self.hwnd)
    }

    /// Requests a keyframe — notably at the browser's request, relayed
    /// from `Event::KeyframeRequest` by `transport/evenements.rs` through the
    /// `VideoSource::request_keyframe` implementation below.
    pub fn request_keyframe(&mut self) -> Result<()> {
        self.encoder_mut()?.request_keyframe()
    }

    // `set_encode_size` (changing only the encoding size) lives in the
    // child module `encodage.rs` — see its declaration at the head of the file.

    /// Removes from the pipeline everything that is ready, without ever waiting, and returns
    /// the oldest access unit still queued.
    ///
    /// Three things in the same round, and that is the point: (1) empty the
    /// outputs already produced, (2) give back to the encoder the inputs this
    /// drain has just allowed it to accept, (3) return only one unit
    /// to the caller, the others waiting for the following rounds.
    ///
    /// Without (1) and (2) in the same round, the encoder's full cycle
    /// (submission → production → retrieval → new input request)
    /// only crossed one step per round of `Session::run`: measured at
    /// `need_input_hz=23` for `ticks_hz=60` and `captured_hz=46`, that is a
    /// throughput divided by ~2.6 whereas capture and encoder both
    /// had the necessary margin (`desktop_updates_hz=68`, `ENCODE_TEST` at
    /// 66 fps). See `H264Encoder::flush_pending_inputs`.
    ///
    /// Draining is bounded: an encoder that returned output endlessly
    /// must not be able to hold the transport loop, which also has ICE,
    /// RTCP and data channels to serve.
    fn drain_ready_output(&mut self) -> Option<AccessUnit> {
        let t_drain = std::time::Instant::now();
        let unit = self.drain_ready_output_inner();
        DRAIN_NS.fetch_add(
            t_drain.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        unit
    }

    /// The drain proper; separated from the timing above so that
    /// all exit paths (including error `break`s) are measured.
    fn drain_ready_output_inner(&mut self) -> Option<AccessUnit> {
        const MAX_DRAIN: usize = 8;
        for _ in 0..MAX_DRAIN {
            // `None`: encoder destroyed by a failed `set_encode_size` (see
            // `encoder_mut`). Nothing more to drain, but what is already
            // queued is still good to return.
            match self.encoder.as_mut().map(H264Encoder::poll_output) {
                Some(Ok(Some(unit))) => {
                    self.encoder_warmed_up = true;
                    self.telemetrie.produite();
                    self.ready.push_back(unit);
                }
                Some(Ok(None)) | None => break,
                Some(Err(e)) => {
                    tracing::warn!(error = %crate::cause::chain(&e), "retrieving the encoded frame failed");
                    break;
                }
            }
        }
        // The input slots freed by the drain above are
        // reusable right now: do not wait for the next round.
        if let Err(e) = self
            .encoder_mut()
            .and_then(H264Encoder::flush_pending_inputs)
        {
            tracing::warn!(error = %crate::cause::chain(&e), "refeeding the encoder failed");
        }
        self.ready.pop_front()
    }

    /// Presentation timestamp of the frame just captured, read from
    /// a real clock.
    ///
    /// **Latency fix (07/28).** The previous version counted
    /// frames rather than time (`next_pts_90k += CLOCK_RATE_HZ / fps`
    /// at each successful submission), which assumes a source with a perfectly
    /// regular cadence. This one is not and cannot be:
    /// Desktop Duplication only returns a frame when the desktop changes,
    /// so submissions are spaced 16.7 ms, 33 ms, or
    /// several seconds apart on a still screen — whereas the counter
    /// invariably advanced by 16.7 ms.
    ///
    /// The RTP timeline therefore drifted from real time without ever
    /// realigning: at a real 30 fps it advanced twice too slowly, and
    /// after a still-screen pause it restarted as if that pause
    /// had not happened. The WebRTC receiver computes its jitter from the gap
    /// between arrival spacing and the spacing announced by the
    /// timestamps: a systematically positive offset makes it inflate
    /// its buffer target, frame after frame, until a brutal
    /// resynchronisation. It is the signature noted during acceptance
    /// (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`, criterion 3):
    /// 641.8 → 877.6 → 1164.0 → 1449.9 ms of latency, then a clean return to
    /// 83.0 ms — without any freeze being detected, which already ruled out a
    /// capture or encoding stop.
    ///
    /// Reading the clock at capture gives the browser the timeline
    /// it expects, whatever the source's regularity.
    fn next_pts_90k(&mut self) -> u64 {
        let elapsed = self.clock_origin.elapsed();
        // Nanoseconds → 1/90000 s, in 128 bits: `as_nanos() * 90_000`
        // would overflow a `u64` after about 57 hours of session.
        let pts = (elapsed.as_nanos() * CLOCK_RATE_HZ as u128 / 1_000_000_000) as u64;
        // Two captures in the same tick (11 µs) cannot
        // arrive at the rhythm of one call per round of `Session::run`, but a
        // timestamp that does not progress would make the decoder reject the
        // frame: we rule it out structurally rather than relying on
        // the caller's cadence.
        let pts = match self.last_pts_90k {
            Some(last) if pts <= last => last + 1,
            _ => pts,
        };
        self.last_pts_90k = Some(pts);
        pts
    }
}

// The `TICKS`/`CAPTURED`/`PRODUCED` counters lived here, as process
// statics — dead on both sides since D4. Replaced by the
// `telemetrie` field of `WindowsSource`, per session (see `windows_source/telemetrie.rs`).

/// Cumulative time (ns) spent in each step of a `next_frame` round.
///
/// The counters above say HOW MANY frames cross each stage;
/// these say WHERE the time goes. The distinction is the one that was missing to
/// decide between "the encoder cannot go faster" and "we do not
/// solicit it often enough": a stage that plateaus without occupying the thread
/// is waiting for something, a stage that occupies it is the real bottleneck.
///
/// Relative to the duration of the observation window, they give an
/// occupation rate of the `Session::run` thread — the single thread that also serves ICE,
/// RTCP and the data channels.
pub static CAPTURE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SUBMIT_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DRAIN_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
