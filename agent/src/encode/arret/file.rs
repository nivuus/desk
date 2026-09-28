//! The serialized Media Foundation work queue imposed on the encoder's MFT,
//! and the barrier that waits for it to drain: the second half of the
//! stop + barrier pair described in `arret.rs`.
//!
//! Split out of `arret.rs` when `cargo fmt` pushed that file past 500 lines.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::E_NOTIMPL;
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncCallback_Impl, IMFAsyncResult, IMFRealTimeClientEx, IMFTransform,
    MFAllocateSerialWorkQueue, MFPutWorkItem, MFUnlockWorkQueue,
    MFASYNC_CALLBACK_QUEUE_MULTITHREADED,
};

/// Safeguard of the wait for a work queue barrier.
///
/// The barrier bears on an OBSERVABLE CONDITION (has our own work
/// item been executed), not on a duration. This delay is only a
/// safeguard against a queue that would no longer dispatch anything; crossing it is
/// logged at `error` and **changes the behaviour of `Drop`** (see
/// `Drop for FileMft`).
const DELAI_BARRIERE: Duration = Duration::from_secs(2);

/// **Serialised** Media Foundation work queue dedicated to ONE MFT, and
/// imposed on it.
///
/// **Why.** The fault noted by task 2bis runs under
/// `CSerialWorkQueue::QueueItem::ExecuteWorkItem`: a work item of the
/// NVIDIA MFT is still running when we release the encoder. Two documented
/// mechanisms were measured unable to prevent it — `IMFShutdown::Shutdown`
/// (1 recurrence out of 10) and the removal of `MFShutdown` (1 recurrence out of 5).
///
/// `IMFRealTimeClientEx::SetWorkQueueEx` lets the client DICTATE to the MFT the
/// queue on which it will drop its asynchronous work — surveyed: both
/// MFTs of the encoder expose the interface. By imposing on it a
/// **serialised** queue, whose semantics is to run one item at a time in
/// drop order, we aim for a barrier: drop our own item and
/// wait for it to run.
///
/// It is a wait **bounded on an observable condition**, not a delay.
///
/// # Identified residual failure mode — nesting of serialised queues
///
/// **This is not a stylistic caveat, it is the precise mechanism by which
/// this barrier could bar nothing.** The idiomatic implementation of
/// `SetWorkQueueEx` in a Media Foundation object is
/// `MFAllocateSerialWorkQueue(client_queue, &its_own_queue)`: the object
/// stacks ITS serialised queue on ours, its items wait at its place and
/// are dispatched to us **only one at a time**. Our sentinel, dropped
/// directly on our queue, would then be scheduled behind **a single**
/// one of its items, not behind all of them — and the barrier would only be a
/// window reduction, not a guarantee.
///
/// The clue that makes the hypothesis serious: the stack from BEFORE the fix already carries
/// `CSerialWorkQueue` frames although no serialised queue was
/// allocated by us — the MFT therefore already had one of its own.
///
/// What is measured, and which only settles half of the question:
/// blocking our queue during encoding **stops the encoder** (test
/// `MULTIFENETRE_EPREUVE_FILE_MS`, see the 2ter report). The MFT's work
/// does go through our queue — but that remains true whether the nesting
/// exists or not, since blocking the target queue also blocks the queue stacked
/// on it. **What would decide**: symbolising a recurrence occurring despite
/// the barrier, or observing the MFT's internal queue (no API
/// exposes it).
pub(in crate::encode) struct FileMft {
    /// `None` if the allocation or the imposition on the MFT failed: we then continue
    /// without a barrier rather than refusing to build the encoder.
    ///
    /// **What the encoder is then worth**: the "stop only" configuration,
    /// **measured at 1 recurrence out of 10** — the original defect, mitigated but
    /// present. Not silent for all that: the three paths that lead here
    /// log a `warn!` (`allouer`, then the two failures of `confier`)
    /// and the nominal case an `info!` — a log therefore says, encoder by
    /// encoder, which one is protected. The release, for its part, does not say it again:
    /// `barriere` returns without a word if the queue is missing.
    id: Option<u32>,
    /// True if a barrier was not crossed within the delay. The queue then carries,
    /// at the very least, our unexecuted sentinel: releasing it would be
    /// worse than keeping it (see `Drop`).
    compromise: AtomicBool,
}

