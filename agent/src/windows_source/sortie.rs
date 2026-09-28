//! Building a `WindowsSource` on a DXGI output, and the mode discriminant
//! that distinguishes this case from the other.
//!
//! It is sub-block D1's mode: one window per virtual output.
//!
//! ❌ **This paragraph said "so nothing left to crop — the output *is* the
//! window", and sub-block D10 refuted it** (tasks 4 to 9). A virtual
//! output **is not born at the requested size** but at the last size
//! left in the registry: it can therefore be LARGER than the window. The
//! supervisor now accepts it instead of handing it back to the driver, places the
//! window on it at the **retained size**, and `sur_sortie` below **crops that
//! rectangle in the output's duplication**. So there is indeed still a
//! crop, and the output is the window only in the — not guaranteed — case
//! where it is born at the requested size.
//!
//! ❌ **The sentence that followed — "What has not changed: `resize` still does not
//! resize the window in this mode" — has been WRONG since batch 33.**
//! `resize` now resizes the window and redoes the crop to follow
//! the viewport, **without ever touching the output's display mode**:
//! see `ModeCapture::suit_le_viewport` and `size_for_viewport` below.
//!
//! Two halves, separated by a `#[cfg(windows)]` in the middle of the file:
//! above, the pure region computation and `ModeCapture`, both testable on
//! the Linux host; below, the real constructor, which handles COM types.

use crate::geometry::Rect;

/// What the source captures — hence what a resize requested by the
/// browser must do.
///
/// **The discriminant was missing, and its absence was a safety defect.**
/// `WindowsSource::new` and `WindowsSource::sur_sortie` both converged
/// on the same assembly, retaining nothing of their difference: `resize`
/// therefore applied the "window crop" path even to a source
/// that captures a whole output. On that path, the window was
/// shrunk (it left its virtual output, which the supervisor's periodic check
/// immediately tried to catch up with), the output's duplication
/// was released, and `DesktopCapture::new()` duplicated **the primary physical
/// desktop** — after which the backup fallback installed that duplication
/// with a region computed for the virtual output. The browser window
/// then displayed a corner of the VM's real desktop instead of its
/// application, for a single `warn!`. In single-window mode this fallback was
/// acceptable; in multi-window it leaks one monitor's content into
/// someone else's session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCapture {
    /// The whole desktop is captured, then cropped to the window. The window
    /// is resizable and the crop region follows its size: it is
    /// the historical single-window mode.
    FenetreRecadree,
    /// A DXGI output is captured, with no Windows window to follow: the
    /// window occupies its own virtual output. Sub-block D1's mode.
    ///
    /// ❌ **This variant's name, and the text it carried — "the
    /// output **is** the window" —, no longer describe the behaviour
    /// since sub-block D10.** An output can be born LARGER than the
    /// requested size (polluted registry), in which case `sur_sortie` crops the
    /// **retained size** at the output's origin. The name is kept
    /// rather than renamed: what it really discriminates — and the only
    /// thing `recapture_le_bureau` below depends on — is **the absence
    /// of a physical desktop to recapture**, which stays true.
    ///
    /// ❌ **This text said "the absence of a Windows window to resize", and
    /// batch 33 refuted it**: there is indeed a Windows window, and it is
    /// now resized to follow the viewport. What this mode really
    /// discriminates, and always has, is that we capture **a DXGI
    /// output** and not the desktop — hence the predicate's renaming.
    SortieEntiere,
}

impl ModeCapture {
    /// True if a resize must **release the current duplication and
    /// recapture the desktop** — the historical single-window path.
    ///
    /// ❌ **THIS METHOD WAS CALLED `redimensionne_la_fenetre`, AND THAT NAME
    /// BECAME A LIE IN BATCH 33.** Since that batch, `SortieEntiere` does
    /// resize the Windows window (see `suit_le_viewport` below):
    /// a predicate named "resizes the window" that returns `false` for a
    /// mode that resizes it is exactly the pattern `CLAUDE.md`
    /// names "the 487 wreck". The name now says what the predicate
    /// REALLY discriminates, and always has: **releasing
    /// the duplication to open one of the physical desktop**.
    ///
    /// False in `SortieEntiere`, and it is D1's fix C1: on that
    /// path, releasing the virtual output's duplication for
    /// `DesktopCapture::new()` broadcast **the corner of the VM's physical
    /// desktop** into the browser window — in multi-window, a leak
    /// of one monitor's content into someone else's session.
    #[cfg(test)]
    pub fn recapture_le_bureau(self) -> bool {
        matches!(self, ModeCapture::FenetreRecadree)
    }

