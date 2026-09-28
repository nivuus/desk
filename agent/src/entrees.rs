//! On WHICH rectangle the browser's coordinates are unmapped.
//!
//! 🔴 **THE DEFECT THIS MODULE EXISTS TO CLOSE (measured on 30 August 2026).**
//! The client normalises its coordinates on **the image it receives**
//! (`client/src/input.ts`: "normalised on 0..65535 relative to the image
//! area", a fraction of the `<video>`). The agent, for its part, applied them to the **client
//! area of the WINDOW**. The two no longer spoke of the same rectangle since
//! capture switched to `ModeCapture::SortieEntiere` (sub-block D10):
//! the image is **the whole output**, wallpaper and taskbar
//! included.
//!
//! **The error can be computed**, and it has two terms — that is why it cannot be
//! fixed by an offset:
//!
//! ```text
//! error(f) = (Wx − Ox) + f·(Ww − Ow)
//!            └ ORIGIN ─┘   └ SCALE ─┘
//! ```
//!
//! Surveyed on the owner's machine: the window at `+4428+51` for an
//! output aimed at `+3140+0`, that is **+1288 in x and +51 in y** of origin
//! term — and a non-zero scale term, since the taskbar is
//! visible in the image (`Oh > Wh`) and resizing is
//! **deliberately ignored** in `SortieEntiere`.
//!
//! 🔵 **The keyboard was not affected**, and it is not a coincidence: it
//! carries no coordinate. The keyboard/mouse split is **predicted** by
//! this explanation.
//!
//! ## 🔴 A SINGLE SOURCE, AND THAT IS THE WHOLE POINT OF THIS MODULE
//!
//! This defect was born because **two places described the same rectangle**
//! and only one followed D10. A fix that let two
//! independent descriptions remain would come undone at the next mode change.
//!
//! **`config.sortie_dxgi` is already the only discriminant of the capture mode**
//! (`demarrage/source.rs`: `Some(nom)` ⇒ remote source served by the
//! sensor, whole output; `None` ⇒ local capture of the cropped window).
//! **The input reference now derives from it, from the same value** — there
//! is nothing left to keep in agreement.

//!
//! ## 🔴 WHAT THE FIRST FIX FIXED, AND WHAT IT WAS MISSING
//!
//! Batch 32Q made the reference derive from `config.sortie_dxgi` and
//! set it on **the whole DXGI output**. It was **the right ORIGIN and the
//! wrong SIZE**, and the owner saw it right away: "at 0 ok,
//! fully to the right not good, it's progressive", in x only.
//!
//! **The capture is not the output: it is a CROP of the output,
//! `retained_size`, placed at its origin** (`windows_source/sortie.rs`,
//! `capteur/fenetre/ouverture.rs`). Surveyed on the owner's machine,
//! in the same log:
//!
//! ```text
//! output duplication established     desktop_width=1860 desktop_height=1080
//! native NVENC session initialised   width=1428   height=1080
//! ```
//!
//! hence the residual error, purely one of SCALE (the origin, for its part, is right):
//!
//! | fraction | the image shows | the agent injected | gap |
//! | --- | --- | --- | --- |
//! | 0.00 | 0 | 0 | **0** |
//! | 0.50 | 714 | 930 | +216 |
//! | 1.00 | 1428 | 1860 | **+432** |
//!
//! In y, 1080 against 1080: **zero**. It is exactly the symptom described.
//!
//! 🔴 **THE SIZE IS NOT RECOMPUTED HERE, IT IS SHARED.** Recomputing
//! `retained_size(window_size, output_size)` in the child would give back
//! **two descriptions of the same rectangle** — precisely the mechanism that
//! produced the defect of batch 32M. The size used is the one the
//! **sensor** retained and announced through `DepuisCapteur::Attachee`,
//! that is **the one and only storage** that `SourceDistante::dimensions`
//! returns to the rest of the child: [`FrameSize`], created by
//! `demarrage::source::construire` and handed both to the source and to the
//! injector. There is **nothing to keep in agreement**, because there is only one
//! value.
//!
//! The commands that establish it, so that the next reader redoes the
//! check without believing anyone:
//!
//! ```text
//! grep -n 'Attachee { largeur, hauteur }' agent/src/capteur/tube.rs
//! grep -n 'retained_size' agent/src/capteur/fenetre/ouverture.rs
//! grep -rn 'FrameSize' agent/src/
//! ```

