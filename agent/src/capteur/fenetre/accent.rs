//! The window thread's accent round — sub-block **A1**.
//!
//! **Extracted from `fenetre.rs` and not added into it**, exactly as
//! `transitions.rs` was in sub-block D5: the addition would have taken it to
//! **exactly 500 lines**, that is to ZERO margin, and the repository's rule
//! is "extraction, never compression". ⚠️ **A1's plan had estimated
//! this addition at ~18 lines; it weighs 55**, and it was measurement that
//! said so, not re-reading. `fenetre.rs` has already crossed 500 twice — 508 in
//! D9, 505 in D10 —, and it was saved by an extraction both times.
//!
//! 🔴 **NO HOST TEST COVERS THIS FILE**: it is `#[cfg(windows)]` through
//! its caller and through `accent::win32`. Its only check is **criterion ④**
//! of the acceptance run — exactly ONE `accent de la fenetre Windows` line per
//! `session` over a 60 s plateau.
//!
//! ⚠️ **The read lives HERE, on the WINDOW THREAD, and not on the registry's
//! wheel round**, contrary to what the specification's decision D9
//! prescribed. The reason is mechanical, not aesthetic: **the wheel round
//! does not have the `hwnd`** — none of the fifteen fields of `Etat`
//! (`capteur/sommeil/registre.rs`) carries it, and `inscrire(session, pid)` does not
//! take it. The clipboard lives there because it is **global to the window
//! station**; the accent is **per window**, which is precisely the property
//! D9 claims. The pattern is D8's fullscreen, which stayed in
//! `fenetre.rs` just above the call to this module.

use std::sync::mpsc::SyncSender;
use std::time::Instant;

use super::commandes::deposer;
use super::{AEcrire, Contexte, Fin};
use crate::accent;
use crate::capteur::protocole::DepuisCapteur;
use crate::windows_source::WindowsSource;

/// One accent round: reads the icon if the timer is due, and drops an
/// announcement if — and only if — the tint has CHANGED.
///
/// Returns `Some(Fin::Terminer(motif))` if the drop brought down the media
/// connection, `None` in all other cases — **including when there is nothing to
/// announce, and including when the icon is unreadable.**
///
/// 🔴 **`accent::actif()` IS TESTED FIRST**: `ACCENT=0` must prevent
/// even the `SendMessageTimeout`, not only the sending. That is what
/// `Sondeur::tour` does for the clipboard, and for the same reason — a
/// variable that disarms a mechanism must disarm its **READ**, otherwise
/// it saves nothing and proves nothing.
///
/// ⚠️ **`accent::PERIODE_ACCENT` is a constant SPECIFIC to this mechanism**: do
/// not couple it to `plein_ecran::PERIODE_STYLE`, which bounds a different
/// read for a different reason.
///
/// ⚠️ **An unreadable icon is NOT an error**: `lire_icone` returns `None`
/// on a timeout, a null `HICON` or a failed `GetDIBits`, and
/// `dominante` returns `None` on an entirely grey or empty icon. Both are
/// handled the same way — the round produces no announcement, and it logs
/// nothing either: one trace per round would be twelve lines per minute and per
/// window to say that nothing is happening.
#[cfg(windows)]
pub(super) fn tour(
    suivi: &mut accent::SuiviAccent,
    dernier: &mut Instant,
    hwnd: windows::Win32::Foundation::HWND,
    ecritures: &SyncSender<AEcrire>,
    source: Option<&mut WindowsSource>,
    ctx: &Contexte,
) -> Option<Fin> {
    if !accent::actif() || dernier.elapsed() < accent::PERIODE_ACCENT {
        return None;
    }
    *dernier = Instant::now();

    let (rgba, largeur, hauteur) = accent::win32::lire_icone(hwnd)?;
    let rgb = accent::dominante(&rgba, largeur, hauteur)?;
    let couleur = suivi.observer(&accent::en_hexa(rgb))?;

    // ⚠️ The trace carries `session` BECAUSE ALL CHILDREN HAVE SHARED
    // `agent.log` SINCE D4: a trace without this field there is a number in an
    // anonymous multiset (D6's record no. 2, paid for in the middle of an acceptance run).
    // And it only comes out ON CHANGE: it is on it, and on it alone, that
    // criterion ④ is counted.
    tracing::info!(session = %ctx.session, couleur = %couleur, "accent de la fenetre Windows");

    match deposer(
        AEcrire::Etat(DepuisCapteur::Accent { couleur }),
        ecritures,
        source,
        ctx,
    ) {
        Fin::Terminer(motif) => Some(Fin::Terminer(motif)),
        Fin::Continuer => None,
    }
}
