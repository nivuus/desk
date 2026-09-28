//! Among which DXGI outputs the supervisor is allowed to pair a
//! window — and why the question does not reduce to "which one just
//! appeared".
//!
//! 🔴 **THE DEFECT THIS MODULE EXISTS TO CLOSE (batch 30, measured on the
//! VM).** Without QEMU's VGA, session 1 starts with zero PnP monitors
//! but ONE display (`\\.\DISPLAY5`), whose active target carries
//! `statusFlags = 0x11` (`IN_USE | FORCED_AVAILABILITY_SYSTEM`): a **forced
//! target**, which Windows fabricates when it has no display left. The
//! first virtual monitor created **REPLACES this forced target on the SAME
//! source** — it therefore inherits the same GDI name, and never "appears". The
//! product only pairing among outputs that APPEARED, it refused the window
//! ("no display output can serve this window") and handed back to the
//! driver a perfectly usable output. The loop never reached
//! the second window.
//!
//! ⚠️ **The hypothesis "the polluted registry prevents attachment" was
//! REFUTED** by the same batch: intact registry, 4 outputs created → 4
//! attached. It is not a limit of Windows nor of SudoVDA, it is a
//! code defect.
//!
//! **The rule, in two paths and in this order.** ① DESIGNATE the output by
//! what we gave the driver (see `moniteurs_virtuels::config_affichage`);
//! ② only failing that, the historical FALLBACK — the set difference.
//!
//! 🔴 **THE FALLBACK IS NOT DEAD AND MUST NOT BE REMOVED.** It runs as soon
//! as designation returns nothing: driver without a known adapter, mute or failing
//! CCD, target not yet in an active path, ambiguous pair. It is
//! what guarantees that a wrong hypothesis about `identifiant_cible` — which
//! `sudovda.rs` itself declares "not confirmed" — degrades to the
//! KNOWN behaviour instead of breaking. Without it, the chosen path would
//! not have been shippable before being measured on the VM.
//!
//! Outside `#[cfg(windows)]`, in its own file rather than in
//! `placement.rs`: the latter was at 441 lines, and putting it there would have
//! brought it to ~496 — the margin this repository measured being lost again six times, once
//! the very same day in the branch that had gained it.

use std::sync::OnceLock;

use crate::sortie_dxgi::SortieDxgi;

/// Is path ① (DESIGNATE) armed?
///
/// **`SORTIE_DESIGNEE=0` DISARMS; mere PRESENCE does not enable** —
/// convention of `PLEIN_ECRAN`, `AUDIO`, `SUPERVISEUR`, `CAPTEUR`,
/// `PART_SONDAGE`, `PRESSE_PAPIER`, `APPS` and `PONT_ECRITURE`, and for the same
/// reason: testing `is_ok()` would arm the mechanism for whoever writes
/// `SORTIE_DESIGNEE=0` to cut it.
///
/// 🔴 **BENCH VARIABLE, NEVER A SHIPPED CONFIGURATION**, same status as
/// `PART_SONDAGE`, `PONT_ECRITURE`, `PONT_CACHE` and `PRESSE_PAPIER_GARDE`.
/// Disarmed, it gives **exactly the product from before batch 32**: it is the
/// acceptance run's RED arm, and it exists because "the red arm is the
/// earlier binary" is not reproducible — in three weeks that binary
/// no longer exists.
///
/// **The predicate is REUSED, not copied**: `crate::apps::desarme` already carries
/// exactly this argument, and the precedent is `ICONES` (G2) then
/// `APPS_SURVEILLANCE` (G4).
///
/// ⚠️ **Forced at supervisor STARTUP, not at the first pairing** —
/// else the trace would appear only with the first window, i.e. after the
/// first steps of a short acceptance run. `PONT_MESURE` taught this
/// lesson in F4.
pub fn armee() -> bool {
    static ARMEE: OnceLock<bool> = OnceLock::new();
    *ARMEE.get_or_init(|| {
        let armee = !crate::apps::desarme(std::env::var("SORTIE_DESIGNEE").ok().as_deref());
        // Emitted ONLY when disarmed: an unconditional trace would suggest
        // an arming to someone who has none.
        //
        // 🔴 **IT PROVES THE VARIABLE REACHED THE PROCESS, NEVER THAT
        // THE MECHANISM IS OFF** — P1's lesson on `PRESSE_PAPIER=0`. What
        // discriminates is the EMPTY `designee` field of the refusal, and the window
        // not served.
        if !armee {
            tracing::warn!(
                "output designation DISARMED (SORTIE_DESIGNEE=0): bench arm, never a shipped configuration"
            );
        }
        armee
    })
}

