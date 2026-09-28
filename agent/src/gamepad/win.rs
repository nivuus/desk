//! The final ViGEmBus module: `VirtualPad` plugs in a virtual Xbox 360
//! gamepad and applies the received states to it, `spawn_rumble` relays its
//! vibration notifications to the client through the control channel.
//!
//! The real API of `vigem-client` differs from the one envisaged in the brief of
//! this task on two points, both established empirically by the
//! `probe` probe (`gamepad/probe.rs` — see also
//! `docs/superpowers/plans/2026-07-28-input-jeu-sondes.md`):
//!
//! 1. There is no `notification.wait_timeout(Duration)`. The usage
//!    intended by the crate is `request_notification()` then
//!    `spawn_thread(f)`: the latter consumes the request and runs the
//!    request/wait loop on A DEDICATED THREAD created by the crate itself,
//!    calling `f` back at each notification — without an adjustable delay, it
//!    blocks as long as no notification arrives. We therefore relay each
//!    raw notification to an `mpsc` specific to this module, queried
//!    with a delay (`recv_timeout`) to get back a bounded wait
//!    behaviour and be able to check periodically for a requested stop.
//! 2. `wait_ready()` does not guarantee that an immediate `update()` succeeds:
//!    the virtual USB bus may not have finished its PnP enumeration on the
//!    Windows side, and the first `update()` then fails with `WinError(259)`
//!    (`ERROR_NO_MORE_ITEMS`), a variant that `vigem-client` does NOT
//!    translate into `Error::TargetNotReady` (only `ERROR_DEV_NOT_EXIST` is). A
//!    single retry was enough during the probe's only measurement; having
//!    characterised this behaviour only once, we keep here the same margin
//!    as the probe (up to 20 retries, 250 ms each) rather than the
//!    observed minimum.

use super::{plus_recent, LimiteurVibration, PERIODE_MIN};
use anyhow::{Context, Result};
use proto::control::AgentControl;
use proto::input::GamepadState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Maximum number of RETRIES of `update()` after `wait_ready()` before
/// giving up (hence at most 1 + `TENTATIVES_MAX` calls to `update()`
/// in total) — same count and same values as task 1's probe.
const TENTATIVES_MAX: u32 = 20;
const DELAI_TENTATIVE: Duration = Duration::from_millis(250);

/// Virtual Xbox 360 gamepad. Plugged in at the FIRST reception of a
/// state, never at start-up: a permanently present gamepad
/// disturbs applications that react to its mere presence
/// (`demarrage.rs` performs this lazy plugging).
pub struct VirtualPad {
    target: vigem_client::Xbox360Wired<vigem_client::Client>,
    derniere_seq: Option<u16>,
}

impl VirtualPad {
    /// Plugs the target in and waits for it to really accept a state,
    /// not only for `wait_ready()` to claim so (see point 2 of the
    /// module comment). Returns an error if ViGEmBus is
    /// absent or stays unavailable after all retries:
    /// `demarrage.rs` treats this failure as non-blocking for the session.
    ///
    /// May block for up to `TENTATIVES_MAX * DELAI_TENTATIVE` (5 s with
    /// the current values): to be called outside the thread that drives the
    /// session (see `spawn_connect` below), never directly
    /// from the `Session::run` loop.
    pub fn connect() -> Result<Self> {
        let client = vigem_client::Client::connect().context("connexion au pilote ViGEmBus")?;
        let mut target =
            vigem_client::Xbox360Wired::new(client, vigem_client::TargetId::XBOX360_WIRED);
        target.plugin().context("plugging in the virtual gamepad")?;
        target.wait_ready().context("waiting for availability")?;

        // Neutral state: this first write only serves to confirm
        // that the target really accepts an `update()`, not to reflect
        // a received gamepad state — `derniere_seq` stays `None` after
        // this call, so as not to stand in for the first real state.
        let neutre = vigem_client::XGamepad::default();
        let mut tentatives = 0u32;
        loop {
            match target.update(&neutre) {
                Ok(()) => break,
                // Only the two variants documented as
                // "not ready yet" (see the module comment):
                // a different error (absent bus, permission denied,
                // etc.) is final, retrying would change nothing and
                // would waste up to 5 s for nothing.
                Err(
                    e @ (vigem_client::Error::WinError(259) | vigem_client::Error::TargetNotReady),
                ) if tentatives < TENTATIVES_MAX => {
                    tentatives += 1;
                    tracing::warn!(
                        tentative = tentatives,
                        error = ?e,
                        "update() not ready yet, retrying"
                    );
                    std::thread::sleep(DELAI_TENTATIVE);
                }
                Err(e) => {
                    return Err(e)
                        .context("first state sent to the virtual gamepad, after all the retries")
                }
            }
        }
        if tentatives > 0 {
            tracing::info!(tentatives, "update() finally succeeded after waiting");
        }
        tracing::info!("virtual gamepad plugged in");
        Ok(Self {
            target,
            derniere_seq: None,
        })
    }

