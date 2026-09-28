//! The clipboard's Win32 calls, and **nothing else**.
//!
//! This module decides nothing: it reads the sequence number, it reads the text,
//! it writes the text. The whole decision — normalise, denormalise, bound,
//! refuse, compare to the last emitted, arm the guards — lives in the parent,
//! which is pure and tested on the host.
//!
//! ❌ **This module said "it NEVER WRITES the clipboard; the
//! browser → VM direction is sub-block P2". That sub-block took place**, and
//! `ecrire_texte` now lives here. The `diagnostics/presse_papier.rs` probe
//! keeps its own private `mod win`, on purpose: it measures, its phases C and
//! D write the VM's clipboard to exercise it, and the product must not
//! inherit a bench path.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// The clipboard sequence counter of the window station.
///
/// ⚠️ **Zero has TWO causes**, and the poller need not tell them apart: either
/// the call failed (no `WINSTA_ACCESSCLIPBOARD` access), or nothing has
/// ever been copied since the station started. Measured: a freshly
/// started VM returns `0, 0, 0`, then **53** five copies later
/// (probe P0, run no. 1 of August 20th, 2026, log recorded). In both
/// cases the `Sondeur`'s conduct is the right one — it takes `0` as reference at the
/// first turn and announces nothing as long as the counter does not move.
pub fn numero_de_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Ouvre le presse-papier, lit `CF_UNICODETEXT`, referme.
///
/// - `Ok(None)`: the clipboard carries no Unicode text (an image,
///   for example). It is not an error.
/// - `Err`: opening was refused — **a NORMAL case under Windows**, another
///   application holds the clipboard, and not a failure. The caller
///   then does not advance its reference and will retry at the next turn.
///
/// `String::from_utf16_lossy` and not a fallible conversion: a lone
/// surrogate is possible (`CF_UNICODETEXT` is not validated by Windows) and must
/// neither make the read fail nor kill a thread.
pub fn lire_texte() -> Result<Option<String>> {
    unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
    // 🔴 THE GUARD IS BUILT IMMEDIATELY AFTER OPENING, and nothing
    // slips in between: from here on, ALL exit paths close.
    let _garde = PressePapierOuvert;
    unsafe {
        let poignee = match GetClipboardData(CF_UNICODETEXT.0 as u32) {
            Ok(poignee) if !poignee.is_invalid() => poignee,
            // No Unicode text: an image, files. `Ok(None)`, not
            // an error — the poller has nothing to announce and does not advance its
            // reference.
            _ => return Ok(None),
        };
        let global = HGLOBAL(poignee.0);
        let pointeur = GlobalLock(global) as *const u16;
        if pointeur.is_null() {
            anyhow::bail!("GlobalLock a rendu un pointeur nul");
        }
        let mut longueur = 0usize;
        while *pointeur.add(longueur) != 0 {
            longueur += 1;
        }
        // 🔴 THE DATA IS COPIED BEFORE ANY CLOSING: the handle belongs
        // to the clipboard and is no longer valid after `CloseClipboard`.
        let texte = String::from_utf16_lossy(std::slice::from_raw_parts(pointeur, longueur));
        let _ = GlobalUnlock(global);
        Ok(Some(texte))
    }
}

/// Writes `texte` into the VM's clipboard, and returns the sequence number
/// reread **AFTER** closing.
///
/// **No decision here.** The text arrives already normalised, bounded and
/// denormalised (`\r\n`) by the parent: this module merely writes it.
///
/// 🔴 **The number is reread AFTER `CloseClipboard`, and this order is
/// LOAD-BEARING.** Rereading it before closing would return a counter that
/// closing can still move — D5's guard no. 1 would then be off
/// by one, that is, **silently inoperative**: no failure,
/// only a spurious round trip per paste, which nothing would signal.
///
/// - `Err` on an opening refusal: **a NORMAL case under Windows** (another
///   application holds the clipboard — the spec's risk R7), and not a
///   failure. **We NEVER loop waiting**: the caller logs and
///   does not inject, the `V` key being lost rather than postponed (D6).
/// - `EmptyClipboard` **precedes** `SetClipboardData`, otherwise the
///   formats of the previous application would survive in other
///   `CF_*`s and the paste would become unpredictable: an application that
///   prefers `CF_RTF` or `CF_HTML` would paste the old content.
pub fn ecrire_texte(texte: &str) -> Result<u32> {
    // UTF-16 terminated by a `\0`: `CF_UNICODETEXT` requires it, and a
    // non-terminated block would make any pasting application read beyond it.
    let mut unites: Vec<u16> = texte.encode_utf16().collect();
    unites.push(0);

    unsafe { OpenClipboard(None) }.context("OpenClipboard")?;
    // 🔴 THE GUARD IS BUILT IMMEDIATELY AFTER OPENING, as in
    // `lire_texte`: from here on all exit paths close, a
    // panic included. A clipboard left open blocks the WHOLE window
    // station, not just the agent.
    let ecriture = (|| unsafe {
        let _garde = PressePapierOuvert;
        EmptyClipboard().context("EmptyClipboard")?;
        let octets = unites.len() * std::mem::size_of::<u16>();
        let global = GlobalAlloc(GMEM_MOVEABLE, octets).context("GlobalAlloc")?;
        let pointeur = GlobalLock(global) as *mut u16;
        if pointeur.is_null() {
            anyhow::bail!("GlobalLock a rendu un pointeur nul");
        }
        std::ptr::copy_nonoverlapping(unites.as_ptr(), pointeur, unites.len());
        let _ = GlobalUnlock(global);
        // 🔴 The clipboard TAKES OWNERSHIP of the block: do not free it.
        // `GlobalFree` here would leave the station's clipboard pointing at
        // memory returned to the heap — a defect with deferred effect, and global to
        // the Windows session.
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0)))
            .context("SetClipboardData")?;
        Ok(())
    })();
    // The guard ran on leaving the closure above: the
    // clipboard is closed, and only now is the counter
    // stable.
    ecriture?;
    Ok(numero_de_sequence())
}

/// The RAII guard that closes the clipboard — **on ALL exit
/// paths**, including a `?`, an early `return` and a PANIC during
/// stack unwinding.
///
/// **It is not elegance, it is the only correct form here.** A
/// clipboard left open blocks **the whole window station**, not
/// just the agent: no application of the Windows session can
/// copy or paste any more as long as this process lives. The previous form — a
/// single close placed after a work block — did cover the `?` and
/// the `return`, because the block returned a `Result` instead of leaving the
/// function; it did **not** cover the panic, which unwinds the stack without
/// ever reaching the close line. The plan (task 12, step 2)
/// explicitly required all three, and the guard is what gives them in
/// one go.
///
/// The return of `CloseClipboard` is deliberately ignored: there is no
/// recovery conduct — either the clipboard was open and it is closed,
/// or it was not and there was nothing to do — and a `Drop` cannot
/// report anything to the caller anyway.
struct PressePapierOuvert;

impl Drop for PressePapierOuvert {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}
