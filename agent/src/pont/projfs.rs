//! The ProjFS virtualisation root: mark it, start it, stop it.
//!
//! **`#[cfg(windows)]`, and called by the SYSTEM: no host test is
//! possible here, and it is declared, not worked around** (spec §4.4). The only
//! compensation is that this module be **thin** — it translates, it does not
//! decide. Every decision that can live in a pure module lives there:
//! `pont::chemins` (normalisation), `pont::errors` (the `HRESULT`s),
//! `pont::decoupe` (the ranges), `pont::table` (the commands in flight),
//! `pont::resolution` (the thirteen entries).
//!
//! # THE THREADING DISCIPLINE, and it is this module's central decision
//!
//! **Three categories of threads, and the boundary between them is strict.**
//!
//! ⚠️ **FOUR THREADS SINCE F2, FOR THREE CATEGORIES.** The **write thread**
//! (`pont::ecriture::fil`) joins category 3: it completes no
//! command, but it shares its essential property — **it runs on
//! no system thread**. It reads files of the root, which the bridge
//! thread must NEVER do (see `pont/service.rs`: "it would wait for
//! itself"), and that is precisely why it is distinct from it.
//!
//! 1. **The CALLBACK threads, which the SYSTEM owns.** ProjFS keeps a pool of them
//!    sized by `PRJ_STARTVIRTUALIZING_OPTIONS.PoolThreadCount` /
//!    `ConcurrentThreadCount` (windows-rs, `ProjectedFileSystem/mod.rs:518-523`).
//!    A callback running there:
//!      - wraps ALL its body in `std::panic::catch_unwind` and returns
//!        `E_UNEXPECTED` — a Rust panic crossing an `extern "system"`
//!        boundary is a **PROCESS ABORT**;
//!      - writes ONLY into the locked states of [`Etat`], and pushes on an
//!        `mpsc::Sender`;
//!      - **NEVER calls `PrjCompleteCommand`**, NEVER touches the
//!        socket, NEVER holds a lock during an I/O;
//!      - returns `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` and returns control
//!        IMMEDIATELY.
//!
//!    Waiting there for a browser round trip would freeze the APPLICATION reading the
//!    file — **not the video**: the stream keeps flowing and the window
//!    shows a frozen application (spec §5.2). It is the only reason to exist
//!    of this discipline.
//!
//! 2. **The TRANSPORT thread**, which owns the `Rtc` and the UDP socket
//!    (`pont::transport::tourner`).
//!
//! 3. **The BRIDGE thread**, which owns the table, completes commands through
//!    `PrjCompleteCommand`, and sweeps expiries
//!    (`pont::service`, task 14).
//!
//! ⚠️ **DELIBERATE DIVERGENCE FROM F1'S PLAN** (task 13, step 2), which
//! writes "**THE** bridge thread, unique. It owns the `Rtc`, reads the socket,
//! completes commands through `PrjCompleteCommand`, and sweeps
//! expiries". **These two roles cannot fit in a single thread**:
//! `pont::transport::tourner` — whose signature is fixed by the §
//! "Shared interfaces" of the same plan, and which has been delivered since task 11 —
//! owns the `Rtc` in a blocking loop and **knows neither ProjFS nor
//! Windows**, which is precisely what makes it testable on the host. Putting
//! `PrjCompleteCommand` there would destroy this property. Hence two threads, 2 and
//! 3, linked by the two `mpsc`s the plan defines itself. **The invariant
//! that matters is preserved**: no callback completes, no callback does
//! I/O, and `pont::table` stays pure.
//!
//! The `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT` is a raw pointer shared between
//! the three: it is wrapped in [`Contexte`], whose `unsafe impl Send +
//! Sync` carries its justification.
//!
//! # What happens when a command exceeds its delay
//!
//! **A callback never exceeds its delay: it returns in microseconds.** What
//! exceeds is the **command** it registered. The bridge thread's sweep
//! removes the expired ones from the table and completes them with
//! `PrjCompleteCommand(command_id, HRESULT_FROM_WIN32(ERROR_SEM_TIMEOUT))`.
//!
//! ⚠️ **THIS PATH HAS NEVER BEEN OBSERVED.** The expired-command log line counts **0** in the
//! five nominal runs of F1's acceptance run, and the only occurrence in the whole
//! corpus (`agent-dbg-plat.log`, one line) comes **after** the driver has
//! closed the browser. Yet the same acceptance run shows reads that **stall
//! without expiring** — `exec1`'s VM measurement does not return in 540 s. What
//! the absence of trace establishes is that the sweep removed nothing; it does
//! **not** say **where** the blockage happens, and a blockage UPSTREAM of the table
//! registration would leave this paragraph literally true while describing a
//! path nothing reaches. **Deciding would require a trace at
//! registration, which does not exist.**
//!
//! The application receives an expired I/O; **nothing is replayed**, ever — an
//! expired command whose request we replayed would produce a second
//! response without a recipient (spec §5.3). If the browser's response arrives
//! **after**, `Table::resoudre` returns `None` and the response is **thrown away**.

