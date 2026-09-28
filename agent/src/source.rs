//! Video sources producing H.264 access units ready to be sent.

use crate::h264::{group_access_units, AccessUnit, CLOCK_RATE_HZ};
use anyhow::{bail, Result};

/// Producer of H.264 access units.
///
/// The Windows implementation (capture + encoding) and the test file source
/// substitute for each other behind this trait.
pub trait VideoSource {
    /// Next access unit, or `None` if nothing is ready this turn.
    ///
    /// `None` does **not** systematically mean "source exhausted": for
    /// a live capture, the absence of a new frame is the common
    /// and normal case (nothing changed on screen since the last call). It is
    /// `is_exhausted()`, queried separately by the caller after a
    /// `None`, that distinguishes this normal case from a definitive stop.
    fn next_frame(&mut self) -> Option<AccessUnit>;
    /// Dimensions of the produced video, in pixels.
    fn dimensions(&self) -> (u32, u32);
    /// True if the source will never produce any frame again (device
    /// gone, unrecoverable error...) and the session must close.
    ///
    /// Only consulted after a `next_frame()` that returned `None`.
    /// By default, a source is never exhausted: it is exactly the case of
    /// `FileSource`, which loops indefinitely and never returns `None`, and
    /// the nominal case of `WindowsSource` as long as the captured window exists
    /// — a still desktop must never, on its own, close the session
    /// (see `transport/piste_video.rs`).
    fn is_exhausted(&self) -> bool {
        false
    }

