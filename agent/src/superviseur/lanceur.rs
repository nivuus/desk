//! Launching an agent process per window, and controlling its life.
//!
//! Separated from `boucle.rs`: it is the only part of the supervisor that talks about
//! Windows processes, and it satisfies a trait whose tests `enfants.rs` carries
//! with a fake launcher.
//!
//! **Two invariants hold this file**, and both are read in the
//! retained `Child` and in the Job Object.
//!
//! 1. **A PID alone designates nothing durable.** Windows only recycles the number
//!    of a dead process when the last handle to the process object
//!    is closed — and `drop` of a `std::process::Child` closes that handle.
//!    Releasing the `Child` and keeping only the PID means accepting that an
//!    `est_vivant` returns `true` on a foreign process and, far worse, that a
//!    `tuer` calls `TerminateProcess` on a third party. We therefore keep the
//!    `Child`: the handle stays open, the number stays reserved, and all
//!    operations go through that handle.
//! 2. **A dead supervisor must not leave N agents behind it.**
//!    `Command::spawn` attaches the child to nothing: when the supervisor dies,
//!    each agent would keep running with its DXGI duplication, its
//!    encoder and its signaling registration. The next startup
//!    would then assign session identifiers colliding
//!    with theirs (`Table::compteur` restarts from zero), and its purge
//!    would destroy outputs UNDER still-living children. The Job Object
//!    with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` closes this at kernel level:
//!    the supervisor's death closes its handles, hence the job, hence the children —
//!    including if it was killed outright, a case no user-space code
//!    can cover.
//!
//! **The contract of `Lanceur::lancer` is atomic** (see its documentation on
//! the trait): `Err` must mean no process is running. Attaching
//! to the job is the only fallible post-processing here, and it therefore
//! kills the child itself before returning `Err`. **Any addition after the
//! `spawn` must do the same.**

#![cfg(windows)]

use std::collections::HashMap;
use std::os::windows::io::AsRawHandle;
use std::sync::Mutex;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use super::enfants::{Consigne, Lanceur};

mod pont;
// `impl Lanceur`: what the supervisor loop asks of this launcher - start a
// window agent, tell whether it is alive, kill it.
mod agents;

/// A tracked child, and the little state to keep about it.
struct Enfant {
    /// Kept for its HANDLE, not for its number — see invariant 1 at the
    /// module head.
    processus: std::process::Child,
    /// True as soon as an unreadable state has been signalled for this child.
    ///
    /// **Fix I2 of the final review.** `est_vivant` is called at EACH
    /// supervisor loop turn, via `Enfants::morts()`: a persistent
    /// `try_wait` error produced about ten lines per
    /// second and PER CHILD there, on a CIFS share. A single report
    /// is enough — the state being persistent by assumption, repeating it teaches
    /// nothing new. Reset to false if the state becomes readable again, so that a
    /// second occurrence, for its part, shows.
    etat_illisible_signale: bool,
}

pub struct LanceurDeProcessus {
    executable: std::path::PathBuf,
    signaling_url: String,
    local_ip: String,
    /// The living children, by PID.
    ///
    /// `Mutex` and not `RefCell`: `Lanceur` takes `&self`, and nothing promises
    /// this launcher will stay consulted from a single thread.
    enfants: Mutex<HashMap<u32, Enfant>>,
    /// The single pooled-capture capturer (sub-block D4), tracked apart
    /// from the children: it has no session, hence no place in `enfants`.
    /// `None` as long as no capturer has been launched yet, or right after
    /// the previous one was observed dead by `capteur_vivant`.
    capteur: Mutex<Option<Enfant>>,
    /// The single files bridge (sub-project ③), tracked apart for the same
    /// reason as the capturer: it has no window, hence no place in
    /// `enfants`. See `lanceur/pont.rs`.
    pont: Mutex<Option<Enfant>>,
    /// This VM's session prefix, delivered at enrolment (sub-block
    /// P3). `Table` already carries it to compose the children's sessions; the
    /// launcher needs it for the only session it composes itself,
    /// the bridge's.
    prefixe: String,
    /// The watch on the platform identity — the CURRENT agent token.
    ///
    /// 🔴 A WATCH, AND NOT A FROZEN STRING: the token lasts ten minutes and
    /// renews at each heartbeat, whereas a supervisor lives for
    /// hours. A snapshot taken at startup would make a window opened
    /// later receive a DEAD token, and its session would not be established.
    ///
    /// `None` when this supervisor has no channel (`AGENT_VM`/`AGENT_SECRET`
    /// absent): its children will have no token either, which is
    /// exactly the previous state — announced by a `warn!`, never silent.
    identite: Option<tokio::sync::watch::Receiver<Option<crate::plateforme::Identite>>>,
    /// The job every child — the capturer and the bridge included — is
    /// attached to. Closing it kills them all.
    job: HANDLE,
}

