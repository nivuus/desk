//! Launching an application the way a double-click would.
//!
//! 🔴 THIS MODULE IS `#[cfg(windows)]`, AND IT DECIDES NOTHING. It receives a
//! `.lnk` path, a fallback target and an `nShow`, and returns the outcome of what
//! it tried. `IssueLancement::Inconnue` is NOT returned here: only
//! the caller knows the catalogue, so only it can say that a key is not
//! there.
//!
//! ⚠️ CHECKED BY `cargo check --target x86_64-pc-windows-gnu` ALONE, like
//! `apps::lecture`. No host test can cover it.

use proto::plateforme::IssueLancement;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
    SHELLEXECUTEINFOW,
};

/// Launches the shortcut, and falls back on the recorded target if it has disappeared.
///
/// 🔴 NO COMMAND LINE IS REBUILT, ANYWHERE. The `.lnk` is
/// passed as is to `ShellExecuteExW`, which itself draws the target, the
/// arguments, the working directory and the verb from it. Rebuilding would
/// reintroduce a home-made command-line parser — that is,
/// exactly the defect being removed from `src/lnkParser.js`, of which this sub-block
/// is the remedy. The fallback itself passes the target as `lpFile`, without ever
/// sticking arguments behind it.
///
/// 🔴 THE LAUNCHED PROCESS ENTERS NO JOB OBJECT, and the supervisor's
/// reasoning DOES NOT CARRY OVER HERE. `agent/src/superviseur/lanceur.rs`
/// sets `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, whose purpose is that the death of the
/// supervisor does not leave N agent processes behind it. An application
/// the USER has just launched is not in that case: assigning it there would
/// kill it with the agent, that is at the first redeployment. `ShellExecuteEx`
/// creates its process outside any job, and that is what we want.
///
/// 🔴 **THIS LAST SENTENCE WAS MEASURED ON 30 AUGUST 2026, AND IT IS
/// FALSE.** It said: "`ShellExecuteEx` creates its process outside any
/// job", by the consequence that `superviseur/lanceur.rs` "assigns only its
/// CHILDREN and never itself". **The supervisor IS in a job** — that of the
/// **Task Scheduler**, which launches the agent — and the applications it
/// launches **inherit** it: in-job flag `True` read for the supervisor, its
/// twelve children, and the `notepad.exe` launched by the catalogue (read in
/// session 1, `journaux-lot32j/`).
///
/// ⚠️ **What this does NOT change**: nothing kills these processes, because the
/// Scheduler's job does not carry `KILL_ON_JOB_CLOSE` through us, and
/// `installation::execution::tue_a_la_fermeture` measures that flag instead of
/// assuming it. **What it does change**: `IsProcessInJob(p, None)` no longer
/// discriminates anything here, and the only question that matters is "in THIS
/// particular job". See `crate::appartenance`.
///
/// 🔴 **G3 DOES NOT RELY ON IT: it MEASURES and it REFUSES.**
/// `apps::installation::execution::in_a_job` calls `IsProcessInJob` before
/// any installer launch, logs the boolean every time, and refuses
/// if the answer is yes. **Nothing of the kind is done here**, and that is accepted: an
/// installer killed halfway leaves a half-installed machine, a killed
/// application leaves nothing.
///
/// ⚠️ BOTH ATTEMPTS ARE LOGGED, including the one that succeeds:
/// without the fallback's trace, `Cible` would be indistinguishable from `Raccourci` in a
/// log, and that is precisely the distinction the outcome exists to make
/// decidable.
pub fn lancer(chemin_lnk: &str, cible: &str, montrer: i32) -> IssueLancement {
    match executer(chemin_lnk, montrer) {
        Ok(()) => {
            tracing::info!(chemin = chemin_lnk, "shortcut launched");
            return IssueLancement::Raccourci;
        }
        Err(error) => tracing::warn!(
            chemin = chemin_lnk,
            %error,
            "shortcut launch failed, falling back to the recorded target"
        ),
    }

    // ⚠️ THE FALLBACK IS NOT AN EQUIVALENCE, and that is why it carries a
    // distinct outcome: the recorded target carries neither the shortcut's
    // arguments nor its working directory. An application launched through that
    // path may therefore start differently — it is better than nothing, it
    // is not the same thing, and the caller must be able to know it.
    if cible.trim().is_empty() {
        tracing::warn!(chemin = chemin_lnk, "no fallback target recorded");
        return IssueLancement::Echec;
    }
    match executer(cible, montrer) {
        Ok(()) => {
            tracing::warn!(
                chemin = chemin_lnk,
                cible,
                "launched through the TARGET, not through the shortcut"
            );
            IssueLancement::Cible
        }
        Err(error) => {
            tracing::error!(chemin = chemin_lnk, cible, %error, "launch failed on both sides");
            IssueLancement::Echec
        }
    }
}

