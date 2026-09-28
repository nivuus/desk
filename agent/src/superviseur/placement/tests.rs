//! The host tests of `superviseur::placement`.
//!
//! **Extracted from `placement.rs` VERBATIM.** Batch 33 corrected there the
//! comment of `win::poser`, which had become wrong (it justified the absence of
//! `SW_MAXIMIZE` by an argument that only holds if the crop stays at the
//! output's size — which is no longer the case), and the file found itself
//! at **EXACTLY 500 lines**, that is, at its gate. The repository has
//! paid six times for "the regained margin treated as acquired" and nine times
//! for the #487 wreck: we extract rather than leave a file a hair's breadth
//! from the ceiling.
//!
//! ⚠️ **This extraction FOLLOWS its addition, whereas that of
//! `windows_source/sortie.rs` PRECEDED it** — and it is said rather than
//! disguised. The comment correction that made it necessary
//! arrived mid-batch, with the taskbar measurement; it
//! was not foreseeable when the ceiling was computed.
//!
//! No assertion, no comment was rewritten in the move. The
//! TWO modules (`tests` and `tests_size`) are kept distinct: they
//! were, and merging them would have been a rewrite.

use super::*;
use crate::geometry::Rect;

fn sortie(a: u32, s: u32, x: i32, l: u32, h: u32, attachee: bool) -> SortieDxgi {
    SortieDxgi {
        index_adaptateur: a,
        index_sortie: s,
        adaptateur: "NVIDIA".into(),
        nom_sortie: format!("\\\\.\\DISPLAY{s}"),
        attachee_au_bureau: attachee,
        rect: Rect {
            x,
            y: 0,
            width: l,
            height: h,
        },
    }
}

#[test]
fn finds_the_output_with_the_requested_dimensions() {
    // The first output must stay INADEQUATE under D10's inequality —
    // otherwise `.find()` would stop on it and the test would no longer prove
    // anything. Its height (800) is therefore below the requested viewport
    // (900), exactly as `n_apparie_pas_une_sortie_aux_mauvaises_
    // dimensions` stays inadequate by staying smaller on both
    // axes: see task 5's report, this data is not in
    // the brief as is, which made this test red as it stood.
    let all = vec![
        sortie(0, 0, 0, 2400, 800, true),
        sortie(0, 1, 2400, 1600, 900, true),
    ];
    let trouvee = sortie_pour_viewport(&all, 1600, 900, &[], None).unwrap();
    assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 1));
}

#[test]
fn ignores_an_unattached_output() {
    // An output created but not yet attached by Windows can
    // display nothing: taking it would give a black capture.
    let all = vec![sortie(0, 1, 2400, 1600, 900, false)];
    assert!(sortie_pour_viewport(&all, 1600, 900, &[], None).is_none());
}

#[test]
fn ignores_an_already_assigned_output() {
    // Two windows with the same viewport: without this filter, the second would
    // be assigned the first one's output, and both streams
    // would show the same image.
    let all = vec![
        sortie(0, 1, 2400, 1600, 900, true),
        sortie(0, 2, 4000, 1600, 900, true),
    ];
    let deja_prises = vec!["\\\\.\\DISPLAY1".to_string()];
    let trouvee = sortie_pour_viewport(&all, 1600, 900, &deja_prises, None).unwrap();
    assert_eq!((trouvee.index_adaptateur, trouvee.index_sortie), (0, 2));
}

#[test]
fn finds_nothing_when_all_are_taken() {
    let all = vec![sortie(0, 1, 2400, 1600, 900, true)];
    let deja_prises = vec!["\\\\.\\DISPLAY1".to_string()];
    assert!(sortie_pour_viewport(&all, 1600, 900, &deja_prises, None).is_none());
}