    /// True if a resize must **follow the viewport inside
    /// an output that does not move**: resize the window, redo the
    /// crop and the encoder, **without ever touching the duplication**.
    ///
    /// 🔴 **THIS ARM IS NEW IN BATCH 33, AND IT REPLACES AN `Ok(())` THAT
    /// DID NOTHING.** Measured on the product in production on August 31st, 2026
    /// (`C:\nivuus\agent.log`, 34 requests): the browser requested
    /// aspect ratios ranging from **1.105 to 3.559** — including a `5118x1438` —
    /// while the window stayed served at **1428×1080, that is 1.3222**, and
    /// **all 34 were dropped**. The client's `#remote` being
    /// `width:100vw; height:100vh; object-fit:contain;
    /// background:var(--video-letterbox)` (`client/src/style.css`), the gap is
    /// literally painted `#000` on each side of the image: those are the
    /// "black borders" reported by the owner.
    ///
    /// ⚠️ **IT IS NOT THE RESURRECTION OF THE PATH D9 REMOVED.** D8
    /// made **the OUTPUT** follow the viewport through `ChangeDisplaySettingsExW`
    /// ; D9 measured it and removed it (the change does not survive the opening of
    /// the next window, and `CDS_UPDATEREGISTRY` pollutes the registry to the
    /// point of blocking the product — finding at the head of
    /// `capteur/plein_ecran.rs`). **Nothing here changes display mode**:
    /// the output keeps the size it was born with, and only **the
    /// crop** and **the Windows window** move inside it.
    ///
    /// ⚠️ **What this arm CANNOT DO**: grow beyond the output.
    /// `size_for_viewport` bounds by an axis-by-axis `min`; a viewport
    /// wider than the output (the measured `5118x1438`) stays served at the
    /// output's size, and the black bars remain. Without a mode
    /// change — which D9 forbids —, there is no other way out.
    pub fn suit_le_viewport(self) -> bool {
        matches!(self, ModeCapture::SortieEntiere)
    }
}

/// The bound a served window, and the crop that follows it,
/// must keep to: **the output's WORK AREA, not its rectangle**.
///
/// 🔴 **NOTHING, IN THE WHOLE REPOSITORY, CONSULTED THE WORK AREA BEFORE THIS
/// BATCH** — a case-insensitive grep for `rcWork`, `SPI_GETWORKAREA` or a work-area name over
/// `agent/ client/ plateforme/ proto/` returned **zero**. The
/// monitor / work area distinction did not exist in this product, and it is the
/// cause of a defect measured on August 31st, 2026 in session 1 (see
/// `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`):
///
/// ```text
/// device=\\.\DISPLAY8 mon=(1280,0)-(2708,1080) 1428x1080 work=1428x1032
/// hwnd=0x201DC cls=Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080) 1428x48
/// hwnd=0x60242 cls=Notepad                rect=(1280,0)-(2708,1080) 1428x1080
/// ```
///
/// **A 48 px SECONDARY taskbar is stuck at the bottom of EACH of the
/// served outputs** (the Windows default, `MMTaskbarEnabled` absent), and the
/// window is placed at the MONITOR's rectangle, not at its work area. Three
/// consequences, two of which the symptom did not tell:
///   ① the taskbar is **in the crop** — 48 of the 1080 rows, 4.4% of
///      the image;
///   ② the application **loses 48 px of content**: its window is indeed 1080, and
///      its last rows are **covered** — it is not a band
///      added below it;
///   ③ **a click in the bottom 4.4% of the video reaches the taskbar**,
///      not the application (`input.rs::move_mouse` unmaps on the image
///      size, so `y = 65535` falls in the taskbar).
///
/// ⚠️ **THIS BOUNDING MUST NOT BE SHIPPED ALONE.** Taken in isolation, cropping to
/// the work area **adds** a 48 px letterbox band below the image
/// — that is, precisely what the owner complains about. It is only worth
/// anything together with viewport following (`size_for_viewport` below), which
/// makes the image match the requested aspect ratio.
///
/// `travail` is an `Option` because `GetMonitorInfoW` can fail: the
/// fallback is **the monitor's rectangle, that is, the exact behaviour
/// from before this batch**. A fallback returning anything else would make
/// framing depend on a call that fails silently.
///
/// A **degenerate** work area (zero on one axis) is refused for the
/// same reason: Windows returns it that way during a transition, and trusting it
/// would give a two-pixel window.
pub fn borne_de_la_sortie(moniteur: (u32, u32), travail: Option<(u32, u32)>) -> (u32, u32) {
    match travail {
        Some((l, h)) if l >= 2 && h >= 2 => (l.min(moniteur.0), h.min(moniteur.1)),
        _ => moniteur,
    }
}

