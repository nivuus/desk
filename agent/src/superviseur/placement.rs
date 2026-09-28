//! Pair a freshly created virtual output with a DXGI output, then put
//! the window on it.
//!
//! 🔴 **"NO MAPPING IS EXPOSED" WAS WRONG, AND THIS SENTENCE
//! GOVERNED THE DESIGN OF PAIRING SINCE D1.** It said: "The
//! driver returns a target identifier of its own, DXGI enumerates by
//! `(adapter_index, output_index)`. No mapping is exposed:
//! pairing is therefore done by dimensions and by elimination." The first
//! fact is exact, so is the second, **the conclusion is not** — and it is
//! from it that pairing by set difference came, which batch 30
//! measured refuses any window when the first virtual output
//! replaces a forced target.
//!
//! **What is true**: Win32's CCD API exposes the mapping. The
//! driver returns `(adapterId, target id)` — `sudovda::SortieAjoutee`, three
//! numbers of which the product kept only one —, and this pair turns into a GDI
//! name through `QueryDisplayConfig` then `DisplayConfigGetDeviceInfo`. See
//! `moniteurs_virtuels::config_affichage`, which does it, and
//! `superviseur::designation`, which uses it.
//!
//! **The commands establishing it, so that the next reader redoes the
//! check without believing anyone** — the first shows the three numbers
//! the driver returns, the second that the API exists in the pinned crate:
//!
//! ```text
//! grep -n 'identifiant_cible\|adaptateur_bas' agent/src/moniteurs_virtuels/sudovda.rs
//! grep -rn 'pub unsafe fn QueryDisplayConfig' \
//!   ~/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/Devices/Display/mod.rs
//! ```
//!
//! ⚠️ **What the fix does NOT claim**: that the pair returned by
//! SudoVDA is the one CCD uses. It is a hypothesis, stated as
//! such in `config_affichage`, and its failure makes the product fall back on
//! the pairing by elimination described below — which therefore stays alive, and
//! stays the raison d'être of this whole module.
//!
//! **`GetDesc`/`DesktopCoordinates` is the source of truth, never WMI** —
//! the WMI field was seen 68 s stale on this ground, and the virtual output
//! was announced there as 5120×1440 while DXGI measured 3413×960 (DPI factor of
//! 1.5). A placement computed on the WMI value would be off by as much.

use crate::geometry::Rect;
// `crate::sortie_dxgi`, not `crate::capture`: `capture` is `#![cfg(windows)]`
// as a whole and does not exist at all when compiling on the Linux host
// — see the header comment of `sortie_dxgi.rs`. `capture.rs` re-exports this
// same type as `crate::capture::SortieDxgi` for Windows code.
use crate::sortie_dxgi::SortieDxgi;

/// Position and size tolerance, in pixels, before replacing.
///
/// DWM's invisible borders commonly shift `GetWindowRect` by
/// a few pixels relative to what `SetWindowPos` requested. Without
/// tolerance, the supervisor would replace the window at each loop turn.
///
/// **Four, and not two**: the retained value takes a deliberate margin
/// beyond the usual shift — a four-pixel gap on a full-frame
/// window is invisible, whereas a replacement loop is not. (The
/// comment said "one or two pixels" facing a constant of 4; it is the
/// text that lagged behind, the constant is the one we want.)
///
/// This same tolerance now also serves the pairing of a freshly
/// created output (`sortie_assez_grande`): it must be declared before
/// that function in the file.
const TOLERANCE_PX: i64 = 4;

/// True if an output can serve a given viewport.
///
/// **One inequality, plus one equality, and that is the whole of sub-block D10.** A
/// virtual output is NOT born at the requested size: it is born at the last
/// size left in the registry by an earlier `CDS_UPDATEREGISTRY` (D8,
/// task 3bis — confirmed, reproduced, never explained). On this VM the registry
/// stayed at 3840×2160, and equality within four pixels therefore refused
/// EVERY output: the product capped at three windows, in all six runs
/// of D9's acceptance run ③, without exception.
///
/// The product no longer writes to the registry since D9, but **nothing cleans what
/// is already written there** — and the scope of the blockage (per GUID or global) stays
/// unknown. Hence the choice to tolerate rather than clean: that way the
/// question becomes **moot**, not resolved.
///
/// The `TOLERANCE_PX` tolerance is kept in the SHORTFALL direction, for the
/// attach race recorded by acceptance run D1 (output created at 1280×713,
/// returned at 1280×720 one try out of two).
pub fn sortie_assez_grande(sortie: (u32, u32), demandee: (u32, u32)) -> bool {
    let assez = |s: u32, d: u32| s as i64 + TOLERANCE_PX >= d as i64;
    assez(sortie.0, demandee.0) && assez(sortie.1, demandee.1)
}