    /// Resizes the source, if it allows it.
    ///
    /// No effect by default: a file source ignores the request. The Windows
    /// source, for its part, resizes the window and rebuilds its chain.
    fn resize(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// False when the source has definitively disappeared — window closed, for
    /// example. To be distinguished from `is_exhausted`, which signals the exhaustion of a
    /// finite stream.
    fn is_alive(&self) -> bool {
        true
    }

    /// Forces a keyframe to be produced on the next produced frame.
    ///
    /// The hardware encoder's group of pictures is open (see
    /// `encode::configure_rate_control`): without an explicit call here, no
    /// keyframe is ever produced again after startup or a
    /// resize, and the slightest packet loss corrupts the video
    /// definitively until reconnection. Wired to str0m's `Event::KeyframeRequest`
    /// in `transport/evenements.rs`, which relays the browser's request
    /// after a loss detected on the decoder side.
    ///
    /// No effect by default: `FileSource` replays a pre-split stream where the
    /// looping itself already restarts on a keyframe (see
    /// `group_access_units`); one more request would change nothing.
    fn request_keyframe(&mut self) -> Result<()> {
        Ok(())
    }

    /// Changes the encoding bitrate without rebuilding anything.
    ///
    /// No effect by default: a file source encodes nothing.
    fn set_bitrate(&mut self, _bitrate: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Changes the size ACTUALLY ENCODED, without touching the captured
    /// window.
    ///
    /// Not to be confused with `resize`, which resizes the real Windows
    /// window because the user dragged an edge. Here the window does not
    /// move: only the carried stream slims down, because the link no longer
    /// carries full resolution.
    ///
    /// No effect by default.
    fn set_encode_size(&mut self, _width: u32, _height: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Tells the producer whether the window is visible to the user, and
    /// whether it has focus.
    ///
    /// No effect by default: a file source has no one to please.
    /// The remote source relays it to the capturer, which arbitrates GLOBALLY — the
    /// resulting decision can therefore concern a window other than
    /// this one, and never comes back through the return value.
    fn set_awake(&mut self, _visible: bool, _focalisee: bool) -> anyhow::Result<()> {
        Ok(())
    }

    /// Returns the sleep change awaiting announcement to the browser, and
    /// consumes it.
    ///
    /// **Current state, not a history**: two changes arriving between
    /// two reads overwrite each other, only the last survives. The transport
    /// loop queries this method at each turn (~100 Hz); since it
    /// consumes, no message is ever re-emitted — otherwise the browser's
    /// control channel would be flooded.
    ///
    /// No effect by default: a file source never sleeps nor wakes
    /// up. Only `SourceDistante` redefines this method — it is the one
    /// that relays the sleep decided GLOBALLY by the capturer's pool.
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        None
    }

    /// Returns the bitrate budget share awaiting application, and
    /// consumes it.
    ///
    /// **Current state, not a history**: two shares arriving between two
    /// reads overwrite each other — same regime as `sommeil_a_annoncer` just
    /// above. Since it consumes, the transport branch that
    /// queries it at each turn cannot reconfigure in a loop.
    ///
    /// No effect by default: a file source shares the link with
    /// no one. Only `SourceDistante` redefines it.
    fn part_a_appliquer(&mut self) -> Option<u32> {
        None
    }

    /// Returns the audio order awaiting application, and consumes it.
    ///
    /// **Current state, not a history**: two orders arriving between two
    /// reads overwrite each other — same regime as `part_a_appliquer` just
    /// above. Since it consumes, the transport branch that queries it
    /// at ~100 Hz cannot replay `Start()`/`Stop()` in a loop.
    ///
    /// No effect by default: a file source has no sound, and a
    /// `WindowsSource` held live by its own process is
    /// single-window, hence never arbitrated. Only `SourceDistante` redefines it.
    fn audio_a_appliquer(&mut self) -> Option<bool> {
        None
    }

    /// Returns the full-screen change awaiting announcement, and consumes it.
    ///
    /// **Current state, not a history**: two changes arriving between
    /// two reads overwrite each other — same regime as `sommeil_a_annoncer`. Since
    /// it consumes, the transport branch that queries it at ~100 Hz cannot
    /// flood the control channel.
    ///
    /// No effect by default: a file source has no Windows window,
    /// and a `WindowsSource` held live by its own process has
    /// no one to push it.
    fn plein_ecran_a_annoncer(&mut self) -> Option<bool> {
        None
    }

    /// Returns the VM's clipboard awaiting announcement, and consumes it.
    ///
    /// `None` in the pair's first member signals a size REFUSAL: the
    /// content exceeded `presse_papier::PRESSE_PAPIER_MAX` and was refused,
    /// never truncated. The second member then carries the refused size, in
    /// UTF-8 bytes after line-ending normalisation.
    ///
    /// **Current state, not a history**: two copies arriving between two
    /// reads overwrite each other — same regime as `plein_ecran_a_annoncer` just
    /// above. Since it consumes, the `a1septies` branch of
    /// `transport/tick.rs`, which queries it at ~100 Hz, cannot flood the
    /// control channel.
    ///
    /// No effect by default: a file source has no clipboard, and
    /// a `WindowsSource` held live by its own process has no
    /// capturer to push it — it is the capturer that holds the
    /// VM's clipboard, and it alone (sub-block P1).
    fn presse_papier_a_annoncer(&mut self) -> Option<(Option<String>, u32)> {
        None
    }

    /// Returns the window's accent colour to announce to the browser, and
    /// CONSUMES it. `#rrggbb`, lowercase (sub-block A1).
    ///
    /// No effect by default, like `presse_papier_a_annoncer` just above:
    /// a file source has no icon, and a `WindowsSource` held
    /// live by its own process has no capturer to push it.
    ///
    /// ⚠️ **Reading the icon lives on the capturer's WINDOW THREAD**, never
    /// on its wheel turn: the registry does not have the `hwnd`, and the accent is PER
    /// WINDOW where the clipboard is GLOBAL to the window station.
    fn accent_a_annoncer(&mut self) -> Option<String> {
        None
    }

    /// True as long as the capturer holds this window to be ASLEEP — encoder
    /// and duplication released (sub-block D5), no frame produced.
    ///
    /// **Current state, and it IS NOT CONSUMED.** It is exactly what
    /// distinguishes it from `sommeil_a_annoncer` just above, which returns a
    /// CHANGE only once: `Session::appliquer_part` needs to reread
    /// this state at each share it applies, and an announcement that runs out could
    /// not tell it. Guessing the state by comparing the received share to
    /// `PART_DORMANTE_BPS` would not do either: it would be a
    /// value coupling between two processes, mute the day one of the two
    /// changed its constant.
    ///
    /// **Why the transport loop asks for it**: a sleeping window's share
    /// is a floor (`capteur::repartiteur::PART_DORMANTE_BPS`, 256 kb/s),
    /// far below the lowest rung of the ladder. Applying it as an ENCODING
    /// ceiling would make `video_bitrate_bps` drop there without any
    /// path raising it again on wake-up — see `Session::appliquer_part`.
    ///
    /// False by default: a file source, like a `WindowsSource` held
    /// live by its own process, never sleeps. Only
    /// `SourceDistante` redefines it.
    fn est_endormie(&self) -> bool {
        false
    }

    /// Signals to the capturer that this window's audio capture is dead.
    ///
    /// **Inert default**, like `est_endormie`: a source without a
    /// capturer facing it has no one to warn. Only `SourceDistante`
    /// really implements it.
    fn signaler_audio_mort(&mut self) {}

    /// Tells the capturer that this window's audio capture has resumed.
    ///
    /// INERT default, like the two neighbouring methods: sources that
    /// talk to no capturer (test, single-window) have nothing to announce.
    fn signaler_audio_vivant(&mut self) {}

    /// Writes `texte` into the VM's clipboard (browser → VM direction,
    /// sub-block P2). The text arrives **already normalised, bounded and denormalised**
    /// (`\r\n`): this method decides nothing about its content.
    ///
    /// 🔴 **`Err` DEFAULT, AND IT IS A PATTERN BREAK IN THIS FILE** —
    /// `est_endormie`, `signaler_audio_mort`, `signaler_audio_vivant`,
    /// `presse_papier_a_annoncer` and `accent_a_annoncer` all have an
    /// INERT default. **Do not align it with its neighbours.**
    ///
    /// ⚠️ *`accent_a_annoncer` was added to this list by the cross-cutting
    /// review of sub-block A1: the inventory had become INCOMPLETE, which
    /// is the most discreet form of the defect this review hunts — the
    /// sentence stays true of what it names, and false of what it omits.*
    ///
    /// The reason is that the caller does not use this return to decide whether it
    /// *logs*, but whether it **INJECTS `Ctrl+V`**. An inert `Ok(())` would make
    /// the key be injected on an **unchanged** Windows clipboard, hence
    /// paste the PREVIOUS content — the silent failure mode D6
    /// exists entirely to avoid, and the only one giving the user a
    /// WRONG result rather than an absent one. An `Err` makes the failure logged and
    /// nothing injected, which D6 prescribes in so many words for this case: "if
    /// the clipboard cannot be written, the `V` key is LOST, not
    /// postponed".
    ///
    /// ⚠️ **Accepted consequence: SINGLE-WINDOW mode has no paste, and
    /// it SAYS so.** Without a capturer, `SourceDistante` does not exist and it is this
    /// default that runs. The single-window owner stays P1's legacy no. 1,
    /// not filled; P2 makes it loud instead of silent.
    fn write_clipboard(&mut self, _texte: &str) -> anyhow::Result<()> {
        anyhow::bail!("no sensor: the VM clipboard is not accessible")
    }

    /// True only once, right after the channel to the capturer has
    /// REATTACHED (capturer restarted, or DXGI access loss absorbed by the
    /// resumption window). Consumed, like `sommeil_a_annoncer`.
    ///
    /// **Why the transport loop needs it**: the audio unfitness
    /// registry lives in memory, in the CAPTURER
    /// (`capteur/sommeil.rs`) — a restarted capturer no longer has the slightest trace
    /// of an `AudioMort` signalled before its death. `Session::audio_mort_signale`
    /// must therefore fall back to `false` so that the next detection of
    /// `capture_morte` (`transport/tick.rs`) informs it again.
    ///
    /// Inert default: a source that never reattaches (file, or
    /// `WindowsSource` held live by its own process) has nothing to
    /// signal.
    fn rattachement_survenu(&mut self) -> bool {
        false
    }
}

/// Test source replaying an Annex-B H.264 file in a loop.
///
/// Serves to validate the transport without depending on Windows: the stream is split
/// once at load time, then replayed indefinitely with strictly
/// increasing timestamps (a decoder would reject going backwards).
#[derive(Debug)]
pub struct FileSource {
    units: Vec<AccessUnit>,
    width: u32,
    height: u32,
    tick_90k: u64,
    index: usize,
    loops: u64,
}

impl FileSource {
    pub fn from_annex_b(data: Vec<u8>, width: u32, height: u32, fps: u32) -> Result<Self> {
        if fps == 0 {
            bail!("the number of frames per second must be greater than zero");
        }
        let units = group_access_units(&data, fps);
        if units.is_empty() {
            bail!("invalid stream: no access unit found");
        }
        if !units.iter().any(|u| u.is_keyframe) {
            bail!("invalid stream: no key frame found");
        }
        Ok(Self {
            tick_90k: CLOCK_RATE_HZ / fps as u64,
            index: 0,
            loops: 0,
            units,
            width,
            height,
        })
    }

    /// Loads a `.264` file from disk.
    pub fn from_path(path: &std::path::Path, width: u32, height: u32, fps: u32) -> Result<Self> {
        let data = std::fs::read(path)?;
        Self::from_annex_b(data, width, height, fps)
    }
}

impl VideoSource for FileSource {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        let total = self.units.len() as u64;
        let mut unit = self.units[self.index].clone();
        unit.pts_90k = (self.loops * total + self.index as u64) * self.tick_90k;

        self.index += 1;
        if self.index >= self.units.len() {
            self.index = 0;
            self.loops += 1;
        }
        Some(unit)
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an Annex-B stream of `frames` frames, the first being an IDR.
    fn flux_de_test(frames: usize) -> Vec<u8> {
        let mut stream = Vec::new();
        for i in 0..frames {
            let nal: Vec<u8> = if i == 0 {
                vec![0x65, 0x88]
            } else {
                vec![0x41, 0x9A]
            };
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&nal);
        }
        stream
    }

    #[test]
    fn exposes_its_dimensions() {
        let source = FileSource::from_annex_b(flux_de_test(2), 1280, 720, 60).unwrap();
        assert_eq!(source.dimensions(), (1280, 720));
    }

    #[test]
    fn rejects_a_stream_without_frames() {
        let err = FileSource::from_annex_b(vec![0xFF, 0xFE], 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("no access unit"));
    }

    #[test]
    fn rejects_a_stream_without_a_key_frame() {
        let stream = {
            let mut s = Vec::new();
            s.extend_from_slice(&[0, 0, 0, 1]);
            s.extend_from_slice(&[0x41, 0x9A]);
            s
        };
        let err = FileSource::from_annex_b(stream, 1280, 720, 60).unwrap_err();
        assert!(err.to_string().contains("no key frame"));
    }

    #[test]
    fn replays_in_a_loop_with_increasing_timestamps() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        let mut horodatages = Vec::new();
        for _ in 0..7 {
            horodatages.push(source.next_frame().unwrap().pts_90k);
        }
        // 1500 ticks per frame at 60 fps; timestamps never restart.
        assert_eq!(horodatages, vec![0, 1500, 3000, 4500, 6000, 7500, 9000]);
    }

