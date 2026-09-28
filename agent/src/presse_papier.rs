//! The VM's clipboard, **VM → browser** direction: detect that it has
//! changed, read its text, and decide what to announce.
//!
//! **Pure, without any `cfg`** — like `capteur/plein_ecran.rs`,
//! `capteur/audio.rs` and `capteur/repartiteur.rs` before it. The whole
//! decision lives here and is exercised on the Linux host; the two Win32 calls
//! live in `presse_papier/win32.rs`, gated, and **decide nothing**.
//!
//! The module is at the **bare root** (`mod presse_papier;` in `main.rs`) and
//! not under `capteur/`, although its owner is today the capturer
//! and it alone. The reason is not the one the spec puts forward — the "Child
//! module convention" of `CLAUDE.md` declares its own scope and does not cover
//! this case, this module being extracted from nothing. It is that decision D1 states
//! that the owner is "the capturer when it exists, the child otherwise": a
//! module filed under `capteur/` would carry a wrong name the day the
//! single-window owner arrives.
//!
//! ❌ **This module said "it NEVER writes the Windows clipboard; the
//! browser → VM direction is sub-block P2, and it is the one that will carry D5's
//! anti-echo guards; the only guard delivered here is no. 2". ALL THREE CLAUSES
//! ARE OUTDATED: that sub-block took place.** Found by the cross-cutting review of
//! August 21st, 2026. The rule it stated still holds, but differently:
//!
//! - **it still does not write itself** — the Win32 call lives in
//!   `presse_papier/win32.rs`, gated, and `write_platform` is only
//!   the platform switch, twin of `Sondeur::lire_la_plateforme`;
//! - **guards no. 1 and no. 2 are here**, both set by
//!   `Sondeur::apres_notre_ecriture` on OUR OWN write. No. 3 lives
//!   on the page side (`client/src/presse-papier.ts`), the only place from which an echo
//!   could set off again;
//! - **no. 2 still absorbs the counter's false positive** — the counter **moves
//!   on an identical rewrite**, measured (probe P0, `q2` answered "moves", two
//!   runs of August 20th, 2026) — but it now ALSO closes the round trip
//!   of a paste, which P1 could not produce.
//!
//! ⚠️ **And "it closes no loop (there is none)" stays TRUE**, against
//! all expectations: in the shipped architecture, no self-sustained
//! oscillation is possible, each turn requiring a human gesture — the
//! client only emits towards the agent on a `paste`. What the guards remove
//! is **one round trip per paste**, not a divergence. See `gardes_armes`,
//! which carries the demonstration and its consequence on criterion ④'s red.

use std::time::Duration;

/// Maximum size, in UTF-8 bytes **after normalisation**, of a content
/// we agree to push to the browser.
///
/// ⚠️ **NOT CALIBRATED.** It joins `BPP_MIN`, `FACTEUR_FOCUS`,
/// `PART_DORMANTE_BPS`, `HYSTERESIS` and `MAX_OUTPUT_SIZE`: no judgement
/// in use has been made on its value. 64 KiB holds an ordinary text
/// document and refuses a clipboard loaded with a whole file.
///
/// **Beyond it, we REFUSE — we do not truncate.** A silently
/// truncated paste is the worst possible outcome, and it is worse than no paste at
/// all: the user cannot see that the end is missing.
pub const PRESSE_PAPIER_MAX: usize = 64 * 1024;

/// Minimum period between two reads of the sequence counter.
///
/// ⚠️ **It is NOT `PERIODE_REARBITRAGE`**, which paces the wheel turn of the
/// sleep registry. The value is of the same order, deliberately, but the
/// constant is specific to this module: making them follow each other
/// would couple two mechanisms nothing links — it is exactly the argument
/// `plein_ecran::PERIODE_STYLE` already makes for rereading the style.
///
/// `Sondeur::tour` therefore carries its own timer and returns `None` without
/// reading anything as long as it has not elapsed, **even if the wheel turn calls it more
/// often**.
pub const PERIODE_PRESSE_PAPIER: Duration = Duration::from_millis(250);

/// What the poller decided to announce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Annonce {
    /// The text to push, **already normalised and under the bound**.
    Texte(String),
    /// A content of `octets` UTF-8 bytes, **after normalisation**, was
    /// REFUSED — never truncated. The count serves the client-side banner, which
    /// must be able to say *how much* rather than "too large".
    Refus { octets: u32 },
}

