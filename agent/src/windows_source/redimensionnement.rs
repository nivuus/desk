//! `WindowsSource::resize`: resize the window and rebuild the encoding
//! chain — through ONE of two paths, depending on the capture mode.
//!
//! ❌ **This title said "— or do nothing at all", and batch 33 made it
//! wrong.** There is no longer a path that does nothing: `FenetreRecadree`
//! recaptures the desktop (historical single-window path), `SortieEntiere` follows
//! the viewport **inside an output that does not move**
//! (`suivre_le_viewport`, at the bottom of the file). See
//! `ModeCapture::suit_le_viewport` for the measurement that motivated it and for the
//! distinction from the `ChangeDisplaySettingsExW` path D9 removed.
//!
//! **CHILD module of `windows_source`**, and not a sibling: that is what gives it
//! access to the private fields of `WindowsSource` without any having to be opened
//! as `pub(crate)` (see the fields' comment in `windows_source.rs`).
//! Extracted from that file because it is in size debt (`CLAUDE.md`) and
//! the final review's fix C1 moreover added
//! `depuis_pieces` and the `mode` field to it: the addition comes with its
//! extraction, as the rule requires.
//!
//! No value, no order of operations changed in the move; the only
//! addition is the mode guard at the head of `resize`.
//!
//! ⚠️ **Sub-block D8 had given this module a child, `mode_sortie`**, which
//! made the virtual output follow the viewport's mode. Sub-block D9
//! measured it — the change does not survive the opening of the next
//! window, and `CDS_UPDATEREGISTRY` pollutes the registry to the point of blocking the
//! product — and **removed** it. See the measurement finding at the head of
//! `capteur/plein_ecran.rs`. **What stays active and shipped of fullscreen
//! is DETECTION and ANNOUNCEMENT** (`capteur/fenetre.rs` →
//! `AgentControl::Fullscreen`), which do not go through here.

use anyhow::{Context, Result};

use super::WindowsSource;
use crate::capture::DesktopCapture;
use crate::encode::H264Encoder;
use crate::geometry::{borner_au_bureau, crop_region, Rect};
use crate::rebuild::{rebuild_or_recover, RebuildOutcome};
use crate::window;

impl WindowsSource {
    /// Resizes the window and rebuilds the encoding chain.
    ///
    /// Media Foundation does not allow changing resolution on the
    /// way: one must start again from a new encoder. The timestamp, for its part, stays
    /// continuous — the browser's decoder would reject going backwards
    /// (`next_pts_90k` is never reset here).
    ///
    /// **No effect when the source captures a whole DXGI output** — see the
    /// guard at the head of the function, and `ModeCapture` for what happened
    /// before it.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        // FIX C1 (final review of the multi-window branch D1). Everything
        // that follows assumes that
        // the window is free to be resized and that the capture is the
        // desktop's. Both are false in `SortieEntiere` mode, and yet the path
        // was taken SYSTEMATICALLY: the client's `ResizeObserver`
        // emits once at initial observation, hence ~200 ms after
        // each connection, with a size that then had no reason
        // to equal the output's (it is `clientWidth × devicePixelRatio`,
        // whereas the output was created on `innerWidth` ALONE, without the dpr
        // factor), so that the "size unchanged" short-circuit below did not
        // catch it. ⚠️ **This unit mismatch is the one task 5
        // of sub-block D9 precisely closed** (`client/src/main.ts`, the viewport
        // announcement now multiplies by `devicePixelRatio`): the two
        // units agree today, which changes nothing for this guard —
        // it stays necessary in `SortieEntiere` mode whatever the unit.
        //
        // What followed then produced, in order: a shrunk window that
        // leaves its virtual output (which the supervisor's 1 Hz check
        // immediately tries to catch up with, the two fighting), the output's
        // duplication released, a `DesktopCapture::new()` that duplicates the primary
        // PHYSICAL DESKTOP, a failing `crop_region`, and the
        // `Recovered` fallback installing that duplication with the region computed
        // for the virtual output — that is, the top-left corner of the VM's real
        // desktop broadcast into the browser window, for a single
        // `warn!`.
        //
        // Doing nothing is the RIGHT behaviour, not a makeshift: spec
        // §3.3 records that resizing an already open window is
        // out of D1's scope (the SudoVDA driver exposes no `SET_MODE`,
        // so the output cannot follow — ⚠️ **and that is still true of
        // the OUTPUT, which batch 33 does not move either; what has
        // changed is that the CROP and the WINDOW do follow it
        // inside**). `Ok(())` and not `Err`: nothing
        // failed, and an error would log an incident at every
        // connection. Network adaptation goes through `set_encode_size` and
        // is not concerned.
        //
        // ⚠️ **Sub-block D8 had established that the PREMISE above was
        // exact, but the CONCLUSION refutable**: another route
        // (`ChangeDisplaySettingsExW`, outside the SudoVDA driver's channel) does
        // make the output follow. Sub-block D9 measured it under product
        // conditions and **removed** it: the change does not survive the
        // opening of the next window, and it pollutes the registry to the
        // point of blocking the product. See the measurement finding at the head of
        // `capteur/plein_ecran.rs`. **Doing nothing has therefore become, once
        // again, the right behaviour — this time on the strength of a
        // measurement, and not of a merely assumed driver limit.**
        if self.mode.suit_le_viewport() {
            return self.suivre_le_viewport(width, height);
        }

