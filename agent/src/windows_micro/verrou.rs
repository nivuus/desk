//! The Windows named mutex behind the `micro::exclusivite::Verrou` trait.
//!
//! 🔴 **It is the ONLY line of the exclusivity mechanism not tested
//! on the host** — the policy, for its part (who wins, when to retry, what gets
//! logged), lives in `micro/exclusivite.rs`, pure and tested under Linux.
//! This file only holds a handle.
//!
//! ## Why a NAMED mutex, and not a flag
//!
//! Since sub-block D1, N windows are N **processes**: an atomic
//! boolean guards nothing between them. And there is only one cable — with two
//! writers, Windows would mix two shifted copies of the same voice, a comb
//! filter (spec §9).
//!
//! ## `Global\` first, `Local\` as fallback, and the fallback is TOLD
//!
//! The cable is a resource **of the machine**, not of a session: the intended
//! namespace is `Global\`. Creating in it however requires
//! `SeCreateGlobalPrivilege`, which the interactive user does not always
//! hold; on refusal (`ERROR_ACCESS_DENIED`) we fall back to `Local\`.
//!
//! ✅ **MEASURED (acceptance E2, task 12, August 20th, 2026): it is `Global\` that is
//! obtained, in all FIVE green runs** — `espace_mutex="Global"`, without a
//! single occurrence of the fallback. The interactive user of this VM therefore does
//! hold `SeCreateGlobalPrivilege`. ⚠️ **The fallback below is consequently
//! code SHIPPED AND NEVER RUN**, and it must not be read as a path
//! in use: its `warn!` has never been emitted, on any machine.
//!
//! ⚠️ **This fallback narrows the guarantee's scope to ONE Windows session, and it
//! is therefore LOGGED, never silent.** A silent fallback on an exclusivity
//! guarantee is exactly the failure class that fix "A-bis"
//! exists to remove: the product would do something weaker than
//! what it announces, without any line saying so.
//!
//! ## `WAIT_ABANDONED` is a SUCCESS
//!
//! It is the release semantics spec §9 asks for: on the owner's
//! death, Windows abandons the mutex and the next one gets it.
//! Treating it as a failure would condemn window B to stay without a microphone for the life
//! of its process after window A died — the latent defect
//! Decision 2 of plan E2 fixes.

#![cfg(windows)]

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

use crate::micro::exclusivite::Verrou;

/// The object's name, without its namespace prefix.
///
/// ⚠️ **It designates the CABLE, not the agent**: it is the resource that is unique,
/// and two agents of different VMs do not see each other anyway. A name
/// carrying the session number would be one mutex per session, that is,
/// exactly the exclusivity this module does not provide.
const NOM: &str = "guacamole-agent-micro-cable";

/// A named mutex, acquired lazily and **never released explicitly**.
///
/// Release is the process's death: Windows then abandons
/// the mutex, and the next `WaitForSingleObject` of another process returns
/// `WAIT_ABANDONED`, hence a success. There is nothing to undo by hand, and
/// it is deliberate — a `ReleaseMutex` must come from the OWNING thread, yet the
/// owner here is the transport thread, which only ends with the
/// session.
pub struct MutexNomme {
    handle: HANDLE,
    /// The namespace actually obtained, for the log.
    espace: &'static str,
    /// ⚠️ **The short-circuit the `Verrou` trait's doc requires.** `tenter` is
    /// called at EACH deposit, that is ~50 times per second, including when the
    /// mutex is already owned: a `WaitForSingleObject` on an already
    /// held mutex increments its recursion count, and as many
    /// `ReleaseMutex` would be needed. This flag avoids that count, and it is this half
    /// that must carry it, not `Exclusivite`.
    tenu: bool,
}