/// The outputs among which `placement::sortie_pour_viewport` is allowed
/// to choose.
///
/// `notre_nom` is what designation returned, `None` if it returned
/// nothing — and **`None` IS the product from before batch 32, line for line**.
/// It is this property that makes the first test's red playable without having
/// to keep the old binary.
///
/// ⚠️ **What this function does NOT do, on purpose: filter on size
/// or on `deja_prises`.** Those two filters stay in
/// `placement::sortie_pour_viewport`.
///
/// ❌ **THIS DOC SAID "designation narrows the set of candidates,
/// it loosens no guard — that is the whole difference with a
/// relaxation of the pairing rule" UNTIL AUGUST 31ST, 2026, AND IT
/// BECAME WRONG.** The designated name is now passed to
/// `placement::sortie_pour_viewport`, which **exempts that output from the SIZE
/// criterion**: the driver does not create the output at the requested size
/// (measured in production, 1614×1080 requested, 1428×1080 returned, eight refusals in
/// a loop and no window served any more). Designation still narrows
/// the set; it now loosens **one** guard, by name.
///
/// **`attachee_au_bureau` and `deja_prises` still run on the designated
/// output**, and the latter is what prevents two windows from showing the
/// same image — see the doc of `placement::sortie_pour_viewport`, which carries
/// the measurement and the reasoning.
pub fn candidates(
    all: &[SortieDxgi],
    notre_nom: Option<&str>,
    before: &[String],
) -> Vec<SortieDxgi> {
    // ① DESIGNATE — by what we GAVE the driver, never by what changed
    // around. Insensitive to the replacement of a forced target, and incidentally
    // to any race: an output created by a third party (Apollo also drives the
    // display configuration of this VM) can no longer be taken for
    // ours, which the set difference does not guarantee.
    if let Some(nom) = notre_nom {
        if let Some(notre) = all
            .iter()
            .find(|s| s.attachee_au_bureau && s.nom_sortie == nom)
        {
            return vec![notre.clone()];
        }
    }

    // ② FALLBACK — yesterday's product, word for word. See the module header:
    // it is not dead, it is what makes a wrong hypothesis harmless.
    all.iter()
        .filter(|s| s.attachee_au_bureau && !before.contains(&s.nom_sortie))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    const NOTRE: &str = "\\\\.\\DISPLAY5";

    fn sortie(nom: &str, largeur: u32, hauteur: u32) -> SortieDxgi {
        SortieDxgi {
            index_adaptateur: 0,
            index_sortie: 0,
            adaptateur: "SudoVDA".into(),
            nom_sortie: nom.to_string(),
            attachee_au_bureau: true,
            rect: Rect {
                x: 0,
                y: 0,
                width: largeur,
                height: hauteur,
            },
        }
    }

    /// Batch 30's setup, to the letter: the FORCED target already carried
    /// `\\.\DISPLAY5`, and our new output replaced it on the same source,
    /// hence under the SAME name.
    fn montage_du_lot_30() -> (Vec<SortieDxgi>, Vec<String>) {
        (vec![sortie(NOTRE, 1860, 1080)], vec![NOTRE.to_string()])
    }

    #[test]
    fn the_output_replacing_a_forced_target_is_retained() {
        let (all, before) = montage_du_lot_30();
        assert_eq!(candidates(&all, Some(NOTRE), &before).len(), 1);
    }

    /// 🔴 **THE RED, and it lives in ITS OWN test.** Two assertions in
    /// one test only prove the first — `assert` stops at the first
    /// failure, and this repository paid for an assertion in second position
    /// being exercised by nothing.
    ///
    /// Passing `None` **IS** the product from before batch 32: it had no
    /// `notre_nom` parameter and always ran the fallback. This test is therefore the
    /// NEGATIVE WITNESS that makes the previous one interpretable — it shows, in the
    /// same survey, that the setup is indeed the one that failed, and not a
    /// setup where everything would have passed anyway.
    #[test]
    fn the_product_before_batch_32_returns_an_empty_set_on_this_same_setup() {
        let (all, before) = montage_du_lot_30();
        assert!(
            candidates(&all, None, &before).is_empty(),
            "the set difference returns an EMPTY vector on a perfectly \
             usable output — this is the defect measured by batch 30"
        );
    }

    /// Designation does not short-circuit the TAKEN guard — and it is
    /// the one that matters, since the size criterion is exempted
    /// for the designated output (August 31st, 2026). Without this test, the exemption
    /// could have silently extended to `deja_prises`, and two windows
    /// would have shown the same image.
    #[test]
    fn the_designation_does_not_bypass_the_plug_filter() {
        let (all, before) = montage_du_lot_30();
        let candidates = candidates(&all, Some(NOTRE), &before);
        assert!(
            crate::superviseur::placement::sortie_pour_viewport(
                &candidates,
                1860,
                1080,
                &[NOTRE.to_string()],
                None,
            )
            .is_none(),
            "an output already assigned to a live session stays refused"
        );
    }

    /// What the FALLBACK still protects, and what the "relax the rule" path
    /// would have loosened: a pre-existing PHYSICAL screen must never be
    /// chosen. The inequality of `sortie_assez_grande` (D10) makes this risk larger,
    /// not smaller — a 4K monitor fits any viewport.
    #[test]
    fn without_designation_a_pre_existing_screen_stays_refused() {
        let physique = "\\\\.\\DISPLAY1";
        let all = vec![sortie(physique, 3840, 2160)];
        let before = vec![physique.to_string()];
        assert!(candidates(&all, None, &before).is_empty());
    }

    /// A DETACHED output cannot be designated: the name can be exact
    /// and the output unusable. Without this guard, the polling would return
    /// control on an output Windows has not attached yet, and
    /// `sortie_pour_viewport` would refuse — while consuming the wait.
    /// The `SORTIE_DESIGNEE` predicate, exercised on the PREDICATE and not on the
    /// variable: `armee()` carries a `OnceLock` that two tests of the same
    /// process could not reset, and a test setting an
    /// environment variable would poison its neighbours.
    #[test]
    fn only_zero_disarms_the_designation() {
        assert!(crate::apps::desarme(Some("0")));
        for value in [None, Some(""), Some("1"), Some("0 "), Some("oui")] {
            assert!(!crate::apps::desarme(value), "{value:?} must NOT disarm");
        }
    }

    #[test]
    fn a_detached_output_is_not_designated() {
        let mut detachee = sortie(NOTRE, 1860, 1080);
        detachee.attachee_au_bureau = false;
        let before = vec![NOTRE.to_string()];
        assert!(candidates(&[detachee], Some(NOTRE), &before).is_empty());
    }
}
