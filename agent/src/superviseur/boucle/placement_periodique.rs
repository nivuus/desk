//! Placement check: puts a window back on its DXGI output if it has
//! left it.
//!
//! Extracted from `boucle.rs` (task 7 of sub-block D3) to stay under the
//! project's 500-line ceiling — not for a design reason: these
//! two functions are part of the loop like the others, in the same
//! logical module, just in a neighbouring file. Same scheme as
//! `superviseur/table/attribution.rs`.

use super::*;
use crate::geometry::Rect;

/// An output's bound: its WORK AREA when Windows gives it, its
/// rectangle otherwise.
///
/// 🔴 **IT IS THE SUPERVISOR HALF OF THE AGREEMENT BETWEEN THE TWO PROCESSES.** The
/// capturer queries the same monitor through ITS WINDOW
/// (`window::zones_du_moniteur_de`), the supervisor through the output's ORIGIN
/// — it knows the DXGI rectangle even before having put the window. Same
/// `HMONITOR`, hence same bound, hence same `size_for_viewport` on both
/// sides: no message to exchange, and no battle at 1 Hz.
///
/// ⚠️ **The supervisor does NOT bound by the duplication's texture**, unlike
/// the capturer: it opens no duplication, and `SortieDxgi.rect`
/// is already in desktop coordinates like `rcWork`. On this machine the
/// texture is LARGER than the rectangle (1860 versus 1428, batch 32T): the
/// capturer's `min` is therefore inert here, and the two bounds coincide. **If
/// they diverged**, the supervisor would win at the next turn — a
/// disagreement bounded to one second, never an oscillation.
pub(super) fn borne_de(sortie: &SortieDxgi) -> (u32, u32) {
    let moniteur = (sortie.rect.width, sortie.rect.height);
    match crate::window::zones_du_moniteur_au_point(sortie.rect.x, sortie.rect.y) {
        Ok((_, travail)) => crate::windows_source_sortie::borne_de_la_sortie(
            moniteur,
            Some((travail.width, travail.height)),
        ),
        Err(error) => {
            tracing::warn!(%error, nom = %sortie.nom_sortie, "unreadable work area: the whole output serves as the bound");
            moniteur
        }
    }
}

/// Makes placement follow the viewport the browser has just announced,
/// on an ALREADY live session.
///
/// 🔴 **WHAT THIS FUNCTION DOES, AND WHAT IT DOES NOT.** It corrects
/// the RETAINED size in the table and puts the window back — hence the
/// SUPERVISOR half of the remedy. It touches neither the crop nor the encoder, which
/// live in the capturer: it is the control channel's `Resize` that makes them
/// follow (`WindowsSource::suivre_le_viewport`), from the same measurement of the
/// same `ResizeObserver`, by the same pure rule.
///
/// ⚠️ **If the `Resize` were lost and only the `viewport` arrived**, the
/// window would change size without the crop following: the image
/// would show desktop, until the next `Resize`. It is not caught up here,
/// and it is said rather than assumed impossible.
pub(super) fn suivre_le_viewport(
    table: &mut Table,
    session: &IdSession,
    largeur: u32,
    hauteur: u32,
) {
    // 🔴 **UNCONDITIONAL TRACE, AND IT IS THE POINT OF THIS SECOND SEND.**
    //
    // The first draft only logged the case where the size changes:
    // the four other paths — no retained output, output gone from the
    // topology, unchanged size — exited silently. A `0` in the log
    // was therefore indistinguishable between "no viewport arrives" and "it
    // arrives and changes nothing", that is, between a defect and the batch's
    // DECLARED LIMIT. **A trace that can only come out on success cannot
    // diagnose a failure** — and this one replaced the
    // "resize ignored" line, which came out at EACH request and is
    // what made batch 33's diagnosis possible.
    //
    // The `decision` field names the branch taken. Volume: the client's
    // `ResizeObserver` is smoothed to 200 ms and only beats during a
    // gesture, hence a few lines per resize — never a per-packet
    // trace in a loop, which `CLAUDE.md` forbids.
    let precedente = table.output_size_of(session);
    let nom = table.nom_sortie_de(session).map(str::to_owned);
    let all = enumerer_sorties_silencieux().unwrap_or_default();
    let sortie = nom
        .as_deref()
        .and_then(|n| all.iter().find(|s| s.nom_sortie == n).cloned());
    let borne = sortie.as_ref().map(borne_de);
    let retenue =
        borne.map(|b| crate::windows_source_sortie::size_for_viewport((largeur, hauteur), b));

    let decision = match (&nom, &sortie, retenue) {
        (None, _, _) => "NO output retained for this session",
        (Some(_), None, _) => "output ABSENT from the DXGI topology",
        (Some(_), Some(_), Some(r)) if Some(r) == precedente => {
            "size UNCHANGED: nothing to place again (viewport saturates the bound, or gesture without effect)"
        }
        _ => "size CHANGED: the table is corrected and the window placed again",
    };
    tracing::info!(
        session = %session.0,
        demande = format!("{largeur}x{hauteur}"),
        nom_sortie = nom.as_deref().unwrap_or(""),
        borne = borne.map(|b| format!("{}x{}", b.0, b.1)).unwrap_or_default(),
        retenue = retenue.map(|r| format!("{}x{}", r.0, r.1)).unwrap_or_default(),
        precedente = precedente.map(|p| format!("{}x{}", p.0, p.1)).unwrap_or_default(),
        decision,
        "viewport received by the supervisor"
    );

    let (Some(_), Some(retenue)) = (sortie, retenue) else {
        return;
    };
    if Some(retenue) == precedente {
        return;
    }
    table.refresh_output_size(session, retenue);
    replacer_si_besoin(table, session, &all);
}