// SAFETY: `MutexNomme` carries a `HANDLE`, that is a `*mut c_void`,
// which Rust does not mark `Send` by default. Marking it here is **necessary** —
// `Session::set_puits_micro` requires `Box<dyn PuitsMicro + Send>`, the session
// being taken by value by `run()` on a blocking thread — and it is **correct**
// for three reasons, in this order:
//
// 1. **A named mutex handle is a KERNEL object, valid for the whole
//    process**, not a reference tied to a COM apartment or a thread. That
//    is not the case of `LoopbackCapture`, whose `unsafe impl Send` must
//    rely on an apartment check at opening: here there is
//    no apartment involved.
// 2. **Nothing is OWNED at the time of the transfer.** `create` passes
//    `bInitialOwner = false`: acquisition is lazy, at the first deposit,
//    hence on the transport thread — the very one that will call `tenter`
//    afterwards. Ownership of a Windows mutex is per thread; what crosses the
//    boundary is only an ownerless handle.
// 3. **`Send` and not `Sync`**, and the gap is the heart of the argument: the sink
//    is moved ONCE, from construction to the transport thread, and is
//    then only touched by it. `Send` allows exactly this transfer, and
//    nothing more — two threads calling `tenter` concurrently would stay
//    forbidden by typing, and rightly so: the `tenu` flag
//    below assumes a single caller.
//
// **What would invalidate this promise**, to check before touching it:
// sharing the sink behind an `Arc` (it would then need `Sync`, which nothing
// establishes), or adding a `ReleaseMutex` — which must come from the
// OWNING thread, and would therefore make the calling thread significant.
unsafe impl Send for MutexNomme {}

impl MutexNomme {
    /// Creates (or opens) the mutex. **Acquires nothing**: acquisition is
    /// lazy, at the first deposit.
    pub fn create() -> Result<Self> {
        match Self::create_in("Global\\") {
            Ok((handle, espace)) => Ok(Self {
                handle,
                espace,
                tenu: false,
            }),
            Err(e) if e == ERROR_ACCESS_DENIED.into() => {
                tracing::warn!(
                    error = %e,
                    "mic: Global namespace refused (SeCreateGlobalPrivilege missing), \
                     FALLING BACK to Local. The cable exclusivity now only holds for THIS Windows \
                     session"
                );
                let (handle, espace) = Self::create_in("Local\\")
                    .context("creating the cable mutex, including in the Local namespace")?;
                Ok(Self {
                    handle,
                    espace,
                    tenu: false,
                })
            }
            Err(e) => Err(anyhow::Error::from(e).context("creating the mic cable mutex")),
        }
    }

    fn create_in(prefixe: &str) -> windows::core::Result<(HANDLE, &'static str)> {
        let nom: Vec<u16> = format!("{prefixe}{NOM}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `nom` stays alive during the whole call and ends with the
        // zero `PCWSTR` requires. `bInitialOwner = false`: we do NOT want to
        // own at creation — acquisition is lazy, and an owner
        // at creation would hold the cable from startup even
        // if no packet ever came up.
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(nom.as_ptr())) }?;
        let espace = if prefixe.starts_with("Global") {
            "Global"
        } else {
            "Local"
        };
        Ok((handle, espace))
    }

    /// The namespace obtained — `Global` or `Local`. Logged once at
    /// opening.
    pub fn espace(&self) -> &'static str {
        self.espace
    }
}

impl Verrou for MutexNomme {
    fn tenter(&mut self) -> bool {
        if self.tenu {
            return true;
        }
        // SAFETY: `handle` comes from a successful `CreateMutexW`. ZERO timeout: the
        // trait's doc requires this call not to block, and it is on the
        // transport loop's path.
        let issue = unsafe { WaitForSingleObject(self.handle, 0) };
        // `WAIT_ABANDONED`: the previous owner died without releasing.
        // Windows gives us ownership anyway — it is the NOMINAL case of
        // resumption after a neighbouring window's death, not an anomaly.
        //
        // ✅ OBSERVED, and it is no longer reasoning (acceptance E2, task 12):
        // a third-party process acquires the mutex, is KILLED by `Stop-Process
        // -Force` — hence without ever calling `ReleaseMutex` —, and the agent
        // gets the cable 46 s after its refusal (the "cable acquired after a
        // refusal" line), the judge going back from 0.000000 to 440.0 Hz on CABLE Output.
        // It is the only path through which it could get it.
        self.tenu = issue == WAIT_OBJECT_0 || issue == WAIT_ABANDONED;
        self.tenu
    }
}

// ⚠️ **No `Drop` closing the handle, and that is a choice.** This lock lives
// as long as the sink, hence as the session; closing it at the end of the
// process is what Windows does anyway. A `CloseHandle` placed here
// would run on the thread that destroys the sink, which is not necessarily the
// mutex's OWNING thread — and closing the handle of an owned mutex does not release
// it, it leaves it abandoned, which is already the behaviour obtained without writing
// anything.