    /// Applies a state received from the client to the virtual gamepad.
    ///
    /// Rejects stale states BEFORE reaching the driver: the channel
    /// carrying the `GamepadState`s is not ordered, an older state
    /// arriving after a more recent one would otherwise wrongly restore it.
    pub fn apply(&mut self, state: &GamepadState) -> Result<()> {
        if let Some(courante) = self.derniere_seq {
            if !plus_recent(state.seq, courante) {
                return Ok(());
            }
        }

        let gamepad = vigem_client::XGamepad {
            buttons: vigem_client::XButtons(state.buttons),
            left_trigger: state.left_trigger,
            right_trigger: state.right_trigger,
            thumb_lx: state.thumb_lx,
            thumb_ly: state.thumb_ly,
            thumb_rx: state.thumb_rx,
            thumb_ry: state.thumb_ry,
        };
        self.target
            .update(&gamepad)
            .context("applying the gamepad state")?;
        // Recorded only after success: a failed `update()` must
        // not mark this sequence as handled, or else it could
        // never be reapplied (`plus_recent` would then
        // reject it as stale).
        self.derniere_seq = Some(state.seq);
        Ok(())
    }
}

/// Explicitly unplugs the target.
///
/// `Xbox360Wired::drop` already does it on its own (checked in the
/// crate's source code, `x360.rs`): this explicit call is therefore
/// redundant in practice, but makes the intent readable without depending
/// on a `Drop` behaviour not visible at the call site.
///
/// **No `join()` of the crate's internal thread here** (unlike
/// an earlier version of this code): `request()`, on the crate side,
/// ignores the return code of its `DeviceIoControl` (`bus.rs`). If the
/// unplugging falls between the return of a `poll()` and the next call
/// to `request()`, no I/O is pending and `poll(true)` (which
/// waits on a blocking `GetOverlappedResult`) never receives
/// `ERROR_OPERATION_ABORTED`: joining this thread could then block
/// indefinitely, without a guard delay or trace — freezing the blocking
/// worker that carries the whole session. This thread owns anyway
/// its own duplicated `Client` (`request_notification` calls
/// `try_clone`): it references nothing in `VirtualPad`, and not
/// joining it therefore leaks nothing beyond the life of the process. Only
/// task 1's probe (nominal case, a single attempt) checked that
/// unplugging alone is enough to unblock this thread; this comment
/// documents why we do not go further.
impl Drop for VirtualPad {
    fn drop(&mut self) {
        let _ = self.target.unplug();
    }
}

