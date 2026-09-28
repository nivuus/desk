//! The host tests of `windows_source_sortie`.
//!
//! **Extracted from `sortie.rs` VERBATIM**, in a DEDICATED task and **BEFORE**
//! the addition that would have taken it beyond the project's 500-line ceiling —
//! it is the strong form `CLAUDE.md` prescribes ("extract, never
//! compress", and "the strong form is the extraction played in a
//! DEDICATED task, BEFORE the one that adds"). No assertion, no comment was
//! rewritten in the move.
//!
//! Declared by `#[path = "sortie/tests.rs"] mod tests;` **inside**
//! `sortie.rs`, and not at the crate root: it is the mechanism used to
//! split an over-long TESTS module in an otherwise portable file
//! (precedent: `superviseur/table.rs`, which thus declares `table/tests.rs` and
//! `table/tests_relance.rs`). ⚠️ **It is NOT `CLAUDE.md`'s child module
//! convention** — that one only governs modules extracted from a
//! `#[cfg(windows)]` parent to compile on the host, and this file is not
//! one.

use super::*;

#[test]
fn a_size_below_the_ceiling_passes_as_is() {
    assert_eq!(clamp_to_max_size((1280, 720)), (1280, 720));
}

#[test]
fn a_4k_size_is_brought_down_to_the_ceiling() {
    // D6 measured the browser's decoder saturated from eight 720p
    // windows: 9× the pixels of a single one is exactly what it copes with
    // worst.
    assert_eq!(clamp_to_max_size((3840, 2160)), MAX_OUTPUT_SIZE);
}

#[test]
fn bounding_preserves_the_aspect_ratio() {
    // A 21:9 bounded independently on each axis would distort the image.
    let (l, h) = clamp_to_max_size((3440, 1440));
    assert!(l <= MAX_OUTPUT_SIZE.0 && h <= MAX_OUTPUT_SIZE.1, "{l}x{h}");
    let ecart = (l as f64 / h as f64) - (3440.0 / 1440.0);
    assert!(ecart.abs() < 0.01, "rapport {l}/{h} contre 3440/1440");
}

#[test]
fn bounding_returns_even_dimensions() {
    // A Windows window imposes even dimensions, and so does an NV12
    // encoder.
    let (l, h) = clamp_to_max_size((3441, 1441));
    assert_eq!(l % 2, 0, "largeur {l}");
    assert_eq!(h % 2, 0, "hauteur {h}");
}

/// IMPORTANT 2 of the review of task 9: **the test above cannot
/// fail on the property it names.**
///
/// On `(3441, 1441)`, `facteur ≈ 0.557977` gives `round(3441×f) = 1920` and
/// `round(1441×f) = 804` — **already even before any masking**. Removing the
/// two `& !1` from `clamp_to_max_size` leaves it GREEN. And the two
/// other even inputs of the neighbouring tests (`(1280, 720)`, `(0, 0)`) do not
/// exercise it either: before this case, **none of the five tests
/// covered even alignment**, whereas one bears its name.
///
/// `(1281, 721)` goes through the **fast branch** (under the cap, hence
/// no scaling factor): alignment is the only mechanism at play there,
/// and removing the `& !1` returns `(1281, 721)` instead of `(1280, 720)`.
/// It is the repository's doctrine applied to a test: **a check never
/// seen red is not a check** (D7, F1).
#[test]
fn even_alignment_is_really_exercised_by_an_odd_input() {
    assert_eq!(clamp_to_max_size((1281, 721)), (1280, 720));
}

/// IMPORTANT 4 (review of task 9): the fast branch did not bound
/// downwards, unlike the scaling branch which already applied
/// `.max(2)`. `(0, 0)` is its real degenerate case: a video box
/// reduced to nothing (collapsed window, fullscreen transition) emits it.
#[test]
fn bounding_never_returns_a_zero_dimension() {
    assert_eq!(clamp_to_max_size((0, 0)), (2, 2));
}

#[test]
fn the_region_starts_at_the_output_origin() {
    assert_eq!(
        region_de_sortie(1600, 900),
        Some(Rect {
            x: 0,
            y: 0,
            width: 1600,
            height: 900
        })
    );
}

#[test]
fn odd_dimensions_are_aligned_downward() {
    assert_eq!(
        region_de_sortie(1601, 901),
        Some(Rect {
            x: 0,
            y: 0,
            width: 1600,
            height: 900
        })
    );
}

#[test]
fn a_degenerate_output_gives_no_region() {
    assert_eq!(region_de_sortie(1, 900), None);
    assert_eq!(region_de_sortie(0, 0), None);
}