        let (width, height) = (width.max(160) & !1, height.max(120) & !1);

        // Bound to what the desktop can really display. A client
        // viewport larger than the VM's desktop would otherwise produce a
        // window that overflows: `crop_region` would trim it at capture,
        // the image would take an aspect ratio the browser's container
        // does not have — hence black bars — and the off-screen part of
        // the application would become unreachable. Observed on 07/29/2026:
        // 1187 px requested for a 1080 desktop.
        //
        // Without a live capture or without a readable position, we let the
        // requested size through: `crop_region` stays the net, and an
        // imperfect resize is better than a failure.
        let (width, height) = match (
            self.capture.as_ref(),
            window::client_rect_on_screen(self.hwnd),
        ) {
            (Some(capture), Ok(actuel)) => {
                let (dw, dh) = capture.desktop_size();
                let (w, h) = borner_au_bureau(actuel.x, actuel.y, width, height, dw, dh);
                (w & !1, h & !1)
            }
            _ => (width, height),
        };

        if (width, height) == (self.width, self.height) {
            return Ok(());
        }

        window::resize_window(self.hwnd, width, height)?;
        // Let the window reach its new size before recapturing.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let window_rect = window::client_rect_on_screen(self.hwnd)?;

        // Explicitly releases the old capture (hence its
        // `IDXGIOutputDuplication`) BEFORE creating a new one. DXGI
        // only allows a single live duplication instance for a
        // given output, within one process: a mere reassignment
        // (`self.capture = Some(DesktopCapture::new()?)`) would evaluate the
        // right-hand side — hence `DuplicateOutput` — before replacing
        // the old `Some`, letting both exist at the same time for the duration
        // of the call. `DuplicateOutput` then fails with the screen output
        // duplication error — observed during task 13's first end-to-end
        // trial.
        //
        // This early release in turn opens a window where
        // `self.capture` may stay `None` if the rebuild fails: we
        // never close it with a mere `?` (see fix round
        // 1 in the comment of the `capture` field). `rebuild_or_recover`
        // (module `rebuild`, tested without Windows dependency) carries this
        // logic: attempt the full rebuild, and if it fails,
        // EXPLICITLY retry a backup capture — with the old
        // `region`/`encoder`/dimensions, still valid since they have not
        // been touched — before returning the error to the caller.
        self.capture = None;
        let fps = self.fps;
        let bitrate = self.bitrate;
        // **Fix C1 of the final review of the ADAPTIVE NETWORK branch**
        // (07/29/2026, commit `3f02545`) — same name as the multi-window C1
        // above, and unrelated to it. A first version of
        // this workstream kept the current encoding size here
        // (`self.encoder.encode_size()`) instead of starting again from the
        // capture size, intending not to erase a resolution reduction
        // applied because of a degraded link (task 9). It was
        // wrong: at startup, encode == capture, so from the FIRST
        // window resize, the encoded size froze for
        // the whole session — enlarging the window never again enlarged
        // the stream, and the controller (whose ladder was, for its part, never
        // rebuilt) could even end up aiming at a size larger than
        // the new capture. The encoded size must therefore again follow
        // the window unconditionally; it is `Session::act_on_timeout`
        // (branch a1, `transport/tick.rs`) that is now in charge of
        // reapplying, right after, the reduction the controller would judge
        // still necessary for the NEW size (see
        // `congestion::Controleur::changer_source`) — instead of preserving it
        // here blindly.