/// `PRESSE_PAPIER=0` disarms the whole mechanism.
///
/// **`=0` DISABLES, mere presence does not enable**, exactly like
/// `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR` and `CAPTEUR`: testing `is_ok()`
/// would arm the mechanism by writing `PRESSE_PAPIER=0` to cut it.
///
/// `OnceLock` and not a read per call: polling runs at 4 Hz, and
/// the environment does not change during the process.
pub fn actif() -> bool {
    static ACTIF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACTIF.get_or_init(|| {
        let actif = std::env::var("PRESSE_PAPIER").as_deref() != Ok("0");
        if !actif {
            tracing::warn!(
                "clipboard DISARMED (PRESSE_PAPIER=0): the content copied in the VM \
                 is no longer pushed to the browser"
            );
        }
        actif
    })
}

/// `PRESSE_PAPIER_GARDE=0` disarms the anti-echo guards of `apres_notre_ecriture`.
///
/// ⚠️ **BENCH VARIABLE, NEVER A SHIPPED CONFIGURATION** — same status as
/// `PART_SONDAGE`. It exists for a single use: making REACHABLE the red
/// of P2's criterion ④, which counts the `clipboard` messages coming back to the
/// window after a paste.
///
/// **`=0` DISARMS; mere presence does not arm.** The guards are armed by
/// default, and testing `is_ok()` would disarm them by writing
/// `PRESSE_PAPIER_GARDE=0` to... disarm them. Convention of `PLEIN_ECRAN`,
/// `AUDIO`, `SUPERVISEUR`, `CAPTEUR`, `PART_SONDAGE` and `PRESSE_PAPIER`.
///
/// 🔴 **IT DISARMS BOTH GUARDS, NOT JUST NO. 1, AND THAT IS THE POINT.**
/// The specification prescribed disarming no. 1 and expecting a count that
/// "grows without bound"; **it stays at one, and the spec had foreseen this case**.
/// Without no. 2 armed, the `Sondeur` rereads our text, announces it **once**,
/// then itself sets `last_emitted` and `reference` — at the next turn
/// `observer` exits on its first line. And nothing restarts it: the client
/// only emits towards the agent on a `paste`, hence on a HUMAN GESTURE, never on
/// receiving a `clipboard`. Disarming no. 1 alone would therefore return **zero
/// messages too**, and the red would be vacuous a second time.
///
/// 🔵 **Design consequence, and it contradicts a sentence of D5**: in
/// the shipped architecture, **no self-sustained oscillation is
/// possible**, each turn requiring a human gesture. What the guards
/// remove is **one round trip per paste**, not a divergence.
/// ⚠️ Deduced from the code, not from a measurement: `client/src/presse-papier-dom.ts`
/// only writes locally on receipt and emits nothing. The condition that
/// would make the loop real is named — a client that re-emitted what it
/// receives —, and it is precisely what guard no. 3 prevents on the page side.
pub(super) fn gardes_armes() -> bool {
    static ARMES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ARMES.get_or_init(|| {
        let armes = std::env::var("PRESSE_PAPIER_GARDE").as_deref() != Ok("0");
        if !armes {
            tracing::warn!(
                "clipboard anti-echo guard DISARMED (PRESSE_PAPIER_GARDE=0): \
                 bench arm, never a shipped configuration"
            );
        }
        armes
    })
}

/// Brings all line endings back to `\n`.
///
/// Windows writes `\r\n`; older applications write a **lone** `\r`.
/// Both must become `\n`, otherwise P2's round trip would double
/// lines at each turn. The function is **idempotent**: replaying it on
/// its own result changes nothing.
pub fn normaliser(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut precedent_cr = false;
    for c in texte.chars() {
        match c {
            '\r' => {
                sortie.push('\n');
                precedent_cr = true;
            }
            '\n' => {
                // The `\n` of a `\r\n` has already been emitted by the `\r`.
                if !precedent_cr {
                    sortie.push('\n');
                }
                precedent_cr = false;
            }
            autre => {
                sortie.push(autre);
                precedent_cr = false;
            }
        }
    }
    sortie
}

