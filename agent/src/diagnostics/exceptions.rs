//! Exception filter: records the call stack AT THE MOMENT of the fault.
//!
//! Work stream `2026-07-31-duplications-paralleles`, task 2bis. An intermittent
//! crash (`0xc0000005` in `ntdll.dll`, offset `0x19daa`) takes down the
//! process at the destruction of an `H264Encoder`. Two rounds tried to
//! **guess** the faulty call by bracketing calls with traces; this module
//! **reads** it.
//!
//! # Three choices that are not cosmetic
//!
//! 1. **`AddVectoredExceptionHandler` at FIRST chance** (and not
//!    `SetUnhandledExceptionFilter` alone): it sees the exception before any
//!    handler, hence even if something swallows it. The final filter
//!    is set as well, to distinguish the fault that kills the process from
//!    those that are caught.
//! 2. **No allocation in the handler, and no `tracing`.** The
//!    fault found is in `RtlpEnterCriticalSectionContended`: if the
//!    critical section involved is a heap's, allocating from the
//!    handler would crash again or block. The module table is
//!    snapshotted at installation, the message is formatted in a stack
//!    buffer and written by a single `write`.
//! 3. **Log separate from `agent.log`.** The agent's standard output
//!    goes through a PowerShell pipe (`scripts/run-agent.sh`): what is in
//!    flight there when the process dies is lost. This file is written
//!    directly, then synced.
//!
//! The handler only runs at the crash: unlike the traces
//! placed in `Drop` by round 2, it does not change the timing before the
//! fault, so it should not scare away a timing-sensitive defect.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, EXCEPTION_ACCESS_VIOLATION};
use windows::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, RtlCaptureStackBackTrace, SetUnhandledExceptionFilter, CONTEXT,
    EXCEPTION_POINTERS, EXCEPTION_RECORD,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
};
use windows::Win32::System::Threading::GetCurrentThreadId;

/// A loaded module, as snapshotted at installation.
struct Module {
    base: usize,
    taille: usize,
    nom: String,
}

/// Snapshot of the module table. Read — never written — from the
/// handler, hence without allocation at the moment of the fault.
static MODULES: OnceLock<Vec<Module>> = OnceLock::new();
/// Dedicated log, opened once at installation.
static JOURNAL: OnceLock<File> = OnceLock::new();
/// Identifier of the thread that installed the filter — that of `main`. Serves to tell
/// whether the fault happens on THIS thread or on a third party's worker thread
/// (Media Foundation, NVIDIA driver), which nothing established until now.
static FIL_PRINCIPAL: AtomicU32 = AtomicU32::new(0);
/// Report cap: a looping access violation must not drown the
/// log nor slow down death.
static RAPPORTS: AtomicU32 = AtomicU32::new(0);
/// Non-reentrancy guard: if the handler itself faults, do not
/// go back into it.
static DANS_LE_GESTIONNAIRE: AtomicBool = AtomicBool::new(false);

const PLAFOND_RAPPORTS: u32 = 8;
const CONTINUER_LA_RECHERCHE: i32 = 0;

/// Installs the filter if `AGENT_TRACE_EXCEPTIONS` is set to anything other than
/// `0`. Silent and without effect otherwise.
///
/// The log path is set through `AGENT_TRACE_EXCEPTIONS_FICHIER`; it
/// defaults to `C:\dev\exceptions.log`, next to `agent.log`.
pub(crate) fn installer() {
    if std::env::var("AGENT_TRACE_EXCEPTIONS").is_ok_and(|v| v != "0") {
        // Do nothing more if opening the log fails: a
        // diagnostic must never prevent the measurement from running.
        let chemin = std::env::var("AGENT_TRACE_EXCEPTIONS_FICHIER")
            .unwrap_or_else(|_| r"C:\dev\exceptions.log".to_string());
        match File::create(&chemin) {
            Ok(fichier) => {
                let _ = JOURNAL.set(fichier);
            }
            Err(e) => {
                tracing::warn!(chemin, erreur = %e, "journal d'exceptions non ouvert");
                return;
            }
        }
        let _ = MODULES.set(photographier_les_modules());
        FIL_PRINCIPAL.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);

        unsafe {
            // `1` = at the head of the chain, hence before any handler already set.
            AddVectoredExceptionHandler(1, Some(filtre_vectorise));
            SetUnhandledExceptionFilter(Some(filtre_final));
        }
        let nombre = MODULES.get().map(|m| m.len()).unwrap_or(0);
        tracing::info!(
            chemin,
            modules = nombre,
            fil_principal = FIL_PRINCIPAL.load(Ordering::SeqCst),
            "filtre d'exception installé"
        );

        if std::env::var("AGENT_TRACE_EXCEPTIONS_AUTOTEST").is_ok_and(|v| v != "0") {
            autotest();
        }
    }
}