/// Starts the vibration relay to the client, on its own thread.
///
/// Takes `&mut VirtualPad` (and not `&VirtualPad` as envisaged in the
/// brief): `request_notification()` requires mutable access to the target
/// — a direct consequence of the crate's real signature, not an arbitrary
/// choice.
///
/// Two distinct threads collaborate here:
/// - the one the crate creates itself through `spawn_thread`: it runs
///   on the driver side and ONLY relays each raw notification into
///   a channel — never any logic on it, so as never to delay the
///   driver's callback.
/// - the one returned by this function: it reads this channel with a bounded
///   delay (`recv_timeout`), applies `LimiteurVibration` so as not to
///   flood the RELIABLE control channel to the session (flooding it
///   would make it accumulate delay exactly when the game vibrates the most),
///   and checks `arret` at each wake-up to be able to stop even without
///   a notification.
///
/// Clean stop: this thread ends when `arret` turns true, when the
/// channel closes (which typically happens shortly after `VirtualPad`
/// is dropped — `Drop`, above, unplugs the target, which OFTEN
/// gets the crate's internal thread out of its wait and thus
/// closes this channel, but not guaranteed for sure depending on where that thread
/// is in its loop — see the comment of `Drop`), or when
/// sending to `tx` fails (session already ended on the receiver side).
/// On any exit other than a send failure, a last
/// `AgentControl::rumble(0, 0)` is attempted: the client must never
/// remain stuck vibrating a physical gamepad because the end
/// of the session arrived between two notifications.
pub fn spawn_rumble(
    pad: &mut VirtualPad,
    tx: Sender<AgentControl>,
    arret: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let requete = pad
        .target
        .request_notification()
        .context("subscribing to rumble notifications")?;

    let (notif_tx, notif_rx) = mpsc::channel::<vigem_client::XNotification>();
    // Thread created and owned by the crate, on its own duplicated `Client`
    // (`request_notification` calls `try_clone` internally): a mere
    // relay, no logic on it. Its `JoinHandle` is deliberately
    // dropped (not stored, not joined): see the comment of `Drop`
    // above for why joining it would be risky (possible indefinite
    // block) for no benefit (this thread references nothing
    // in `VirtualPad`, not joining it leaks nothing beyond the
    // life of the process).
    let _fil_vigem = requete.spawn_thread(move |_requete, notification| {
        let _ = notif_tx.send(notification);
    });

    Ok(std::thread::spawn(move || {
        let mut limiteur = LimiteurVibration::new();
        'relais: while !arret.load(Ordering::Relaxed) {
            match notif_rx.recv_timeout(PERIODE_MIN) {
                Ok(vibration) => {
                    let etat = (vibration.large_motor, vibration.small_motor);
                    if let Some((gauche, droite)) = limiteur.observer(Instant::now(), etat) {
                        if tx.send(AgentControl::rumble(gauche, droite)).is_err() {
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Nothing received in the window: an opportunity to flush a
                    // state deferred by rate limiting.
                    if let Some((gauche, droite)) = limiteur.echu(Instant::now()) {
                        if tx.send(AgentControl::rumble(gauche, droite)).is_err() {
                            return;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break 'relais,
            }
        }
        // Best-effort: the control channel may already be closed on the
        // session side (normal end), in which case there is anyway no one
        // left to read this message.
        let _ = tx.send(AgentControl::rumble(0, 0));
    }))
}

/// Launches `VirtualPad::connect()` on a dedicated thread and returns a
/// non-blocking receiver.
///
/// `connect()` can sleep for up to 5 s (see its documentation) in case
/// of slow PnP enumeration on the Windows side. Calling it directly from the
/// `Session::run` loop would freeze video AND audio during that delay:
/// this loop is sized on the video cadence and the RTCP
/// deadlines, not on the latency of a third-party driver (see the comment on
/// `spawn_blocking` in `demarrage.rs`). `demarrage.rs` polls this receiver with
/// `try_recv()` at each gamepad state received, without ever blocking
/// on it; the states received while the connection is in progress are
/// lost without consequence — not because the client polls at 250 Hz
/// (cadence under load never measured, see `client/src/gamepad.ts`),
/// but because it re-emits a complete state every 100 ms even without
/// change (`RAFRAICHISSEMENT_MS`): it is this periodic refresh,
/// independent of the polling frequency, that carries the
/// self-healing guarantee.
pub fn spawn_connect() -> mpsc::Receiver<Result<VirtualPad>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(VirtualPad::connect());
    });
    rx
}
