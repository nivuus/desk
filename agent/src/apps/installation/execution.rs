//! Launch the installer, wait for it without killing it, and report what
//! happened.
//!
//! 🔴 THIS MODULE IS THE ONLY ONE IN `apps::installation` TO CARRY A `cfg`, AND IT
//! HAS, ON THE HOST, NO POSSIBLE TEST outside
//! `cargo check --target x86_64-pc-windows-gnu`. That is why everything that
//! could leave it has left: the verdict, the counting window, the
//! paths, the extensions, the cadence and parsing HTTP headers are six
//! PURE modules, and that is where the coverage is. Its only other test is
//! the acceptance run on the VM.
//!
//! 🔴 `CreateProcessW`, AND NOT `ShellExecuteExW` — THE OPPOSITE OF `apps::lancement`.
//! For an application, a double-click is what we want: `ShellExecuteEx`
//! honours the verb, the working directory and the `nShow`. For an installer,
//! the decision reverses, and here is why.
//!
//! `ShellExecuteExW` **TRIGGERS elevation** when the target's manifest
//! asks for it: the dialog opens — on the secure desktop, which
//! Desktop Duplication does not capture — and the call **WAITS**. The user
//! sees a frozen screen and the product can say nothing. `CreateProcessW`
//! **never** elevates: it fails with `ERROR_ELEVATION_REQUIRED` (740), which
//! turns a frozen screen into a **typed refusal the hub can display**.
//!
//! ⚠️ **THIS LAST SENTENCE IS A READING OF THE WINDOWS DOCUMENTATION, NOT
//! A MEASUREMENT OF THIS REPOSITORY**, and it is the premise of the whole remedy. The gate
//! of the sub-block (its observation (e)) must confirm or refute it on the
//! VM. **At the date this file is written, it has not been played**: the VM
//! was held by a concurrent work stream. If it is refuted — if the call
//! succeeds, or fails otherwise —, the typed refusal below must be
//! rebuilt on another clue, and **the `elevation-requise` branch will
//! simply be unreachable** rather than false. Saying so keeps anyone from building
//! a family of refusals on a sentence nobody has tested — that is
//! exactly what the wording of `MF_E_UNSUPPORTED_D3D_TYPE` cost this
//! repository on 31 July 2026.
//!
//! ⚠️ **WHAT THE REMEDY DOES NOT CATCH**: an installer that elevates ITSELF
//! along the way — it starts without privilege then calls
//! `ShellExecute … runas`. That one will cause the dialog the
//! gate describes, and **nothing prevents it**: it is bounded by
//! `EXPIRATION_EXECUTION`, and nothing else.

use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ELEVATION_REQUIRED, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::JobObjects::{
    IsProcessInJob, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcess, GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW,
    PROCESS_INFORMATION, STARTUPINFOW,
};

use super::depot::Extension;
use super::journal::queue_octets;
use super::verdict::Motif;

/// Beyond this, the agent STOPS WAITING — **it does not kill**.
///
/// 🔴 KILLING AN INSTALLER MIDWAY IS WORSE THAN STOPPING WAITING FOR IT: it
/// would leave the machine half installed, registry half written, without
/// anything knowing in which state. The outcome becomes `issue-inconnue`, which tells the
/// truth: we do not know.
///
/// ⚠️ **NOT CALIBRATED**, it joins the list this repository has kept since
/// `BPP_MIN`.
pub const EXPIRATION_EXECUTION_MS: u64 = 2 * 60 * 60 * 1000;

/// Between two polls of the process's exit.
const PAS_DE_SCRUTATION_MS: u64 = 500;

/// What a run produced.
pub struct Sortie {
    /// `None` = **the code could not be collected** — expiry, or read
    /// failure. It is not a code, and a `-1` sentinel would confuse them.
    pub code: Option<i32>,
    /// The tail of the log, bounded.
    pub journal: String,
    pub journal_tronque: bool,
    /// `true` if the agent stopped waiting without the process having exited.
    pub expire: bool,
}