pub mod chargement;
mod etat;
mod racine;
mod rappels;

pub use etat::{ContexteProjFs, DataStream, Etat, TamponEntrees};
pub use racine::dossier_etat;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use windows::core::PCWSTR;
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_FLAG_USE_NEGATIVE_PATH_CACHE, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    PRJ_NOTIFICATION_MAPPING, PRJ_STARTVIRTUALIZING_OPTIONS,
};

use crate::pont::errors::Error;
use crate::pont::table::Table;
use crate::pont::transport::VersNavigateur;

/// Period of the hydration survey.
///
/// ⚠️ **Case 4 of spec §6.4 — "not a loss, but it bites" —
/// APPLIES FULLY to F1, and F1 is the sub-block that creates it**: each
/// file READ is written in full to the VM's disk, and **it stays there**.
/// **No eviction policy in F1**: `PrjDeleteFile` is loaded
/// (task 12) so that the policy, when it comes, does not have to reopen the
/// layer. Setting a policy without measurement would be exactly the gesture this
/// repository reproaches its uncalibrated constants for. ✅ **THE MEASUREMENT ARRIVED
/// IN F5** (gate P1), ⛔ **THE EVICTION POLICY DID NOT**: `PrjDeleteFile`
/// stays loaded and without a caller, and sub-project ③ closes behind F5.
pub const PERIODE_HYDRATATION: std::time::Duration = std::time::Duration::from_secs(60);

/// The virtualisation context, shared between the callback threads, the bridge
/// thread and the main thread.
///
/// # Safety
///
/// `PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT` is an opaque `*mut c_void`. It is
/// `Send + Sync` **because ProjFS documents it as such**: it is this same
/// context the system passes simultaneously to all threads of its own
/// callback pool, and all the entry points taking it
/// (`PrjCompleteCommand`, `PrjWriteFileData`, `PrjAllocateAlignedBuffer`…) are
/// callable from any of them. We never do anything with it
/// other than pass it to these entry points; **we never dereference
/// it**.
#[derive(Clone, Copy)]
pub struct Contexte(pub PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT);
// SAFETY: see the justification above. It is written rather than
// assumed — it is the rule this repository imposes on itself for every `unsafe impl`.
unsafe impl Send for Contexte {}
unsafe impl Sync for Contexte {}

/// A live virtualisation root. **Its `Drop` stops the
/// virtualisation** — it is the only stop path, and it is not optional.
pub struct Virtualisation {
    etat: Arc<Etat>,
    /// The `Arc` copy entrusted to ProjFS, to take back after the stop.
    confie: *const Etat,
    racine: PathBuf,
}

// SAFETY: `confie` is a `*const Etat` from `Arc::into_raw`, and `Etat` is
// `Send + Sync` (all its fields are, `Contexte` through the `unsafe impl`
// above). The pointer is never dereferenced by this type; it is only
// handed back to `Arc::from_raw` in `drop`.
unsafe impl Send for Virtualisation {}

