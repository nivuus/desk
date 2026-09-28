//! The OWNERSHIP job object: whom does the window belong to?
//!
//! 🔴 **THE RULE, SETTLED BY THE OWNER (30 August 2026): `desk` only adopts
//! the windows of the applications it launched ITSELF, and of their
//! descendants.** It is an ownership rule, not a deny list: Apollo,
//! Steam and the rest are ignored **without being named anywhere**, which a
//! deny list could not do without going stale.
//!
//! ⚠️ **WHAT IT TAKES AWAY, AND IT IS ACCEPTED, NOT A REGRESSION**: a window
//! **already open before `desk`** is no longer picked up. Measured on 30 August 2026:
//! `cmd.exe` and `Forza Horizon 6` leave the hub, and that is the trade-off
//! the owner accepted knowingly.
//!
//! ## Why a JOB and not the parent chain
//!
//! Walking up the parents works — that is what the 30 August measurement did —
//! but **a chain breaks when an intermediate process dies**, which is the
//! ordinary case of a launcher that hands over, and a parent PID is **reusable**
//! after the process dies. Ownership would become **intermittent**,
//! i.e. the most expensive failure to diagnose. The job, on the other hand, is a
//! property **carried by the process**, inherited by its descendants, and
//! insensitive to the death of whoever created it.
//!
//! ## 🔴 WHAT WAS MEASURED BEFORE WRITING A LINE — and what corrected the product
//!
//! `apps/lancement.rs` claimed that the launched process "enters no
//! job object", **through a consequence it stated itself**: that
//! `superviseur/lanceur.rs` "only assigns its CHILDREN and never itself".
//! 🔴 **MEASURED FALSE on 30 August 2026**: the supervisor **IS** in a job —
//! the one of the **Task Scheduler**, which launches the agent — and the applications
//! it launches **inherit** it (in-job flag `True` for the supervisor, its
//! twelve children, and the `notepad.exe` launched from the catalogue).
//!
//! **Direct consequence for the design**: `IsProcessInJob(p, NULL)` does not
//! discriminate anything here — everything is in a job. The question that matters is
//! **"in THIS job"**, with our handle.
//!
//! And the precondition that remained, **measured as well**:
//!
//! ```text
//! TARGET notepad in_OUR_job_before=False ASSIGNMENT=True err=0
//!               in_OUR_job_after=True
//! SURVIVES_AFTER_CLOSE notepad = 1
//! ```
//!
//! **NESTED assignment succeeds** on a process already in the
//! Task Scheduler's job (Windows 10.0.26100), **and closing our job kills nothing** —
//! because we do NOT set `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. It is that
//! flag, and not the job, that `apps/lancement.rs` was right to refuse:
//! it would kill the user's applications at the first redeployment.
//!
//! ## ⚠️ The race, named and bounded
//!
//! `ShellExecuteExW` cannot create a **suspended** process: there is no
//! "create suspended, assign, resume" pattern on this path. Between the
//! return of `ShellExecuteExW` and `AssignProcessToJobObject`, one
//! system call elapses — **microseconds** —, and a descendant created in that
//! interval **would not inherit** the job.
//!
//! **Why it is acceptable**: the launched process itself is always
//! assigned (it is its handle we hold); only a window created by a
//! grandchild born within those few microseconds would be discarded.
//! It would be discarded **loudly** — see the refusal trace of
//! `superviseur::hook` —, never silently. **A named and bounded race is
//! a risk; a hidden race is a defect.**

use std::sync::OnceLock;

/// The bench variable that DISARMS the ownership rule.
///
/// ⚠️ **`=0` DISARMS; mere PRESENCE does not enable** — the convention of
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE`,
/// `PRESSE_PAPIER`, `APPS`, `PONT_ECRITURE` and `SORTIE_DESIGNEE`. The predicate
/// is **REUSED, not copied**: `crate::apps::desarme`.
///
/// Disarmed, the product once again adopts any window that passes the criterion —
/// **it is the RED arm of the acceptance run**, and it survives the previous binary.
pub fn armee() -> bool {
    static ARMEE: OnceLock<bool> = OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = !crate::apps::desarme(std::env::var("APPARTENANCE").ok().as_deref());
        if !armee {
            tracing::warn!(
                "ownership DISARMED (APPARTENANCE=0): desk adopts again the \
                 windows it did not launch — a bench arm, never a shipped configuration"
            );
        }
        armee
    })
}

#[cfg(windows)]
mod win {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    /// The process's ownership job. **No limit is set on it**, and
    /// that is the whole point: it only serves to answer "is this process one of
    /// ours?". See the module header.
    struct Job(HANDLE);
    // SAFETY: a job HANDLE is a kernel object global to the process, with no
    // thread affinity. It is never closed — closing it kills nothing (measured),
    // and its lifetime is that of the process.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| match unsafe { CreateJobObjectW(None, None) } {
            Ok(h) if !h.is_invalid() => {
                tracing::info!("ownership job created (no limit: it kills nothing)");
                Some(Job(h))
            }
            Ok(_) => {
                tracing::error!("ownership job: invalid handle, the rule will be INERT");
                None
            }
            Err(error) => {
                tracing::error!(%error, "ownership job NOT created — the rule will be INERT");
                None
            }
        })
        .as_ref()
        .map(|j| j.0)
    }

    /// Registers a process WE have just launched in the job.
    ///
    /// Logs both ways: without the failure trace, an application
    /// that never showed up would be indistinguishable from an application that
    /// did not start.
    pub fn adopter(processus: HANDLE) {
        let Some(job) = job() else { return };
        match unsafe { AssignProcessToJobObject(job, processus) } {
            Ok(()) => tracing::info!("launched process added to the ownership job"),
            Err(error) => tracing::error!(
                %error,
                "launched process NOT added to the ownership job — its windows \
                 will be DISCARDED, and that is a failure, not a normal refusal"
            ),
        }
    }

    /// Is this process one of ours?
    ///
    /// ⚠️ **`Some(false)` and `None` are not the same thing**: the first is
    /// an answer ("it is not ours"), the second a failure of the question.
    /// The caller tells them apart, and NEVER discards on a failed question —
    /// refusing for lack of having been able to ask would be the failure this guard exists
    /// to avoid.
    pub fn est_des_notres(pid: u32) -> Option<bool> {
        let job = job()?;
        let processus =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut dedans = windows::core::BOOL(0);
        let issue = unsafe { IsProcessInJob(processus, Some(job), &mut dedans) };
        let _ = unsafe { CloseHandle(processus) };
        issue.ok().map(|()| dedans.as_bool())
    }
}

#[cfg(windows)]
pub use win::{adopter, est_des_notres};