/// 🔴 THE JOB GUARD, AND IT IS WHAT MAKES CRITERION ⑦ DECIDABLE.
///
/// The specification (D8) forbids the installer from being assigned to the supervisor's
/// job object: an agent restart would kill it in the middle of a registry
/// write and leave the machine half installed.
///
/// ❌ **WHAT FOLLOWS WAS A READING, AND THE MEASUREMENT REFUTED IT.** The plan
/// presented it as such — its M5 is titled "READING, NOT MEASUREMENT" —, and the
/// probe of task 2 existed to convert it. It did convert it, and
/// against it.
///
/// The reading said: "**the supervisor does not assign itself** — noted
/// in `superviseur/lanceur.rs`, which only calls `AssignProcessToJobObject`
/// on its CHILDREN. A process it creates therefore inherits no job." The
/// PREMISE is correct — `lanceur.rs` does do that. **The CONCLUSION is
/// false**, because it ignored the launch mode: `run-agent.sh` goes
/// through the **task scheduler**, which puts its task in a job. Measured
/// on the VM: supervisor, sensor and bridge are **all three** in a
/// job, as are the probe's PowerShell and the child it creates.
///
/// ⚠️ **There is no way out of it**: `CREATE_BREAKAWAY_FROM_JOB` is refused
/// (`ERROR_ACCESS_DENIED`, 5) — the job does not carry `JOB_OBJECT_LIMIT_BREAKAWAY_OK`.
///
/// ✅ **But the job DOES NOT KILL ON CLOSE**, and that is what saves
/// criterion ⑦: once the task is finished and its launcher dead, the direct child survives.
/// **⑦ is therefore neither vacuous nor lost — it is HELD, for a reason no
/// reading had found.**
///
/// 🔴 AND THE BRIDGE ITSELF **IS** IN THE JOB. `apps::brancher` is called before
/// the `PONT` switch in `main.rs`; under G1 legacy no. 1 — the bridge
/// enrolled under the same `vm_id` as its parent — the installation order
/// could fall to it, and it would have launched the installer **from a process
/// assigned to the job**, hence killed when the supervisor died. That is exactly what
/// spec D8 forbids, and it can only be seen by cross-reading three files.
///
/// **Hence this guard: the agent REFUSES to install from a process assigned to
/// a job.** A loud refusal is better than an installation that a
/// redeployment will kill midway.
///
/// ⚠️ **IT IS A GUARD, NOT THE REMEDY.** The remedy is G1 legacy no. 1, which
/// does not belong to G3: three decisions are possible there and none is
/// settled. G3 names it, protects itself from it, and does not close it.
/// Does this process's job **kill its members when it closes**?
///
/// 🔴 **IT IS THE QUESTION THAT DECIDES, and it replaced "am I in a
/// job?" on the strength of a MEASUREMENT** — see the module header. The danger that
/// spec D8 names is not membership of a job: it is
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, which makes the installer die with
/// the agent, in the middle of a registry write.
///
/// ⚠️ **`None` FOR THE HANDLE QUERIES THE JOB OF THE CURRENT PROCESS** — that is
/// the semantics of `QueryInformationJobObject`, and it is the only one we
/// have: we do not have the handle of the job the task
/// scheduler created, and we have no way of obtaining it.
///
/// ⚠️ **OUTSIDE ANY JOB, THE CALL FAILS**, and that is the nominal case of an agent
/// launched by hand. We then answer `false`: no job, no killing job.
pub fn job_tue_a_la_fermeture() -> bool {
    let mut infos = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let taille = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
    let ok = unsafe {
        QueryInformationJobObject(
            None,
            JobObjectExtendedLimitInformation,
            (&mut infos as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            taille,
            None,
        )
    };
    match ok {
        Ok(()) => infos
            .BasicLimitInformation
            .LimitFlags
            .contains(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE),
        Err(erreur) => {
            // ⚠️ A FAILURE IS NOT A YES. Outside any job, the call fails, and
            // that is exactly the state where there is nothing to fear. Refusing
            // the installation because the question could not be asked would be the
            // failure this guard has precisely just stopped being.
            tracing::debug!(%erreur, "QueryInformationJobObject a échoué : aucun job, ou job non interrogeable");
            false
        }
    }
}

pub fn dans_un_job() -> bool {
    // ⚠️ `BOOL` LIVES IN `windows::core`, NOT IN `Win32::Foundation` — an
    // API gap of windows-rs 0.62, which `agent/src/window.rs` already documents. The
    // crate is locked at **0.62.2** in `Cargo.lock`: reading a signature
    // from another version would send one looking for an error where there is none.
    let mut dedans = windows::core::BOOL(0);
    // ⚠️ `None` FOR THE JOB: the question is "in A job", not "in THIS
    // job". We do not have the handle of the supervisor's job, and we do not
    // need it.
    //
    // ❌ **THIS FUNCTION NO LONGER DECIDES ANYTHING, and the sentence that
    // justified it — "any job is enough to make the installer die" —
    // IS REFUTED BY THE MEASUREMENT** (see the module header). It stays because
    // it is logged at every installation: knowing that one is in a
    // job, without dying from it, is precisely the fact nobody expected.
    let ok = unsafe { IsProcessInJob(GetCurrentProcess(), None, &mut dedans) };
    match ok {
        Ok(()) => dedans.as_bool(),
        Err(erreur) => {
            // ⚠️ A FAILED QUESTION IS NOT AN ANSWER. We log
            // and answer `false`: refusing every installation because we
            // could not ask the question would be a failure worse than the risk.
            tracing::warn!(%erreur, "IsProcessInJob a échoué : on suppose hors job");
            false
        }
    }
}

/// Launches the installer and waits for it, without ever killing it.
///
/// `maintenant_ms` is a PARAMETER: it is the caller's clock, as
/// everywhere in this repository.
pub fn executer(
    chemin: &Path,
    extension: Extension,
    repertoire: &Path,
    maintenant_ms: impl Fn() -> u64,
) -> Result<Sortie, Motif> {
    // 🔴 THE GUARD GOES FIRST, AND BOTH BOOLEANS ARE LOGGED AT
    // EVERY INSTALLATION — whether the refusal happens or not. It is these lines,
    // and they alone, that make criterion ⑦ decidable.
    //
    // 🔴 THE GUARD IS ON `job_tue_a_la_fermeture`, NOT ON MEMBERSHIP, AND
    // A MEASUREMENT CORRECTED IT. It first tested `dans_un_job()`, on
    // the premise — written in this file — that "any job is enough to
    // make the installer die". **The probe of task 2 refuted it**, on
    // the VM, in one run:
    //
    //   - the THREE processes of the agent (supervisor, sensor, bridge) are
    //     in a job — it is not `lanceur.rs` that puts them there, it is the
    //     TASK SCHEDULER, through which `run-agent.sh` launches them;
    //   - `CREATE_BREAKAWAY_FROM_JOB` is REFUSED there (`ERROR_ACCESS_DENIED`, 5):
    //     there is no way out of it;
    //   - and yet, once the task is finished — launcher REALLY dead,
    //     checked by the absence of its control line —, **the direct child
    //     SURVIVES**. The job does not kill on close.
    //
    // The membership guard would therefore have refused **every** installation in the
    // product's normal launch mode, for a danger that does not materialise.
    // A refusal that can never be lifted is not a protection:
    // it is a failure. The guard now queries the property that KILLS, which
    // is exactly the one spec D8 names.
    let dedans = dans_un_job();
    let tue = job_tue_a_la_fermeture();
    tracing::info!(
        dans_un_job = dedans,
        job_tue_a_la_fermeture = tue,
        "installation : le processus qui lance est-il dans un job, et ce job tue-t-il ?"
    );
    if tue {
        tracing::error!(
            "installation refusée : ce processus est dans un job qui TUE À LA FERMETURE, \
             l'installeur y mourrait avec lui"
        );
        return Err(Motif::JobObject);
    }

    let journal_chemin = repertoire.join("installeur.log");
    // ⚠️ BOTH STREAMS GO TO THE SAME FILE, in the installation's
    // directory: it goes away with it in the age sweep, and it is where
    // someone will look for it.
    let fichier = std::fs::File::create(&journal_chemin).map_err(|erreur| {
        tracing::error!(%erreur, chemin = %journal_chemin.display(), "journal d'installeur non créé");
        Motif::Disque
    })?;

    let mut ligne = ligne_de_commande(chemin, extension);
    let mut ligne_utf16: Vec<u16> = ligne.encode_utf16().chain(std::iter::once(0)).collect();
    ligne.clear();
    let repertoire_utf16: Vec<u16> = repertoire
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let poignee = handle_de(&fichier);
    let depart = STARTUPINFOW {
        cb: u32::try_from(std::mem::size_of::<STARTUPINFOW>()).unwrap_or(0),
        dwFlags: windows::Win32::System::Threading::STARTF_USESTDHANDLES,
        hStdOutput: poignee,
        hStdError: poignee,
        ..Default::default()
    };
    let mut infos = PROCESS_INFORMATION::default();

    // 🔴 NO JOB FLAG, AND IT IS NO LONGER FOR LACK OF NEEDING ONE: it is
    // because there is no usable one. `CREATE_BREAKAWAY_FROM_JOB` was
    // MEASURED refused on this path (`ERROR_ACCESS_DENIED`, 5) — the task
    // scheduler's job does not carry `JOB_OBJECT_LIMIT_BREAKAWAY_OK`.
    // The child therefore inherits the job, and it has no consequence: that job does not kill
    // on close, measured as well.
    //
    // ⚠️ An earlier draft said here that "the guard above has already
    // established that this process is in no job". It was false on both
    // counts: it is in a job, and the guard no longer tests that.
    let resultat = unsafe {
        CreateProcessW(
            None,
            Some(PWSTR(ligne_utf16.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_NO_WINDOW,
            None,
            windows::core::PCWSTR(repertoire_utf16.as_ptr()),
            &depart,
            &mut infos,
        )
    };

    if let Err(erreur) = resultat {
        // 🔴 `ERROR_ELEVATION_REQUIRED` (740) BECOMES A TYPED REFUSAL, and that is
        // what makes G3 shippable even if the gate is unfavourable: a message
        // the hub can display, instead of a wait nobody
        // understands.
        //
        // ⚠️ THE PREMISE IS A READING OF DOCUMENTATION, NOT A MEASUREMENT OF THIS
        // REPOSITORY — see the module header. If the gate refutes it, this
        // branch becomes UNREACHABLE rather than false, and the typed refusal
        // will have to be rebuilt on another clue.
        let code = erreur.code().0 as u32 & 0xFFFF;
        if code == ERROR_ELEVATION_REQUIRED.0 {
            tracing::error!(
                chemin = %chemin.display(),
                "installation refusée : l'installeur exige une élévation, que \
                 cet agent ne peut pas obtenir sans que l'utilisateur voie un \
                 écran figé"
            );
            return Err(Motif::ElevationRequise);
        }
        tracing::error!(%erreur, chemin = %chemin.display(), "CreateProcessW a échoué");
        return Err(Motif::LancementImpossible);
    }

    // ⚠️ THE THREAD HANDLE IS RELEASED AT ONCE: nothing is done with it, and keeping it
    // would leak one handle per installation.
    let _ = unsafe { CloseHandle(infos.hThread) };

    let debut = maintenant_ms();
    let mut expire = false;
    let code = loop {
        let attente = unsafe { WaitForSingleObject(infos.hProcess, 0) };
        if attente == WAIT_OBJECT_0 {
            let mut brut = 0u32;
            match unsafe { GetExitCodeProcess(infos.hProcess, &mut brut) } {
                // ⚠️ THE CODE IS REPORTED, NEVER INTERPRETED: `msiexec` returns
                // 3010 for a success that requires a reboot, and many
                // installers return 0 after a cancellation.
                Ok(()) => break Some(brut as i32),
                Err(erreur) => {
                    tracing::warn!(%erreur, "code de sortie illisible");
                    break None;
                }
            }
        }
        if attente != WAIT_TIMEOUT {
            tracing::warn!(?attente, "attente du processus en erreur");
            break None;
        }
        if maintenant_ms().saturating_sub(debut) >= EXPIRATION_EXECUTION_MS {
            // 🔴 WE STOP WAITING, WE DO NOT KILL.
            tracing::warn!(
                chemin = %chemin.display(),
                "installation expirée : l'agent CESSE D'ATTENDRE, il ne tue pas — \
                 tuer un installeur au milieu laisserait la machine à moitié installée"
            );
            expire = true;
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(PAS_DE_SCRUTATION_MS));
    };
    let _ = unsafe { CloseHandle(infos.hProcess) };
    drop(fichier);

    // ⚠️ THE BOUND LIVES IN A PURE MODULE, WRITTEN ONLY ONCE. It was written
    // twice, and the second one PANICKED on a UTF-8 character boundary.
    let (journal, journal_tronque) = match std::fs::read(&journal_chemin) {
        Ok(octets) => queue_octets(&octets),
        Err(_) => (String::new(), false),
    };
    Ok(Sortie {
        code,
        journal,
        journal_tronque,
        expire,
    })
}

/// ⚠️ **NO SILENT MODE IS IMPOSED** (spec D8): `.exe` is run as
/// is, `.msi` goes through `msiexec /i`. Imposing `/qn` would decide on behalf of
/// the user, and many installers refuse a silent
/// installation they did not plan for.
fn ligne_de_commande(chemin: &Path, extension: Extension) -> String {
    let chemin = chemin.display().to_string();
    match extension {
        Extension::Exe => format!("\"{chemin}\""),
        Extension::Msi => format!("msiexec.exe /i \"{chemin}\""),
    }
}

fn handle_de(fichier: &std::fs::File) -> HANDLE {
    use std::os::windows::io::AsRawHandle;
    HANDLE(fichier.as_raw_handle() as _)
}
