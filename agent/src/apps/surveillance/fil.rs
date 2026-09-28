//! The watch thread: **a single one**, four roots, one wait.
//!
//! 🔴 `#[cfg(windows)]`, and **no host test is possible** — same status
//! as `racine.rs`, and for the same reason.
//!
//! **ONE THREAD, NOT FOUR.** What was ruled out, with its reason:
//!
//! - **four blocking threads** (synchronous `ReadDirectoryChangesW`): four
//!   threads to wait, and **no way to stop them cleanly** — a thread
//!   blocked in a synchronous call does not see a stop flag;
//! - **a completion port**: more machinery than four handles
//!   justify, and a second way of waiting in a process that
//!   already has four;
//! - **`SHChangeNotifyRegister`**: ruled out by the specification itself, and
//!   for good reasons — an `HWND`, a message pump, and PIDLs to
//!   re-resolve.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use windows::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::WaitForMultipleObjects;

use super::partage::Veille;
use super::racine::{Issue, Racine};

/// The wait step.
///
/// 🔴 IT EXISTS **SO THAT THE STOP FLAG IS SEEN**, and to give control back
/// to due backoffs. **It polls nothing**: notifications arrive through the
/// events, never through this delay. Confusing it with a polling interval
/// would make people believe shortening it speeds up detection — it would only
/// wake a thread that has nothing to do.
const PAS_ATTENTE_MS: u32 = 1_000;

/// The body of the thread.
pub(super) fn tourner(veille: Veille) {
    // 🔴 DEDUPLICATION. `lecture::racines()` does not deduplicate, and nothing
    // guarantees that the four known folders are distinct on an
    // unusual configuration. Opening the same directory twice
    // would waste 64 KiB of NON-PAGED pool to count each event
    // twice.
    //
    // ⚠️ `racines()` IS NOT TOUCHED: it is shared with the
    // reconciliation, where duplicates are already harmless
    // (`vus.insert(app.cle)`). Deduplication is a need of THIS
    // consumer, and it lives with it.
    let distinctes: BTreeSet<PathBuf> = crate::apps::lecture::racines().into_iter().collect();
    if distinctes.is_empty() {
        tracing::warn!(
            "shortcut watch inactive: no root resolved \
             (the periodic reconciliation, for its part, goes on)"
        );
        return;
    }

    let mut racines: Vec<Racine> = Vec::new();
    for chemin in distinctes {
        match Racine::ouvrir(chemin.clone()) {
            Ok(racine) => {
                tracing::info!(racine = %chemin.display(), "root watched");
                racines.push(racine);
            }
            // ⚠️ NAMED GAP, NOT CLOSED: a root ABSENT at start-up has
            // no handle, hence no completion error, hence no
            // reopening attempt. If it appears later, **it
            // is never watched**, and only the periodic reconciliation
            // sees it. It is LATENCY, never a loss — and closing it
            // would require a re-resolution timer nothing justifies
            // today.
            Err(error) => tracing::warn!(
                racine = %chemin.display(), %error,
                "root not watched: the periodic reconciliation stays the source of truth"
            ),
        }
    }
    if racines.is_empty() {
        tracing::warn!("shortcut watch inactive: no root open");
        return;
    }
    tracing::info!(racines = racines.len(), "shortcut watch armed");

    boucler(&mut racines, &veille);

    for racine in &mut racines {
        racine.fermer();
    }
    tracing::info!("shortcut watch stopped");
}

