//! The PERSISTENCE check (task 2bis, sub-block D9): does the mode
//! change that made the output UNDER TEST move survive the creation of an
//! additional virtual output — the most mundane event of
//! production, opening one more window?
//!
//! **What task 2 already had before its eyes without naming it.** Its verdict
//! "ACCEPTE" only bore on the TRIGGERING of the movement (a `WARN`
//! later fixed); the return of `\\.\DISPLAY5` to its creation size,
//! between the "after the round" line and the "after creation (control)" line,
//! was only readable by cross-checking two topology blocks ten lines
//! apart — it is this manual cross-checking that made it missed in the first
//! report. Task 2 carried A SINGLE exercised combination
//! (`CDS_TYPE(0)`, dynamic and not persisted by construction): this
//! task answers for `CDS_UPDATEREGISTRY`, the one that persists by
//! construction, by restricting the round to that single arm
//! (`combinaisons::combos_du_tour`).
//!
//! Extracted separately rather than poured into `mode_sortie.rs`: that file is at
//! 496 lines for a ceiling of 500 (`CLAUDE.md`), a margin of 4 — any
//! addition there calls for an extraction, not a compression.

use crate::sortie_dxgi::SortieDxgi;
use crate::survie_verdict::verdict_persistance;

/// Restricts the round to ONE combination, designated by its exact label
/// (`MULTIFENETRE_MODE_SORTIE_DRAPEAUX`). Without it, the current behaviour
/// is unchanged: all four combinations, in order.
///
/// **Why**: `CDS_TYPE(0)` succeeds on the first attempt, which leaves the
/// three following combinations unexercised — including `CDS_UPDATEREGISTRY`,
/// the only one that persists by construction. Without this selector, the question
/// "does the PERSISTENT change, for its part, survive the creation of an output?"
/// cannot be reached.
pub(super) fn combinaison_imposee() -> Option<String> {
    std::env::var("MULTIFENETRE_MODE_SORTIE_DRAPEAUX").ok()
}

/// Logs the PERSISTENCE verdict as a named event, rather than
/// leaving it reconstructible only by cross-checking two topology
/// surveys ten lines apart (see the module's header comment).
///
/// `combinaison_imposee`: the RAW value of `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`,
/// which the operator requested -- NOT necessarily the one that won (`combinaison_gagnante`).
/// ⚠️ **Fix (review of task 2bis, minor points)**: the first version
/// logged ONLY the winning arm, so that a round left EMPTY because
/// the imposed label matched nothing (mojibake, see the
/// module's header comment) displayed `combinaison=None` -- identical
/// to the absence of any constraint. The two distinct fields make it possible to
/// read back directly WHAT the operator requested and WHAT
/// really happened.
///
/// `before_creation`: the size of the output under test surveyed by
/// `temoin::nom_apres_tour`, AFTER the round, duplication still held — not
/// an assumption about what the round just set. `None` if the output had
/// vanished at that moment (`<disparue>`, see `nom_apres_tour`).
///
/// `apres_creation_releve`: the topology read back AFTER the control output
/// (a NEW virtual output) was created; `nom_cible` is looked up in it
/// by name, never by position -- a constant doctrine of this module.
///
/// ⚠️ **Fix (review of task 2bis, Important I4)**: `survit` is
/// NO LONGER `true` when the output vanished on one side or the other. The
/// comparison itself is delegated to `survie_verdict::verdict_persistance`
/// since fix no. 14 of the second review — a PURE predicate, taken out of
/// this `#[cfg(windows)]` tree to be testable on the host, see its
/// header comment for the exact defect it prevents from coming back.
///
/// The comparison itself, when both surveys exist, bears on two
/// sizes obtained by the SAME DXGI read-back (`GetDesc`/`DesktopCoordinates`)
/// as the rest of the probe: never the return code of
/// `ChangeDisplaySettingsExW`, measured elsewhere returning `0` on an output that
/// did not move by one pixel.
pub(super) fn journaliser_verdict(
    combinaison_imposee: Option<&str>,
    combinaison_gagnante: Option<&str>,
    nom_cible: &str,
    before_creation: Option<(u32, u32)>,
    apres_creation_releve: &[SortieDxgi],
) {
    let apres_creation = apres_creation_releve
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_cible)
        .map(|sortie| (sortie.rect.width, sortie.rect.height));

    let (before_measurable, before_w, before_h) = match before_creation {
        Some((l, h)) => (true, l, h),
        None => (false, 0, 0),
    };
    let (apres_mesurable, apres_l, apres_h) = match apres_creation {
        Some((l, h)) => (true, l, h),
        None => (false, 0, 0),
    };
    let survit = verdict_persistance(before_creation, apres_creation);

    tracing::info!(
        combinaison_imposee = ?combinaison_imposee,
        combinaison_gagnante = ?combinaison_gagnante,
        before_creation_measurable = before_measurable,
        before_creation_w = before_w,
        before_creation_h = before_h,
        apres_creation_mesurable = apres_mesurable,
        apres_creation_l = apres_l,
        apres_creation_h = apres_h,
        survit,
        "PERSISTENCE: does the mode change survive the creation of an output?"
    );
}