/// The size at which the window is put, and which the capture crops.
///
/// `min` axis by axis, **without preserving the aspect ratio**: we crop a
/// texture, we do not scale it. It is the opposite of
/// `windows_source_sortie::borner_a_la_taille_max`, which resizes and must
/// therefore preserve that ratio.
///
/// Even dimensions (the NV12 encoder requires them) and never zero (a collapsed
/// video box emits `(0, 0)`, a real case recorded in D8).
pub fn taille_retenue(demandee: (u32, u32), sortie: (u32, u32)) -> (u32, u32) {
    let retenir = |d: u32, s: u32| (d.min(s).max(2)) & !1;
    (retenir(demandee.0, sortie.0), retenir(demandee.1, sortie.1))
}

/// DXGI output able to serve a viewport, among those not
/// already assigned.
///
/// **`deja_prises` is what prevents the inequality from breaking everything.** With
/// the equality from before D10, two windows with the same viewport already contended for
/// one output; with "at least as large", a single large output
/// would suit ALL windows, and all would show the same image.
/// The filter designates by DXGI NAME (`\\.\DISPLAYn`), stable, and not by a
/// positional pair of enumeration indexes.
///
/// ⚠️ **The caller must look ONLY among the outputs that APPEARED** (see the
/// comment of `creation_sortie::creer_sortie`): the viewport announced by the
/// browser can equal the resolution of a PHYSICAL monitor, and the inequality
/// makes this risk larger, not smaller — a 4K monitor would now suit
/// any viewport.
///
/// 🔴 **`designee` EXEMPTS FROM THE SIZE CRITERION ALONE, AND IT IS INDEED A GUARD
/// BEING LOOSENED — said rather than disguised.** The header of
/// `superviseur::designation` writes that "designation narrows the set
/// of candidates, it loosens no guard": that sentence stops being
/// true here, and here is what refuted it.
///
/// **Measured on the production agent on August 31st, 2026**, eight times in a loop,
/// in the same run: `demande="1614x1080" designee="\\.\DISPLAY6"
/// candidates=["\\.\DISPLAY6 1428x1080"]`. The refused output was
/// **ours, named by CCD** — the SudoVDA driver does not create it at the
/// requested size, and the same request returned 1860×1080 at 12:14Z (served)
/// then 1428×1080 at 20:46Z (refused). Create → refuse → destroy, and **no
/// window showed any more, whatever the application**.
///
/// On a **designated** output, size is therefore no longer a refusal criterion
/// but a CONSTRAINT: `windows_source_sortie::taille_pour_viewport` already
/// fits the window there with the aspect ratio preserved (batch 33). Refusing meant
/// refusing the only output we could serve.
///
/// ⚠️ **WHAT IS NOT LOOSENED, and without which it would be a regression**:
/// `attachee_au_bureau` (an unattached output has nothing to duplicate) and
/// `deja_prises` — it is THAT, and not size, which prevents two windows from
/// showing the same image. The exemption is **by name**: it only applies
/// to the output designation named, never to its neighbours.
///
/// ⚠️ **`None` stays yesterday's product, line for line** — so the fallback by
/// set difference keeps refusing a too-small PHYSICAL screen, and
/// it is the exemption's negative witness.
pub fn sortie_pour_viewport(
    sorties: &[SortieDxgi],
    largeur: u32,
    hauteur: u32,
    deja_prises: &[String],
    designee: Option<&str>,
) -> Option<SortieDxgi> {
    sorties
        .iter()
        .find(|s| {
            let notre = designee == Some(s.nom_sortie.as_str());
            s.attachee_au_bureau
                && (notre || sortie_assez_grande((s.rect.width, s.rect.height), (largeur, hauteur)))
                && !deja_prises.contains(&s.nom_sortie)
        })
        .cloned()
}