impl LanceurDeProcessus {
    pub fn new(
        executable: std::path::PathBuf,
        signaling_url: String,
        local_ip: String,
        prefixe: String,
        identite: Option<tokio::sync::watch::Receiver<Option<crate::plateforme::Identite>>>,
    ) -> Result<Self> {
        let job = unsafe { CreateJobObjectW(None, None) }
            .context("création du job object des enfants")?;
        let limites = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation:
                windows::Win32::System::JobObjects::JOBOBJECT_BASIC_LIMIT_INFORMATION {
                    LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    ..Default::default()
                },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limites as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .context("pose de KILL_ON_JOB_CLOSE sur le job object des enfants")?;
        Ok(Self {
            executable,
            signaling_url,
            local_ip,
            enfants: Mutex::new(HashMap::new()),
            capteur: Mutex::new(None),
            pont: Mutex::new(None),
            prefixe,
            identite,
            job,
        })
    }

    /// Access to the table without panicking on a poisoned lock.
    ///
    /// The lock gets poisoned as soon as a panic crosses a holder — and there
    /// is one: `lancer` can panic between the `spawn` and the insertion. After
    /// which an `.expect(…)` would make **all** following calls panic,
    /// that is, `est_vivant` and `tuer`: the supervisor would lose at once
    /// the ability to observe a death and the ability to kill. The job
    /// object would catch the children at the end, but much later and without
    /// anything explaining it. `into_inner` returns the table as is: at
    /// worst an interrupted insertion is missing from it.
    ///
    /// (`Drop` does NOT go through here: it only touches the job handle.)
    fn enfants(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Enfant>> {
        self.enfants
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Same reason as `enfants` above: do not panic on a poisoned
    /// lock, otherwise an isolated panic in `lancer_capteur` would make
    /// `capteur_vivant` unusable forever, and the supervisor could
    /// never again observe nor restart the capturer.
    fn capteur(&self) -> std::sync::MutexGuard<'_, Option<Enfant>> {
        self.capteur
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// Same reason as `capteur` above, for the files bridge.
    fn pont(&self) -> std::sync::MutexGuard<'_, Option<Enfant>> {
        self.pont
            .lock()
            .unwrap_or_else(|empoisonne| empoisonne.into_inner())
    }

    /// The CURRENT agent token, reread at each launch.
    ///
    /// `None` if this supervisor has no channel, or if enrolment has not
    /// succeeded yet. The watch keeps the last known identity even with the
    /// channel cut: a token can therefore be stale, and that is accepted — a
    /// stale token is refused loudly at the handshake, where the absence
    /// of a token refuses it just as much. The real remedy for a long outage is
    /// the channel's resumption, not an abstention here.
    fn current_token(&self) -> Option<String> {
        let veille = self.identite.as_ref()?;
        let courante = veille.borrow();
        courante.as_ref().map(|identite| identite.jeton.clone())
    }

    /// Sets the INHERITED identity on a child process, and removes the parent's.
    ///
    /// 🔴 **THE ENROLMENT SECRET NEVER CROSSES THIS LINE**, and it is
    /// the fix of August 20th, 2026. A child inheriting `AGENT_VM` and
    /// `AGENT_SECRET` opened ITS OWN `/agent` channel under the same identity
    /// as its parent: the platform only admitting one socket per VM, each
    /// evicted the other, the evicted one resumed at once, and the cycle had
    /// no end — **95 enrolments and 94 evictions in 64 s**, recorded on
    /// the VM with only two processes, the supervisor and the bridge.
    ///
    /// What is passed on is the TOKEN, because a child and the bridge
    /// need it — each opens its own `PeerConnection`, and the guard refuses an
    /// agent handshake without a token since P3 — and because they need
    /// NOTHING else from the channel: they beat no heart, push
    /// no catalogue, receive no launch order.
    ///
    /// ⚠️ **`AGENT_JETON` IS REMOVED WHEN THERE IS NONE**, rather than left
    /// to inheritance: a supervisor launched by hand in an environment that
    /// carried an old one would otherwise pass it to all its children, and the
    /// failure — refused handshakes, no session — would have no
    /// trace linking it to a forgotten environment variable.
    fn identite_heritee(&self, commande: &mut std::process::Command) {
        commande.env_remove("AGENT_VM").env_remove("AGENT_SECRET");
        match self.current_token() {
            Some(jeton) => commande.env("AGENT_JETON", jeton),
            None => commande.env_remove("AGENT_JETON"),
        };
    }

    /// Removes ALL platform identity — for the only process that has
    /// no need for it, the capturer.
    ///
    /// ⚠️ **THE CAPTURER WAS NOT AT FAULT**, and it is checked rather than
    /// assumed: `main.rs` returns control to it BEFORE enrolment, so a
    /// capturer carrying `AGENT_VM` never enrolled. We remove them
    /// anyway, by the rule this file already imposes on itself three times for
    /// `SUPERVISEUR`, `CAPTEUR` and `PONT`: **a test order is a
    /// property that changes, an `env_remove` is not.** The day a capturer
    /// needed to talk to signaling, it would go through enrolment and
    /// reopen the defect, with nothing having announced it.
    fn sans_identite(commande: &mut std::process::Command) {
        commande
            .env_remove("AGENT_VM")
            .env_remove("AGENT_SECRET")
            .env_remove("AGENT_JETON");
    }

    /// Launches the single pooled-capture capturer (`agent/src/capteur.rs`):
    /// same executable, `CAPTEUR=1`, **attached to the same job object** as the
    /// children — otherwise it would outlive the supervisor while holding N
    /// DXGI duplications and N virtual outputs captive from a pool that only
    /// counts ten.
    ///
    /// **Same atomic contract as `lancer`** (see the trait's doc): `Err`
    /// means no process is running. Attaching to the job is the
    /// only fallible post-processing, and it therefore kills the capturer itself
    /// before returning `Err`.
    pub fn lancer_capteur(&self) -> Result<u32> {
        let mut commande = std::process::Command::new(&self.executable);
        commande
            .env("CAPTEUR", "1")
            // Same reason as for a child (see `lancer` below): a
            // capturer inheriting `SUPERVISEUR` would take itself for a
            // supervisor and launch its own children, indefinitely.
            .env_remove("SUPERVISEUR")
            // And `PONT` by the same symmetry rule. The case is HARMLESS
            // today — `main.rs` tests `CAPTEUR` BEFORE `PONT`, so a
            // capturer carrying `PONT` stays a capturer —, the exact opposite of the
            // case of a bridge inheriting `CAPTEUR`, which is FATAL. We
            // remove it anyway: **a test order is a property that
            // changes, an `env_remove` is not**, and it is the reasoning this
            // file already holds for `SUPERVISEUR` here and for `CAPTEUR` in
            // `lancer`. An asymmetry is a dormant trap.
            .env_remove("PONT")
            // Same reason as for a child: these two variables change the
            // MEANING of a source (a test file to broadcast, a window to
            // look up by title) — and the capturer has no single source
            // designated by them, it holds N, each described by the child
            // attaching to it via the named pipe.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE");
        Self::sans_identite(&mut commande);
        let mut capteur = commande.spawn().context("lancement du capteur")?;
        let pid = capteur.id();
        let handle = HANDLE(capteur.as_raw_handle());
        if let Err(error) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            // Same atomic contract as `lancer`: a capturer not attached to the
            // job would outlive the supervisor WHILE HOLDING N duplications.
            if let Err(mise_a_mort) = capteur.kill() {
                tracing::error!(pid, %mise_a_mort,
                    "capteur NON rattaché au job ET NON tué — il survivra au superviseur");
            }
            let _ = capteur.wait();
            return Err(anyhow::Error::new(error)
                .context(format!("rattachement du capteur {pid} au job object")));
        }
        tracing::info!(pid, "capteur lancé");
        *self.capteur() = Some(Enfant {
            processus: capteur,
            etat_illisible_signale: false,
        });
        Ok(pid)
    }

