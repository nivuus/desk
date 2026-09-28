use std::time::{Duration, Instant};

/// Work stream B probe: is ViGEmBus usable, and does its vibration
/// callback restore the magnitudes?
///
/// Initially served to pin the real API of `vigem-client` before
/// task 10 wrote `win::VirtualPad`/`win::spawn_rumble` above. Kept
/// afterwards, at the explicit request of task 10's brief: `diagnostics.rs`
/// still calls it behind `VIGEM_PROBE`, and task 16 (acceptance) plans
/// to reuse it to read the gamepad's state back through `XInputGetState`.
///
/// **Gap from the form initially envisaged**: there is no
/// `notification.wait_timeout(Duration)`. The real API (checked on the
/// crate's published source code, docs.rs 0.1.4) is:
///
/// - `target.request_notification() -> Result<XRequestNotification, Error>`,
///   only available with the crate feature
///   `unstable_xtarget_notification` (without it the method does not exist at
///   all — not a runtime error, an absence at compile time).
/// - `XRequestNotification` is not directly pollable: its low-level
///   methods (`request`, `poll`) require a `Pin<&mut Self>` because the
///   structure contains a `PhantomPinned`. The usage intended by the crate
///   itself is its `spawn_thread(self, f)` method, which runs the
///   request/wait loop in a dedicated thread and calls `f` back at each
///   notification received — no adjustable delay either, it blocks as long as
///   no notification arrives.
/// - The received structure is `XNotification { large_motor: u8, small_motor:
///   u8, led_number: u8 }` — the field names assumed in the brief
///   were correct.
///
/// We therefore relay the notifications from the `spawn_thread` thread to this one
/// through an `mpsc` channel, and it is THIS channel we query with a delay
/// (`recv_timeout`) to get back the "wait up to N
/// seconds" behaviour the probe must have.
pub fn probe(secondes: u64) -> anyhow::Result<String> {
    use anyhow::Context;
    use std::sync::mpsc;

    let client = vigem_client::Client::connect().context("connexion au pilote ViGEmBus")?;
    let id = vigem_client::TargetId::XBOX360_WIRED;
    let mut target = vigem_client::Xbox360Wired::new(client, id);
    target
        .plugin()
        .context("branchement de la manette virtuelle")?;
    target.wait_ready().context("attente de disponibilité")?;

    // A non-neutral state: if a Windows tool (joy.cpl) is open on the
    // VM, it must show it.
    let etat = vigem_client::XGamepad {
        buttons: vigem_client::XButtons!(A),
        left_trigger: 128,
        right_trigger: 0,
        thumb_lx: 16384,
        thumb_ly: 0,
        thumb_rx: 0,
        thumb_ry: 0,
    };
    // Finding of this probe: `wait_ready()` can return `Ok(())` while
    // the virtual USB bus has not finished its PnP enumeration on the Windows side — the
    // first `update()` then fails with `WinError(259)`
    // (`ERROR_NO_MORE_ITEMS`), a variant that `vigem-client` does NOT translate
    // into `Error::TargetNotReady` (only `ERROR_DEV_NOT_EXIST` is).
    // `wait_ready` is therefore not a sufficient guarantee before the first
    // state send: one must retry with a short backoff. Documented here
    // for task 10, which will have to do the same in the final module.
    //
    // Exact count: the first call to `update()` below counts as
    // attempt no. 1 and is not subject to the `tentatives < 20` guard (it is only
    // evaluated after a first failure); in case of repeated failures, the
    // loop therefore makes at most 21 in total (1 initial + 20 retries), not
    // 20 — that is indeed what `tentatives` counts at the end (number of
    // RETRIES, not of total calls).
    let mut tentatives = 0u32;
    loop {
        match target.update(&etat) {
            Ok(()) => break,
            Err(e) if tentatives < 20 => {
                tentatives += 1;
                tracing::warn!(
                    tentative = tentatives,
                    error = ?e,
                    "update() pas encore prêt, nouvelle tentative"
                );
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                tracing::warn!(error = ?e, "update() a échoué — variante brute, abandon");
                return Err(e).context("application d'un état");
            }
        }
    }
    if tentatives > 0 {
        tracing::info!(tentatives, "update() a fini par réussir après attente");
    }

    // `request_notification()` then `spawn_thread`: see the module comment
    // above for why this detour is necessary rather than a
    // hypothetical `wait_timeout`.
    let (tx, rx) = mpsc::channel::<vigem_client::XNotification>();
    let requete = target.request_notification().context(
        "requête de notification (nécessite la fonctionnalité unstable_xtarget_notification)",
    )?;
    let fil = requete.spawn_thread(move |_requete, notification| {
        let _ = tx.send(notification);
    });

    let mut recu = 0u32;
    let echeance = Instant::now() + Duration::from_secs(secondes);
    loop {
        let restant = echeance.saturating_duration_since(Instant::now());
        if restant.is_zero() {
            break;
        }
        match rx.recv_timeout(restant.min(Duration::from_millis(500))) {
            Ok(vibration) => {
                recu += 1;
                tracing::info!(
                    grand = vibration.large_motor,
                    petit = vibration.small_motor,
                    "vibration reçue"
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Unplug the gamepad before joining the thread: `poll` is blocked there
    // waiting for a notification, and only unplugging (which cancels the
    // pending request on the driver side) gets it out of its loop.
    drop(target);
    let _ = fil.join();

    Ok(format!(
        "branchement OK, {recu} notification(s) de vibration reçue(s)"
    ))
}
