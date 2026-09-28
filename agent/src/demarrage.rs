//! Starting a session: capture, encoder, source, WebRTC `Session`
//! and signaling, assembled in this order.

#[cfg(windows)]
use std::time::Duration;

use anyhow::Result;

use crate::signaling;
use crate::source::{FileSource, VideoSource};
use crate::transport::Session;
use crate::Config;
#[cfg(windows)]
use crate::{cursor, encode, gamepad, input};

/// Construction of the Windows video source, extracted to keep within the
/// 500-line cap of this file — see its head comment.
#[cfg(windows)]
mod source;

/// Construction and wiring of the audio source, extracted for the same
/// reason (task 7 of sub-block D7) — see its head comment.
#[cfg(windows)]
mod audio;

/// The real-path trace thread (`SOURCE_TRACE=1`), extracted to keep within the
/// 500-line cap of this file — see its head comment.
#[cfg(windows)]
mod trace;

/// The mic MEASUREMENT sink (`MICRO_MESURE=1`, work stream E) — see its
/// head comment, which carries all the reasoning. **Without
/// `#[cfg(windows)]`**: this sink is pure, it is tested on the host.
pub(crate) mod micro;

pub(crate) async fn executer(config: Config) -> Result<()> {
    // Filled in the Windows branch below: the captured window is
    // also the one receiving the injected inputs (task 12). `None` in
    // test file mode (no Windows window to drive) or outside
    // Windows.
    //
    // Kept as a raw address (`isize`, which is `Send`) rather
    // than as an `HWND` directly: `HWND` wraps a `*mut c_void`, not
    // `Send` in windows-rs 0.62, and therefore cannot cross the
    // `move` closure of `spawn_blocking` below as is. An HWND is only an
    // opaque identifier (not a pointer actually dereferenced on the
    // process side), passing it through its address and rebuilding it
    // in the target thread is safe — same technique as the window
    // shaking thread of the `CAPTURE_TEST` diagnostic mode (see
    // `diagnostics/capture.rs`).
    #[cfg(windows)]
    let mut window_hwnd_addr: Option<isize> = None;

    // 🔴 **`Option`, and above all NOT a `ZoneClientDeLaFenetre` default.** A
    // default would be a perfectly legitimate variant, hence a silent
    // fallback: the multi-window path that forgot to set it
    // would unmap onto the window — exactly the defect of batch 32M, back
    // through the back door. `None` cannot build any injector.
    #[cfg(windows)]
    let mut reference_entrees: Option<crate::entrees::Reference> = None;

    // Single clock origin of the session. Both media use it:
    // it is what makes their timelines comparable, and hence the A/V sync
    // exact by construction. Creating it here, only once, guarantees
    // that no initialisation duration shifts one against the other.
    let clock_origin = std::time::Instant::now();

    // Video bitrate ceiling, in bits per second. Read from `BITRATE` in
    // the Windows branch below (the only branch where the environment makes
    // sense — the test source drives no hardware encoder); otherwise the
    // default value. Brought up here, outside that branch, so that
    // `Session::new` receives the same ceiling as the one applied to
    // the encoder, without re-reading it a second time from the environment.
    // `mut` is only useful in the `#[cfg(windows)]` branch below:
    // on the test host (Linux, always `TEST_FILE`), the value never
    // varies, hence the `allow` — no point splitting the declaration by cfg
    // for a value that is read further down on both
    // platforms anyway.
    #[allow(unused_mut)]
    let mut bitrate: u32 = 12_000_000;

    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => {
            #[cfg(windows)]
            {
                let construite = source::construire(&config, clock_origin)?;
                window_hwnd_addr = Some(construite.hwnd_addr);
                bitrate = construite.bitrate;
                reference_entrees = Some(construite.reference_entrees);
                construite.source
            }
            #[cfg(not(windows))]
            {
                anyhow::bail!("TEST_FILE is required outside Windows")
            }
        }
    };

    // `receiver_task`/`sender_task`: kept by `SignalingHandle` so as not
    // to be silently abandoned (I6), but this single-session task
    // has nothing more to do with them once `closed` is observed below — we
    // therefore leave them explicitly detached rather than ignoring them by
    // accident.
    let signaling::SignalingHandle {
        mut offers,
        answers,
        mut closed,
        ice_config,
        retry_apres_s,
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(
        &signaling::url_du_relais(&config.signaling_url),
        &config.session_id,
        config.jeton.as_deref(),
    )
    .await?;
    let mut session = Session::new(source, config.local_ip, clock_origin, bitrate)?;
    // For the periodic cadence line of `piste_video` (see its doc):
    // pairing this reading with the sensor's in an `agent.log` that
    // several windows share (see `capteur/fenetre.rs`).
    session.set_session_id(&config.session_id);

    // Audio source: its absence never compromises the video session — see
    // the head comment of `demarrage::audio` for the detail of the two
    // modes (session mix, or per-window process loopback) and the fallback
    // deliberately ruled out.
    #[cfg(windows)]
    audio::brancher(&config, &mut session, clock_origin);

    // Without `MICRO_MESURE=1` it sets nothing: `micro_disponible()` stays false,
    // `ready` carries `mic: false`, and the browser's button does not appear —
    // which is what we want as long as the real cable (block E2) does not exist.
    micro::brancher(&config, &mut session);

    // The real-path trace thread (`SOURCE_TRACE=1`) has lived in
    // `demarrage/trace.rs` since sub-block P2 of the clipboard work stream —
    // extraction prior to the addition, see its head comment, which carries
    // all the reasoning about the counters it reads and those that are dead.
    #[cfg(windows)]
    let _source_trace = trace::brancher();

    // 🔴 FIX FOR THE MISSING-BRAKES HAND-OVER (fix round 1,
    // critical ②) — same gesture as `pont.rs::executer`: a volume refusal
    // (`trop-de-requetes`) closes `offers` without an offer, this process is going
    // to die, and we honour the delay suggested by the relay BEFORE giving
    // control back — see `signaling::honour_suggested_retry`.
    //
    // 🔴 **DECLARED, NOT FIXED (review, fix round 2)**: THIS
    // PROCESS IS A WINDOW'S CHILD, NOT THE BRIDGE — and the sleep
    // that follows (up to 30 s, `REPLI_MAX_MS`) delays BY AS MUCH the moment
    // `superviseur::table::orphelines::relancer_les_orphelines` sees this
    // entry become `SansSession` again and restarts it. With `RELANCES_MAX = 3`
    // tolerated (`superviseur/table.rs`) and a refusal that recurs at
    // each restart, a window's attach failure — the message "the
    // session did not hold after 3 attempts" — may therefore take from
    // a few tens of seconds to a few minutes to become visible to
    // the user, instead of a few seconds before this batch. Not measured
    // in an acceptance run; the mechanism, for its part, can be checked by cross-reading
    // `orphelines.rs` and this file.
    let Some(offer) = offers.recv().await else {
        signaling::honour_suggested_retry(&retry_apres_s).await;
        return Err(anyhow::anyhow!("the signaling closed before the offer"));
    };
    tracing::info!("offer received");

    // The web client has no trickle ICE: it sends ONE offer after
    // complete gathering and waits for ONE answer. The relayed candidate must therefore
    // exist BEFORE the answer is produced — afterwards, there is no longer
    // any way to transmit it.
    //
    // Bounded at 2 s: well below the 15 s after which the client
    // gives up (`ANSWER_TIMEOUT_MS` of `client/src/webrtc.ts`), and enough
    // for the two round trips of an authenticated allocation (bare Allocate →
    // 401 → signed Allocate).
    // Fully qualified: the import of `Duration` at the head of the file is
    // conditioned on Windows, and this sequence is common to both targets.
    const DELAI_ALLOCATION: std::time::Duration = std::time::Duration::from_secs(2);

    if let Some(config) = ice_config.borrow().clone() {
        match session.allouer_relais(config, DELAI_ALLOCATION) {
            Ok(()) => tracing::info!("TURN relay allocated before the SDP answer"),
            Err(e) => tracing::warn!(
                error = %e,
                "TURN allocation impossible: the session goes on without a relay"
            ),
        }
    }

    let answer = session.accept_offer(&offer)?;
    answers.send(answer).await?;
    tracing::info!("answer sent");
    // Note: `AgentControl::ready` is no longer sent here. At this instant SCTP
    // is not yet open (the control channel is still `None`), so
    // sending it now would be silently lost (I3 of the review).
    // `Session` queues it itself as soon as `Event::ChannelOpen("control")`.

    // I6: a loss of signaling after the initial exchange must be visible
    // rather than silent. The transport no longer depends on signaling once
    // the offer/answer are exchanged (no renegotiation in this
    // task), so we do nothing more than observe and log — but
    // we do observe it.
    tokio::spawn(async move {
        if closed.changed().await.is_ok() && *closed.borrow() {
            tracing::warn!(
                "signaling connection lost (no renegotiation possible for this session)"
            );
        }
    });

    // Cursor polling thread: decides the absolute/relative mode and the
    // shape to display. The flag is shared with the input injector,
    // messages go through the session (control channel).
    #[cfg_attr(not(windows), allow(unused_variables))]
    let mode_relatif = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let arret_sondes = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // `control_tx` only has a reader (`cursor::spawn_probe`) under Windows:
    // same reason as `mode_relatif` above, same treatment.
    #[cfg_attr(not(windows), allow(unused_variables))]
    let (control_tx, control_rx) = std::sync::mpsc::channel();
    session.set_control_source(control_rx);

    #[cfg(windows)]
    let sonde_curseur = cursor::spawn_probe(
        control_tx.clone(),
        mode_relatif.clone(),
        arret_sondes.clone(),
    );

    // True at this stage: ViGEmBus is only probed at the first gamepad state
    // received, and a possible failure will send a second `Capabilities` as false.
    // Announcing optimism avoids showing "gamepad unavailable" to a
    // user who simply has not plugged one in.
    //
    // ⚠️ **The second argument is the CLIPBOARD, and it has nothing to do with
    // the gamepad**: it is `PRESSE_PAPIER`, read by the same `actif()` as the
    // sensor. The two fallbacks below therefore replay `actif()` and NOT
    // `false` — a copied `capabilities(false, false)` would switch off pasting
    // because a gamepad is missing, which makes no sense. The validity condition
    // of this announcement lives in the field's doc
    // (`proto/src/control.rs`, `Capabilities::clipboard`): it holds because
    // sensor and child read the SAME inherited variable.
    #[cfg(windows)]
    let _ = control_tx.send(proto::control::AgentControl::capabilities(
        true,
        crate::presse_papier::actif(),
    ));

    // Clone dedicated to the transport thread below (`spawn_blocking` is
    // `move`: it must be given its own copy of the `Arc`, otherwise
    // it would capture `arret_sondes` whole and make it unavailable
    // for `arret_sondes.store(...)` after `transport.await`, below).
    #[cfg_attr(not(windows), allow(unused_variables))]
    let arret_sondes_manette = arret_sondes.clone();

    // I6: `Session::run` blocks on purpose (synchronous UDP read bounded
    // by the video cadence and str0m's deadlines). Running it on a tokio
    // async worker would freeze the other tasks of this process — here, the
    // signaling send loop — for up to a second per round, or even
    // much more once the session is no longer alive. We therefore move it
    // to tokio's blocking thread pool, dedicated to this use.
    // Cloned before the `move` closure below: it is this copy that
    // gives the `control received` trace its `session` (see `on_control`
    // below), without which it is indistinguishable from that of any other
    // child sharing the same `agent.log` (D4).
    let session_id = config.session_id.clone();
    let transport = tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        let mut injector = window_hwnd_addr
            .zip(reference_entrees)
            .map(|(addr, reference)| {
                let hwnd = windows::Win32::Foundation::HWND(addr as *mut core::ffi::c_void);
                // The input reference comes AS IS from
                // `demarrage::source`: two independent descriptions of the same
                // rectangle are what produced the defect of batch 32M, and a
                // size recomputed next to the source's is what
                // produced that of batch 32Q.
                input::InputInjector::new(hwnd, reference, mode_relatif.clone())
            });

        // Lazy wiring: at the FIRST reception of a gamepad
        // state, not at start-up — see the comment on
        // `gamepad::win::VirtualPad`. `pad_indisponible` guarantees we only
        // retry once: at the first failure, we give up for the rest
        // of the session rather than retrying at every state received.
        #[cfg(windows)]
        let mut pad: Option<gamepad::VirtualPad> = None;
        #[cfg(windows)]
        let mut pad_indisponible = false;
        // `gamepad::VirtualPad::connect()` may sleep for up to 5 s (waiting
        // for PnP enumeration on the Windows side, see its documentation): we
        // therefore NEVER call it directly here, this closure running
        // in the loop of `Session::run` which also carries video and RTCP
        // (see the comment on `spawn_blocking` above). `spawn_connect`
        // does it on a separate thread; this receiver is polled without blocking.
        #[cfg(windows)]
        let mut connexion_manette: Option<
            std::sync::mpsc::Receiver<anyhow::Result<gamepad::VirtualPad>>,
        > = None;

        let mut on_input = |message: proto::input::InputMessage| {
            #[cfg(windows)]
            if let proto::input::InputMessage::Gamepad(state) = message {
                if pad.is_none() && !pad_indisponible {
                    let rx = connexion_manette.get_or_insert_with(gamepad::spawn_connect);
                    match rx.try_recv() {
                        Ok(Ok(mut new)) => {
                            match gamepad::spawn_rumble(
                                &mut new,
                                control_tx.clone(),
                                arret_sondes_manette.clone(),
                            ) {
                                Ok(_) => {}
                                Err(e) => tracing::warn!(error = %e, "rumble unavailable"),
                            }
                            pad = Some(new);
                            connexion_manette = None;
                        }
                        Ok(Err(e)) => {
                            // Only once: `pad_indisponible` prevents any
                            // new attempt, and hence any second sending of
                            // `Capabilities` for this session.
                            pad_indisponible = true;
                            connexion_manette = None;
                            tracing::warn!(error = %e, "virtual gamepad unavailable");
                            let _ = control_tx.send(proto::control::AgentControl::capabilities(
                                false,
                                crate::presse_papier::actif(),
                            ));
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            // Connection still in progress (up to 5 s
                            // observed): this gamepad state is lost,
                            // without consequence — not thanks to the client's polling
                            // frequency (cadence under load of
                            // `setInterval(4 ms)` never measured), but because
                            // it re-emits a complete state every 100 ms
                            // even without change until the target
                            // is ready (see `client/src/gamepad.ts`,
                            // `RAFRAICHISSEMENT_MS`).
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            pad_indisponible = true;
                            connexion_manette = None;
                            tracing::warn!(
                                "connection thread to the virtual gamepad interrupted unexpectedly"
                            );
                            let _ = control_tx.send(proto::control::AgentControl::capabilities(
                                false,
                                crate::presse_papier::actif(),
                            ));
                        }
                    }
                }
                if let Some(pad) = pad.as_mut() {
                    if let Err(e) = pad.apply(&state) {
                        tracing::warn!(error = %e, "applying the gamepad state failed");
                    }
                }
                return;
            }

            // The only input log available: it was previously guarded
            // by `#[cfg(not(windows))]`, hence dead on the real target — the
            // acceptance run of work stream B (measurement 4) had to do without it and
            // reconstruct the evidence another way (instrumenting the channel
            // on the client side). Making it available under Windows too lets
            // the next diagnosis read `agent.log` directly.
            tracing::debug!(?message, "input received");

            #[cfg(windows)]
            if let Some(injector) = injector.as_mut() {
                if let Err(e) = injector.inject(message) {
                    tracing::warn!(error = %e, "input injection failed");
                }
            }
        };
        // `session`: without this field the trace is NOT attributable — all children
        // have inherited the same `agent.log` since D4. That is exactly what made
        // "2 `Resize` for 5 sessions" undecidable (D8's hand-over 10, fix I8):
        // the two lines carried no session, so nothing established
        // that they came from two distinct sessions.
        let mut on_control =
            |message| tracing::info!(session = %session_id, ?message, "control received");
        session.run(&mut on_input, &mut on_control)
    });

    match transport.await {
        Ok(Ok(())) => tracing::info!("session ended"),
        Ok(Err(e)) => {
            // `Session::run` only returns an error for a problem deemed
            // unrecoverable at the session level (see
            // `Session::begin_ending` for what is, on the contrary, treated
            // as a clean session end, via `Ok(())`).
            tracing::error!(error = %e, "fatal error in the transport loop");
            return Err(e);
        }
        Err(join_err) => {
            tracing::error!(error = %join_err, "the transport loop panicked");
            return Err(join_err.into());
        }
    }

    arret_sondes.store(true, std::sync::atomic::Ordering::Relaxed);
    #[cfg(windows)]
    let _ = sonde_curseur.join();

    Ok(())
}