    /// True as long as the capturer launched by the last successful `lancer_capteur`
    /// is alive. Returns `false` if no capturer was ever launched, or if
    /// the previous one died — callers must call `lancer_capteur` again.
    ///
    /// Same logic as `est_vivant` (see below): an unreadable state is
    /// NOT treated as a death — declaring it dead would restart a
    /// capturer that may still be running, doubling the DXGI duplications —
    /// and is only logged once as long as it persists.
    pub fn capteur_vivant(&self) -> bool {
        let mut capteur = self.capteur();
        let Some(en_cours) = capteur.as_mut() else {
            return false;
        };
        match en_cours.processus.try_wait() {
            Ok(None) => {
                en_cours.etat_illisible_signale = false;
                true
            }
            Ok(Some(code)) => {
                tracing::info!(pid = en_cours.processus.id(), ?code, "capteur terminé");
                *capteur = None;
                false
            }
            Err(error) => {
                if !en_cours.etat_illisible_signale {
                    en_cours.etat_illisible_signale = true;
                    tracing::warn!(
                        %error,
                        "état du capteur illisible, tenu pour vivant \
                         (signalé une seule fois tant que l'état reste illisible)"
                    );
                }
                true
            }
        }
    }
}

impl Drop for LanceurDeProcessus {
    fn drop(&mut self) {
        // Closing the job kills what it contains (`KILL_ON_JOB_CLOSE`). The
        // remaining `Child`ren go with the table and their handles close
        // too; the order does not matter, the job is the underlying guarantee.
        let _ = unsafe { CloseHandle(self.job) };
    }
}