fn executer(file: &str, montrer: i32) -> windows::core::Result<()> {
    let large: Vec<u16> = file.encode_utf16().chain(std::iter::once(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        // ⚠️ `SEE_MASK_NOASYNC` IS REQUIRED, and this is NOT a measurement here:
        // it is a reading of the Windows documentation, declared as such.
        // `ShellExecuteEx` may return BEFORE the child process
        // is created; if the calling thread ends in the meantime, the
        // launch is lost. Our reconciliation thread survives, but
        // the call must stay synchronous so that the returned outcome really
        // describes what happened — otherwise `Raccourci` would mean
        // "the request was accepted", not "the application started".
        //
        // `SEE_MASK_FLAG_NO_UI` suppresses error dialog boxes: the
        // VM's interactive session has nobody to close them, and a
        // modal would block the call until the next reboot.
        // 🔴 `SEE_MASK_NOCLOSEPROCESS` GIVES US THE PROCESS HANDLE, and
        // that is the only reason for its presence: without it, there is no way
        // to enrol the application in the OWNERSHIP job (`crate::appartenance`),
        // hence no way to tell our windows from Apollo's or
        // Steam's. ⚠️ It also makes us OWNERS of the handle: it must be
        // closed, otherwise every launch leaks a kernel handle.
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOCLOSEPROCESS,
        lpFile: PCWSTR(large.as_ptr()),
        // The shortcut's `nShow` is replayed as is, so that a launch
        // by the agent and a double-click in Explorer give the same
        // window.
        nShow: montrer,
        ..Default::default()
    };
    // SAFETY: FFI call. `info` lives until the end of the function, and
    // so does `large` — the `lpFile` pointer therefore cannot dangle during
    // the call, which is synchronous through `SEE_MASK_NOASYNC`.
    let issue = unsafe { ShellExecuteExW(&mut info) };

    // ⚠️ THE RACE, NAMED AND BOUNDED. `ShellExecuteEx` cannot create a
    // SUSPENDED process: the "create suspended, assign, resume" pattern
    // does not exist on this path. Between the return above and the assignment,
    // one system call elapses — microseconds —, and a descendant born
    // in that interval would not inherit the job. The LAUNCHED process, for its part,
    // is always assigned: it is its handle we hold. And a window
    // discarded for that reason would be discarded LOUDLY (see the refusal trace
    // of `superviseur::hook`), never silently.
    if !info.hProcess.is_invalid() {
        crate::appartenance::adopter(info.hProcess);
        // `SEE_MASK_NOCLOSEPROCESS` makes us their owners.
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(info.hProcess) };
    } else if issue.is_ok() {
        // A successful launch WITHOUT a handle: the application joined an
        // existing instance (Chrome without a distinct `--user-data-dir` does so,
        // this repository has recorded it). Its windows will belong to the original
        // process, which is not in our job — and will therefore be discarded.
        tracing::warn!(
            "launch succeeded without a process handle: the application joined \
             an existing instance, its windows will NOT be adopted"
        );
    }
    issue
}