/// Deliberately causes an access violation, to **test the instrument
/// before trusting its silence**.
///
/// A campaign without a crash proves nothing if we have not shown that the
/// filter would have spoken. This path shows it: it kills the process, produces a
/// report in the dedicated log, and makes WER produce its dump — the
/// three links of the diagnostic chain, tested at once, without
/// waiting for the intermittent defect.
///
/// Only runs with `AGENT_TRACE_EXCEPTIONS_AUTOTEST` set to anything other
/// than `0`, in addition to `AGENT_TRACE_EXCEPTIONS`.
fn autotest() {
    tracing::warn!(
        "AUTOTEST du filtre d'exception : violation d'accès délibérée, le processus va mourir"
    );
    // Write to an unmapped address deliberately low and recognisable
    // in the report ("adresse fautive").
    let adresse = 0x24usize as *mut u32;
    unsafe { std::ptr::write_volatile(adresse, 0xdead_beef) };
}

/// Snapshots base/size/name of each loaded module.
fn photographier_les_modules() -> Vec<Module> {
    let mut modules = Vec::new();
    unsafe {
        let Ok(instantane) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0) else {
            return modules;
        };
        let mut entree = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if Module32FirstW(instantane, &mut entree).is_ok() {
            loop {
                let fin = entree
                    .szModule
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entree.szModule.len());
                modules.push(Module {
                    base: entree.modBaseAddr as usize,
                    taille: entree.modBaseSize as usize,
                    nom: String::from_utf16_lossy(&entree.szModule[..fin]),
                });
                entree.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
                if Module32NextW(instantane, &mut entree).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(instantane);
    }
    modules
}

/// Formatting buffer on the stack: `write!` without ever touching the heap.
struct Tampon {
    octets: [u8; 16384],
    ecrits: usize,
}

impl Tampon {
    fn neuf() -> Self {
        Self {
            octets: [0; 16384],
            ecrits: 0,
        }
    }
    fn contenu(&self) -> &[u8] {
        &self.octets[..self.ecrits]
    }
}

impl std::fmt::Write for Tampon {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let reste = self.octets.len() - self.ecrits;
        let n = s.len().min(reste);
        self.octets[self.ecrits..self.ecrits + n].copy_from_slice(&s.as_bytes()[..n]);
        self.ecrits += n;
        Ok(())
    }
}

/// Locates an address in the snapshotted table: `module+0xoffset`.
fn situer(tampon: &mut Tampon, adresse: usize) {
    if let Some(modules) = MODULES.get() {
        for m in modules {
            if adresse >= m.base && adresse < m.base + m.taille {
                let _ = write!(tampon, "{}+0x{:x}", m.nom, adresse - m.base);
                return;
            }
        }
    }
    let _ = write!(tampon, "<hors module>");
}

