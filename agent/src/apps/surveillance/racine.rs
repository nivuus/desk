//! ONE watched root: open it, arm a read, complete, reopen.
//!
//! 🔴 `#[cfg(windows)]`, AND **NO HOST TEST IS POSSIBLE** — like
//! `apps/lecture.rs`, which says so of itself. The only available check
//! is `cargo check --target x86_64-pc-windows-gnu`, which covers **types,
//! borrows, visibility and lifetimes**, and **NOT** linking nor
//! behaviour. Everything that DECIDES something therefore lives elsewhere, in the
//! four pure modules of this directory.
//!
//! 🔴 **THE BUFFER'S CONTENT IS NEVER READ, AND IT IS THE CENTRAL
//! SIMPLIFICATION OF THE SUB-BLOCK.** A reconciliation re-reads the WHOLE disk: there is
//! therefore nothing to draw from the name of the file that moved. What this removes from the
//! product:
//!
//! - no UTF-16 string to decode, no `NextEntryOffset` chain to
//!   follow, **no buffer aliasing** — three families of defect that
//!   will not exist;
//! - **no temptation to filter on `.lnk`**, which would in any case be
//!   IMPOSSIBLE TO HOLD: an overflow **throws away the whole buffer**, so the
//!   "I do not know what changed" path must exist no matter what.
//!   Writing a filter that does not cover that case would mean writing two paths to
//!   serve one.
//!
//! ⚠️ **THE COST, NAMED**: any write under the four
//! trees triggers a reconciliation, including a temporary file
//! that has nothing to do with a shortcut. That is exactly what the debounce
//! bounds, and it is also what makes measurable — instead of theoretical — the
//! question "is a root noisy at rest?".

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NOTIFY_ENUM_DIR, ERROR_OPERATION_ABORTED, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Threading::CreateEventW;
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use super::faute::{self, Famille};
use super::TAMPON_NOTIFICATIONS;

/// What a completion tells us.
#[derive(Debug)]
pub(super) enum Issue {
    /// Something moved. **We do not know what, and we do not want to
    /// know** (see the header).
    Notification,
    /// The buffer overflowed: its content is lost.
    ///
    /// ⚠️ **THE EVENT ITSELF IS NOT** — the completion happened, and
    /// it is what will trigger the reconciliation that catches up on everything.
    Debordement,
    /// The wait was cancelled: it is the stop, not a failure.
    Annulee,
    /// The root can no longer be watched as it is.
    Perte(anyhow::Error),
    /// 🔵 **INJECTED, AND ONLY INJECTABLE**: the completion is SWALLOWED —
    /// neither counted, nor logged, nor triggering. No real path
    /// produces it, and that is its whole purpose: it fabricates the only failure the
    /// periodic reconciliation really buys, that of a watch
    /// that **stops delivering WITHOUT AN ERROR**.
    ///
    /// ✅ **MEASURED, AND IT IS THE ONLY DISCRIMINATING SET-UP FOR CRITERION ③**
    /// (two runs per arm): `notifications=0` on both sides — the
    /// completions are indeed swallowed —, and the catalogue moves to `cles=157` through
    /// `declencheur="periode"` when the period is armed, against `cles=156`
    /// **indefinitely** under `APPS_SURVEILLANCE=seule`.
    ///
    /// ⚠️ **IT ESTABLISHES THAT THE REMEDY WORKS, NEVER THAT A CAUSE EXISTS.**
    Avalee,
}

/// An open root, with its read in flight.
///
/// ⚠️ `OVERLAPPED` AND THE BUFFER ARE `Box`es: the kernel writes into them while
/// the call is in flight, and their addresses must therefore stay stable. A
/// by-value field would move with the struct.
pub(super) struct Racine {
    chemin: PathBuf,
    repertoire: HANDLE,
    evenement: HANDLE,
    overlapped: Box<OVERLAPPED>,
    /// 🔴 A `Vec<u32>` AND NOT A `Vec<u8>`: `FILE_NOTIFY_INFORMATION` must
    /// be aligned on a `DWORD` boundary, and it is a **contract of the
    /// call**, not a precaution. A `Vec<u8>` is only aligned on 1.
    ///
    /// ⚠️ We never read this buffer (see the header) — the alignment is
    /// therefore required for what the KERNEL writes into it, not for what we would
    /// do with it.
    tampon: Box<[u32]>,
    /// The number of consecutive failures, for the exponential backoff.
    echecs: u32,
    /// `Some` if the root is failed: the instant of the next attempt.
    reprise: Option<std::time::Instant>,
}