/// Watchdog thread of the encoding pipeline: logs every second
/// the current Media Foundation step and the hot-path counters.
///
/// Indispensable to distinguish a call that NEVER returns (the step
/// stays frozen on the same name) from a loop that spins without progressing
/// (the step varies, the counters do not). No trace set *around* the calls
/// can make this distinction, since a blocked call never reaches its
/// exit trace — that is exactly what made the block of 28/07
/// invisible for several investigation cycles.
///
/// Returns what is needed to stop it: calling the returned closure joins the thread.
///
/// `pub(crate)` and not `pub(super)`: only `diagnostics::capture` calls it
/// today, from the other branch of the module tree.
#[cfg(windows)]
pub(crate) fn watch_encoder(telemetry: std::sync::Arc<encode::EncoderTelemetry>) -> impl FnOnce() {
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        while !flag.load(Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                etape = encode::phase_name(telemetry.phase.load(Relaxed)),
                submit_calls = telemetry.submit_calls.load(Relaxed),
                need_input_events = telemetry.need_input_events.load(Relaxed),
                have_output_events = telemetry.have_output_events.load(Relaxed),
                converter_inputs = telemetry.converter_inputs.load(Relaxed),
                converter_outputs = telemetry.converter_outputs.load(Relaxed),
                encoder_inputs = telemetry.encoder_inputs.load(Relaxed),
                encoder_outputs = telemetry.encoder_outputs.load(Relaxed),
                queued_nv12 = telemetry.queued_nv12.load(Relaxed),
                pending_input_requests = telemetry.pending_input_requests.load(Relaxed),
                skipped_busy = telemetry.skipped_busy.load(Relaxed),
                awaiting_drain = telemetry.awaiting_drain.load(Relaxed),
                conv_in_status = telemetry.converter_input_status.load(Relaxed),
                conv_out_status = telemetry.converter_output_status.load(Relaxed),
                conv_refus = telemetry.converter_not_accepting.load(Relaxed),
                "encoding pipeline supervision"
            );
        }
    });
    move || {
        stop.store(true, Relaxed);
        let _ = handle.join();
    }
}