impl Virtualisation {
    /// Prepares the root, marks it if needed, and starts virtualisation.
    pub fn start(
        projfs: chargement::ProjFs,
        sortant: std::sync::mpsc::Sender<VersNavigateur>,
        vers_ecriture: std::sync::mpsc::Sender<crate::pont::ecriture::fil::Ordre>,
        inscriptible: bool,
        mutations_armees: bool,
        cache_arme: bool,
    ) -> Result<Self> {
        let racine = racine::racine()?;
        let etat = Arc::new(Etat {
            projfs,
            contexte: Mutex::new(None),
            table: Arc::new(Mutex::new(Table::new())),
            sessions: Mutex::new(HashMap::new()),
            en_attente: Mutex::new(HashMap::new()),
            sortant,
            vers_ecriture,
            inscriptible,
            mutations_armees,
            // ⚠️ **`false` AT START, and it is not a style precaution**:
            // ProjFS can call a callback DURING `PrjStartVirtualizing`,
            // that is, well before the browser has opened its channel.
            // Starting from `true` would allow a write that would have no one
            // to be pushed to.
            canal_ouvert: std::sync::atomic::AtomicBool::new(false),
            compteurs: crate::pont::compteurs::Compteurs::nouveaux(),
            latences: crate::pont::latence::Histogramme::new(),
            cache: Mutex::new(crate::pont::cache::CacheEnumeration::new()),
            cache_arme,
            octets_hydrates: AtomicU64::new(0),
            entrees_hydratees: AtomicU64::new(0),
        });

        racine::preparer(&etat.projfs, &racine)?;

        // The callback block and the options must stay valid during the whole
        // call. `Box::leak` rather than a local variable: ProjFS's documentation
        // does not say whether the service RETAINS the pointer of the
        // `NotificationMappings` beyond the call, and **an assumption that
        // turned out false would produce a use-after-free read in
        // a system service**. An allocation of a few dozen
        // bytes, once per process, buys certainty — and the bridge
        // is a dedicated process that starts only one.
        let rappels = Box::leak(Box::new(rappels::bloc()));
        // An empty mask as `NotificationRoot` designates the root itself:
        // the path is RELATIVE to the virtualisation root.
        let racine_relative = Box::leak(Box::new([0u16; 1]));
        let mappings = Box::leak(Box::new([PRJ_NOTIFICATION_MAPPING {
            // The mask lives in the PURE module, where it is pinned: exactly SEVEN bits
            // since F2 — five in F1 —, and none of them falls back
            // into the catch-all arm of `notifications::decider`.
            NotificationBitMask: windows::Win32::Storage::ProjectedFileSystem::PRJ_NOTIFY_TYPES(
                crate::pont::notifications::MASQUE,
            ),
            NotificationRoot: PCWSTR(racine_relative.as_ptr()),
        }]));
        let options = Box::leak(Box::new(PRJ_STARTVIRTUALIZING_OPTIONS {
            Flags: PRJ_FLAG_USE_NEGATIVE_PATH_CACHE,
            // Zero = ProjFS's default sizing. Setting a value
            // without having measured it would join this repository's list of
            // uncalibrated constants, for an unknown gain.
            PoolThreadCount: 0,
            ConcurrentThreadCount: 0,
            NotificationMappings: mappings.as_mut_ptr(),
            NotificationMappingsCount: 1,
        }));

        // The copy entrusted to ProjFS. Created BEFORE `PrjStartVirtualizing`:
        // a callback can occur during the call, and it must already be able to
        // find the state.
        let confie = Arc::into_raw(Arc::clone(&etat));
        let chemin = racine::utf16(&racine);
        let mut contexte = PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT::default();
        // SAFETY: `chemin`, `rappels`, `options` and `mappings` are valid;
        // `contexte` is an initialised output slot. The called signature
        // is the transcription of the `link!` at `mod.rs:101`, with FIVE
        // parameters — the fifth is this output parameter, which windows-rs's
        // wrapper hides behind its `Result`.
        let issue = unsafe {
            (etat.projfs.start_virtualizing)(
                PCWSTR(chemin.as_ptr()),
                rappels,
                confie as *const core::ffi::c_void,
                options,
                &mut contexte,
            )
        };
        if issue.is_err() {
            // Take back the entrusted copy: virtualisation did not
            // start, so no callback can reach it any more.
            // SAFETY: `confie` comes from `Arc::into_raw` just above and has
            // been handed to no one else.
            drop(unsafe { Arc::from_raw(confie) });
            bail!(
                "PrjStartVirtualizing sur « {} » : {issue}",
                racine.display()
            );
        }
        *etat.contexte.lock().expect("context lock") = Some(Contexte(contexte));
        tracing::info!(racine = %racine.display(), "ProjFS virtualisation root started");
        Ok(Self {
            etat,
            confie,
            racine,
        })
    }