/// DWM's **invisible fringe**: how much larger `GetWindowRect` is than
/// the window actually painted.
///
/// 🔴 **MEASURED ON THE PRODUCT, IN SESSION 1, ON AUGUST 31ST, 2026** — it is the
/// residue the owner still saw after the aspect fix
/// ("there is less border, but there still is some"):
///
/// ```text
/// GetWindowRect = 1732x1032+1280+0   <- exactly the RETAINED size
/// DWM frame     = 1718x1025+1287+0
/// fringe: left=7 top=0 right=7 bottom=7
/// ```
///
/// Since Windows 10, a window's resize borders are
/// **transparent**: `GetWindowRect` includes them, the eye does not see them.
/// Since `poser` placed in that space and the crop follows the placed
/// size, the image contained **7 px of desktop on the left, 7 on the right, 7 at the bottom**
/// — constant, **insensitive to the aspect ratio**, and therefore perfectly
/// distinct from the letterbox bands the aspect fix removed.
/// Two superposed defects, two different signatures; it is the contrast
/// "varies with the shape" / "constant" that told them apart.
///
/// ⚠️ **`top = 0` and it is not an error**: the title bar is painted,
/// so the top edge of `GetWindowRect` coincides with the visible frame.
/// The fringe is not symmetric, and assuming it is would shift the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lisere {
    pub gauche: i32,
    pub haut: i32,
    pub droite: i32,
    pub bas: i32,
}

impl Lisere {
    /// The zero fringe — the fallback when DWM refuses to answer, and therefore
    /// **exactly the behaviour from before this fix**.
    #[cfg(test)]
    pub const NUL: Lisere = Lisere {
        gauche: 0,
        haut: 0,
        droite: 0,
        bas: 0,
    };

    /// True if there is nothing to compensate: avoids a second `SetWindowPos` and,
    /// above all, makes the correction inert where it has no reason to be.
    #[cfg(test)]
    pub fn est_nul(self) -> bool {
        self == Lisere::NUL
    }
}

/// The total envelope to add around the crop: DWM's **invisible** fringe,
/// **plus** the border Windows **PAINTS** around the window.
///
/// 🔴 **THE SECOND HALF WAS FOUND BY LOOKING AT THE IMAGE, which this batch
/// had never done.** Capture of the virtual output in session 1, on
/// August 31st, 2026, and colour survey on the edges of the crop
/// (1548×1032):
///
/// ```text
/// row 0       (TOP edge)    : #494949    | row 1       (neighbour) : #F3F3F3
/// row 1031    (BOTTOM edge) : #2F2F2F    | row 1030    (neighbour) : #F0F0F0
/// column 0    (LEFT edge)   : #2F2F2F    | column 1    (neighbour) : #FFFFFF
/// column 1547 (RIGHT edge)  : #2F2F2F    | column 1546 (neighbour) : #F0F0F0
/// ```
///
/// **Exactly ONE dark pixel on all four edges, and the immediate neighbour
/// is light.** `#2F2F2F` is Windows's window border in the dark
/// theme. By making the crop coincide with
/// `DWMWA_EXTENDED_FRAME_BOUNDS` **to the pixel**, the previous fix
/// framed right on it: it is not a calculation error, it is **the
/// definition of the rectangle we had chosen as target**.
///
/// 🔴 **AND THIS MEASUREMENT ALSO ELIMINATES THE BROWSER CACHE LEAD**: the
/// pixels are read **in the VM**, without any browser. The border
/// is IN the image, whatever the page displays.
///
/// ⚠️ **`bordure` IS NOT A NUMBER WRITTEN HERE**: it comes from
/// `GetSystemMetrics(SM_CXBORDER/SM_CYBORDER)`, a documented metric that
/// **follows the DPI** — read as `1` for a system DPI of `96` on this
/// machine. Hard-coding `1` would be the #487 wreck all over again.
pub fn enveloppe(dwm: Lisere, bordure: (i32, i32)) -> Lisere {
    Lisere {
        gauche: dwm.gauche + bordure.0,
        haut: dwm.haut + bordure.1,
        droite: dwm.droite + bordure.0,
        bas: dwm.bas + bordure.1,
    }
}

/// The **crop** rectangle corresponding to a given visible frame:
/// the frame, **stripped of the painted border**.
///
/// 🔴 **THE EXACT COUNTERPART OF `enveloppe`, AND IT MUST STAY SO.** `poser` puts
/// the window 1 px wider than the crop; if `rectangle_de` returned
/// the **raw** visible frame, the periodic check would compare `crop + 1` to
/// `crop` and see a permanent gap. It would fall under `TOLERANCE_PX`
/// today — but relying on that would rest a property
/// on a tolerance made for something else (DWM's roundings). The two
/// functions answer each other, and the round-trip test holds them together.
pub fn sans_la_bordure(cadre: &Rect, bordure: (i32, i32)) -> Rect {
    Rect {
        x: cadre.x + bordure.0,
        y: cadre.y + bordure.1,
        width: cadre.width.saturating_add_signed(-2 * bordure.0),
        height: cadre.height.saturating_add_signed(-2 * bordure.1),
    }
}