/// Writes a complete report to the dedicated log. No allocation.
unsafe fn consigner(
    etiquette: &str,
    enregistrement: *const EXCEPTION_RECORD,
    contexte: *const CONTEXT,
) {
    let Some(journal) = JOURNAL.get() else {
        return;
    };
    let mut t = Tampon::neuf();
    let code = unsafe { (*enregistrement).ExceptionCode.0 } as u32;
    let adresse = unsafe { (*enregistrement).ExceptionAddress } as usize;
    let fil = unsafe { GetCurrentThreadId() };

    let _ = writeln!(t, "=== exception ({etiquette}) ===");
    let _ = write!(t, "code=0x{code:08x} adresse=0x{adresse:016x} (");
    situer(&mut t, adresse);
    let _ = writeln!(t, ")");
    let _ = writeln!(
        t,
        "fil={} (fil principal={})",
        fil,
        FIL_PRINCIPAL.load(Ordering::Relaxed)
    );

    let nb_params = unsafe { (*enregistrement).NumberParameters } as usize;
    if code == EXCEPTION_ACCESS_VIOLATION.0 as u32 && nb_params >= 2 {
        let genre = unsafe { (*enregistrement).ExceptionInformation[0] };
        let fautive = unsafe { (*enregistrement).ExceptionInformation[1] };
        let libelle = match genre {
            0 => "lecture",
            1 => "écriture",
            8 => "exécution (DEP)",
            _ => "genre inconnu",
        };
        let _ = writeln!(
            t,
            "violation d'accès : {libelle} à l'adresse 0x{fautive:016x}"
        );
    }

    if !contexte.is_null() {
        let c = unsafe { &*contexte };
        let _ = writeln!(
            t,
            "registres : rip=0x{:016x} rsp=0x{:016x} rbp=0x{:016x}",
            c.Rip, c.Rsp, c.Rbp
        );
        let _ = writeln!(
            t,
            "            rax=0x{:016x} rcx=0x{:016x} rdx=0x{:016x} rbx=0x{:016x}",
            c.Rax, c.Rcx, c.Rdx, c.Rbx
        );
        let _ = writeln!(
            t,
            "            r8 =0x{:016x} r9 =0x{:016x} r12=0x{:016x} r13=0x{:016x}",
            c.R8, c.R9, c.R12, c.R13
        );
        let _ = writeln!(
            t,
            "            r14=0x{:016x} r15=0x{:016x} rsi=0x{:016x} rdi=0x{:016x}",
            c.R14, c.R15, c.Rsi, c.Rdi
        );
    }

    let mut cadres: [*mut core::ffi::c_void; 62] = [std::ptr::null_mut(); 62];
    let nombre = unsafe { RtlCaptureStackBackTrace(0, &mut cadres, None) } as usize;
    let _ = writeln!(t, "pile ({nombre} cadres, du plus récent au plus ancien) :");
    for (i, cadre) in cadres.iter().take(nombre).enumerate() {
        let a = *cadre as usize;
        let _ = write!(t, "  #{i:02} 0x{a:016x}  ");
        situer(&mut t, a);
        let _ = writeln!(t);
    }
    let _ = writeln!(t, "=== fin ===");

    // A single `write`, then sync: the process may die right
    // after this handler returns.
    let mut fichier = journal;
    let _ = fichier.write_all(t.contenu());
    let _ = fichier.flush();
    let _ = journal.sync_data();
}

/// True if this handler may record (cap not reached, no
/// reentrancy).
fn autorise() -> bool {
    if RAPPORTS.fetch_add(1, Ordering::SeqCst) >= PLAFOND_RAPPORTS {
        return false;
    }
    DANS_LE_GESTIONNAIRE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn relacher() {
    DANS_LE_GESTIONNAIRE.store(false, Ordering::SeqCst);
}

/// Vectored handler, first chance. Catches NOTHING: it records and
/// gives control back to the normal chain (`EXCEPTION_CONTINUE_SEARCH`), so that the
/// process's behaviour — including WER producing the dump —
/// stays exactly the one from before.
unsafe extern "system" fn filtre_vectorise(infos: *mut EXCEPTION_POINTERS) -> i32 {
    if infos.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    let enregistrement = unsafe { (*infos).ExceptionRecord };
    if enregistrement.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    // Only access violations interest us: a C++ exception or
    // a first-chance breakpoint would fill the log for nothing.
    if unsafe { (*enregistrement).ExceptionCode } != EXCEPTION_ACCESS_VIOLATION {
        return CONTINUER_LA_RECHERCHE;
    }
    if autorise() {
        unsafe { consigner("première chance", enregistrement, (*infos).ContextRecord) };
        relacher();
    }
    CONTINUER_LA_RECHERCHE
}

/// Final filter: only called if nobody handled the exception, hence
/// only for the one that kills the process. Returns
/// `EXCEPTION_CONTINUE_SEARCH` to let WER produce its dump.
unsafe extern "system" fn filtre_final(infos: *const EXCEPTION_POINTERS) -> i32 {
    if infos.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    let enregistrement = unsafe { (*infos).ExceptionRecord };
    if enregistrement.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    // Here, no filter on the code: an unhandled exception is fatal
    // whatever it is, and it is exactly the one we want to see.
    RAPPORTS.store(0, Ordering::SeqCst);
    if autorise() {
        unsafe { consigner("NON GÉRÉE — fatale", enregistrement, (*infos).ContextRecord) };
        relacher();
    }
    CONTINUER_LA_RECHERCHE
}