        // A NEW D3D11 device is created by the opening called just
        // below — `DesktopCapture::new_sans_attente`, and no longer
        // `DesktopCapture::new`: this path runs on the blocking thread of
        // `Session::run`, where the three-second retry window
        // would at the same time suspend keyframe requests and network
        // adaptation. Both go through `DesktopCapture::ouvrir`, which sets
        // `SetMultithreadProtected(TRUE)` on THIS device at each call
        // (`capture/ouverture.rs::create_device_and_context`, local `multithread`) — the
        // protection is therefore rebuilt with it, not merely inherited from
        // the old device that has just been released. Without it the
        // intermittent `AcquireNextFrame` blocking documented in task 10
        // would reappear after any resize.
        let outcome = rebuild_or_recover(
            || -> Result<(DesktopCapture, Rect, H264Encoder)> {
                let new_capture = DesktopCapture::new_sans_attente()?;
                let (dw, dh) = new_capture.desktop_size();
                let region = crop_region(window_rect, dw, dh)
                    .ok_or_else(|| anyhow::anyhow!("la fenêtre est hors de l'écran"))?;
                let mut encoder = H264Encoder::new(
                    new_capture.device(),
                    (region.width, region.height),
                    (region.width, region.height),
                    fps,
                    bitrate,
                )?;
                encoder.request_keyframe()?;
                Ok((new_capture, region, encoder))
            },
            // Backup factory: just a valid capture, so as never to
            // leave `self.capture` at `None` without `fatal` true on return.
            // `region`/`encoder`/`width`/`height` stay the previous ones:
            // only the capture had to be released, not the parameters that
            // depend on it, which never stopped being valid.
            //
            // `new_sans_attente` like the main factory above: these
            // two calls run on the blocking thread of `Session::run` (see
            // `transport/redimensionnement.rs`), where the opening retry
            // added in task 11 bis would freeze the session loop for up to
            // TWO full windows. The right to block is decided here, not
            // in `capture::ouvrir`.
            DesktopCapture::new_sans_attente,
        );