#[test]
fn does_not_pair_an_output_with_the_wrong_dimensions() {
    // ❌ **This comment said: "The DPI scale factor has already
    // produced a 1.5 gap on this ground (5120x1440 announced,
    // 3413x960 measured): an approximate pairing would make this trap
    // invisible." Sub-block D10 refuted it** — and the test
    // `a_scale_factor_is_now_cropped_not_refused`, twenty
    // lines below, now says the opposite. Pairing HAS
    // deliberately become approximate (inequality, plus a tolerance of
    // 4 px): a DPI gap is no longer a reason to refuse, it is cropped.
    // The protection migrated to `retained_size`, which bounds the window to
    // what the output can really carry. Found by the cross-cutting
    // review: it is the only comment of this file that the
    // `sortie_par_dimensions` → `sortie_pour_viewport` renaming left
    // intact without rereading it.
    //
    // What this test still exercises, and which stays right: an output
    // TOO SMALL on one axis is still not paired.
    let all = vec![sortie(0, 1, 2400, 1067, 600, true)];
    assert!(sortie_pour_viewport(&all, 1600, 900, &[], None).is_none());
}

/// §3.1 of acceptance run D1: an output created at 1280×713 was returned by
/// DXGI at 1280×720 once, then at 1280×713 the next try. Strict
/// equality then made opening the window impossible.
#[test]
fn a_gap_within_tolerance_still_matches() {
    let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1280, 717)];
    let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &[], None);
    assert_eq!(
        trouvee.map(|s| s.nom_sortie),
        Some("\\\\.\\DISPLAY7".into())
    );
}

/// The 1.5 DPI factor (5120×1440 announced by WMI, 3413×960 measured by
/// DXGI) is no longer a reason to REFUSE: a larger output is
/// cropped. What protected against it — putting the window on a
/// texture of the wrong dimensions — is now ensured by
/// `retained_size`, not by pairing.
#[test]
fn a_scale_factor_is_now_cropped_not_refused() {
    let sorties = vec![sortie_nommee("\\\\.\\DISPLAY7", 1920, 1080)];
    assert!(sortie_pour_viewport(&sorties, 1280, 720, &[], None).is_some());
    assert_eq!(retained_size((1280, 720), (1920, 1080)), (1280, 720));
}

#[test]
fn an_already_taken_output_is_ignored() {
    let sorties = vec![
        sortie_nommee("\\\\.\\DISPLAY7", 1280, 720),
        sortie_nommee("\\\\.\\DISPLAY8", 1280, 720),
    ];
    let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &["\\\\.\\DISPLAY7".to_string()], None);
    assert_eq!(
        trouvee.map(|s| s.nom_sortie),
        Some("\\\\.\\DISPLAY8".into())
    );
}

/// D9's product case: the output is born at 3840×2160 for a viewport of
/// 1280×720, and must now be paired.
#[test]
fn pairs_an_output_born_much_larger() {
    let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160)];
    let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &[], None);
    assert_eq!(
        trouvee.map(|s| s.nom_sortie),
        Some("\\\\.\\DISPLAY8".into())
    );
}

#[test]
fn does_not_pair_a_too_small_output() {
    let sorties = vec![sortie_nommee("\\\\.\\DISPLAY8", 1024, 576)];
    assert!(sortie_pour_viewport(&sorties, 1280, 720, &[], None).is_none());
}

/// The filter on ALREADY TAKEN outputs becomes more important, not
/// less: with an inequality, a single large output would suit
/// all windows, and all would show the same image.
#[test]
fn a_large_output_already_taken_is_not_reassigned() {
    let sorties = vec![
        sortie_nommee("\\\\.\\DISPLAY8", 3840, 2160),
        sortie_nommee("\\\\.\\DISPLAY9", 3840, 2160),
    ];
    let trouvee = sortie_pour_viewport(&sorties, 1280, 720, &["\\\\.\\DISPLAY8".to_string()], None);
    assert_eq!(
        trouvee.map(|s| s.nom_sortie),
        Some("\\\\.\\DISPLAY9".into())
    );
}

#[test]
fn always_ignores_an_unattached_output() {
    // An output Windows did not attach can display nothing:
    // taking it would give a black capture, whatever its size.
    let all = vec![sortie(0, 1, 2400, 3840, 2160, false)];
    assert!(sortie_pour_viewport(&all, 1280, 720, &[], None).is_none());
}