/// The rectangle to pass to `SetWindowPos` so that the **VISIBLE** frame occupies
/// exactly `cible`.
///
/// 🔴 **THE WHOLE FIX FITS IN THESE FOUR ADDITIONS**, and its difficulty
/// is not arithmetic: it is that the repository reasoned end to end
/// in the space of `GetWindowRect` — `poser` wrote there, `rectangle_de`
/// reread there, `doit_etre_replacee` compared there — without anything saying that this
/// space **is not the one we see**.
///
/// ⚠️ **`rectangle_de` now returns the VISIBLE frame**, precisely so that
/// the periodic comparison happens in the same space as the target. Changing
/// them separately would replace the window **every second**: the
/// check would see a permanent 7 px gap and never manage to
/// absorb it. The two halves go together or not at all.
pub fn rect_a_poser(cible: &Rect, lisere: Lisere) -> Rect {
    Rect {
        x: cible.x - lisere.gauche,
        y: cible.y - lisere.haut,
        // `saturating_add_signed`: an aberrant fringe must not make
        // the width overflow, which would give a tiny window.
        width: cible
            .width
            .saturating_add_signed(lisere.gauche + lisere.droite),
        height: cible.height.saturating_add_signed(lisere.haut + lisere.bas),
    }
}

/// The size to pass to `SetWindowPos` so that the **VISIBLE** frame measures
/// `taille`. The counterpart of [`rect_a_poser`] for the CAPTURER path, which
/// resizes without moving (`SWP_NOMOVE`) — the origin being already compensated by
/// the supervisor's placement, only the size remains to correct.
pub fn taille_a_poser(taille: (u32, u32), lisere: Lisere) -> (u32, u32) {
    (
        taille
            .0
            .saturating_add_signed(lisere.gauche + lisere.droite),
        taille.1.saturating_add_signed(lisere.haut + lisere.bas),
    )
}

/// True if the window left its output or changed size to the point that
/// it must be put back in place.
///
/// It is the case spec §6 foresees: an application can move or
/// resize itself, and a window overflowing its output gives a
/// truncated capture without anything signalling it.
pub fn doit_etre_replacee(actuel: &Rect, cible: &Rect) -> bool {
    let ecart = |a: i64, b: i64| (a - b).abs() > TOLERANCE_PX;
    ecart(actuel.x as i64, cible.x as i64)
        || ecart(actuel.y as i64, cible.y as i64)
        || ecart(actuel.width as i64, cible.width as i64)
        || ecart(actuel.height as i64, cible.height as i64)
}

#[cfg(windows)]
mod win {
    use anyhow::{Context, Result};