/// The size to which a source in `SortieEntiere` mode must crop,
/// and at which its Windows window must be placed, for a given viewport.
///
/// 🔴 **IT IS THE FUNCTION THAT PREVENTS THE TWO PROCESSES FROM FIGHTING, AND
/// IT IS THE WHOLE DESIGN OF THIS BATCH.** The Windows window has **two**
/// claimants: the SENSOR, which receives the browser's `Resize`, and the
/// SUPERVISOR, which puts the window back **every second** at the size its
/// table retains (`boucle::placement_periodique::replacer_si_besoin`, target
/// read in `Table::output_size_of`). A remedy living only in the
/// sensor would be **undone one second later**, and the symptom would be
/// "it works, then it comes back".
///
/// The countermeasure is neither a lock nor one more message: it is that **both
/// processes compute the SAME value through THIS function**, from the same
/// two inputs — the viewport announced by the browser, and the bound returned
/// by `borne_de_la_sortie` for the SAME monitor (the supervisor queries it
/// by the output's origin, the sensor by its window, which is on it:
/// same `HMONITOR`, same answer). Whichever acts first, the
/// other's gesture is then a `no-op`: `doit_etre_replacee` sees no
/// gap, and the short-circuit of `suivre_le_viewport` returns without rebuilding
/// anything.
///
/// ⚠️ **And if the two diverged anyway**, the disagreement would be **bounded
/// and self-resolving**: the supervisor puts the window back at the next round, hence
/// within one second at most (`PERIODE_PLACEMENT`), and the sensor follows at the
/// next `Resize`. A divergence costs one badly framed image, never an
/// endless oscillation.
///
/// Composition of two rules that already existed, **reused and not
/// copied**:
///   ① `clamp_to_max_size` — the viewport arrives in device pixels
///      since D9, and a client at `devicePixelRatio > 1` would rush in without
///      limit; it is the bounding `create_output` and `viewport_recu`
///      already apply, at the two entry points of creation;
///   ② `superviseur::placement::retained_size` — axis-by-axis `min`, even and
///      never zero: we **crop** a texture, we do not scale it,
///      so no aspect ratio to preserve here.
pub fn size_for_viewport(demande: (u32, u32), borne: (u32, u32)) -> (u32, u32) {
    // ⑵ **ASPECT-RATIO-PRESERVING FIT, AND NOT AN AXIS-BY-AXIS `min`.**
    //
    // 🔴 **IT WAS A REAL DEFECT, MEASURED ON THE PRODUCT IN PRODUCTION, AND IT
    // WAS MY DOING.** The first draft composed
    // `clamp_to_max_size` (which preserves aspect) with
    // `placement::retained_size` (an axis-by-axis `min`, which does not preserve
    // it) — so that **the second destroyed what the first had just
    // guaranteed**. Network capture of August 31st, 2026, 28 `viewport` frames
    // relayed while the owner dragged the edges of their window:
    //
    // ```text
    // requested 1723x1303 (1.322) -> served 1428x1032 (1.384): gap 4.6%
    // requested 2058x851  (2.418) -> served 1860x794  (2.343): gap 3.1%
    // requested 1922x1092 (1.760) -> served 1860x1032 (1.802): gap 2.4%
    // ```
    //
    // An aspect gap of a few percent is **exactly** a band of
    // `--video-letterbox` (`#000`) along a pair of edges, whose
    // thickness **varies with the requested ratio** — what the owner
    // described word for word ("the size of the borders differs depending on the
    // size/ratio").
    //
    // ⚠️ **IT WAS NOT THE DECLARED LIMIT, AND CONFUSING THEM COST A
    // ROUND TRIP.** The limit says "we cannot grow BEYOND the
    // bound"; this defect served **a wrong shape INSIDE the
    // bound**. `1723x1303` fits perfectly in `1364x1032` — under the bound
    // on both axes, and at the exact ratio.
    //
    // ⚠️ **THE AXIS-BY-AXIS `min` STAYS RIGHT WHERE IT COMES FROM**:
    // `placement::retained_size` crops a texture in an output born too
    // large, where there is no aspect ratio to honour. Here we choose the
    // SHAPE of the served rectangle, and that shape must be the one the
    // browser requests. Two needs, two rules; the defect came from having
    // confused them.
    //
    // ⚠️ **THE CAP AT `1.0` NEVER ENLARGES A SMALL REQUEST**, and
    // ❌ **it is NOT what keeps the taskbar out of the frame —
    // the first draft of this comment claimed so, and the mutation
    // refuted it.** Removing the cap leaves the result INSIDE the bound (it only
    // climbs up to it): the taskbar does not come in for all that, and
    // the test that claimed to guard this property stayed GREEN under the
    // mutation. It is `borne_de_la_sortie`, and it alone, that takes the taskbar
    // out of the frame.
    //
    // What the cap really does: **never serve an image LARGER
    // than what the browser requested.** Without it, a viewport of
    // 900×500 would be encoded at 1854×1030 — four times the macroblocks, for
    // pixels the page cannot display.
    let (dl, dh) = clamp_to_max_size(demande);
    let f = f64::min(
        f64::min(borne.0 as f64 / dl as f64, borne.1 as f64 / dh as f64),
        1.0,
    );
    // `.max(2)` then `& !1`: the NV12 encoder requires even dimensions, and
    // a collapsed video box emits `(0, 0)` (a real D8 case).
    let mets = |x: u32| (((x as f64 * f).round() as u32).max(2)) & !1;
    // ⚠️ Rounding can, on a single axis, return one pixel more than the
    // bound (`round` goes up). The final `min` is a net: `region_de_sortie`
    // must stay INSIDE the texture, and a region that overflows would make
    // cropping fail.
    (
        mets(dl).min(borne.0 & !1).max(2),
        mets(dh).min(borne.1 & !1).max(2),
    )
}

