//! The `CDS_*` flag combinations the P1 probe tries, from least to
//! most insistent, and the mechanism that applies them.
//!
//! Extracted from `mode_sortie.rs` at task 1 of sub-block D9: the fourth
//! combination (and its justification comment, which must stay next to
//! it) pushed the parent file above the 500-line ceiling
//! (`CLAUDE.md`). None of these functions touches a private field of
//! `mode_sortie` — they take the output name and the dimensions as
//! parameters — so no access reason required keeping them in the
//! parent file.

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    ChangeDisplaySettingsExW, CDS_NORESET, CDS_RESET, CDS_TYPE, CDS_UPDATEREGISTRY, DEVMODEW,
    DM_PELSHEIGHT, DM_PELSWIDTH,
};

/// A `CDS_*` flag combination to try, from least to most insistent.
/// `CDS_SET_PRIMARY` is deliberately absent from both variants: we do not
/// touch the primary monitor (brief, step 7 of D8).
pub(super) enum Combo {
    /// Un seul appel `ChangeDisplaySettingsExW`, drapeaux directs.
    Simple(&'static str, CDS_TYPE),
    /// The standard multi-screen idiom of the Win32 API: the targeted output is
    /// modified with `CDS_NORESET` (the change is deferred and not
    /// applied), then a second call — without a device name, without
    /// `DEVMODE` — applies everything pending with `CDS_RESET` alone.
    NoresetPuisReset(&'static str),
}

impl Combo {
    pub(super) fn etiquette(&self) -> &'static str {
        match self {
            Combo::Simple(etiquette, _) | Combo::NoresetPuisReset(etiquette) => etiquette,
        }
    }
}

/// The four arms, in increasing order of insistence.
///
/// Rebuilt at each call rather than memoised: `CDS_TYPE` and
/// `&'static str` are `Copy`, rebuilding costs nothing, and it avoids
/// a state shared between the main round and the control (`combo_pour_temoin`),
/// which needs its own copy to extract ONE arm from it without borrowing
/// the main round's.
pub(super) fn combos() -> [Combo; 4] {
    [
        // `CDS_TYPE(0)` — DYNAMIC change, not written to the registry. It was
        // the candidate remedy for the registry pollution that D8 had noted
        // ("C1" in its results document): the product then wrote
        // `CDS_UPDATEREGISTRY` at each successful full screen, and an output is BORN
        // at the last size left in the registry (chain
        // `before(N) = after(N-1)`, task 3bis of D8) — so that it would have
        // blocked its own later window openings. The three
        // combinations tested by D8 ALL carried `CDS_UPDATEREGISTRY`:
        // this arm had never been tried. ⚠️ Sub-block D9 measured that
        // even this dynamic arm was not enough — the change does not survive
        // the opening of one more output — and removed the whole
        // mechanism rather than fixing it (see `capteur/plein_ecran.rs`):
        // this combination stays here for the value of its measurement, not as
        // a remedy used by the product.
        Combo::Simple("no flag (dynamic, not persisted)", CDS_TYPE(0)),
        Combo::Simple("CDS_UPDATEREGISTRY alone", CDS_UPDATEREGISTRY),
        Combo::Simple(
            "CDS_UPDATEREGISTRY | CDS_RESET",
            CDS_UPDATEREGISTRY | CDS_RESET,
        ),
        Combo::NoresetPuisReset(
            "CDS_UPDATEREGISTRY|CDS_NORESET then CDS_RESET alone (multi-screen idiom)",
        ),
    ]
}

/// The combos to try THIS round: all four, in order, unless `imposee`
/// restricts the round to a single one (task 2bis, sub-block D9,
/// `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`) — since `CDS_TYPE(0)` ALWAYS succeeds
/// on the first attempt (survey of task 2), the three other arms, including
/// `CDS_UPDATEREGISTRY`, are otherwise never exercised.
///
/// `imposee` must match EXACTLY one of the four labels of
/// `combos()`; otherwise the round tries nothing (empty vector), rather than a silent
/// fallback to all four that would mask a misspelled label —
/// hence the warning if the filter retains nothing.
pub(super) fn combos_du_tour(imposee: Option<&str>) -> Vec<Combo> {
    let all = combos();
    let Some(etiquette) = imposee else {
        return all.into_iter().collect();
    };
    let filtrees: Vec<Combo> = all
        .into_iter()
        .filter(|combo| combo.etiquette() == etiquette)
        .collect();
    if filtrees.is_empty() {
        tracing::warn!(
            etiquette_demandee = etiquette,
            etiquettes_connues = ?combos().map(|c| c.etiquette()),
            "MULTIFENETRE_MODE_SORTIE_DRAPEAUX matches NO known combination -- \
             the round will try nothing"
        );
    }
    filtrees
}

/// The arm to replay for the control (step 3, D9 brief): the SAME one that won
/// the main round, or the first if none won. Returns an owned
/// value — not a reference into the main round's array, which has already
/// ended its life at the place where the control runs.
pub(super) fn combo_pour_temoin(gagnante: Option<&'static str>) -> Combo {
    match gagnante {
        Some(etiquette) => combos()
            .into_iter()
            .find(|combo| combo.etiquette() == etiquette)
            // Cannot happen: `etiquette` necessarily comes from an
            // earlier call to `combos()`, which always returns the same
            // four `&'static str` literals. Fallback to the first arm
            // rather than an `expect`: a degraded control is better than a
            // probe that panics on its own fallback.
            .unwrap_or_else(|| combos().into_iter().next().unwrap()),
        None => combos().into_iter().next().unwrap(),
    }
}

/// Builds the `DEVMODEW` targeting `largeur`×`hauteur` and attempts the
/// change with the given flags. Returns the raw code of
/// `ChangeDisplaySettingsExW` — a `DISP_CHANGE_SUCCESSFUL` here proves
/// nothing by itself, see the header comment of `mode_sortie.rs`.
fn changer_mode(nom_sortie: &str, largeur: u32, hauteur: u32, drapeaux: CDS_TYPE) -> i32 {
    let nom: Vec<u16> = nom_sortie
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        dmFields: DM_PELSWIDTH | DM_PELSHEIGHT,
        dmPelsWidth: largeur,
        dmPelsHeight: hauteur,
        ..Default::default()
    };
    unsafe {
        ChangeDisplaySettingsExW(
            PCWSTR(nom.as_ptr()),
            Some(&dm as *const DEVMODEW),
            None,
            drapeaux,
            None,
        )
        .0
    }
}

/// Applies a combination and returns the code that counts for the verdict — the
/// second call for `NoresetPuisReset`, since it is the one that actually
/// applies the change deferred by the first.
pub(super) fn appliquer_combo(nom_sortie: &str, largeur: u32, hauteur: u32, combo: &Combo) -> i32 {
    match combo {
        Combo::Simple(etiquette, drapeaux) => {
            let code = changer_mode(nom_sortie, largeur, hauteur, *drapeaux);
            tracing::info!(etiquette, code, "flag combination tried (single call)");
            code
        }
        Combo::NoresetPuisReset(etiquette) => {
            let premier = changer_mode(
                nom_sortie,
                largeur,
                hauteur,
                CDS_UPDATEREGISTRY | CDS_NORESET,
            );
            // Second call: NULL device name, NULL DEVMODE — that is
            // how Win32 documents the grouped application of
            // changes deferred by CDS_NORESET.
            let second =
                unsafe { ChangeDisplaySettingsExW(PCWSTR::null(), None, None, CDS_RESET, None).0 };
            tracing::info!(
                etiquette,
                code_premier_appel = premier,
                code_second_appel = second,
                "flag combination tried (two calls: NORESET then RESET alone)"
            );
            second
        }
    }
}