/// Defect C1 in one line: it is this boolean that prevents `resize` from
/// releasing the duplication of a virtual output to substitute
/// the physical desktop's for it — the leak of one monitor's content into
/// someone else's session.
#[test]
fn only_the_cropped_mode_recaptures_the_desktop() {
    assert!(ModeCapture::FenetreRecadree.recapture_le_bureau());
    assert!(!ModeCapture::SortieEntiere.recapture_le_bureau());
}

/// The counterpart of the test above, and **both together are batch
/// 33's contract**: the two modes do not merely share a boolean, they
/// take two EXCLUSIVE paths. Without this second assertion, making
/// `suit_le_viewport` return `false` for both would bring back yesterday's
/// `no-op` without any test flinching.
#[test]
fn only_the_whole_output_mode_follows_the_viewport() {
    assert!(ModeCapture::SortieEntiere.suit_le_viewport());
    assert!(!ModeCapture::FenetreRecadree.suit_le_viewport());
    // Exclusive, and exhaustive: every mode takes exactly one path.
    for mode in [ModeCapture::FenetreRecadree, ModeCapture::SortieEntiere] {
        assert!(
            mode.recapture_le_bureau() ^ mode.suit_le_viewport(),
            "{mode:?} must take exactly one of the two paths"
        );
    }
}

/// 🔴 **THIS TEST'S EXPECTED VALUE COMES FROM THE PRODUCT'S PRODUCTION LOG, NOT
/// FROM A COMPUTATION ON WHAT IT JUDGES** — it is the rule batch 32R paid for
/// ("an expected value derived from the measurement cannot refute it").
///
/// The numbers were noted on August 31st, 2026 on the running agent:
///   - `778x491`: the MOST FREQUENT request of the 34 that yesterday's guard
///     dropped (15 occurrences out of 34, `C:\nivuus\agent.log`, the
///     "resize ignored …" lines);
///   - `1428x1032`: the real bound of the served output — `mon=1428x1080`,
///     `work=1428x1032`, noted in SESSION 1 by
///     `docs/superpowers/plans/2026-08-31-barre-des-taches-diagnostic.md`;
///   - `1428x1080`: what the product served, **whatever it was asked**.
///
/// 🔴 **AND HERE IS WHAT THIS TEST'S FIRST DRAFT GOT WRONG, CAUGHT BY THE
/// TEST ITSELF.** It expected `(1723, 1080)` for a `1723x1303` request,
/// believing `clamp_to_max_size` an axis-by-axis CLIPPING. **It is
/// a SCALING one, with the aspect ratio PRESERVED**: `1723x1303` comes out as
/// `1428x1080`, that is, exactly the size the product already served.
/// A consequence that changes the diagnosis and is stated here rather than forgotten:
/// **the black bars do NOT come from the cap**, which respects the requested
/// aspect, but from the fact that the retained size is **FROZEN at opening** and
/// that any later request is dropped. It is that freeze this batch lifts.
#[test]
fn the_most_frequent_request_is_now_honoured_up_to_the_aspect() {
    // Under the bound on both axes: it goes through as is (up to
    // evenness), so the image matches EXACTLY the requested ratio.
    assert_eq!(size_for_viewport((778, 491), (1428, 1032)), (778, 490));
    // …and it is no longer 1428×1080, yesterday's frozen size. Without this second
    // assertion, a rule ignoring its request would stay green if the
    // bound were 778×490.
    assert_ne!(size_for_viewport((778, 491), (1428, 1032)), (1428, 1080));
}

/// 🔴 **WHAT THE BATCH DOES NOT SOLVE, WRITTEN AS A TEST SO THAT NOBODY BELIEVES
/// THE CASE CLOSED** — the limit is on SIZE, never on SHAPE.
///
/// ❌ **THIS TEST WAS CALLED `un_viewport_plus_large_que_la_borne_garde_ses_
/// bandes_noires` AND EXPECTED `(1428, 538)`. BOTH WERE THE EXPRESSION
/// OF THE DEFECT**, not of the limit: an axis-by-axis `min` returned a rectangle at the
/// wrong RATIO, hence bars. Since the aspect-preserving fit, a
/// viewport wider than the bound is served **SMALLER, but at its exact
/// shape** — and there are no more bars at all.
///
/// What stays true, and what this test guards: we never exceed the bound.
#[test]
fn a_viewport_wider_than_the_bound_is_reduced_without_distortion() {
    let borne = (1428, 1032);
    let (l, h) = size_for_viewport((5118, 1438), borne);
    assert!(
        l <= borne.0 && h <= borne.1,
        "{l}x{h} must fit in {borne:?}"
    );
    let ecart = ((l as f64 / h as f64) - (5118.0 / 1438.0)).abs() / (5118.0 / 1438.0);
    assert!(
        ecart < 0.005,
        "rapport {l}/{h} contre 5118/1438 : ecart {ecart}"
    );
}