/// Maximum size a virtual output will take on a viewport request.
///
/// ⚠️ **NOT CALIBRATED.** It is a safeguard set out of prudence, without any
/// visual judgement having judged it — exactly the gap `BPP_MIN` has carried
/// since workstream C part 1. Its reason, for its part, is measured: D6 noted the
/// browser's decoder saturated from eight 1280×720 windows (18.03%
/// of frames dropped at the full rung, one run), and a 4K screen
/// would require 9× the pixels of a single one of those windows.
pub const MAX_OUTPUT_SIZE: (u32, u32) = (1920, 1080);

/// Brings a requested size under `MAX_OUTPUT_SIZE`, aspect ratio
/// preserved, in even dimensions, and never zero.
///
/// ⚠️ **IMPORTANT 4 (review of task 9): the fast branch below
/// applied no floor**, unlike the scaling branch
/// (`.max(2)` already present on it). `(0, 0)` — a video box reduced to
/// nothing, transiently true during a collapsed window or a fullscreen
/// transition — went through as is. This `.max(2)` stays a minimal net.
///
/// ❌ **THIS FUNCTION HAS FOUND ITS CALLERS AGAIN, and it is WRONG to say here
/// that it has none — fixed in task 8 of sub-block D10.** It
/// stayed without a caller from the removal of the output mode change (D9,
/// task 3) until D9's legacy 5, closed by tasks 6 and 7 of THIS
/// sub-block: `superviseur::boucle::creation_sortie::create_output`
/// bounds with it at the CREATION of an output, and `superviseur::table::attribution::viewport_recu`
/// bounds with it at the REUSE of a retained output, by the same bounding and
/// for the same reason — the viewport announced by the browser arrives in
/// device pixels since D9, and without this cap a request at
/// `devicePixelRatio > 1` would rush in without limit. Its own role
/// stays limited to the CAP (`MAX_OUTPUT_SIZE`); the 160×120 application
/// floor the old caller applied (the mode change removed by
/// D9) still exists nowhere.
///
/// ✅ **The CREATION SIZE of an output IS NOW BOUNDED**, at the two
/// entry points above. **No real HiDPI client has been measured for
/// all that**: the acceptance setup stays a headless Chrome, and nothing
/// has exercised `deviceScaleFactor > 1` on the COST (eight windows at
/// `MAX_OUTPUT_SIZE` rather than at 720p, encoder ceiling never measured
/// beyond 720p) — only unit symmetry has been (D9, task 5). This
/// last sentence is the same reservation D9 left; the bounding that
/// followed it was missing, and it is that which is fixed here, not the reservation
/// itself.
pub fn clamp_to_max_size((l, h): (u32, u32)) -> (u32, u32) {
    let (max_l, max_h) = MAX_OUTPUT_SIZE;
    if l <= max_l && h <= max_h {
        return (l.max(2) & !1, h.max(2) & !1);
    }
    // The most constraining factor of the two axes: bounding each axis
    // separately would distort the image.
    let facteur = f64::min(max_l as f64 / l as f64, max_h as f64 / h as f64);
    let borne = |x: u32| (((x as f64 * facteur).round() as u32).max(2)) & !1;
    (borne(l), borne(h))
}