// Distinct from `sortie` above (which sets `nom_sortie` from
// `index_sortie`): the tests using it want an explicit name
// to check the identity of the paired output, not only its
// existence. *(They were three when this sentence was written; sub-block
// D10 added three more. The count is no longer given
// here — a number written in a comment drifts at the first
// addition, and this repository paid for it six times.)*
fn sortie_nommee(nom: &str, largeur: u32, hauteur: u32) -> SortieDxgi {
    SortieDxgi {
        index_adaptateur: 0,
        index_sortie: 0,
        adaptateur: "trial".into(),
        nom_sortie: nom.into(),
        attachee_au_bureau: true,
        rect: Rect {
            x: 0,
            y: 0,
            width: largeur,
            height: hauteur,
        },
    }
}

#[test]
fn a_window_in_its_place_is_not_placed_again() {
    let cible = Rect {
        x: 2400,
        y: 0,
        width: 1600,
        height: 900,
    };
    assert!(!doit_etre_replacee(&cible, &cible));
}

#[test]
fn a_window_moved_off_its_output_is_placed_again() {
    let cible = Rect {
        x: 2400,
        y: 0,
        width: 1600,
        height: 900,
    };
    let ailleurs = Rect {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };
    assert!(doit_etre_replacee(&ailleurs, &cible));
}

#[test]
fn a_window_resized_by_the_application_is_placed_again() {
    let cible = Rect {
        x: 2400,
        y: 0,
        width: 1600,
        height: 900,
    };
    let retaillee = Rect {
        x: 2400,
        y: 0,
        width: 800,
        height: 600,
    };
    assert!(doit_etre_replacee(&retaillee, &cible));
}

#[test]
fn a_one_pixel_gap_does_not_trigger_a_replacement() {
    // DWM's invisible borders commonly shift the rectangle
    // returned by `GetWindowRect` by one or two pixels. Without tolerance, the
    // supervisor would replace the window at each loop turn, in
    // a loop, and would steal focus indefinitely.
    let cible = Rect {
        x: 2400,
        y: 0,
        width: 1600,
        height: 900,
    };
    let presque = Rect {
        x: 2401,
        y: 1,
        width: 1599,
        height: 899,
    };
    assert!(!doit_etre_replacee(&presque, &cible));
}

mod tests_size {
    // ⚠️ `super::super::*` and not `super::*`: the ONLY change of the
    // move, and it is mechanical. This module stays nested (it was),
    // but its parent is no longer `placement` — it is the `tests` module this
    // file now IS. Without this one extra notch, `retained_size` and
    // `sortie_assez_grande` would not resolve.
    use super::super::*;

    /// D9's product fact: on this VM, outputs are born at 3840×2160
    /// because the registry stayed there. the size-compatibility check refused, and the
    /// product capped at three windows.
    #[test]
    fn an_output_born_too_large_now_fits() {
        assert!(sortie_assez_grande((3840, 2160), (1280, 720)));
    }

    #[test]
    fn an_output_born_too_small_does_not_fit() {
        assert!(!sortie_assez_grande((1024, 576), (1280, 720)));
    }

    /// D1's attach race (1280×713 returned as 1280×720) stays
    /// covered: four pixels of tolerance, like replacement.
    #[test]
    fn a_four_pixel_shortfall_stays_accepted() {
        assert!(sortie_assez_grande((1276, 716), (1280, 720)));
    }

    #[test]
    fn a_seven_pixel_shortfall_is_refused() {
        assert!(!sortie_assez_grande((1280, 713), (1280, 720)));
    }

    #[test]
    fn the_retained_size_crops_an_oversized_output() {
        assert_eq!(retained_size((1280, 720), (3840, 2160)), (1280, 720));
    }

    /// Born too small, the output is honoured at what it offers: the client
    /// scales. No case hands an output back to the driver for a
    /// question of size any more.
    #[test]
    fn the_retained_size_is_clamped_to_the_output_when_it_is_smaller() {
        assert_eq!(retained_size((1280, 720), (1024, 576)), (1024, 576));
    }

    /// The NV12 encoder requires even dimensions, and an output born at an
    /// odd size is a real case (odd viewport, D1).
    #[test]
    fn the_retained_size_is_always_even_and_never_zero() {
        assert_eq!(retained_size((1281, 721), (3840, 2160)), (1280, 720));
        assert_eq!(retained_size((0, 0), (1280, 720)), (2, 2));
    }