impl Racine {
    /// Opens a root and arms its first read.
    pub(super) fn ouvrir(chemin: PathBuf) -> Result<Self> {
        let (repertoire, evenement) = ouvrir_les_deux_handles(&chemin)?;
        let mut racine = Self {
            chemin,
            repertoire,
            evenement,
            overlapped: Box::new(OVERLAPPED {
                hEvent: evenement,
                ..Default::default()
            }),
            // `TAMPON_NOTIFICATIONS` is in BYTES; the `Vec` is in `u32`.
            tampon: vec![0u32; TAMPON_NOTIFICATIONS / 4].into_boxed_slice(),
            echecs: 0,
            reprise: None,
        };
        racine.armer()?;
        Ok(racine)
    }

    pub(super) fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// The event to pass to `WaitForMultipleObjects`.
    pub(super) fn evenement(&self) -> HANDLE {
        self.evenement
    }

    /// Is this root failed?
    pub(super) fn en_echec(&self) -> bool {
        self.reprise.is_some()
    }

    /// Is the backoff due?
    pub(super) fn reprise_due(&self, maintenant: std::time::Instant) -> bool {
        self.reprise.is_some_and(|due| maintenant >= due)
    }

    /// Arms — or re-arms — a read.
    ///
    /// ⚠️ **`lpBytesReturned` IS `None` HERE, AND IT IS MANDATORY**: on an
    /// overlapped handle, this parameter is *undefined* and reading it would be a
    /// race. The real count is taken at completion, through
    /// `GetOverlappedResult`.
    pub(super) fn armer(&mut self) -> Result<()> {
        let octets = std::mem::size_of_val(&*self.tampon) as u32;
        // SAFETY: FFI call. `repertoire` comes from a successful `CreateFileW`;
        // `tampon` and `overlapped` are `Box`es, hence at a stable address for
        // the whole lifetime of `self`, which the kernel requires of a read
        // in flight.
        unsafe {
            ReadDirectoryChangesW(
                self.repertoire,
                self.tampon.as_mut_ptr().cast(),
                octets,
                // `bWatchSubtree`: the Start menu carries the vast majority
                // of this VM's shortcuts, almost all in
                // per-publisher subfolders. A flat watch would see
                // almost none of them — same reason as the recursive walk of
                // `lecture::lnk_sous`.
                true,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE,
                None,
                Some(&mut *self.overlapped),
                None,
            )
        }
        .with_context(|| format!("ReadDirectoryChangesW sur {}", self.chemin.display()))
    }

    /// Reads what the completion says, **without ever reading the buffer**.
    ///
    /// 🔴 THE INJECTION IS CONSULTED **BEFORE** THE REAL CLASSIFICATION, and in this
    /// order: `Muette` first, since it must short-circuit up to the
    /// counting. The budget is **process-global** (see `faute.rs`), so
    /// a root that reopens does not re-arm it — that is the measurement failure
    /// sub-block D10 paid for on `AUDIO_FAUTE_LECTURE`.
    pub(super) fn completer(&mut self) -> Issue {
        if faute::consommer(Famille::Muette) {
            return Issue::Avalee;
        }
        if faute::consommer(Famille::Debordement) {
            return Issue::Debordement;
        }
        if faute::consommer(Famille::Perte) {
            return Issue::Perte(anyhow::anyhow!("faute injectée (APPS_FAUTE=perte)"));
        }
        let mut octets: u32 = 0;
        // SAFETY: FFI call. `bWait = false`: the event is already signalled
        // when we get here, and waiting would block the thread serving the three
        // other roots.
        let issue =
            unsafe { GetOverlappedResult(self.repertoire, &*self.overlapped, &mut octets, false) };
        match issue {
            // 🔴 ZERO BYTES IS AN OVERFLOW, NOT AN EMPTY COMPLETION. It is
            // how the kernel says "the buffer was not enough, I threw it
            // away" when it does not return `ERROR_NOTIFY_ENUM_DIR`.
            Ok(()) if octets == 0 => Issue::Debordement,
            Ok(()) => Issue::Notification,
            Err(erreur) if erreur.code() == ERROR_NOTIFY_ENUM_DIR.to_hresult() => {
                Issue::Debordement
            }
            // Cancellation is what `CancelIoEx` causes at stop: classifying
            // it as a loss would log a failure at every
            // clean shutdown.
            Err(erreur) if erreur.code() == ERROR_OPERATION_ABORTED.to_hresult() => Issue::Annulee,
            Err(erreur) => Issue::Perte(
                anyhow::Error::new(erreur)
                    .context(format!("GetOverlappedResult sur {}", self.chemin.display())),
            ),
        }
    }