impl FileMft {
    /// Allocates a serialised queue, **without yet handing it to anyone**.
    ///
    /// Separated from `confier` for a lifetime reason, not a stylistic one: local
    /// variables are destroyed in the **reverse** order of their
    /// declaration. By allocating here, before the MFT exists, the `FileMft`
    /// is the oldest local and therefore the **last** destroyed if the
    /// construction of the encoder fails further on a `?` — the queue is
    /// then released only after the release of the MFT that holds it. The
    /// reverse order (allocating after the MFT) released the queue first, exactly
    /// the inversion the field order of `H264Encoder` is designed to
    /// avoid. This path is not theoretical: `windows_source.rs` handles
    /// the construction failure of an encoder and continues the session.
    ///
    /// Requires Media Foundation to be started.
    pub(in crate::encode) fn allouer() -> Self {
        match unsafe { MFAllocateSerialWorkQueue(MFASYNC_CALLBACK_QUEUE_MULTITHREADED) } {
            Ok(id) => Self {
                id: Some(id),
                compromise: AtomicBool::new(false),
            },
            Err(err) => {
                tracing::warn!(error = %err, "allocation de file sérialisée refusée");
                Self {
                    id: None,
                    compromise: AtomicBool::new(false),
                }
            }
        }
    }

    /// Imposes the queue on an MFT. To be called **before** any stream starts.
    pub(in crate::encode) fn confier(&mut self, mft: &IMFTransform, quoi: &'static str) {
        let Some(id) = self.id else { return };
        let client = match mft.cast::<IMFRealTimeClientEx>() {
            Ok(client) => client,
            Err(err) => {
                tracing::warn!(mft = quoi, error = %err, "MFT sans IMFRealTimeClientEx : pas de barrière");
                self.rendre();
                return;
            }
        };
        // Priority 0: the base priority of items, not a real-time
        // setting — we ask for no scheduling privilege.
        if let Err(err) = unsafe { client.SetWorkQueueEx(id, 0) } {
            tracing::warn!(mft = quoi, error = %err, file = id, "la MFT refuse la file imposée");
            self.rendre();
            return;
        }
        tracing::info!(mft = quoi, file = id, "file de travail sérialisée imposée");
    }

    /// Releases the queue immediately, when no one holds it yet.
    fn rendre(&mut self) {
        if let Some(id) = self.id.take() {
            let _ = unsafe { MFUnlockWorkQueue(id) };
        }
    }

    /// Waits for everything dropped on the queue before this call to have finished
    /// running.
    pub(super) fn barriere(&self, quoi: &'static str, quand: &'static str) {
        let Some(id) = self.id else { return };
        let fait = Arc::new((Mutex::new(false), Condvar::new()));
        let rappel: IMFAsyncCallback = Sentinelle { fait: fait.clone() }.into();
        if let Err(err) = unsafe { MFPutWorkItem(id, &rappel, None) } {
            // The queue no longer dispatches: our sentinel is not in it, but the
            // MFT's work, for its part, may have stayed there.
            tracing::error!(mft = quoi, error = %err, quand, "dépôt de la sentinelle refusé : file NON barrée");
            self.compromise.store(true, Ordering::SeqCst);
            return;
        }
        let debut = Instant::now();
        let (verrou, signal) = &*fait;
        let mut pose = verrou.lock().unwrap_or_else(|e| e.into_inner());
        while !*pose {
            let (garde, issue) = signal
                .wait_timeout(pose, DELAI_BARRIERE)
                .unwrap_or_else(|e| e.into_inner());
            pose = garde;
            if !*pose && issue.timed_out() {
                // We cannot refuse to proceed: `Drop` must finish. What
                // we can do is not MAKE IT WORSE — see `Drop`.
                tracing::error!(
                    mft = quoi,
                    quand,
                    delai_ms = DELAI_BARRIERE.as_millis() as u64,
                    "barrière non franchie : les références COM vont être relâchées avec du \
                     travail possiblement encore en file — c'est le défaut d'origine, non barré"
                );
                self.compromise.store(true, Ordering::SeqCst);
                return;
            }
        }
        tracing::debug!(
            mft = quoi,
            quand,
            attente_ms = debut.elapsed().as_millis() as u64,
            "barrière franchie"
        );
    }