    use super::*;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOP, SWP_NOACTIVATE, SW_SHOWNORMAL,
    };

    /// Puts the window on the output and gives it exactly its size.
    ///
    /// **No maximising** — but ❌ **THE REASON WRITTEN HERE BECAME
    /// WRONG, AND BATCH 33 CORRECTS IT.** It said: "`SW_MAXIMIZE` would make
    /// the window adopt the monitor's work area, taskbar
    /// deducted: the captured image would then not fill the output, and the bottom
    /// of the stream would be a band of empty desktop. We set the exact size of the
    /// output."
    ///
    /// **That argument only holds if the crop stays at the output's
    /// size**, which is no longer the case: since batch 33, the caller bounds
    /// the target by the WORK AREA (`placement_periodique::borne_de`) and
    /// the capturer crops on that same bound
    /// (`windows_source_sortie::borne_de_la_sortie`). There is therefore no more
    /// "band of empty desktop" to fear: the 48 px band that
    /// maximising would have left is precisely the one now REMOVED
    /// from the crop, because it contained the taskbar.
    ///
    /// **What stays true, and why there is still no
    /// `SW_MAXIMIZE`**: a maximised window silently ignores
    /// `SetWindowPos` (see the next paragraph), so viewport tracking
    /// — which resizes the window at each `Resize` — could no longer set
    /// anything. We set the exact size, and that is what makes tracking
    /// possible.
    ///
    /// **The commands establishing the measurement**, so that the next
    /// reader believes no one — they must be played IN SESSION 1, through
    /// an `/it` scheduled task (a WinRM survey is in session 0, where
    /// `EnumWindows` returns nothing):
    ///
    /// ```text
    /// GetMonitorInfo(\\.\DISPLAY8) -> mon=1428x1080  work=1428x1032
    /// EnumWindows: Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080) 1428x48
    /// ```
    ///
    /// Survey of August 31st, 2026:
    /// `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`.
    ///
    /// The window is first restored: a minimised or already
    /// maximised window silently ignores `SetWindowPos`.
    pub fn poser(hwnd: HWND, cible: &Rect) -> Result<()> {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            // The fringe is reread AFTER `ShowWindow`: on a minimised
            // window, DWM returns a meaningless frame. A DWM
            // failure returns `Lisere::NUL`, hence the previous behaviour.
            // The TOTAL envelope: DWM's invisible fringe, plus the
            // border Windows paints (measured at 1 px on all four edges
            // of the crop — see `enveloppe`). The window is therefore put
            // slightly WIDER than the crop, and its border falls
            // outside the image.
            let enveloppe_totale = enveloppe(
                crate::window::lisere_dwm(hwnd).unwrap_or_default(),
                crate::window::bordure_peinte(),
            );
            let pose = rect_a_poser(cible, enveloppe_totale);
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                pose.x,
                pose.y,
                pose.width as i32,
                pose.height as i32,
                // `SWP_NOACTIVATE`: putting a window must not steal the
                // foreground from the one the user is handling.
                SWP_NOACTIVATE,
            )
            .context("SetWindowPos vers la sortie virtuelle")?;
        }
        Ok(())
    }

    /// The window's **VISIBLE** rectangle, in virtual desktop coordinates.
    ///
    /// ❌ **THIS FUNCTION RETURNED `GetWindowRect`, AND IT WAS THE READ HALF
    /// OF THE FRINGE DEFECT.** Its comment said "`GetWindowRect`
    /// and not `GetClientRect`: it is the position in desktop space
    /// that we compare with the output's" — true, but incomplete: since
    /// Windows 10, `GetWindowRect` includes **transparent** borders of
    /// ~7 px, so that the space where we compared was **not the one we
    /// see** (measurement in session 1, see [`Lisere`]).
    ///
    /// 🔴 **IT MUST CHANGE AT THE SAME TIME AS `poser`, NEVER SEPARATELY.**
    /// `poser` now writes a rectangle inflated by the fringe; if the reread
    /// still returned `GetWindowRect`, `doit_etre_replacee` would see a
    /// permanent 7 px gap and would replace the window **every second**, without
    /// ever converging.
    ///
    /// The fallback to `GetWindowRect` when DWM refuses is **exactly the
    /// previous behaviour**, and it is consistent with `poser`'s, which
    /// then falls back on a zero fringe: both halves degrade together.
    pub fn rectangle_de(hwnd: HWND) -> Result<Rect> {
        let brut = crate::window::rectangle_brut(hwnd)?;
        Ok(match crate::window::cadre_visible(hwnd) {
            // The visible frame INCLUDES the painted border; the crop, for its part,
            // stops just inside it. We therefore return what the target is
            // comparable to — see `sans_la_bordure`.
            Ok(visible) => sans_la_bordure(&visible, crate::window::bordure_peinte()),
            Err(_) => brut,
        })
    }
}

#[cfg(windows)]
pub use win::{poser, rectangle_de};

// This module's host tests live in `placement/tests.rs` — batch 33's
// extraction, which brought this file from EXACTLY 500 lines (its gate) back to its
// margin. `#[path]` rather than a module subdirectory: precedent of
// `superviseur/table.rs`. The extracted file's header says what it must say.
#[cfg(test)]
#[path = "placement/tests.rs"]
mod tests;

// DWM's fringe and the painted border have their own cases, moved out of
// `placement/tests.rs` on August 31st, 2026: that file was at 485 lines,
// hence at its gate, and the DESIGNATED output fix had to write in it.
// Extraction PLAYED BEFORE the addition it prepared, and in its own
// task — the strong form this repository imposes on itself after missing it six times.
#[cfg(test)]
#[path = "placement/tests_lisere.rs"]
mod tests_lisere;