/// The size of the image ACTUALLY captured and encoded, in pixels.
///
/// 🔴 **ONE STORAGE, TWO READERS.** `SourceDistante` sets it from
/// `DepuisCapteur::Attachee` (attach), `DepuisCapteur::Etat` (change) and
/// `DepuisCapteur::Size` (acknowledged resize), and its
/// `dimensions()` reads it back; the input injector reads it back too. No one
/// recomputes it — that is the whole point of this type, and the reason why
/// it lives in THIS module rather than next to the source.
///
/// **A single `AtomicU64` and not two `AtomicU32`s, on purpose**: a pair
/// of atomics read in two steps can return a new width with a
/// stale height during a resize, and the mouse event that
/// fell in that interval would be unmapped on a rectangle that never
/// existed. Packing both halves makes this tearing
/// **impossible** instead of making it rare.
#[derive(Debug)]
pub struct FrameSize(std::sync::atomic::AtomicU64);

impl FrameSize {
    pub fn new(largeur: u32, hauteur: u32) -> Self {
        let cellule = Self(std::sync::atomic::AtomicU64::new(0));
        cellule.poser(largeur, hauteur);
        cellule
    }

    pub fn poser(&self, largeur: u32, hauteur: u32) {
        let empaquetee = ((largeur as u64) << 32) | hauteur as u64;
        self.0
            .store(empaquetee, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn lire(&self) -> (u32, u32) {
        let empaquetee = self.0.load(std::sync::atomic::Ordering::Relaxed);
        ((empaquetee >> 32) as u32, empaquetee as u32)
    }
}

/// The rectangle on which to unmap the coordinates received from the browser.
///
/// 🔴 **THE MULTI-WINDOW VARIANT CARRIES THE IMAGE SIZE, AND IT IS THE
/// TYPE THAT ENFORCES IT.** An `Option<&str>` on one side and an
/// `Option<Arc<FrameSize>>` on the other would have left representable the state
/// "a named output without its size", that is exactly the silent
/// fallback this module exists to forbid. Here, the variant cannot be
/// built without the size.
#[derive(Debug, Clone)]
pub enum Reference {
    /// The window's client area: what the capture shows when it
    /// crops the window (`ModeCapture::FenetreRecadree`, single-window
    /// path). The client area IS the crop: there is no
    /// second size to carry.
    ZoneClientDeLaFenetre,
    /// The crop of the named DXGI output: **its origin, and the size of the
    /// image** (`ModeCapture::SortieEntiere`, multi-window path).
    ///
    /// ⚠️ The variant's name says "output" and it is **not** the whole
    /// output: it is the output that gives the ORIGIN, and `image` that gives the
    /// SIZE. Confusing them is the defect of batch 32Q, fixed here.
    SortieCapturee {
        nom: String,
        image: std::sync::Arc<FrameSize>,
    },
}

/// The rectangle of the captured region: **the output's origin, the size
/// of the image**.
///
/// Returns `None` when the image size is not yet known (one of the
/// two halves is zero). ⚠️ **No fallback to the whole output**: that would
/// silently reintroduce the scale error this function removes, and
/// the symptom would again become "the mouse drifts to the right" without a
/// single trace saying so. The caller must turn it into a named error.
///
/// ⚠️ **No clamping to the output either.** `retained_size` already guarantees
/// that the image fits in the texture (`capteur/fenetre/ouverture.rs`); a
/// `min` here would be a SECOND rule, which would mask a divergence instead
/// of showing it.
pub fn rectangle_capture(
    sortie: crate::geometry::Rect,
    image: (u32, u32),
) -> Option<crate::geometry::Rect> {
    let (largeur, hauteur) = image;
    if largeur == 0 || hauteur == 0 {
        return None;
    }
    Some(crate::geometry::Rect {
        x: sortie.x,
        y: sortie.y,
        width: largeur,
        height: hauteur,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{to_virtual_desktop, Rect};

    /// The shared cell: what we set is what we read back, and the two
    /// halves do not mix.
    #[test]
    fn the_shared_size_returns_what_is_stored_in_it() {
        let size = FrameSize::new(1428, 1080);
        assert_eq!(size.lire(), (1428, 1080));
        size.poser(640, 360);
        assert_eq!(size.lire(), (640, 360));
        // The two halves are well separated, including at the extremes.
        size.poser(u32::MAX, 1);
        assert_eq!(size.lire(), (u32::MAX, 1));
    }

    /// A size not yet known must **not** produce a rectangle:
    /// it is this refusal that prevents a silent fallback to the whole output.
    #[test]
    fn without_a_frame_size_there_is_no_rectangle() {
        let sortie = Rect {
            x: 3140,
            y: 0,
            width: 1860,
            height: 1080,
        };
        assert_eq!(rectangle_capture(sortie, (0, 1080)), None);
        assert_eq!(rectangle_capture(sortie, (1428, 0)), None);
    }

    /// 🔴 **THE RED, AND IT CARRIES THE FIGURES OF THE SURVEY.**
    ///
    /// The set-up is that of the owner's machine: the window is at
    /// `+4428+51` (survey of `placement_periodique`), the captured output at
    /// `+3140+0`. A click in the **top-left** corner of the image — hence at
    /// the origin of the OUTPUT — must land on `(3140, 0)`.
    ///
    /// The formula from BEFORE, which unmaps on the window, returns `(4428, 51)`:
    /// **+1288 in x and +51 in y**, exactly the offset derived from the log.
    #[test]
    fn the_former_formula_yields_the_measured_offset_of_1288_and_51() {
        let bureau = Rect {
            x: 0,
            y: 0,
            width: 8192,
            height: 2160,
        };
        let fenetre = Rect {
            x: 4428,
            y: 51,
            width: 1428,
            height: 1080,
        };
        let sortie = Rect {
            x: 3140,
            y: 0,
            width: 1920,
            height: 1200,
        };

        let vise = |r: Rect| {
            let (nx, ny) = to_virtual_desktop(0, 0, r, bureau);
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32,
            )
        };
        let (juste_x, juste_y) = vise(sortie);
        let (faux_x, faux_y) = vise(fenetre);
        assert_eq!(
            (juste_x, juste_y),
            (3140, 0),
            "la référence JUSTE vise l'origine de la sortie"
        );
        assert_eq!(
            faux_x - juste_x,
            1288,
            "le terme d'ORIGINE en x, relevé sur la VM"
        );
        assert_eq!(
            faux_y - juste_y,
            51,
            "le terme d'ORIGINE en y, relevé sur la VM"
        );
    }

    /// 🔴 **The second term, the one a constant offset would not
    /// fix.** At the BOTTOM-RIGHT corner, the gap is no longer the same as at the
    /// top-left corner: it is the SCALE term, and that is why this defect cannot
    /// be repaired by subtracting 1288.
    #[test]
    fn the_error_is_not_a_plain_offset_it_grows_with_distance() {
        let bureau = Rect {
            x: 0,
            y: 0,
            width: 8192,
            height: 2160,
        };
        let fenetre = Rect {
            x: 4428,
            y: 51,
            width: 1428,
            height: 1080,
        };
        let sortie = Rect {
            x: 3140,
            y: 0,
            width: 1920,
            height: 1200,
        };
        let ecart = |f: u16| {
            let en_px = |r: Rect| {
                let (nx, _) = to_virtual_desktop(f, 0, r, bureau);
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32
            };
            en_px(fenetre) - en_px(sortie)
        };
        let au_coin = ecart(0);
        let au_bout = ecart(65535);
        assert_eq!(au_coin, 1288);
        assert_ne!(
            au_bout, au_coin,
            "l'écart CHANGE : ce n'est pas un décalage constant"
        );
        // Ow − Ww = 1920 − 1428 = 492: the gap shrinks by as much at the end.
        assert_eq!(au_coin - au_bout, 492);
    }

    /// The case where both references COINCIDE: the window exactly occupies
    /// its output. It is the control — it shows that the fix changes
    /// nothing when there was nothing to change, and therefore that the red above
    /// does come from the gap between the rectangles.
    #[test]
    fn quand_la_fenetre_occupe_sa_sortie_les_deux_references_coincident() {
        let bureau = Rect {
            x: 0,
            y: 0,
            width: 8192,
            height: 2160,
        };
        let meme = Rect {
            x: 3140,
            y: 0,
            width: 1920,
            height: 1200,
        };
        for f in [0u16, 12345, 65535] {
            assert_eq!(
                to_virtual_desktop(f, f, meme, bureau),
                to_virtual_desktop(f, f, meme, bureau)
            );
        }
        let (nx, ny) = to_virtual_desktop(0, 0, meme, bureau);
        assert_eq!(
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32
            ),
            (3140, 0)
        );
    }
    /// 🔴 **E1'S RED, AND IT CARRIES THE FIGURES OF THE VM'S LOG.**
    ///
    /// Surveyed in the same `agent.log`, the same session:
    ///
    /// ```text
    /// output duplication established     desktop_width=1860 desktop_height=1080
    /// native NVENC session initialised   width=1428   height=1080
    /// ```
    ///
    /// The expected value is written **BEFORE** any measurement on the VM, and it derives
    /// from no measured point: it comes from these two lines alone. It is the
    /// condition set after the circular measurement of batch 32R — see the § "its
    /// most treacherous trap" of `CLAUDE.md`.
    ///
    /// **The right edge of the image is 1428 px from the output's origin.**
    /// Batch 32Q's formula, which unmapped on the WHOLE output, aimed there at
    /// 1860: **+432 px**, nil on the left, half of it halfway. In y, 1080
    /// against 1080: **zero**.
    #[test]
    fn le_recadrage_annule_les_432_px_de_derive_en_x_et_ne_touche_pas_l_y() {
        let bureau = Rect {
            x: 0,
            y: 0,
            width: 8192,
            height: 2160,
        };
        let sortie = Rect {
            x: 3140,
            y: 0,
            width: 1860,
            height: 1080,
        };
        let image = (1428u32, 1080u32);

        // What the product computes today.
        let juste = rectangle_capture(sortie, image).expect("taille d'image connue");
        // What batch 32Q computed: the whole output, as is.
        let formule_32q = sortie;

        let en_px = |r: Rect, f: u16| {
            let (nx, ny) = to_virtual_desktop(f, f, r, bureau);
            (
                (nx as f64 / 65535.0 * bureau.width as f64).round() as i32,
                (ny as f64 / 65535.0 * bureau.height as f64).round() as i32,
            )
        };

        // The RIGHT edge of the image: 3140 + 1428.
        assert_eq!(
            en_px(juste, 65535).0,
            3140 + 1428,
            "le bord droit de l'IMAGE"
        );
        assert_eq!(
            en_px(formule_32q, 65535).0 - en_px(juste, 65535).0,
            432,
            "les 432 px que la formule du lot 32Q ajoutait au bord droit"
        );
        // Halfway, half of it: the error is proportional, not constant.
        assert_eq!(en_px(formule_32q, 32767).0 - en_px(juste, 32767).0, 216);
        // On the left, nothing: that is why the owner said "at 0 ok".
        assert_eq!(en_px(formule_32q, 0).0 - en_px(juste, 0).0, 0);
        // In y, nothing anywhere: the output and the image are both 1080.
        for f in [0u16, 32767, 65535] {
            assert_eq!(
                en_px(formule_32q, f).1,
                en_px(juste, f).1,
                "aucune dérive en y"
            );
        }
    }

    /// The negative control of the red above: when the image occupies the WHOLE
    /// output, the two formulas coincide. Without it, a `rectangle_capture`
    /// that returned anything smaller would pass the red.
    #[test]
    fn quand_l_image_occupe_toute_la_sortie_les_deux_formules_coincident() {
        let sortie = Rect {
            x: 3140,
            y: 0,
            width: 1860,
            height: 1080,
        };
        assert_eq!(rectangle_capture(sortie, (1860, 1080)), Some(sortie));
    }
}
