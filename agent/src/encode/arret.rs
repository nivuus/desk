//! Putting an encoder's MFTs at rest before releasing them.
//!
//! Separated from `encode.rs` (already in size debt, see `CLAUDE.md`) rather
//! than added to it.

use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Media::MediaFoundation::{
    IMFShutdown, IMFTransform, MFSHUTDOWN_COMPLETED, MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    MFT_MESSAGE_NOTIFY_END_STREAMING, MFT_MESSAGE_TYPE,
};

// The serialized work queue imposed on the MFT, and its barrier.
mod file;
pub(super) use file::FileMft;

/// Puts an encoder's two MFTs at rest, in order, before their
/// COM references are released.
///
/// **Why this sequence exists** — survey of task 2bis (two
/// crashes, identical stacks, symbolised): the fault is raised by
/// `RtlEnterCriticalSection` in code of `nvEncMFTH264x.dll` running under
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem`, that is **on a thread of
/// the Media Foundation work queue**, on a critical section whose
/// `DebugInfo` is `NULL`. The MFT therefore still has a work item in flight
/// when we destroy the encoder.
///
/// In both dumps of 2bis, the main thread was simultaneously in
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown`. Removing
/// `MFShutdown` from the path did NOT prevent the fault: this call is therefore not
/// **necessary** for the fault (see `super::start_media_foundation` for the
/// exact scope of this survey).
///
/// What handles it is the PAIR stop + barrier, and each of the two had to be
/// removed separately to establish it: the stop alone lets the fault
/// come back (1 out of 10), the barrier alone too (2 out of 5). Neither
/// is redundant — see `arreter` and `FileMft`.
///
/// Upstream (the converter) is put at rest before downstream (the encoder): it
/// must no longer produce anything while the one consuming is being stopped.
///
/// Each step logs itself: these calls can block without returning
/// control, and the last line written is then the only way to know which one.
/// They only run at the destruction of an encoder, never per frame. The
/// detail is at `debug`, but the two traces that bracket
/// `IMFShutdown::Shutdown` — the only call whose freeze was OBSERVED — are at
/// `info`, hence readable under the `RUST_LOG=info` of `scripts/run-agent.sh`:
/// a mitigation that is silent in operation is not one.
///
/// # When this sequence runs, and what it costs at worst
///
/// **It is not a path reserved for the upcoming multi-window work: it runs
/// TODAY, in single-window production.** `Drop for H264Encoder` runs
/// at each replacement of the session's encoder — `set_encode_size`,
/// called by `transport::adaptation` at **each rung change** of the
/// network, and `resize` / the chain rebuild on resize (both
/// in `windows_source.rs`). Both run on the single thread of
/// `Session::run` (`spawn_blocking`): whatever blocks here freezes the **whole**
/// session — capture, encoding, RTP, ICE — without recovery.
///
/// **Worst-case bound, per encoder destruction**: the bounded part is at
/// most `2 × DELAI_BARRIERE + 2 × DELAI_ARRET_MFT` = **8 s** (two barriers,
/// plus one stop confirmation per MFT), reduced to **6 s** where the
/// converter does not expose `IMFShutdown`. **The total is bounded by nothing
/// for all that**: neither the four `ProcessMessage` nor the two
/// `IMFShutdown::Shutdown` (see `arreter`). Nominal survey, out of all
/// proportion: 0.5 ms per encoder, 4.0 ms for eight in a row, `attente_ms=0`
/// everywhere (`paralleles-n8.log`) — **but a nominal value is not a bound**.
pub(super) fn put_to_rest(
    convertisseur: &IMFTransform,
    encodeur: &IMFTransform,
    file_encodeur: &FileMft,
) {
    // End of stream: unchanged, it is what `Drop` already did.
    message(
        convertisseur,
        "convertisseur",
        "END_OF_STREAM",
        MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    );
    message(
        convertisseur,
        "convertisseur",
        "END_STREAMING",
        MFT_MESSAGE_NOTIFY_END_STREAMING,
    );
    message(
        encodeur,
        "encodeur",
        "END_OF_STREAM",
        MFT_MESSAGE_NOTIFY_END_OF_STREAM,
    );
    message(
        encodeur,
        "encodeur",
        "END_STREAMING",
        MFT_MESSAGE_NOTIFY_END_STREAMING,
    );

    // TWO MESSAGES DELIBERATELY ABSENT, and it is not an oversight.
    //
    // `MFT_MESSAGE_COMMAND_FLUSH` and `MFT_MESSAGE_SET_D3D_MANAGER` to zero
    // (two leads of brief 2ter) were set here, then removed on evidence:
    // with them, at N = 4 encoders, **one run out of four blocked
    // without returning** in this block, right after putting encoder
    // no. 0 at rest and during that of no. 1 (log stopped at "libération d'un
    // encodeur : avant id=1", process still alive thirteen minutes
    // later, `Responding: True`). The block is **bounded to this block**: the next
    // trace was never written. The two end-of-stream messages above, for their part,
    // predate this work stream and never blocked. Evidence committed:
    // `docs/superpowers/plans/journaux-duplications-paralleles/2ter-blocage-n4-flush-setd3dmanager.log`.
    // NOT established: which of the two blocked, nor why.

    // THE CONVERTER HAS NO IMPOSED QUEUE, and it is a measured trade-off,
    // not an oversight. The review is right in principle: `create_color_converter`
    // first tries `find_hardware_video_processor()`, and on a host where a
    // HARDWARE Video Processor is registered, the converter would be a hardware
    // MFT with its own asynchronous work, which nothing here covers.
    // On this VM it is always the software fallback that comes out, hence a
    // synchronous MFT.
    //
    // Imposing a queue and a barrier on it was done, then removed: in that
    // form (8 serialised queues at N = 4 instead of 4), one run out of six at
    // N = 4 **froze in the encoder's `IMFShutdown::Shutdown`**, trace
    // "Shutdown : avant" written, "après" never
    // (`2ter-gel-n4-shutdown.log`). The form without a queue on the converter had,
    // for its part, passed 16 runs at N = 4 without a freeze. Covering a case that exists
    // on no tested machine, at the cost of a freeze observed on the one being
    // tested, is a bad trade.
    //
    // NOT established: that the converter's queue is the CAUSE of this freeze. It is
    // the only structural difference between the two forms, and the freeze only
    // appeared with it — over six runs. `Shutdown()` may just as well
    // carry this risk on its own (see `arreter`).

    // Barrier: nothing that was already queued is still running.
    file_encodeur.barriere("encodeur", "après END_STREAMING");

    // Explicit stop of the MFTs, then a SECOND barrier: the stop itself drops
    // work onto the queue, and it is precisely that work that must be
    // waited for. See `arreter` for what makes these two calls necessary.
    arreter(convertisseur, "convertisseur");
    arreter(encodeur, "encodeur");

    file_encodeur.barriere("encodeur", "après IMFShutdown");
}