/// The bound is the WORK AREA, and that is what takes the taskbar out
/// of the crop. Measurement of August 31st, 2026: `mon=1428x1080`, `work=1428x1032`,
/// `Shell_SecondaryTrayWnd rect=(1280,1032)-(2708,1080)` — 48 rows.
#[test]
fn the_work_area_removes_the_forty_eight_taskbar_rows() {
    assert_eq!(
        borne_de_la_sortie((1428, 1080), Some((1428, 1032))),
        (1428, 1032)
    );
    // And the crop of a full-frame window therefore loses them too: it is the
    // same 1032, and not 1080, that goes to `region_de_sortie`.
    //
    // ⚠️ The WIDTH goes down with it, to 1364: it is the aspect-preserving fit, and
    // it is intended. Returning `(1428, 1032)` — the old axis-by-axis `min` —
    // would serve a 1.384 for a requested 1.322, hence the bars.
    assert_eq!(size_for_viewport((1428, 1080), (1428, 1032)), (1364, 1032));
}

/// 🔴 **THE FALLBACK IS THE BEHAVIOUR FROM BEFORE THE BATCH, AND IT MUST BE
/// EXACTLY THAT.** `GetMonitorInfoW` can refuse; a degenerate work area
/// (Windows returns one during a transition) must be
/// refused the same way. In both cases the bound becomes the
/// monitor's rectangle again — hence yesterday's framing, taskbar included,
/// rather than a two-pixel window.
#[test]
fn a_missing_or_degenerate_work_area_returns_the_monitor_rectangle() {
    assert_eq!(borne_de_la_sortie((1428, 1080), None), (1428, 1080));
    assert_eq!(borne_de_la_sortie((1428, 1080), Some((0, 0))), (1428, 1080));
    assert_eq!(
        borne_de_la_sortie((1428, 1080), Some((1428, 1))),
        (1428, 1080)
    );
}

/// A work area Windows announced LARGER than its monitor
/// must not make the region leave the texture: the `min` is a net,
/// and nothing else holds it.
#[test]
fn a_work_area_larger_than_the_monitor_is_brought_back_to_it() {
    assert_eq!(
        borne_de_la_sortie((1428, 1080), Some((4096, 4096))),
        (1428, 1080)
    );
}

/// The sensor's short-circuit compares the returned value with the current
/// size: it must therefore be **stable**, otherwise each round
/// would rebuild the encoder. A fixed point, tested.
#[test]
fn the_rule_is_stable_on_its_own_result() {
    let sortie = (1860, 1080);
    let une = size_for_viewport((1723, 1303), sortie);
    assert_eq!(size_for_viewport(une, sortie), une);
}

/// A collapsed video box emits `(0, 0)` (real case noted in D8): the
/// rule must never return a zero dimension, which the NV12 encoder
/// would refuse.
#[test]
fn a_folded_video_box_never_returns_a_zero_dimension() {
    assert_eq!(size_for_viewport((0, 0), (1860, 1080)), (2, 2));
}

/// The EIGHT sizes the owner's browser ACTUALLY requested,
/// noted on August 31st, 2026 in a network capture of the `viewport` frames
/// relayed to the VM while they dragged the edges of their window (28
/// frames, 8 distinct values).
///
/// 🔴 **THE EXPECTED VALUE DOES NOT COME FROM WHAT IT JUDGES.** The 0.5% threshold is
/// derived from the RULE, not from a computation on the result: both axes are
/// rounded to an EVEN value, so each moves by at most 1 pixel, which
/// shifts the ratio by at most `1/l + 1/h` — of the order of 0.17% at the
/// dimensions served here. **0.5% is that rounding ceiling with some
/// margin**, and nothing else.
///
/// 🔴 **SEEN RED**: with the axis-by-axis `min` from before the fix, the
/// gaps measured on these same eight values are **2.4% to 4.7%** —
/// up to nine times the threshold. Each is a `--video-letterbox` band
/// whose thickness varies with the ratio, which the owner described.
const VIEWPORTS_MESURES: [(u32, u32); 8] = [
    (1724, 1304),
    (1723, 1303),
    (2058, 851),
    (1922, 1092),
    (1865, 1303),
    (1785, 1303),
    (1652, 1206),
    (1438, 1062),
];

/// The error ceiling that even rounding imposes by itself. See the doc of
/// `VIEWPORTS_MESURES` for its derivation — it is not copied from a
/// result.
const ECART_D_ARRONDI_MAX: f64 = 0.005;