    /// The shared state, for the bridge thread.
    pub fn etat(&self) -> Arc<Etat> {
        Arc::clone(&self.etat)
    }

    /// The root, for the hydration survey.
    pub fn racine(&self) -> &Path {
        &self.racine
    }
}

impl Drop for Virtualisation {
    /// **The order is not negotiable** (plan, task 13 step 6):
    ///
    /// 1. empty the table, and **complete EACH command** with `ERROR_IO_DEVICE`;
    /// 2. `PrjStopVirtualizing`;
    /// 3. take back the `Arc` copy entrusted to ProjFS.
    ///
    /// ⚠️ **It is the remedy to the last defect of the spec's appendix §13**:
    /// the old bridge's forced stop did NOT call the pending FUSE callbacks
    /// (`src/file.js:359-365`), and the kernel never got a
    /// response. Here, `PrjStopVirtualizing` is **preceded** by the error
    /// completion of everything left — a command left in flight would wait there for
    /// a response nothing can deliver any more.
    ///
    /// **`Drop` never panics**: each lock is taken through
    /// `lock().ok()`, never through `expect`, because a lock poisoned by another
    /// thread's panic would make a double panic here, hence an `abort`
    /// — and the `abort` would leave the root mounted, that is, the exact case
    /// this `Drop` exists to avoid.
    fn drop(&mut self) {
        let contexte = self.etat.contexte.lock().ok().and_then(|c| *c);
        let Some(Contexte(contexte)) = contexte else {
            tracing::warn!("bridge shutdown: no virtualisation context to release");
            return;
        };

        // 1. The commands in flight, completed with an I/O error. The table is
        //    emptied BEFORE the stop, never after: after, the context is no longer
        //    valid and `PrjCompleteCommand` has nowhere left to write.
        let restantes = match self.etat.table.lock() {
            Ok(mut table) => table.drain(),
            Err(empoisonne) => {
                tracing::error!("table lock poisoned at shutdown: the table is emptied anyway");
                empoisonne.into_inner().drain()
            }
        };
        let echec = windows::core::HRESULT(self.etat.compteurs.rendre(Error::CanalFerme));
        for (commande, correlation) in &restantes {
            // 🔴 **A WRITE IN FLIGHT IS EMPTIED FROM THE TABLE LIKE THE OTHERS,
            // BUT IS NOT REMOVED FROM THE JOURNAL** — it is exactly the case
            // the journal exists to cover. The restarted bridge will push it again.
            //
            // There is nothing to complete: `command_id` is `None`, and there
            // is no ProjFS callback behind a write.
            let Some(commande) = commande else {
                tracing::debug!(
                    correlation,
                    "write in flight at shutdown: nothing to complete, the entry STAYS in the journal"
                );
                continue;
            };
            // SAFETY: valid context (virtualisation is not stopped
            // yet), `commande` comes from the table, and the fourth parameter
            // is null — `PrjCompleteCommand` accepts the absence of extended
            // parameters, which windows-rs's wrapper expresses as an
            // `Option::None` turned into a null pointer (`mod.rs:14`).
            let issue = unsafe {
                (self.etat.projfs.completer_commande)(contexte, *commande, echec, std::ptr::null())
            };
            if issue.is_err() {
                tracing::warn!(commande, correlation, %issue, "shutdown completion refused");
            }
        }
        if !restantes.is_empty() {
            tracing::info!(
                commandes = restantes.len(),
                "in-flight commands completed with an I/O error before the virtualisation stops"
            );
        }

        // 2. The stop itself. ProjFS guarantees that no callback runs after
        //    this call returns: it is what makes step 3 safe.
        // SAFETY: `PrjStopVirtualizing` returns NOTHING (`mod.rs:109`), and the
        // transcription reflects it.
        unsafe { (self.etat.projfs.arreter_virtualisation)(contexte) };
        tracing::info!(racine = %self.racine.display(), "ProjFS virtualisation stopped");

        // 3. The entrusted copy, taken back.
        // SAFETY: `confie` comes from `Arc::into_raw` in `start`, has only been
        // handed to ProjFS, and no callback can run any more.
        drop(unsafe { Arc::from_raw(self.confie) });
    }
}
