//! The PURE PERSISTENCE predicate (task 2bis, sub-block D9, coordinator's
//! review, fix no. 14): do two measurements, each potentially absent,
//! say the same size?
//!
//! Extracted from `diagnostics::multifenetre::mode_sortie::persistance`, which
//! stays `#[cfg(windows)]` (through the `#[cfg(windows)]` set on `mod multifenetre`
//! in `diagnostics.rs`, which gates everything living below it — a pure predicate
//! nested deeper in this tree would therefore never compile on the host,
//! whatever its own attributes). This predicate touches no
//! Windows type — only `Option<(u32, u32)>` — and CAN therefore
//! compile and be tested on the Linux host: same precedent as
//! `geometry.rs` and `sortie_dxgi.rs`, extracted for the same reason.
//!
//! **Bare root, and not `#[path]` under a parent** (convention settled in
//! task 17, sub-block D10, see `docs/claude/module-conventions.md` §"Child module
//! convention"): a module whose name is written `<parent>_<child>`
//! (`capture_reprise`, `windows_source_sortie`, `windows_source_telemetrie`)
//! stays physically under that parent and is hoisted through
//! `#[path]` in `main.rs`. A module whose name is understood WITHOUT
//! prefixing a parent — the case here, `survie_verdict` carries the name
//! of no top-level module — lives at the bare root, like
//! `geometry.rs` and `sortie_dxgi.rs`. The depth it is extracted from
//! (`diagnostics::multifenetre::mode_sortie::persistance`, four levels)
//! changes nothing: there is in any case no short and
//! unique parent name to prefix.
//!
//! It is the very predicate that carried Important I4 of the review of
//! task 2bis: the old version folded a size not found onto
//! `(0, 0)`, so that an output gone BEFORE and AFTER returned
//! `(0, 0) == (0, 0)` → `survit=true` — the instrument announced "the
//! change survived" exactly when the output had vanished.
//! Isolated here, this defect would have been visible in a test from the first
//! try — the repository's doctrine: a check never seen RED is not
//! one.

/// Returns `"true"` or `"false"` when BOTH measurements exist and can
/// therefore be compared, `"undetermined (output vanished)"` as soon as EITHER
/// one is missing — never a comparison on a numeric sentinel
/// that could be confused with a success.
pub(crate) fn verdict_persistance(
    before: Option<(u32, u32)>,
    apres: Option<(u32, u32)>,
) -> &'static str {
    match (before, apres) {
        (Some(a), Some(b)) if a == b => "true",
        (Some(_), Some(_)) => "false",
        _ => "undetermined (output vanished)",
    }
}

#[cfg(test)]
mod tests {
    use super::verdict_persistance;

    #[test]
    fn two_equal_measurements_return_true() {
        assert_eq!(
            verdict_persistance(Some((1920, 1080)), Some((1920, 1080))),
            "true"
        );
    }

    #[test]
    fn two_different_measurements_return_false() {
        assert_eq!(
            verdict_persistance(Some((1920, 1080)), Some((1280, 720))),
            "false"
        );
    }

    /// The case the old version confused with `"true"`: BOTH
    /// measurements absent (output gone before AND after) must NEVER
    /// read as a survival -- nor as a comparison on `(0, 0)`.
    #[test]
    fn a_measure_missing_on_each_side_returns_undetermined() {
        assert_eq!(
            verdict_persistance(None, Some((1280, 720))),
            "undetermined (output vanished)"
        );
        assert_eq!(
            verdict_persistance(Some((1280, 720)), None),
            "undetermined (output vanished)"
        );
        assert_eq!(
            verdict_persistance(None, None),
            "undetermined (output vanished)"
        );
    }
}