/// `\n` → `\r\n`, the reciprocal of `normaliser`. **Windows expects `\r\n`.**
///
/// It is NOT a `replace("\n", "\r\n")`: the text arriving from the
/// browser can ALREADY carry `\r\n`s — a copy from a local Windows
/// editor carries them —, and the naive replacement would then return `\r\r\n`,
/// hence one more empty line at each paste. The function is **idempotent**
/// exactly as `normaliser` is in the other direction, and the round trip
/// `normaliser(denormaliser(x)) == x` is what a test must see red
/// first (spec §7.1).
pub fn denormaliser(texte: &str) -> String {
    let mut sortie = String::with_capacity(texte.len() + texte.len() / 16);
    let mut precedent_cr = false;
    for c in texte.chars() {
        match c {
            '\r' => {
                sortie.push_str("\r\n");
                precedent_cr = true;
            }
            '\n' => {
                // The `\n` of a `\r\n` has already been emitted by the `\r`.
                if !precedent_cr {
                    sortie.push_str("\r\n");
                }
                precedent_cr = false;
            }
            autre => {
                sortie.push(autre);
                precedent_cr = false;
            }
        }
    }
    sortie
}

/// Bounds the INCOMING text, in UTF-8 bytes. **We REFUSE, we do not truncate.**
///
/// ⚠️ **It is not the same bound as the outgoing direction's, and the asymmetry
/// is intended.** On the outgoing side, `PRESSE_PAPIER_MAX` protects the **control
/// channel** (D4): the text has not gone through it yet. On the incoming side, the text
/// has **already** crossed that channel when the agent sees it — the bound there protects the
/// capturer↔child pipe and memory, not the channel. It is the client that must
/// apply its own BEFORE emitting; this one is the belt.
///
/// **The incoming refusal is logged and raises no banner**: the client has
/// already refused and said why, and a second banner for the same gesture would be
/// noise.
///
/// The bound bears on `len()`, that is, **bytes**, never on
/// `chars().count()`: it is the channel's unit, and a text of emojis whose
/// character count fits overflows fourfold in bytes.
pub fn borner_entrant(texte: &str) -> Option<String> {
    (texte.len() <= PRESSE_PAPIER_MAX).then(|| texte.to_owned())
}

/// Writes the VM's clipboard, and returns the sequence number reread AFTER
/// closing — the one to pass to `Sondeur::apres_notre_ecriture`.
///
/// **Exact twin of `Sondeur::lire_la_plateforme`**, and placed at the same
/// spot for the same reason: the caller (`capteur/sommeil/presse_papier.rs`)
/// is not gated and must compile on the Linux host.
///
/// ⚠️ **The text must arrive ALREADY denormalised** (`\r\n`): this function
/// decides nothing, it passes through.
#[cfg(windows)]
pub fn write_platform(texte: &str) -> anyhow::Result<u32> {
    win32::write_text(texte)
}

/// Non-Windows fallback. **An `Err`, never an `Ok`**: returning `Ok(0)` would suggest
/// a success, and the caller would inject `Ctrl+V` on an
/// unchanged clipboard — that is, would paste the PREVIOUS content, the
/// silent failure mode D6 exists entirely to avoid.
#[cfg(not(windows))]
pub fn write_platform(_texte: &str) -> anyhow::Result<u32> {
    anyhow::bail!("the VM clipboard does not exist outside Windows")
}

/// `Sondeur`, extracted VERBATIM in sub-block P3 (task 3), BEFORE the addition that
/// made it necessary. An ORDINARY `mod`, and not a `#[path]`: the
/// "Child module convention" of `CLAUDE.md` only targets modules
/// extracted from a `#[cfg(windows)]` parent, and this file is not gated.
mod sondeur;
pub use sondeur::Sondeur;

#[cfg(windows)]
mod win32;

// This module's tests have lived apart since sub-block P2 of the
// clipboard work item: the file was at 428 lines for a ceiling of 500, and P2
// adds D5's guard no. 1, the reciprocal of `normaliser` and their tests.
// The extraction precedes the addition, as the repository's rule requires.
//
// ⚠️ This use of `#[path]` is OUTSIDE the scope of `CLAUDE.md`'s "Child module
// convention": it is the same Rust mechanism used for another
// reason — the 500-line rule —, exactly like `superviseur/table.rs`.
// This module is NOT hoisted to the crate root.
#[cfg(test)]
#[path = "presse_papier/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "presse_papier/tests_entrant.rs"]
mod tests_entrant;