    #[test]
    fn the_first_frame_of_each_loop_is_a_keyframe() {
        let mut source = FileSource::from_annex_b(flux_de_test(3), 640, 480, 60).unwrap();
        assert!(source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(!source.next_frame().unwrap().is_keyframe);
        assert!(source.next_frame().unwrap().is_keyframe); // start of the next loop
    }

    #[test]
    fn loads_the_real_test_stream() {
        let path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let mut source = FileSource::from_path(path, 1280, 720, 60).expect("loading the stream");
        assert_eq!(source.dimensions(), (1280, 720));

        let first = source.next_frame().unwrap();
        assert!(first.is_keyframe, "the first unit must be a key frame");
        assert!(first.data.len() > 100, "a real key frame is not tiny");
    }

    #[test]
    fn loops_with_300_real_frames_and_strictly_increasing_timestamps() {
        // Non-regression check on the over-splitting bug: the
        // real file contains 300 frames despite its 2400 slices, so the
        // loop counter must increment after 300 calls, not 2400.
        let path =
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/testsrc.264"));
        let mut source = FileSource::from_path(path, 1280, 720, 60).expect("loading the stream");

        // A complete loop (300 frames), plus the first frame of the next turn.
        let mut horodatages = Vec::with_capacity(301);
        let mut keyframes = Vec::with_capacity(301);
        for _ in 0..301 {
            let unit = source.next_frame().unwrap();
            horodatages.push(unit.pts_90k);
            keyframes.push(unit.is_keyframe);
        }

        assert!(keyframes[0], "the first frame of the file is an IDR");
        assert!(
            keyframes[300],
            "the first frame of the next loop must also be an IDR"
        );
        assert!(
            horodatages.windows(2).all(|w| w[0] < w[1]),
            "timestamps must be strictly increasing, including at the loop point"
        );
        // 300 frames at 1500 ticks (90000 / 60): the timestamp at wraparound
        // continues the linear progression instead of restarting at zero.
        assert_eq!(horodatages[300], 300 * (CLOCK_RATE_HZ / 60));
    }

    #[test]
    fn default_resize_ignores_the_request_and_keeps_the_dimensions() {
        // `FileSource` does not redefine `resize`: the trait's default
        // method must be a no-op that succeeds, without ever touching the
        // file source's dimensions (task 13, source.rs).
        let mut source = FileSource::from_annex_b(flux_de_test(2), 640, 480, 60).unwrap();
        source
            .resize(1920, 1080)
            .expect("the default no-op must never fail");
        assert_eq!(
            source.dimensions(),
            (640, 480),
            "the dimensions must not move"
        );
    }

    #[test]
    fn is_alive_defaults_to_always_true() {
        // `FileSource` loops indefinitely and never "dies": the
        // trait's default method must reflect that.
        let source = FileSource::from_annex_b(flux_de_test(2), 640, 480, 60).unwrap();
        assert!(source.is_alive());
    }
}