/// Puts back on its output any window that has left it.
///
/// **`&Table`, and not `&mut Table`.** Until sub-block D10, this function
/// also refreshed `output_size` from the output's raw DXGI size
/// (legacy of IMPORTANT 5, review of D8's task 9, which kept this
/// field up to date with a mode change made outside this table by
/// `WindowsSource::changer_mode_de_sortie`). That path was removed in
/// sub-block D9; the refresh, for its part, had survived as a precaution,
/// although it could already no longer cause any drift.
///
/// **D10 makes it downright WRONG, and that is why it disappeared rather than
/// being kept.** Since `sortie_pour_viewport` (an output can be
/// much larger than the viewport, polluted registry oblige),
/// `Table::output_size_of` carries the RETAINED size — the one at which the
/// window is put and which the capture crops —, which no longer has any reason
/// to equal the DXGI output's `GetDesc`/`DesktopCoordinates`. Refreshing
/// from the latter would therefore have overwritten the retained size with the output's
/// FULL size at each turn — putting the window back large a
/// second after `create_output` put it at its cropped size. The table
/// is now the only source of truth for this size, set once at
/// creation (task 6) and at reuse (task 7): this periodic check
/// rereads it, it no longer recomputes it.
pub(super) fn controler_le_placement(table: &Table) {
    let all = enumerer_sorties_silencieux().unwrap_or_default();
    for session in table.sessions_vivantes() {
        replacer_si_besoin(table, &session, &all);
    }
}

/// Puts a window back on its output if it has left it.
///
/// Called by the periodic check, **and by the `LancerEnfant` arm**: on
/// the path reusing a retained output (§7.1 of sub-block D3),
/// `create_output` is not called, hence neither is `placement::poser`. Between
/// the child's death and its restart, the application may have moved or resized
/// its window; without this call, the child would capture a badly placed window
/// until the next periodic check — up to `PERIODE_PLACEMENT`
/// later.
///
/// Idempotent: `doit_etre_replacee` guards the call, so the creation
/// path — where the window has just been put — **normally** emits no
/// second `SetWindowPos`. "Normally" and not "never": if Windows
/// clamped the requested size (the window's minimum size, DPI constraint),
/// the obtained rectangle differs from the target, `doit_etre_replacee` is true, and a
/// second `SetWindowPos` **is** emitted — with no more effect than the first.
///
/// **The position comes from the DXGI output, the size from the table** (sub-block
/// D10): `sortie.rect` gives the origin in the virtual desktop, but
/// `Table::output_size_of` gives the RETAINED size — the one, possibly
/// much smaller than the output, at which the window was put and which the
/// capture crops. The **THIRD** `let Some` — the one of
/// `Table::output_size_of` — can, in practice, never fail once
/// the FIRST has passed (`nom_sortie_de`): `sortie_creee` sets `nom_sortie` and
/// `output_size` together, never one without the other (`table.rs`) — it is
/// therefore not `Etat::Vivante` that governs here, but that invariant. Kept
/// as is rather than assumed, so as to owe nothing to a neighbouring file.
///
/// ❌ **This sentence said "the second `let Some`", and it designated the
/// THIRD** (finding of the review of task 6, deferred then taken up at the
/// branch's final review). ⚠️ **The error was not only one of counting**:
/// the real second — the DXGI search by name, `all.iter().find(...)` —
/// **CAN** fail after the first, an output possibly having disappeared from the
/// topology between two turns. A reader counting the `let Some`s
/// therefore attributed the "can never fail" clause to the **wrong guard**,
/// the one for which it is false.
pub(super) fn replacer_si_besoin(table: &Table, session: &IdSession, all: &[SortieDxgi]) {
    let Some(nom) = table.nom_sortie_de(session) else {
        return;
    };
    let Some(sortie) = all.iter().find(|s| s.nom_sortie == nom) else {
        return;
    };
    let Some((largeur, hauteur)) = table.output_size_of(session) else {
        return;
    };
    let cible = Rect {
        x: sortie.rect.x,
        y: sortie.rect.y,
        width: largeur,
        height: hauteur,
    };
    let Some(fenetre) = table.fenetre_de(session) else {
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(fenetre.0 as *mut core::ffi::c_void);
    let Ok(actuel) = placement::rectangle_de(hwnd) else {
        return;
    };
    if placement::doit_etre_replacee(&actuel, &cible) {
        // 🔴 `hwnd` AND `fenetre_vivante`: without them, the hypothesis "the
        // table's handle is STALE" is UNDECIDABLE, and batch 32O
        // paid for it — twenty minutes after the burst, two `hwnd`s read from the
        // log returned `IsWindow=false`, which proved NOTHING: those
        // windows had simply closed since.
        //
        // The phenomenon is **intermittent and tied to a session**: without a
        // trace carrying the answer AT THE INSTANT of the replacement, one would have
        // to watch the log live to hope to measure it. **A failure
        // that cannot be diagnosed costs more than one more trace.**
        //
        // ⚠️ `fenetre_vivante = false` **would explain IN ONE GO** the two
        // open facts: a `SetWindowPos` that "succeeds" without moving
        // anything, and a minimised window rectangle read on an object that
        // no longer exists. `true` would leave them both whole.
        let fenetre_vivante =
            unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)) }.as_bool();
        tracing::info!(
            session = %session.0,
            hwnd = format!("{:#x}", fenetre.0),
            fenetre_vivante,
            de = format!("{}x{}+{}+{}", actuel.width, actuel.height, actuel.x, actuel.y),
            vers = format!("{}x{}+{}+{}", cible.width, cible.height, cible.x, cible.y),
            "window left its output, placing it again"
        );
        if let Err(error) = placement::poser(hwnd, &cible) {
            tracing::warn!(session = %session.0, %error, "placing again failed");
        }
    }
}