fn ecart_de_rapport(servi: (u32, u32), demande: (u32, u32)) -> f64 {
    let (rs, rd) = (
        servi.0 as f64 / servi.1 as f64,
        demande.0 as f64 / demande.1 as f64,
    );
    (rs - rd).abs() / rd
}

#[test]
fn the_eight_measured_viewports_are_served_at_their_own_ratio() {
    // The bound noted on THEIR session (output 1860×1080, work area
    // minus the taskbar's 48 rows).
    let borne = (1860, 1032);
    for demande in VIEWPORTS_MESURES {
        let servi = size_for_viewport(demande, borne);
        let ecart = ecart_de_rapport(servi, demande);
        assert!(
            ecart < ECART_D_ARRONDI_MAX,
            "{demande:?} served {servi:?}: ratio gap of {:.3} % — this is a \
             --video-letterbox band along a pair of edges",
            ecart * 100.0
        );
        assert!(
            servi.0 <= borne.0 && servi.1 <= borne.1,
            "{servi:?} must fit in {borne:?}"
        );
    }
}

/// ⚠️ **NEITHER 1428 NOR 1860 ARE CONSTANTS OF THIS PRODUCT**, and this test is
/// there so that no reader copies either. The bound in force was
/// noted **different from one session to the next on the SAME machine**
/// (1428×1032 after a reboot, 1860×1032 the following session): it
/// comes from `borne_de`, every time, and never from a number written
/// somewhere. This repository has paid nine times for the 487 wreck.
#[test]
fn the_rule_honours_whatever_bound_it_is_given() {
    for borne in [
        (1428, 1032),
        (1860, 1032),
        (1280, 752),
        (3840, 2160),
        (800, 600),
    ] {
        for demande in VIEWPORTS_MESURES {
            let servi = size_for_viewport(demande, borne);
            assert!(
                servi.0 <= borne.0 && servi.1 <= borne.1,
                "{demande:?} on {borne:?} returns {servi:?}, which OVERFLOWS"
            );
            assert!(
                ecart_de_rapport(servi, demande) < ECART_D_ARRONDI_MAX,
                "{demande:?} on {borne:?} returns {servi:?}, at the wrong ratio"
            );
        }
    }
}

/// 🔴 **THE ASPECT FIX MUST NOT GIVE BACK THE TASKBAR.**
///
/// ❌ **THIS TEST'S FIRST DRAFT WAS VACUOUS, AND THE MUTATION
/// SHOWED IT**: it only exercised `VIEWPORTS_MESURES`, all LARGER than
/// the bound on at least one axis, hence all at factor ≤ 1 — the property
/// "we do not scale up" was never exercised there, and removing the `1.0` cap
/// left this test GREEN. *A test never seen red is not a
/// test.*
///
/// 🔵 **And the mutation refuted the REASON attributed to it**: removing the
/// cap does not bring the taskbar in (the result stays inside the bound).
/// What takes the taskbar out of the frame is `borne_de_la_sortie`, and it alone.
///
/// This test therefore guards the property that really matters and is
/// falsifiable: **the served height NEVER exceeds the work area
/// it is given**, including for a work area nobody
/// hardcoded. The height `900` below is that of no reading:
/// it is chosen DIFFERENT from the values lying around in this file
/// (1032, 1080) precisely so that a hardcoded number turns it red.
#[test]
fn the_served_height_never_exceeds_the_given_work_area() {
    for travail in [(1860, 1032), (1860, 900), (1428, 700), (1280, 752)] {
        for demande in VIEWPORTS_MESURES {
            let servi = size_for_viewport(demande, travail);
            assert!(
                servi.1 <= travail.1,
                "{demande:?} on a work area {travail:?} returns a height of {}: \
                 the taskbar rows would enter the frame",
                servi.1
            );
        }
    }
    // The most tempting case: a request ALREADY at the monitor's height. It
    // must be brought down to the work area, never served at 1080.
    assert!(size_for_viewport((1860, 1080), (1860, 1032)).1 <= 1032);
}

/// The `1.0` cap has its OWN property, distinct from the one above, and
/// it needs its own test — that is what the mutation revealed:
/// **we never serve an image larger than the request.** Without it, a
/// 900×500 viewport would be encoded at 1854×1030, that is four times the
/// macroblocks for pixels the page cannot display.
#[test]
fn a_request_smaller_than_the_bound_is_never_enlarged() {
    let borne = (1860, 1032);
    for demande in [(900u32, 500u32), (640, 480), (1280, 720)] {
        let servi = size_for_viewport(demande, borne);
        assert!(
            servi.0 <= demande.0 && servi.1 <= demande.1,
            "{demande:?} served {servi:?}: bigger than what the browser requested"
        );
    }
}