        match outcome {
            RebuildOutcome::Rebuilt((new_capture, region, encoder)) => {
                self.capture = Some(new_capture);
                self.region = region;
                self.encoder = Some(encoder);
                self.width = region.width;
                self.height = region.height;
                // New encoder: its very first output falls into the
                // same case as the initial startup (see `SUBMIT_POLL_BUDGET`).
                self.encoder_warmed_up = false;
                tracing::info!(self.width, self.height, "chaîne d'encodage reconstruite");
                Ok(())
            }
            RebuildOutcome::Recovered(new_capture, primary_error) => {
                // Usable state restored (old region/encoder/
                // dimensions, new capture): the session continues, as
                // the brief requires for a resize failure. The
                // OS window, for its part, has already changed size
                // (`resize_window` above succeeded): a transient
                // offset between the real window and the captured region
                // is possible until the next successful resize —
                // far preferable to an agent that crashes.
                self.capture = Some(new_capture);
                tracing::warn!(error = %primary_error, "reconstruction de la chaîne d'encodage échouée, capture de secours restaurée");
                Err(primary_error)
            }
            RebuildOutcome::Fatal(primary_error) => {
                // Neither the full chain, nor a mere backup capture
                // could be obtained: `self.capture` stays `None`.
                // `fatal` reports it for good — `next_frame` stops
                // before touching `capture` (see its guard), and
                // `is_exhausted()` will make the session close cleanly at the next
                // round, rather than a panic on the empty field.
                self.fatal = true;
                tracing::error!(error = %primary_error, "reconstruction de la chaîne d'encodage et capture de secours toutes deux échouées, source déclarée épuisée");
                Err(primary_error)
            }
        }
    }

    /// Makes the crop — and the Windows window — follow the viewport
    /// announced by the browser, **inside an output that does not
    /// move**.
    ///
    /// 🔴 **THIS PATH REPLACES AN `Ok(())` THAT DID NOTHING**, and it is
    /// not a regression of D9: see `ModeCapture::suit_le_viewport`, which
    /// carries the measurement of August 31st, 2026 (34 requests dropped, aspect ratios
    /// from 1.105 to 3.559 served at 1.3222) and the distinction from the
    /// `ChangeDisplaySettingsExW` path D9 removed. **No display mode
    /// is changed here**, and the registry is not touched.
    ///
    /// 🔴 **THE DUPLICATION IS NEVER RELEASED, AND IT IS D1's FIX C1
    /// THAT MUST NOT BE UNDONE.** `resize` in `FenetreRecadree` mode sets
    /// `self.capture = None` then reopens `DesktopCapture::new()`, which duplicates
    /// **the primary physical desktop** — on this VM, the QEMU VGA
    /// `\\.\DISPLAY1`, which no session serves. Here we keep the
    /// duplication of OUR output and only rebuild the region and
    /// the encoder, like `set_encode_size` (see `encodage.rs`): no
    /// DXGI duplication constraint, hence no need for
    /// `rebuild_or_recover`.
    fn suivre_le_viewport(&mut self, width: u32, height: u32) -> Result<()> {
        // Same guard as `set_encode_size`, and for the same reason: on a
        // permanently exhausted source, `capture_mut()` would panic, and a
        // panic crosses `spawn_blocking` and takes down the WHOLE process —
        // hence the sensor's eight other windows with it.
        if self.fatal {
            anyhow::bail!("source épuisée : recadrage inchangé");
        }

        // The bound comes from the WORK AREA of the monitor carrying the
        // window — that is what takes the taskbar out of the crop. The
        // supervisor queries the same monitor by the output's origin:
        // same `HMONITOR`, same bound, hence no fight between the two
        // processes (see `size_for_viewport`).
        //
        // ⚠️ **The fallback is the behaviour from before this batch**: when
        // `GetMonitorInfoW` refuses, we bound by the duplication's texture,
        // exactly as `sur_sortie` did alone.
        let texture = self.capture_mut().desktop_size();
        let borne = match crate::window::zones_du_moniteur_de(self.hwnd) {
            Ok((moniteur, travail)) => {
                let borne = crate::windows_source_sortie::borne_de_la_sortie(
                    (moniteur.width, moniteur.height),
                    Some((travail.width, travail.height)),
                );
                // 🔴 AND THE TEXTURE STAYS A BOUND, NOT INFORMATION.
                // `rcMonitor` is in DESKTOP coordinates, the crop region
                // in TEXTURE pixels, and the two do not coincide
                // on this machine: the duplication returns 1860×1080 where
                // `GetDesc().DesktopCoordinates` returns 1428×1080 (gap measured
                // by batch 32T, reconfirmed on August 31st, 2026). Without this `min`,
                // a work area wider than the texture would make
                // the region leave the image and `crop_region` would fail.
                (borne.0.min(texture.0), borne.1.min(texture.1))
            }
            Err(error) => {
                tracing::warn!(%error, "zone de travail illisible : recadrage borné par la texture");
                texture
            }
        };

        let (l, h) = crate::windows_source_sortie::size_for_viewport((width, height), borne);
        // 🔴 **UNCONDITIONAL TRACE — it replaces the one this batch had
        // REMOVED.** "resize ignored" came out at EVERY request, and
        // it is what made batch 33's diagnosis possible (34 requests
        // noted, aspect ratios from 1.105 to 3.559). Its replacement only
        // came out on CHANGE: a `0` in the log therefore no longer distinguished
        // "no `Resize` arrives" from "it arrives and saturates the
        // bound", that is, a defect of the declared limit.
        //
        // **A trace that can only come out on success cannot
        // diagnose a failure.**
        tracing::info!(
            demande = format!("{width}x{height}"),
            borne = format!("{}x{}", borne.0, borne.1),
            texture = format!("{}x{}", texture.0, texture.1),
            retenue = format!("{l}x{h}"),
            courante = format!("{}x{}", self.width, self.height),
            change = (l, h) != (self.width, self.height),
            "Resize recu par le capteur"
        );
        // Short-circuit BEFORE any destruction, and it is not cosmetic:
        // the client's `ResizeObserver` emits every 200 ms while an edge
        // is dragged, and each pass would otherwise rebuild an encoder.
        if (l, h) == (self.width, self.height) {
            return Ok(());
        }

        // The window first: `resize_window` carries `SWP_NOMOVE`, so
        // the origin — the output's, set by the supervisor — does not move,
        // and cropping at the origin stays right.
        //
        // ⚠️ `resize_window` imposes a 160×120 floor that
        // `size_for_viewport` does not have (its own is 2): below 160×120 the
        // window stays larger than the region, and the image then shows a
        // corner of the application. Degenerate case, not fixed, stated here.
        // 🔴 **COMPENSATE DWM's INVISIBLE EDGE, like `placement::poser`.**
        // `GetWindowRect` — the space `SetWindowPos` writes to — includes
        // TRANSPARENT resize borders (measured in session 1:
        // 7 px on the left, right and bottom, 0 at the top). Resizing to `l x h`
        // in that space leaves the VISIBLE frame smaller by as much, and
        // the crop — which, for its part, is indeed `l x h` — then shows desktop
        // on three sides. It is the residue the owner saw after
        // the aspect fix, and it is CONSTANT, insensitive to the ratio:
        // it is that signature that told it apart from a shape defect.
        //
        // `SWP_NOMOVE`: the origin does not move, and it is already compensated
        // by the supervisor's placement — only the SIZE remains to correct here.
        // A DWM failure returns `Lisere::NUL`, hence the previous behaviour.
        // The TOTAL envelope, like `placement::poser`: DWM's invisible edge
        // **plus** the border Windows paints. Without the second, the
        // one-pixel dark line noted on the four edges of the crop
        // (measurement of August 31st, 2026) enters the image.
        let lisere = crate::superviseur::placement::enveloppe(
            crate::window::lisere_dwm(self.hwnd).unwrap_or_default(),
            crate::window::bordure_peinte(),
        );
        let (lp, hp) = crate::superviseur::placement::size_to_set((l, h), lisere);
        crate::window::resize_window(self.hwnd, lp, hp)?;

        let region = crate::windows_source_sortie::region_de_sortie(l, h)
            .ok_or_else(|| anyhow::anyhow!("recadrage inexploitable ({l}x{h})"))?;

        let device = self.capture_mut().device().clone();
        // Destroy BEFORE building: at `vivier::PLAFOND_EVEIL` live encoders,
        // the transient at N+1 is refused by the NVIDIA MFT
        // (`MF_E_UNSUPPORTED_D3D_TYPE`, 18 refusals out of 18 in D4). Same remedy,
        // same price, as `set_encode_size`.
        drop(self.encoder.take());
        let neuf = H264Encoder::new(&device, (l, h), (l, h), self.fps, self.bitrate);
        let mut encoder = match neuf {
            Ok(encoder) => encoder,
            Err(error) => {
                self.fatal = true;
                return Err(error).context(
                    "encodeur neuf refusé après destruction de l'ancien : source épuisée",
                );
            }
        };
        if let Err(error) = encoder.request_keyframe() {
            self.fatal = true;
            return Err(error).context("image clé refusée par l'encodeur neuf : source épuisée");
        }

        self.region = region;
        self.width = l;
        self.height = h;
        self.encoder = Some(encoder);
        // The new encoder has produced nothing: the startup polling budget
        // starts again, as after `resize` and after `set_encode_size`.
        self.encoder_warmed_up = false;
        tracing::info!(
            demande = format!("{width}x{height}"),
            borne = format!("{}x{}", borne.0, borne.1),
            retenue = format!("{l}x{h}"),
            "recadrage et fenêtre alignés sur le viewport (la sortie, elle, n'a pas bougé)"
        );
        Ok(())
    }
}