    /// Axes are bounded SEPARATELY: we crop, we do not
    /// scale, so there is no aspect ratio to preserve here —
    /// unlike `clamp_to_max_size`, which, for its part, resizes.
    #[test]
    fn the_two_axes_are_bounded_separately() {
        assert_eq!(retained_size((1920, 720), (1280, 2160)), (1280, 720));
    }
}

/// The DESIGNATED output, and the production defect it closes.
///
/// 🔴 **MEASURED ON THE PRODUCTION AGENT ON AUGUST 31ST, 2026, NOT DEDUCED.** The
/// log returned EIGHT times, in a loop, in the same run of the agent:
///
/// ```text
/// ERROR no candidate output can serve this viewport
///   demande="1614x1080" designee="\\.\DISPLAY6"
///   candidates=["\\.\DISPLAY6 1428x1080"]
/// ```
///
/// The output was **ours, certified by CCD** (non-empty `designee`), and
/// it was refused on its size alone: the SudoVDA driver does not create
/// the output at the requested size. **The same request, the same
/// run, two sizes depending on the time** — 1860×1080 at 12:14Z (served),
/// 1428×1080 at 20:46Z (refused). Create → refuse → destroy, and NO
/// window showed any more, whatever the application.
///
/// It is the unknown open since D8 ("an output is NOT born at the requested
/// size"), which batch 33 turned into a total failure by making the
/// request follow the browser's viewport: a wide window requests more than
/// what the driver returns.
///
/// **The rule set here**: when designation has POSITIVELY named
/// our output, its size is no longer a REFUSAL criterion — it is a
/// CONSTRAINT, and `windows_source_sortie::size_for_viewport` already knows how to
/// fit the window to it with the aspect ratio preserved. Refusing meant refusing the
/// only output we could have served.
mod sortie_designee {
    use super::*;

    const NOTRE: &str = "\\\\.\\DISPLAY6";

    /// The production case, to the byte.
    #[test]
    fn a_designated_output_smaller_than_the_viewport_is_served() {
        let all = vec![sortie(0, 6, 1280, 1428, 1080, true)];
        let trouvee = sortie_pour_viewport(&all, 1614, 1080, &[], Some(NOTRE));
        assert_eq!(
            trouvee.map(|s| s.nom_sortie),
            Some(NOTRE.to_string()),
            "the output CCD named as ours cannot be refused \
             on its size: it is the only one we can serve"
        );
    }

    /// 🔴 **THE GUARD THAT REMAINS, AND WITHOUT WHICH THIS FIX WOULD BE A
    /// REGRESSION**: two windows would show the same image. Size
    /// stops being a criterion; `deja_prises` does not stop being one.
    #[test]
    fn an_already_taken_designated_output_stays_refused() {
        let all = vec![sortie(0, 6, 1280, 1428, 1080, true)];
        let prises = vec![NOTRE.to_string()];
        assert!(sortie_pour_viewport(&all, 1614, 1080, &prises, Some(NOTRE)).is_none());
    }

    /// An output Windows has not attached yet stays unusable,
    /// designated or not: the capture would have nothing to duplicate.
    #[test]
    fn an_unattached_designated_output_stays_refused() {
        let all = vec![sortie(0, 6, 1280, 1428, 1080, false)];
        assert!(sortie_pour_viewport(&all, 1614, 1080, &[], Some(NOTRE)).is_none());
    }

    /// 🔴 **WHAT THE RELAXATION DOES NOT TOUCH.** Without designation, the
    /// product from yesterday holds line for line — it is what prevents the
    /// set-difference fallback from choosing a pre-existing PHYSICAL screen,
    /// and it is the negative witness of the previous test.
    #[test]
    fn without_designation_a_too_small_output_stays_refused() {
        let all = vec![sortie(0, 6, 1280, 1428, 1080, true)];
        assert!(sortie_pour_viewport(&all, 1614, 1080, &[], None).is_none());
    }

    /// A designation naming ANOTHER output relaxes nothing on
    /// this one: the exemption is by name, never global.
    #[test]
    fn the_exemption_only_holds_for_the_named_output() {
        let all = vec![sortie(0, 6, 1280, 1428, 1080, true)];
        let autre = "\\\\.\\DISPLAY7";
        assert!(sortie_pour_viewport(&all, 1614, 1080, &[], Some(autre)).is_none());
    }
}