/// Safeguard of the wait for an MFT's stop confirmation.
const DELAI_ARRET_MFT: Duration = Duration::from_secs(2);

/// Asks an MFT to stop its work queues, and waits for it to
/// confirm.
///
/// `IMFShutdown::Shutdown` is the documented mechanism by which an MFT client
/// obtains this stop — it is what the Media Foundation pipeline
/// itself does, through `MFShutdownObject`, when it tears down a topology node. We
/// call it directly rather than through `MFShutdownObject` to know, and
/// be able to log, whether the MFT even exposes this interface
/// (`MFShutdownObject` returns `S_OK` silently when it ignores it), and to
/// be able to wait for the confirmation through `GetShutdownStatus`.
///
/// # Two surveys that seem to contradict each other, and what they say
///
/// 1. **On its own, this call is not enough.** It returns `MFSHUTDOWN_COMPLETED` in 0 ms
///    and the fault occurs anyway: 1 recurrence out of 10 runs, the
///    last log line before death being precisely the stop
///    confirmation (`2ter-recidive-apres-imfshutdown-pile.log`).
///    **An MFT's `MFSHUTDOWN_COMPLETED` therefore does not prove the absence
///    of a work item in flight concerning it.**
/// 2. **But it is necessary.** Removed from the path leaving the barrier
///    alone, the fault came back **2 times out of 5 runs**, identical stack and offset,
///    even though the barrier had been crossed
///    (`2ter-recidive-barriere-seule-agent.log` and `…-pile.log`). Barrier and
///    stop are not
///    redundant: the stop makes the MFT cease, the barrier waits for what it
///    leaves behind. That is why the second barrier follows this call.
///
/// # What this call costs as a risk, and why it stays
///
/// `Shutdown()` is bounded by NOTHING — the safeguard below only bounds the
/// confirmation loop that follows. An unbounded call in a `Drop` freezes the
/// whole session, which would be worse than the crash being fixed. **And it
/// is not a risk deferred to multi-window**: this `Drop` already runs in
/// single-window production — see `put_to_rest`, which names both paths
/// and writes the worst-case bound.
///
/// **And this freeze was OBSERVED, in this precise call.** At N = 4, a run
/// stopped on `IMFShutdown::Shutdown : before mft="encodeur"` (id=2,
/// 18:00:27,347197) without ever writing its `après`, process still alive
/// thirteen minutes later:
/// `docs/superpowers/plans/journaux-duplications-paralleles/2ter-gel-n4-shutdown.log`.
/// It is therefore not a theoretical risk.
///
/// **The cause is NOT attributed.** This run also carried a queue
/// imposed on the converter, removed since (see `put_to_rest`); the
/// decision between the two was not made, and six runs would not have
/// allowed it. That the freeze disappeared with this queue does not prove that it
/// came from it.
///
/// The call stays all the same, for lack of a safe alternative: moving it to another
/// thread would require passing a COM interface across an apartment
/// boundary (the main thread is in an STA — the
/// `ClassicSTAThreadWaitForHandles` frames of the 2bis dump), which would trade a
/// risk for a certain defect. What remains established, and nothing more:
/// the call is bracketed by two **`info`** traces — and not `debug`, otherwise the
/// mitigation would be silent under the operational `RUST_LOG=info` —, so
/// that a freeze can be read instead of staying silent; that is how this one was
/// seen.
fn arreter(mft: &IMFTransform, quoi: &'static str) {
    let arret: IMFShutdown = match mft.cast() {
        Ok(arret) => arret,
        Err(err) => {
            // Surveyed, not assumed: if the interface is missing, the log says so,
            // and we know this path stopped nothing at all. It is the case
            // of the software converter on this VM (`0x80004002`).
            tracing::debug!(mft = quoi, error = %err, "MFT sans IMFShutdown : pas d'arrêt explicite");
            return;
        }
    };

    // `info` and not `debug`: the only mitigation of the only unbounded call, and
    // operation runs at `RUST_LOG=info`. Two lines per encoder
    // destruction, never per frame. DO NOT LOWER IT.
    tracing::info!(mft = quoi, "IMFShutdown::Shutdown : avant");
    if let Err(err) = unsafe { arret.Shutdown() } {
        tracing::warn!(mft = quoi, error = %err, "IMFShutdown::Shutdown refusé");
        return;
    }
    tracing::info!(mft = quoi, "IMFShutdown::Shutdown : après");

    let debut = Instant::now();
    loop {
        match unsafe { arret.GetShutdownStatus() } {
            Ok(statut) if statut == MFSHUTDOWN_COMPLETED => {
                tracing::debug!(
                    mft = quoi,
                    attente_ms = debut.elapsed().as_millis() as u64,
                    "arrêt de la MFT confirmé"
                );
                return;
            }
            // `MFSHUTDOWN_INITIATED`: the stop is still running, we go round again.
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(
                    mft = quoi,
                    error = %err,
                    "GetShutdownStatus indisponible : arrêt demandé mais non confirmable"
                );
                return;
            }
        }
        if debut.elapsed() >= DELAI_ARRET_MFT {
            tracing::warn!(
                mft = quoi,
                delai_ms = DELAI_ARRET_MFT.as_millis() as u64,
                "arrêt de la MFT non confirmé dans le délai : on relâche quand même"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Sends a message to an MFT, bracketing the call with two traces: a call
/// that does not return can then be read in the log.
fn message(mft: &IMFTransform, quoi: &'static str, nom: &'static str, message: MFT_MESSAGE_TYPE) {
    tracing::debug!(mft = quoi, message = nom, "mise au repos : avant");
    let issue = unsafe { mft.ProcessMessage(message, 0) };
    tracing::debug!(
        mft = quoi,
        message = nom,
        refuse = issue.is_err(),
        "mise au repos : après"
    );
}