/// Region to capture in the texture of a duplicated output.
///
/// **Relative to the output, not to the virtual desktop.** `DesktopCapture::sur_sortie`
/// returns a texture covering that output alone; its origin in the
/// virtual desktop's space (for example x=2400 for an output placed to the right of the
/// physical desktop) has no meaning there. Passing desktop coordinates
/// would give a shifted or empty image.
///
/// Dimensions are aligned on even values: the NV12 encoder
/// requires them, and a virtual output created at an odd size by an odd
/// viewport is a real case.
///
/// ⚠️ **Since sub-block D10, the caller passes it the RETAINED size, not
/// the output's**: an output born too large (polluted registry) is
/// accepted and cropped at the origin. The function itself is unchanged —
/// it is its argument that has changed meaning.
pub fn region_de_sortie(largeur: u32, hauteur: u32) -> Option<Rect> {
    let largeur = largeur & !1;
    let hauteur = hauteur & !1;
    if largeur < 2 || hauteur < 2 {
        return None;
    }
    Some(Rect {
        x: 0,
        y: 0,
        width: largeur,
        height: hauteur,
    })
}

// This module's host tests live in `sortie/tests.rs`, extracted there
// in a DEDICATED task and BEFORE batch 33's addition, which would have taken this
// file beyond the 500-line ceiling. `#[path]` rather than a module
// subdirectory: precedent of `superviseur/table.rs`.
#[cfg(test)]
#[path = "sortie/tests.rs"]
mod tests;