    /// Occupies the queue for `duree`, to test whether the MFT's work really
    /// goes through it (see the residual failure mode documented above).
    ///
    /// Measurement probe, only called under an environment variable. Returns
    /// immediately: it is the queue that stays occupied.
    pub(in crate::encode) fn bloquer(&self, duree: Duration) {
        let Some(id) = self.id else {
            tracing::warn!("épreuve de file demandée mais aucune file imposée");
            return;
        };
        let rappel: IMFAsyncCallback = Bouchon { duree }.into();
        match unsafe { MFPutWorkItem(id, &rappel, None) } {
            Ok(()) => tracing::info!(
                file = id,
                duree_ms = duree.as_millis() as u64,
                "épreuve : file bouchée"
            ),
            Err(err) => tracing::warn!(error = %err, "épreuve : dépôt du bouchon refusé"),
        }
    }
}

impl Drop for FileMft {
    fn drop(&mut self) {
        let Some(id) = self.id else { return };
        if self.compromise.load(Ordering::SeqCst) {
            // Releasing a queue some of whose items were not dispatched
            // would add a defect to the one we could not avoid. We
            // keep it: an unlock with pending items costs a crash.
            //
            // WHAT THE LEAK COSTS, without minimising it: not "a few
            // bytes" but a platform object backed by the RTWorkQ thread
            // pool, registered for the life of the process. Each expiry
            // leaks one permanently, and nothing counts them or
            // caps them: a trade-off to reopen if these `error!`s stopped being
            // exceptional — no expiry observed to date.
            tracing::error!(
                file = id,
                "file compromise : NON rendue, délibérément fuitée"
            );
            return;
        }
        // Field declared last in `H264Encoder`, and local declared
        // first in `H264Encoder::new`: in both cases the queue is only
        // released after the release of the MFT that uses it.
        let _ = unsafe { MFUnlockWorkQueue(id) };
    }
}

/// Work item without effect, whose only purpose is to signal
/// its own passage: it is what makes the barrier observable.
#[implement(IMFAsyncCallback)]
struct Sentinelle {
    fait: Arc<(Mutex<bool>, Condvar)>,
}

impl IMFAsyncCallback_Impl for Sentinelle_Impl {
    fn GetParameters(&self, _drapeaux: *mut u32, _file: *mut u32) -> windows::core::Result<()> {
        // Normal answer of a callback that imposes neither queue nor flag: Media
        // Foundation expects it and falls back on its default values.
        Err(E_NOTIMPL.into())
    }

    fn Invoke(&self, _result: Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        let (verrou, signal) = &*self.fait;
        *verrou.lock().unwrap_or_else(|e| e.into_inner()) = true;
        signal.notify_one();
        Ok(())
    }
}

/// Work item that occupies the queue: instrument of the test, never
/// dropped in operation.
#[implement(IMFAsyncCallback)]
struct Bouchon {
    duree: Duration,
}

impl IMFAsyncCallback_Impl for Bouchon_Impl {
    fn GetParameters(&self, _drapeaux: *mut u32, _file: *mut u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Invoke(&self, _result: Ref<IMFAsyncResult>) -> windows::core::Result<()> {
        std::thread::sleep(self.duree);
        Ok(())
    }
}
