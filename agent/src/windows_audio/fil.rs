//! The body of the WASAPI capture thread.
//!
//! Extracted from `windows_audio.rs` in task 3 of sub-block D10, at the landing
//! point `CLAUDE.md` has named since sub-block D7. The extraction precedes
//! the addition of task 13 (`AUDIO_FAUTE_LECTURE`).
//!
//! `#![cfg(windows)]` like its parent: no host test can cover it,
//! and that is precisely why task 13 gives it fault injection.

#![cfg(windows)]

use super::*;

/// What the thread shares with `WindowsAudioSource`: the packet ring and the
/// four atomic flags. Fields keep the names of the caller's `_fil` variables so
/// the two files still read side by side.
pub(super) struct PartageFil {
    pub(super) ring_fil: PacketRing,
    pub(super) arret_fil: Arc<AtomicBool>,
    pub(super) perte_desiree_fil: Arc<AtomicI32>,
    pub(super) emet_fil: Arc<AtomicBool>,
    pub(super) capture_morte_fil: Arc<AtomicBool>,
}

/// Body of the audio capture thread, launched by `WindowsAudioSource::start`.
///
/// Every value is passed explicitly; none is captured by a closure any more.
pub(super) fn tourner(
    mut capture: Capture,
    mut encodeur: OpusEncoder,
    origin: Instant,
    partage: PartageFil,
    pid_fil: Option<u32>,
) {
    let PartageFil {
        ring_fil,
        arret_fil,
        perte_desiree_fil,
        emet_fil,
        capture_morte_fil,
    } = partage;
    // This thread itself calls COM methods — `read()` at each
    // round, and `Stop()` through `LoopbackCapture`'s `Drop` when
    // leaving — whereas `open()` initialised COM on the
    // CALLING thread, not on this one. Microsoft requires every thread
    // invoking COM methods to have first joined an
    // apartment.
    //
    // Result deliberately ignored HERE — unlike
    // `wasapi::open`, which **checks** its `HRESULT` and refuses
    // `RPC_E_CHANGED_MODE` (see its comment, on which
    // `unsafe impl Send for LoopbackCapture` depends): this thread has just
    // been created by `thread::Builder::spawn` right above,
    // so it has not yet joined any COM apartment, and
    // `CoInitializeEx` necessarily returns `S_OK` there. `open()`,
    // for its part, runs on an arbitrary thread — potentially
    // recycled, potentially already bound to an STA — hence the
    // check that has no reason to be repeated here.
    //
    // Symmetrically, NO `CoUninitialize`: see the reason
    // detailed in `wasapi.rs`.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let mut assembleur = FrameAssembler::new(origin);
    let mut last_report = Instant::now();
    // Last value actually set on the encoder. A
    // CTL call per 10 ms frame would be waste on this
    // hot path: we only rewrite when the target has changed.
    let mut derniere_perte: i32 = 0;
    // Last order for which a toggle was ATTEMPTED (whether
    // `capture.emettre` succeeded or not). Only used not to
    // call `Start()`/`Stop()` again at each round at ~200 Hz: it
    // is NOT the real state of the stream, see `emettait`.
    let mut voulu_applique = false;
    // REAL state of the stream: true only when `Start()` has
    // actually succeeded, written ONLY in the
    // `Ok` branch below. Governs both the
    // reading/encoding gate below and the `actif` trace of
    // "audio counters" — the only window onto a frozen
    // arbitration. If a `Start()` refusal made this value lie
    // (as an unconditional `emettait = veut_emettre` would),
    // the trace would announce an audible window that captures
    // nothing, exactly the silent failure mode
    // this trace exists to reveal.
    let mut emettait = false;
    // Consecutive read errors. Reset to zero by any
    // read that succeeds — including `Ok(None)`, which is the common
    // case: the stream simply has nothing new to return.
    let mut lectures_echouees: u32 = 0;

    // BENCH VARIABLE, never a shipped configuration — same status as
    // `PART_SONDAGE`. It exists because no natural trigger of capture
    // death could be found: D9's FOUR (Restart-Service
    // Audiosrv, Stop/Start, Stop-Process audiodg, Disable/Enable-PnpDevice)
    // produced NO "audio read failed" line in nine filed
    // runs — *process loopback* capture follows the PROCESS TREE, not
    // the service or the device.
    //
    // ⚠️ It establishes that the REMEDY works, never that a natural cause
    // exists. Do not read an acceptance run using it as proof of
    // robustness in production.
    //
    // ⚠️ **PROCESS-GLOBAL, not local to this thread** (found during VM acceptance,
    // task 14): a per-thread budget re-arms fully at each
    // rebuild — `std::env::var` reread identically by the new thread —,
    // so EACH rebuilt capture dies in turn before any real call
    // to `capture.read()`, whatever the number of rebuilds. A
    // check that can never return the other value ("real audio
    // after rebuild") is not one — exactly the pattern this
    // repository has just paid for on this same file. A shared `static`,
    // decremented by `fetch_update`, makes the budget run out ONCE
    // for the whole process: the first thread consumes the 10 faults and
    // dies, and the very first rebuild finds the counter at zero,
    // reaches the `else` branch, and reads for real.
    //
    // ⚠️ **`AUDIO_FAUTE_LECTURE_MS` bounds this arming IN TIME**, and
    // it is what makes the fallback on promotion demonstrable — see
    // `crate::audio::injection_encore_armee`, which carries the full
    // arithmetic. Absent: `None`, unlimited, D10's behaviour strictly
    // preserved.
    //
    // **The origin is captured when the BUDGET is initialised, not at the first
    // `read()`**: both processes — the carrier and its neighbour — start
    // together, and that is what gives them the same origin without any
    // session having to be named.
    static FAUTES_A_INJECTER: std::sync::OnceLock<(
        std::sync::atomic::AtomicU32,
        Option<Duration>,
        std::time::Instant,
    )> = std::sync::OnceLock::new();
    let (fautes_a_injecter, fenetre_injection, origine_injection) =
        FAUTES_A_INJECTER.get_or_init(|| {
            let v: u32 = std::env::var("AUDIO_FAUTE_LECTURE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let fenetre = std::env::var("AUDIO_FAUTE_LECTURE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_millis);
            if v > 0 {
                // A SINGLE `warn!`, enriched with `fenetre_ms`: two traces at the
                // same instant would count as two events (sub-block D6's
                // home-grown trap). `fenetre_ms` says which of the two
                // configurations is running — without it, a log does not let one
                // distinguish a bounded arming from an unlimited one.
                tracing::warn!(
                    fautes_a_injecter = v,
                    fenetre_ms = fenetre.map(|f| f.as_millis() as u64),
                    "injection de fautes de lecture audio ARMEE (banc)"
                );
            }
            (
                std::sync::atomic::AtomicU32::new(v),
                fenetre,
                std::time::Instant::now(),
            )
        });

    while !arret_fil.load(Ordering::Relaxed) {
        let veut_emettre = emet_fil.load(Ordering::Relaxed);
        if veut_emettre != voulu_applique {
            voulu_applique = veut_emettre;
            match capture.emettre(veut_emettre) {
                Ok(()) => {
                    // Real resumption (Start() succeeded after an
                    // outage, or first start): re-anchor
                    // the assembler BEFORE it replays the whole
                    // outage as a burst of silence (see
                    // `FrameAssembler::reancrer`).
                    if veut_emettre && !emettait {
                        assembleur.reancrer();
                    }
                    emettait = veut_emettre;
                }
                Err(e) => {
                    // A refusal does not kill the session: we
                    // log and will retry at the next
                    // order change rather than at every round
                    // (thanks to `voulu_applique`, updated
                    // above). `emettait` DOES NOT MOVE: it is
                    // the real state of the stream, and it has not changed.
                    tracing::warn!(
                        error = %e,
                        actif = veut_emettre,
                        "bascule d'emission audio refusee"
                    );
                }
            }
        }
        // ⚠️ **THIS BLOCK IS BEFORE THE `!emettait` GATE, AND THAT IS
        // ITS WHOLE POINT** (F1, final branch review of
        // sub-block D7). It lived at the end of the loop body,
        // that is, AFTER the `continue` of the silent branch:
        // the trace was therefore only reachable when `emettait`
        // was true, and its `actif` field was
        // structurally `true` — D8's entry check
        // (`grep -c 'actif=true'` against the total count) was
        // **unsatisfiable**, and the defect it exists to
        // reveal — no window carries the sound any more — returned
        // 0 and 0, which those same documents classified as benign.
        // A silent window now reports too, every
        // `REPORT_INTERVAL`.
        //
        // The three counters stay readable while silent: they
        // live on `ring_fil` and `assembleur`, which this thread
        // owns, and their accessors only take `&self`.
        if last_report.elapsed() >= REPORT_INTERVAL {
            last_report = Instant::now();
            // `info!`, not `debug!`: the default filter
            // (`agent/src/main.rs`, `EnvFilter` falling back to
            // `"info"` when `RUST_LOG` is absent) never emits
            // `debug!` logs in normal operation. The
            // spec (§5, §9) promises counters "logged
            // periodically and never silent" — one
            // record every `REPORT_INTERVAL` (30 s)
            // is not noise, and a silent rejection counter
            // is exactly what would make an audio degradation
            // invisible during acceptance.
            //
            // `pid` and `actif` are the only way to observe a
            // frozen arbitration: if no window ever carried the
            // sound again, all would report `actif=false`
            // — the symptom would otherwise be total silence, without a
            // `WARN`, without an error. It is the entry `grep` of the
            // next sub-block (spec §6).
            tracing::info!(
                pid = pid_fil,
                actif = emettait,
                rejetes = ring_fil.rejetes(),
                complements = assembleur.complements(),
                echantillons_jetes = assembleur.echantillons_jetes(),
                "compteurs audio"
            );
        }

        if !emettait {
            // Silent: read nothing, encode nothing, deposit
            // nothing. An encoded silence frame would cost
            // a few bytes thanks to DTX, but it would reach
            // the browser — and two windows of the same process
            // would both be heard.
            std::thread::sleep(POLL_INTERVAL);
            continue;
        }

        // `fetch_update`: atomically decrements IF the global budget
        // is not already at zero (`checked_sub(1)` returns `None` at zero, which
        // makes `fetch_update` fail without touching it). A budget exhausted by
        // ANOTHER thread (the very first capture, typically) therefore lets
        // this one — and any thread born after it — read for real from its
        // first round.
        //
        // ⚠️ **THE ORDER OF THE `&&` OPERANDS MATTERS**: `fetch_update`
        // DECREMENTS. Placing it second guarantees that no fault is
        // consumed once the window has closed — the short-circuit of `&&`
        // then never evaluates the decrement. Swapped, the carrier would
        // keep burning its budget after the deadline, and the promoted
        // neighbour would find it empty: the check would again be unable to
        // return the other value.
        let lecture = if crate::audio::injection_encore_armee(
            origine_injection.elapsed(),
            *fenetre_injection,
        ) && fautes_a_injecter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .is_ok()
        {
            Err(anyhow::anyhow!("faute injectée (AUDIO_FAUTE_LECTURE)"))
        } else {
            capture.read()
        };

        match lecture {
            Ok(Some(bloc)) => {
                lectures_echouees = 0;
                assembleur.push(&bloc);
            }
            Ok(None) => lectures_echouees = 0,
            Err(e) => {
                // **An ISOLATED error does not condemn a whole
                // PID group** (F3, final branch review of
                // sub-block D7): we retry, and only give up
                // after `LECTURES_ECHOUEES_MAX` failures in a row.
                // The full reason and the backoff live on
                // `audio::LECTURES_ECHOUEES_MAX` and
                // `audio::temporisation_de_reprise`, tested on
                // the host.
                lectures_echouees = lectures_echouees.saturating_add(1);
                if lectures_echouees < LECTURES_ECHOUEES_MAX {
                    tracing::warn!(
                        error = %e,
                        consecutives = lectures_echouees,
                        "lecture audio échouée, nouvelle tentative"
                    );
                    std::thread::sleep(temporisation_de_reprise(lectures_echouees));
                    continue;
                }
                // Final abandonment. The witness is set BEFORE the
                // trace, so that no order handled between the two
                // can declare itself applied to an already
                // dead capture.
                capture_morte_fil.store(true, Ordering::Relaxed);
                // The counters are included here because it is the
                // last log line of this thread: without them, a
                // capture dead mid-session would be
                // indistinguishable from mere silence —
                // `next_packet` would keep returning `None` as
                // in the nominal case.
                tracing::warn!(
                    error = %e,
                    consecutives = lectures_echouees,
                    rejetes = ring_fil.rejetes(),
                    complements = assembleur.complements(),
                    echantillons_jetes = assembleur.echantillons_jetes(),
                    "lecture audio échouée, capture arrêtée définitivement"
                );
                // ⚠️ **THIS COMMENT ANNOUNCED A GAP ALREADY FILLED AT THE
                // MOMENT IT WAS WRITTEN HERE** — orphan found by
                // task 13 of sub-block D10 (`git show
                // c9b7a31:agent/src/windows_audio.rs`, the divergence point
                // of this branch, already carries the signal it
                // says is missing). The `capture_morte_fil` witness is
                // indeed not observable by the sensor — it lives
                // here, in the child — but `transport/tick.rs` (branch
                // a1sexies) reads it and pushes `VersCapteur::AudioMort`
                // (`capteur/protocole.rs`) since D9; it is NO LONGER "to be
                // framed". **And since D10 (tasks 11-12), it is not even
                // the first gesture any more**: the session FIRST tries to
                // rebuild the capture (`reconstruire_ou_signaler`,
                // `transport/piste_audio.rs`); `AudioMort` is only its
                // fallback, when the attempt budget is exhausted — and
                // it is ONLY in that fallback that `capteur/audio.rs`
                // can promote a neighbour of the same PID group. A
                // window alone in its group therefore depends entirely on
                // the rebuild.
                return;
            }
        }

        for trame in assembleur.drain_due(Instant::now()) {
            let voulue = perte_desiree_fil.load(Ordering::Relaxed);
            if voulue != derniere_perte {
                match encodeur.set_packet_loss_perc(voulue) {
                    Ok(()) => derniere_perte = voulue,
                    Err(e) => {
                        // Encoder refusal: we will retry at the
                        // next target change rather than
                        // replaying this call at every frame.
                        derniere_perte = voulue;
                        tracing::warn!(
                            error = %e,
                            value = voulue,
                            "réglage du taux de perte Opus refusé"
                        );
                    }
                }
            }
            match encodeur.encode(&trame.pcm) {
                Ok(data) => ring_fil.push(AudioPacket {
                    data,
                    pts_48k: trame.pts_48k,
                    captured_at: trame.captured_at,
                }),
                Err(e) => {
                    // Same reasoning as for the read
                    // error above: last log line
                    // of this thread, hence last chance to make
                    // the accumulated counters usable — and
                    // same witness, for the same reason (F3).
                    //
                    // **No tolerance here**, unlike
                    // reading: an Opus encoder refusal on
                    // a well-formed frame stems from no known
                    // transient cause, whereas a WASAPI refusal
                    // has several.
                    capture_morte_fil.store(true, Ordering::Relaxed);
                    tracing::warn!(
                        error = %e,
                        rejetes = ring_fil.rejetes(),
                        complements = assembleur.complements(),
                        echantillons_jetes = assembleur.echantillons_jetes(),
                        "encodage Opus échoué, capture arrêtée définitivement"
                    );
                    return;
                }
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}