// The block below only compiles under Windows: it builds a real
// `WindowsSource` (COM types `HWND`/`DesktopCapture`/`H264Encoder`,
// all themselves gated `#[cfg(windows)]`). `region_de_sortie` and its tests
// stay ABOVE this `cfg`, at module scope, to keep
// running on the host — see this file's `#[path]` declaration as
// module `windows_source_sortie`, outside any `#[cfg(windows)]`, in
// `main.rs`.
//
// This module (`windows_source_sortie`) is a SIBLING of `windows_source`, not
// a descendant (both declared separately at the crate root, see
// `main.rs`): Rust's default private visibility would therefore NOT give
// access to `WindowsSource`'s fields from here.
//
// **That is why `sur_sortie` does NOT assemble the `Self { … }` literal
// itself** and goes through `WindowsSource::depuis_pieces`, kept in
// `windows_source.rs` as a `pub(crate) fn`. A first version had done
// the opposite — also moving `depuis_pieces` — which forced opening the
// THIRTEEN fields of `WindowsSource` as `pub(crate)`, including `capture` and `fatal`
// which carry a cross-field invariant documented as fragile (see their
// comments). A `pub(crate)` call to a single function costs one line
// and opens nothing.
#[cfg(windows)]
use anyhow::{Context, Result};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[cfg(windows)]
use crate::capture::DesktopCapture;
#[cfg(windows)]
use crate::encode::H264Encoder;
#[cfg(windows)]
use crate::windows_source::WindowsSource;

#[cfg(windows)]
impl WindowsSource {
    /// Builds a source capturing a DXGI output, cropped to the RETAINED
    /// size.
    ///
    /// Sub-block D1's mode: the window has its own virtual output, so there
    /// is no Windows window to follow — `resize` still does
    /// nothing (`ModeCapture::SortieEntiere`). ⚠️ **But since sub-block
    /// D10, the output itself can be born LARGER than what the
    /// supervisor requested** (polluted registry, see the measurement finding at the
    /// head of `capteur/plein_ecran.rs`): `size` carries what the
    /// supervisor retained, and the captured region is cropped at the output's origin
    /// to that size — never to the whole output if
    /// it overflows. `hwnd` stays filled in — input injection and the
    /// liveness check need it — but it is still not used to compute
    /// the region.
    pub fn sur_sortie(
        hwnd: HWND,
        nom_sortie: &str,
        size: (u32, u32),
        fps: u32,
        bitrate: u32,
        clock_origin: std::time::Instant,
    ) -> Result<Self> {
        let capture = DesktopCapture::sur_sortie(nom_sortie)?;
        let (dw, dh) = capture.desktop_size();
        // The output can be LARGER than the window since sub-block
        // D10: we crop at the output's origin, where the supervisor
        // placed the window.
        //
        // 🔴 **THE BOUND HAS BEEN THE WORK AREA SINCE BATCH 33, NO LONGER THE
        // TEXTURE ALONE** — that is what takes the 48 rows of the secondary
        // taskbar out of the crop from the FIRST frame, and not
        // only at the first resize. The texture stays a bound
        // (`min` below): it is in TEXTURE pixels whereas `rcWork`
        // is in DESKTOP coordinates, and the two do not coincide on
        // this machine (1860 against 1428 in width, batch 32T). The fallback of a
        // failing `GetMonitorInfoW` is the behaviour from before this batch.
        let borne = match crate::window::zones_du_moniteur_de(hwnd) {
            Ok((moniteur, travail)) => {
                let b = borne_de_la_sortie(
                    (moniteur.width, moniteur.height),
                    Some((travail.width, travail.height)),
                );
                (b.0.min(dw), b.1.min(dh))
            }
            Err(error) => {
                tracing::warn!(%error, "zone de travail illisible : recadrage borné par la texture");
                (dw, dh)
            }
        };
        let (rl, rh) = size_for_viewport(size, borne);
        let region = region_de_sortie(rl, rh).with_context(|| {
            format!("sortie {nom_sortie} de dimensions inexploitables ({dw}x{dh})")
        })?;
        let (width, height) = (region.width, region.height);

        let mut encoder = H264Encoder::new(
            capture.device(),
            (width, height),
            (width, height),
            fps,
            bitrate,
        )?;
        encoder.request_keyframe()?;

        Ok(Self::depuis_pieces(
            hwnd,
            capture,
            encoder,
            region,
            width,
            height,
            fps,
            bitrate,
            clock_origin,
            // The discriminant that was missing: without it, `resize` would resize
            // this window and substitute the physical desktop for it.
            ModeCapture::SortieEntiere,
        ))
    }
}
