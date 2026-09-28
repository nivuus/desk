//! `CAPTURE_TEST` diagnostic mode: checks finding a window by
//! title fragment, then that the capture does return its content.
//!
//! Two optional measurements are grafted onto it, active only if
//! `CAPTURE_TEST` already is — they reuse its window and its capture:
//! `ENCODE_TEST`, which encodes really captured frames (throughput of the
//! complete capture+encoding path), and `ENCODER_THROUGHPUT_TEST`, which
//! reinjects a single texture in a loop to isolate the throughput of the
//! conversion+encoding pipeline from that of the source.

use std::time::Duration;

use anyhow::Result;

use super::pixels::capture_center_pixel;
use crate::demarrage::watch_encoder;
use crate::{capture, encode, geometry, h264, window};

pub(super) fn executer(fragment: &str) -> Result<()> {
    let hwnd = window::find_window_by_title(fragment)?;
    let window_rect = window::client_rect_on_screen(hwnd)?;
    tracing::info!(?window_rect, "window found");

    let mut capture = capture::DesktopCapture::new()?;
    let (dw, dh) = capture.desktop_size();
    let region = geometry::crop_region(window_rect, dw, dh)
        .ok_or_else(|| anyhow::anyhow!("the window is off the screen"))?;
    tracing::info!(?region, bureau = ?(dw, dh), "crop region");

    // Desktop Duplication only returns a frame when the desktop changes
    // (see the brief's note). A CSS animation in the test page
    // is usually enough, but it may be throttled by the browser
    // as soon as its window loses focus (observed in practice: the very
    // first next_frame succeeds, the following ones all time out — including
    // for 5 s on this VM). A first attempt tried to work around this by
    // making the cursor oscillate via `SetCursorPos` in the background,
    // without effect: the hardware cursor seems composed outside the pipeline
    // Desktop Duplication watches on this configuration (dual
    // virtual/RTX 4070 adapter). We therefore move the window
    // itself by one pixel instead, in a loop: moving a window
    // always forces a real DWM recomposition of the desktop, whatever the
    // display pipeline, and unblocks `AcquireNextFrame` for ANY
    // sampled region (the API returns the whole desktop's frame
    // as soon as ONE area changes, not only the one that changed).
    let stop_jitter = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let jitter_flag = stop_jitter.clone();
    // windows-rs 0.62 API gap: `HWND` wraps a `*mut c_void`, which
    // is not `Send` — `hwnd` cannot be moved as is into the
    // shaking thread. An HWND is only an opaque identifier (not a
    // pointer actually dereferenced on the process side), so passing
    // it through its raw address (`isize`, which is `Send`) and
    // rebuilding it in the target thread is safe.
    let jitter_hwnd_addr = hwnd.0 as isize;
    let jitter_thread = std::thread::spawn(move || {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };
        let jitter_hwnd = HWND(jitter_hwnd_addr as *mut core::ffi::c_void);
        // Oscillation anchor: the origin of the WINDOW, not that of its
        // client area. `window_rect` is a client rectangle converted into
        // screen coordinates (`client_rect_on_screen`): passing it back as
        // is to `SetWindowPos`, which expects window coordinates,
        // shifted the window right by the thickness of its border
        // (~9 px) at each run. The shift accumulated from one attempt
        // to the next until the window overlapped the control
        // region, which made the content proof fail — a failure
        // of the test bench, not of the capture.
        let mut origin = RECT::default();
        let anchor = match unsafe { GetWindowRect(jitter_hwnd, &mut origin) } {
            Ok(()) => (origin.left, origin.top),
            // Fallback on the old behaviour: better to shake the
            // window to within a few pixels than not to shake it at all.
            Err(_) => (window_rect.x, window_rect.y),
        };
        let mut toggle = false;
        while !jitter_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let dx = if toggle { 0 } else { 1 };
            let _ = unsafe {
                SetWindowPos(
                    jitter_hwnd,
                    None,
                    anchor.0 + dx,
                    anchor.1,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            toggle = !toggle;
            std::thread::sleep(Duration::from_millis(30));
        }
    });
    let stop_jitter_on_exit = stop_jitter.clone();
    // `result` carries the body of the diagnostic: it goes through a
    // closure to guarantee the window shaking thread stops on
    // ALL exit paths (success as well as error via `?`), without
    // duplicating the `store` before each `return`/`?`.
    let result = (|| -> Result<()> {
        // Proof based on CONTENT, not only dimensions
        // (review 1/5): `CapturedFrame.width/height` are copied
        // from `region` by construction, so seeing them match
        // the window's size proves nothing about what
        // `CopySubresourceRegion` actually copied — a box frozen on
        // the desktop origin would give exactly the same log. We
        // therefore read a real pixel:
        //   - once cropped to `region` (the window, expected green
        //     — see the test page used for the run),
        //   - once cropped to a control rectangle of the same
        //     size, placed in the desktop corner farthest from the
        //     window (and not at the origin (0, 0): for a window
        //     close to the top-left corner, a control rectangle at
        //     the origin and of the same size may overlap the window
        //     itself, which would invalidate the comparison without
        //     anyone noticing).
        let (rw, rh, rr, rg, rb, ra) =
            capture_center_pixel(&mut capture, region, Duration::from_secs(5))?;
        tracing::info!(
            width = rw,
            height = rh,
            r = rr,
            g = rg,
            b = rb,
            a = ra,
            "pixel read at the centre of the real region (crop on the window)"
        );

        let control_region = geometry::Rect {
            x: dw.saturating_sub(region.width) as i32,
            y: dh.saturating_sub(region.height) as i32,
            width: region.width,
            height: region.height,
        };
        anyhow::ensure!(
            !geometry::rects_overlap(window_rect, control_region),
            "control region {control_region:?} overlaps the window {window_rect:?}: \
             the window is too large for this test, the content proof would be invalid"
        );
        let (cw, ch, cr, cg, cb, ca) =
            capture_center_pixel(&mut capture, control_region, Duration::from_secs(5))?;
        tracing::info!(
            ?control_region,
            width = cw,
            height = ch,
            r = cr,
            g = cg,
            b = cb,
            a = ca,
            "pixel read at the centre of the control region (opposite corner of the desktop, same size)"
        );
        anyhow::ensure!(
            (rr, rg, rb) != (cr, cg, cb),
            "the pixel of the real region ({rr},{rg},{rb}) is identical to that of the \
             control ({cr},{cg},{cb}): the crop does not tell the two areas apart"
        );
        tracing::info!(
            "content proof: the crop does tell the window apart from the rest of the desktop (different pixels)"
        );

        let mut captured = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Some(frame) = capture
                .next_frame(region)
                .map_err(|e| anyhow::anyhow!("{e}"))?
            {
                captured += 1;
                if captured == 1 {
                    tracing::info!(frame.width, frame.height, "first image captured");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        tracing::info!(captured, "images captured in 3 s");
        anyhow::ensure!(captured > 0, "no image captured");

        // Verification encoding: ENCODE_TEST=1 encodes 120 captured frames.
        //
        // Gap from the brief: the encoder's dimensions come from `region`
        // (the area actually cropped by `crop_region`, always even)
        // rather than from `window::client_size(hwnd)` — they are exactly the
        // dimensions of the `CapturedFrame` produced by `capture.next_frame`,
        // which may differ from the raw client area if the window overflows
        // the screen. Using a dimension different from that of the textures
        // actually submitted could have made `SetInputType`/
        // `ProcessInput` fail in a confusing way.
        if std::env::var("ENCODE_TEST").is_ok() {
            let mut encoder = encode::H264Encoder::new(
                capture.device(),
                (region.width, region.height),
                (region.width, region.height),
                60,
                8_000_000,
            )?;
            encoder.request_keyframe()?;
            // Same watchdog as the throughput measurement: here it serves to
            // check that REALLY DISTINCT frames go through the
            // converter (`converter_inputs` must progress), and not
            // only that access units come out of the encoder.
            let stop_watchdog = watch_encoder(encoder.telemetry());

            let mut encoded = 0usize;
            let mut keyframes = 0usize;
            let mut submitted = 0usize;
            let mut first_unit_has_params = None;
            let mut pts = 0u64;
            // Adjustable bounds: the brief's contract (120 frames, 5 s)
            // stays the default value, but a measurement of the REAL
            // pipeline over several hundred frames requires a longer
            // window.
            //
            // Stale figure removed (28/07): this comment cited a
            // ceiling of ~48 fps for Desktop Duplication capture.
            // The milestone 1 acceptance run
            // (`docs/superpowers/plans/2026-07-27-jalon1-recette.md`,
            // "What was learned") establishes that this figure was
            // obsolete — re-measured, isolated capture (`CAPTURE_TEST`)
            // sustains ~90 fps, and this capture+encoding loop
            // itself (measured here, `ENCODE_TEST`) sustains ~80 fps.
            // 120 frames therefore last ~1.5 s in practice, not 2.5 s.
            let encode_target: usize = std::env::var("ENCODE_TEST_TARGET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(120);
            let encode_secs: u64 = std::env::var("ENCODE_TEST_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5);
            let start = std::time::Instant::now();
            let deadline = start + std::time::Duration::from_secs(encode_secs);
            let phase = encoder.telemetry();
            // Mark the acquisition: a block in the capture and a
            // block in the encoder produce the same signature as seen from
            // the watchdog thread (frozen counters, step at rest). The
            // capture itself refines into sub-steps (acquisition, GPU
            // copy, release) once the marker is plugged in.
            capture.set_phase_marker(phase.phase.clone());
            while std::time::Instant::now() < deadline && encoded < encode_target {
                phase
                    .phase
                    .store(encode::PHASE_CAPTURE, std::sync::atomic::Ordering::Relaxed);
                let acquired = capture
                    .next_frame(region)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                phase
                    .phase
                    .store(encode::PHASE_IDLE, std::sync::atomic::Ordering::Relaxed);
                if let Some(frame) = acquired {
                    encoder.submit(&frame, pts)?;
                    submitted += 1;
                    pts += 1500; // 90000 / 60
                }
                while let Some(unit) = encoder.poll_output()? {
                    if encoded == 0 {
                        // A valid H.264 access unit must open with
                        // parameter NALs (SPS then PPS) before the first
                        // IDR slice: without them the browser's decoder
                        // cannot initialise (task 11). We check it
                        // here rather than assuming `group_access_units`
                        // did attach them.
                        let nals = h264::split_annex_b(&unit.data);
                        let types: Vec<u8> = nals
                            .iter()
                            .map(|n| n.first().map_or(0, |b| b & 0x1F))
                            .collect();
                        const NAL_SPS: u8 = 7;
                        const NAL_PPS: u8 = 8;
                        const NAL_IDR: u8 = 5;
                        let sps_idx = types.iter().position(|&t| t == NAL_SPS);
                        let pps_idx = types.iter().position(|&t| t == NAL_PPS);
                        let idr_idx = types.iter().position(|&t| t == NAL_IDR);
                        let ordered = matches!((sps_idx, pps_idx, idr_idx),
                            (Some(s), Some(p), Some(i)) if s < p && p < i);
                        tracing::info!(?types, ordered, "NALs of the first access unit");
                        first_unit_has_params = Some(ordered);
                    }
                    encoded += 1;
                    if unit.is_keyframe {
                        keyframes += 1;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            stop_watchdog();
            let elapsed = start.elapsed();
            // `converter_inputs` proves that it is indeed NEW
            // frames that went through the converter, and not the same one
            // re-encoded: it is the difference between a pipeline that
            // works and a counter that goes up.
            let converter_inputs = encoder
                .telemetry()
                .converter_inputs
                .load(std::sync::atomic::Ordering::Relaxed);
            tracing::info!(
                encoded,
                keyframes,
                submitted,
                converter_inputs,
                elapsed_ms = elapsed.as_millis() as u64,
                fps = encoded as f64 / elapsed.as_secs_f64(),
                "images encoded"
            );
            anyhow::ensure!(encoded > 0, "no image encoded");
            anyhow::ensure!(keyframes > 0, "no key frame produced");
            anyhow::ensure!(
                first_unit_has_params == Some(true),
                "the first access unit does not start with SPS then PPS then IDR: {:?}",
                first_unit_has_params
            );
        }

        // Isolated throughput measurement of the conversion+encoding pipeline:
        // ENCODER_THROUGHPUT_TEST=1 reinjects a SINGLE already
        // captured texture, in a tight loop, without ever going back through
        // `capture.next_frame` — unlike `ENCODE_TEST` above,
        // which mixes the throughput of the source (Desktop Duplication, limited
        // by real screen changes) with that of the encoder
        // itself. It is this isolated measurement, made by the reviewer
        // during fix round 1/5, that established that
        // the ~1 fps ceiling observed with `ENCODE_TEST` came from a
        // converter driving bug (see `encode.rs`), not from the
        // GPU: replaying the same texture proves/disproves the hardware
        // theory without depending on the availability of fresh frames.
        // Kept as is for task 14, which will need it
        // for its own throughput measurements.
        if std::env::var("ENCODER_THROUGHPUT_TEST").is_ok() {
            let mut encoder = encode::H264Encoder::new(
                capture.device(),
                (region.width, region.height),
                (region.width, region.height),
                60,
                8_000_000,
            )?;
            encoder.request_keyframe()?;

            // A single real frame, captured once then reinjected
            // as is at each iteration: no other call to
            // `capture.next_frame` in this loop.
            // Bounded: without a deadline, a capture that no longer returns a frame
            // would make it wait indefinitely without the slightest trace.
            let frame_deadline = std::time::Instant::now() + Duration::from_secs(10);
            let frame = loop {
                if let Some(frame) = capture
                    .next_frame(region)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                {
                    break frame;
                }
                anyhow::ensure!(
                    std::time::Instant::now() < frame_deadline,
                    "no image captured within 10 s to prime the throughput measurement"
                );
                std::thread::sleep(std::time::Duration::from_millis(2));
            };

            let target: usize = std::env::var("ENCODER_THROUGHPUT_TARGET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600);
            // Adjustable deadline, and short by default: a ten-minute
            // deadline turns the slightest pipeline block into
            // ten minutes of complete silence, which really cost
            // several investigation cycles on this task.
            let deadline_secs: u64 = std::env::var("ENCODER_THROUGHPUT_DEADLINE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            // Submission cadence, in frames per second. 0 = no
            // limit (we submit as fast as the loop spins).
            //
            // Pacing changes what the measurement means, and it is
            // intended. Without a limit, `submit` is called far more
            // often than any real source would: the
            // converter, constantly solicited, refuses almost
            // all new frames and the measured throughput no longer reflects
            // anything but the encoder replaying the last converted frame.
            // With a cadence, we measure what the milestone really requires:
            // how many NEW frames per second go through conversion
            // THEN encoding (`converter_inputs` in the result
            // line).
            let submit_hz: u64 = std::env::var("ENCODER_THROUGHPUT_SUBMIT_HZ")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let submit_interval =
                (submit_hz > 0).then(|| Duration::from_nanos(1_000_000_000 / submit_hz));
            tracing::info!(
                target,
                deadline_secs,
                submit_hz,
                "start of the isolated throughput measurement"
            );
            let stop_watchdog = watch_encoder(encoder.telemetry());

            let mut encoded = 0usize;
            let mut keyframes = 0usize;
            let mut submitted = 0usize;
            let mut pts = 0u64;
            let start = std::time::Instant::now();
            let deadline = start + std::time::Duration::from_secs(deadline_secs);
            let loop_result = (|| -> Result<()> {
                let mut next_submit = std::time::Instant::now();
                while encoded < target && std::time::Instant::now() < deadline {
                    let before = encoded;
                    if let Some(interval) = submit_interval {
                        let now = std::time::Instant::now();
                        if now < next_submit {
                            std::thread::sleep(next_submit - now);
                        }
                        next_submit += interval;
                    }
                    encoder.submit(&frame, pts)?;
                    submitted += 1;
                    pts += 1500; // 90000 / 60
                    while let Some(unit) = encoder.poll_output()? {
                        encoded += 1;
                        if unit.is_keyframe {
                            keyframes += 1;
                        }
                    }
                    if encoded == before && submit_interval.is_none() {
                        // Nothing progressed: yield briefly.
                        // Hammering `submit` without respite (measured: 90
                        // million calls in 30 s) does not measure a
                        // throughput, it destroys it — each call queries the
                        // converter and keeps it under a pressure
                        // no real source would produce. The pause
                        // only happens on an unproductive round, so it
                        // cannot cap the measured throughput.
                        std::thread::sleep(Duration::from_micros(100));
                    }
                }
                Ok(())
            })();
            stop_watchdog();
            let elapsed = start.elapsed();
            let fps = encoded as f64 / elapsed.as_secs_f64();
            // A throughput measurement must say what it actually did:
            // `converter_inputs` is the number of DISTINCT conversions,
            // `conv_refus` the number of frames skipped for lack of
            // converter availability. Without these two figures, a
            // high throughput could be nothing but the same frame re-encoded.
            let telemetry = encoder.telemetry();
            let converter_inputs = telemetry
                .converter_inputs
                .load(std::sync::atomic::Ordering::Relaxed);
            let conv_refus = telemetry
                .converter_not_accepting
                .load(std::sync::atomic::Ordering::Relaxed);
            // Logged BEFORE propagating a possible error from the
            // loop: a partial measurement remains data, whereas an
            // error raised without figures says nothing about where the
            // pipeline stopped.
            tracing::info!(
                encoded,
                keyframes,
                submitted,
                elapsed_ms = elapsed.as_millis() as u64,
                fps,
                converter_inputs,
                conv_refus,
                "isolated throughput of the conversion+encoding pipeline (constant source, no capture)"
            );
            loop_result?;
            anyhow::ensure!(encoded > 0, "no image encoded in the isolated measurement");
        }

        Ok(())
    })();

    stop_jitter_on_exit.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = jitter_thread.join();
    result?;
    Ok(())
}