fn boucler(racines: &mut [Racine], veille: &Veille) {
    loop {
        if veille.arretee() {
            return;
        }
        // The LIVE roots only: a failed root no longer has a
        // valid handle, and including it would make the whole wait return
        // `WAIT_FAILED` — one dead root would take down the other three.
        let vivantes: Vec<usize> = (0..racines.len())
            .filter(|i| !racines[*i].en_echec())
            .collect();
        if vivantes.is_empty() {
            // All failed: there is nothing to wait for, but there are
            // backoffs to let fall due. Sleeping the wait step is
            // exactly what `WaitForMultipleObjects` would do if it
            // accepted zero handles — it does not.
            std::thread::sleep(std::time::Duration::from_millis(u64::from(PAS_ATTENTE_MS)));
            reprendre_les_echues(racines);
            continue;
        }
        let handles: Vec<_> = vivantes.iter().map(|i| racines[*i].evenement()).collect();
        // SAFETY: FFI call. `bWaitAll = false`: we want the FIRST
        // signalled root, not all four.
        let issue = unsafe { WaitForMultipleObjects(&handles, false, PAS_ATTENTE_MS) };
        if issue == WAIT_TIMEOUT {
            reprendre_les_echues(racines);
            continue;
        }
        if issue == WAIT_FAILED {
            // 🔴 A SINGLE LINE, THEN WE QUIT. A thread that loops on a
            // wait failure is a thread that burns a core SILENTLY — and
            // in a log shared by the supervisor, the sensor and all
            // the children, it would flood it too.
            // SAFETY: FFI call. `GetLastError` has no precondition;
            // it is `unsafe` because its value only makes sense here, right
            // after the call that failed.
            let code = unsafe { windows::Win32::Foundation::GetLastError() };
            tracing::error!(
                error = %windows::core::Error::from_hresult(code.to_hresult()),
                "watch wait failed: thread stopped (the periodic reconciliation goes on)"
            );
            return;
        }
        let rang = (issue.0 - WAIT_OBJECT_0.0) as usize;
        let Some(&indice) = vivantes.get(rang) else {
            tracing::error!(?issue, "watch wait: rank outside the roots, thread stopped");
            return;
        };
        servir(&mut racines[indice], veille);
    }
}

/// Completes a signalled root, re-arms it, and publishes what it said.
fn servir(racine: &mut Racine, veille: &Veille) {
    match racine.completer() {
        // 🔵 SWALLOWED: neither counted, nor logged, nor triggering — and the
        // root is nonetheless RE-ARMED, otherwise injecting a single
        // fault would stop the watch instead of making it miss one
        // completion.
        Issue::Avalee => rearmer(racine),
        Issue::Notification => {
            veille.signaler();
            rearmer(racine);
        }
        Issue::Debordement => {
            veille.signaler_debordement();
            // 🔴 NO RATE LIMITING ON THIS `warn!`, AND IT IS REASONED.
            // An overflow requires more than a thousand events between two
            // re-arms, that is in the microseconds separating
            // them: it is RARE BY CONSTRUCTION. Rate-limiting it
            // would hide exactly the pathological case one would want to see.
            // ✅ **"RARE BY CONSTRUCTION" HAS BECOME A MEASUREMENT**: over
            // seven runs of gate S1 (up to 60,000 files at
            // 2,850/s) and two bursts on the product (~96,000 real
            // notifications each), this line came out **NOT ONCE**. The
            // only reading that shows it is under injection
            // (`APPS_FAUTE=debordement:3`: exactly three lines).
            // ⚠️ If an acceptance run one day records a deluge, it is a RESULT:
            // it is recorded, and the limiting is handed over — it is not added
            // in a rush.
            tracing::warn!(
                racine = %racine.chemin().display(),
                debordements = veille.debordements(),
                "notifications lost: watch buffer overflowed, \
                 the following reconciliation re-reads the whole disk"
            );
            rearmer(racine);
        }
        Issue::Annulee => {}
        Issue::Perte(error) => {
            // 🔴 ONE LINE PER TRANSITION, NEVER ONE PER ATTEMPT. It is the
            // pattern of the `ecartes` set in `apps/boucle.rs`, whose
            // comment quantifies what it avoids: "without this set, the
            // seven discards on this VM would make 20,160 lines a day".
            if !racine.en_echec() {
                tracing::warn!(
                    racine = %racine.chemin().display(), %error,
                    "watch root LOST: reopening scheduled \
                     (the periodic reconciliation stays the source of truth)"
                );
            }
            racine.programmer_la_reprise(Instant::now());
        }
    }
}

fn rearmer(racine: &mut Racine) {
    if let Err(error) = racine.armer() {
        if !racine.en_echec() {
            tracing::warn!(
                racine = %racine.chemin().display(), %error,
                "watch re-arm failed: reopening scheduled"
            );
        }
        racine.programmer_la_reprise(Instant::now());
    }
}

/// Tries to reopen the failed roots **whose backoff is due**, and
/// nothing else.
fn reprendre_les_echues(racines: &mut [Racine]) {
    let maintenant = Instant::now();
    for racine in racines.iter_mut() {
        if !racine.reprise_due(maintenant) {
            continue;
        }
        match racine.rouvrir() {
            Ok(()) => tracing::warn!(
                racine = %racine.chemin().display(),
                "watch root RESTORED"
            ),
            // ⚠️ NO LINE HERE: the root was already failed, and its
            // entry into failure has already been reported. One line per attempt would make
            // 2,880 lines per day and per root at the backoff ceiling.
            Err(_) => racine.programmer_la_reprise(maintenant),
        }
    }
}