    /// Marks the root as failed and schedules its recovery.
    ///
    /// ⚠️ `delai_de_repli` is **REUSED, NOT COPIED**: it is already tested,
    /// and already protected against shift overflow (`checked_shl`, without
    /// which the thirteenth hour of waiting becomes a `panic` in `debug`).
    pub(super) fn programmer_la_reprise(&mut self, maintenant: std::time::Instant) {
        let delai = crate::plateforme::repli::delai_de_repli(self.echecs);
        self.echecs = self.echecs.saturating_add(1);
        self.reprise = Some(maintenant + std::time::Duration::from_millis(delai));
    }

    /// Closes, **re-resolves the path**, reopens and re-arms.
    ///
    /// ⚠️ THE HANDLES ARE CLOSED FIRST, otherwise each attempt would
    /// leak two — and a failing root is precisely the one that
    /// will retry for a long time.
    pub(super) fn rouvrir(&mut self) -> Result<()> {
        self.fermer();
        let (repertoire, evenement) = ouvrir_les_deux_handles(&self.chemin)?;
        self.repertoire = repertoire;
        self.evenement = evenement;
        *self.overlapped = OVERLAPPED {
            hEvent: evenement,
            ..Default::default()
        };
        self.armer()?;
        self.echecs = 0;
        self.reprise = None;
        Ok(())
    }

    /// Cancels the read in flight and closes both handles.
    ///
    /// 🔴 `CancelIoEx` **BEFORE** `CloseHandle`, and the order is not
    /// indifferent: closing a handle with a read in flight lets the
    /// kernel write into a buffer we are about to free.
    pub(super) fn fermer(&mut self) {
        if !self.repertoire.is_invalid() {
            // SAFETY: FFI calls. Errors are ignored on purpose — there
            // is nothing to do with a cancellation failure during a
            // shutdown, and `ERROR_NOT_FOUND` is the nominal case when no
            // read is in flight.
            unsafe {
                let _ = CancelIoEx(self.repertoire, Some(&*self.overlapped));
                let _ = CloseHandle(self.repertoire);
            }
            self.repertoire = HANDLE::default();
        }
        if !self.evenement.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.evenement);
            }
            self.evenement = HANDLE::default();
        }
    }
}

impl Drop for Racine {
    fn drop(&mut self) {
        self.fermer();
    }
}

/// Opens the directory and its event, or leaves NEITHER of the two open.
///
/// 🔴 IF THE SECOND FAILS, THE FIRST IS CLOSED. Without it, a root whose
/// event cannot be created would leak a directory handle **at every
/// recovery attempt**, that is indefinitely.
fn ouvrir_les_deux_handles(chemin: &Path) -> Result<(HANDLE, HANDLE)> {
    let large: Vec<u16> = chemin
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: FFI call. `FILE_FLAG_BACKUP_SEMANTICS` is MANDATORY to
    // open a DIRECTORY; `FILE_SHARE_DELETE` is in practice, otherwise
    // our handle would prevent anyone from renaming or deleting the root —
    // a watch that gets in the way of what it observes.
    let repertoire = unsafe {
        CreateFileW(
            PCWSTR(large.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OVERLAPPED.0),
            None,
        )
    }
    .with_context(|| format!("ouverture de la racine surveillée {}", chemin.display()))?;

    // `bManualReset = true`: the event is reset to non-signalled by
    // `ReadDirectoryChangesW` itself at the moment it queues the read.
    // An auto-reset event would be consumed by the wait, which
    // is correct too — but manual makes the state observable in
    // between, and that is what we want from a thread serving four roots.
    // SAFETY: FFI call.
    match unsafe { CreateEventW(None, true, false, PCWSTR::null()) } {
        Ok(evenement) => Ok((repertoire, evenement)),
        Err(erreur) => {
            // SAFETY: FFI call, on a handle we have just opened.
            unsafe {
                let _ = CloseHandle(repertoire);
            }
            Err(anyhow::Error::new(erreur).context(format!(
                "CreateEventW pour la racine surveillée {}",
                chemin.display()
            )))
        }
    }
}
