//! N virtual outputs, one window and one DXGI duplication each.
//!
//! **This is the arrangement the recommended path of work stream D actually
//! proposes**, and that nothing had exercised: the probe's bench and measurement
//! ③ both placed N windows on ONE output. DXGI only allowing one
//! duplication per output, N duplications side by side was an open question,
//! not an implementation detail.
//!
//! ✅ **It no longer is, and it is this module that closed it** (31 July
//! 2026): the path is **accepted** — 90.1 fps per window in capture+encoding
//! up to N=8, zero wrong verdicts, eight duplications open side by side
//! (`docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`). policy: allow-fr (file path)
//! **One run per rank, hence no rate**, and nothing beyond 8 outputs.
//!
//! Second difference from everything before: each output is 1280×720, so
//! **the total area grows with N**. The probe's frame rates were taken at
//! fixed total area (`disposition::tuiles` splits a desktop), where the pixel
//! throughput is quasi constant by construction and where the number of windows
//! is not proven neutral in itself.
//!
//! No covering here — one window per output, nothing can hide
//! another — hence no elimination gate. The risk is the pairing:
//! that path *i* captures output *j*, or black. That is what the rotation
//! of the check (`mire::voie_controlee`) detects, with one reading per round.
//!
//! # Trap: a crash here leaves up to EIGHT orphaned outputs
//!
//! This probe creates up to 8 virtual outputs from a pool that only counts
//! **10** (measured ceiling, `montee.rs`). The `moniteurs_virtuels::
//! Sorties` guard destroys them when going out of scope, including during the unwinding
//! of a panic — but **not on a crash of the process**: these APIs
//! fail with `0xc0000005`, and the encoding pass of the `duplication` path
//! had precisely this defect until its fix on 31 July 2026, by
//! this very work stream (`capture_virtuelle.rs`, section "The trap this
//! probe revealed"; fix in `encode::arret`). A crash would leave
//! **8 of the 10 outputs** behind it, and the next run would fail to
//! create 8 without the cause being readable.
//!
//! Recovery: `MULTIFENETRE_VDD_PURGE=1`, tested on exactly this state.
//! Check the state BEFORE concluding anything from a creation refusal,
//! and from a fresh process (`MULTIFENETRE_DXGI=1`).
//!
//! # Split
//!
//! This file is *the output driver*: creation, designation, survival
//! check, restoration of the initial state. *The pass loop* — opening
//! of the N duplications, cadence, encoding — lives in `paralleles/passes.rs`.

mod passes;

use std::collections::HashSet;

use anyhow::{Context, Result};

use super::compteurs;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::capture::SortieDxgi;
use crate::mire;

pub(super) fn mesurer(count: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&count),
        "MULTIFENETRE_VDD_PARALLELE must be 1 to {}",
        mire::MIRES_MAX
    );

    let before = relever_topologie("before creation")?;
    let names_before = noms_attaches(&before);
    let connues: HashSet<String> = before
        .iter()
        .map(|sortie| sortie.nom_sortie.clone())
        .collect();

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope of the guard: the outputs must be destroyed BEFORE
    // the final survey, otherwise the latter would describe a transient state.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=count {
            let id = sorties
                .create(largeur, hauteur, hertz)
                .with_context(|| format!("creating virtual output no. {rang}"))?;
            tracing::info!(rang, id, "virtual output created");
        }
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("after creation")?;
        let virtuelles = designer_sorties_neuves(&apres, &connues, count)?;
        // Built right after `attendre_en_pinguant`, hence right after the
        // last known ping. The seam between the two is NOT counted in
        // the maximum interval — `Garde::new` sets its origin at its own
        // construction —, it is made negligible by the adjacency of the two
        // calls: 66 µs in the survey. See `compteurs::Garde::new`.
        let mut garde = compteurs::Garde::new(&pilote);
        garde.battre()?;
        let issue = passes::executer_passes(&mut garde, &virtuelles);
        // The figure that tells whether the watchdog was beaten without a gap over
        // the whole measurement. Logged even when the passes fail: it is
        // precisely when a measurement goes wrong that one must know whether an
        // output could have been taken back under it.
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "watchdog: largest gap between two beats over the whole measurement"
        );
        // Last finding, that of the ERROR PATH: `executer_passes`
        // already checks survival after each of its passes, but a `?`
        // exits it without going through these checks. This one runs whatever
        // happens, and it is the only one covering an abandonment midway.
        constater_survie("summary", &virtuelles);
        issue
    };

    // Second attempt at the removals the guard did not obtain: last chance
    // of THIS process, beyond that only the inter-process purge will reach them.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(
            rejoues,
            "due removals replayed successfully after the guard"
        );
    }

    // A virtual output outlives the process. This check remains that of the
    // measuring process, hence judge and party — the check that counts is a
    // `MULTIFENETRE_DXGI=1` survey from a fresh process, afterwards.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("after destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == names_before {
        tracing::info!(noms = ?noms_final, "initial state restored — same outputs, by name");
    } else {
        tracing::error!(
            names_before = ?names_before,
            noms_apres = ?noms_final,
            "the topology did NOT return to its initial state — purge required"
        );
    }

    issue
}

/// Finds the `count` outputs this probe has just created, by
/// DIFFERENCE OF SETS OF NAMES.
///
/// Not by index: DXGI renumbers its outputs at each topology
/// reconfiguration. Not by cardinality: Apollo drives the display configuration of
/// this VM and can add an output at any moment — an external addition
/// would exactly compensate a removal, and a check by number would pass
/// while an output has disappeared. The cardinality is only tested AFTER the
/// difference of names, never in its place.
///
/// `pub(super)`: the `reprise.rs` bench creates exactly the same starting
/// set-up, and a second designation written next to this one would diverge —
/// it is here that the two refusals live (external addition, non-attached output)
/// that make a measurement unattributable, and they must hold for both
/// benches.
pub(super) fn designer_sorties_neuves(
    apres: &[SortieDxgi],
    connues: &HashSet<String>,
    count: u8,
) -> Result<Vec<SortieDxgi>> {
    let neuves: Vec<SortieDxgi> = apres
        .iter()
        .filter(|sortie| !connues.contains(&sortie.nom_sortie))
        .cloned()
        .collect();
    let noms: Vec<&str> = neuves.iter().map(|s| s.nom_sortie.as_str()).collect();
    anyhow::ensure!(
        neuves.len() == count as usize,
        "{} new DXGI outputs after creating {count} ({noms:?}) — \
         an external addition or removal makes the measurement unattributable",
        neuves.len()
    );
    // An output enumerated but NOT attached to the desktop is not capturable:
    // Windows composes nothing on it, DXGI announces it as 0×0, and the defect would
    // only show much further on — at `facteur_echelle` (zero
    // dimension) or at the swapchain of a 0×0 test pattern, far from its cause. The refusal is
    // taken here, where it can be read.
    let detachees: Vec<&str> = neuves
        .iter()
        .filter(|sortie| !sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    anyhow::ensure!(
        detachees.is_empty(),
        "{} new output(s) NOT attached to the desktop ({detachees:?}) — \
         the driver published the output but Windows composes nothing on it",
        detachees.len()
    );
    for sortie in &neuves {
        tracing::info!(
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur_annoncee = sortie.rect.width,
            hauteur_annoncee = sortie.rect.height,
            "virtual output retained"
        );
    }
    Ok(neuves)
}

/// Tells whether the N virtual outputs are still there, and still ATTACHED, after
/// the pass named by `passe`.
///
/// Does not fail: the measurement is done, denying it now would not make it
/// better. This survey serves to INTERPRET the verdicts, not to replace them —
/// an output removed by the watchdog along the way would return black,
/// and we would blame Windows for a defect of the measurement protocol.
///
/// **Present is not enough: it must be attached.** An output that the driver
/// detached from the desktop during a pass remains perfectly enumerable by
/// DXGI — Windows simply stops composing on it, and the capture turns black.
/// A check that only looked at enumeration would declare this output
/// "surviving" on an image that turned black: exactly the misattribution
/// this function exists to prevent. The rest of the module already compares
/// sets of attached outputs (`montee::noms_attaches`), not of enumerated ones.
///
/// The two defects are logged SEPARATELY because they do not say the
/// same thing: `disparues` is a removal, `detachees` an output that the
/// driver keeps but that Windows no longer displays.
/// Private: `passes.rs` accesses it as a child module (`super::`), without this
/// check becoming a surface offered to the rest of `multifenetre`.
fn constater_survie(passe: &str, virtuelles: &[SortieDxgi]) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(error) => {
            tracing::error!(
                passe,
                causes = %super::causes(error),
                "topology unreadable after the pass — survival of the outputs unknown"
            );
            return;
        }
    };
    let enumerees: HashSet<&str> = vivantes
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    let attachees: HashSet<&str> = vivantes
        .iter()
        .filter(|sortie| sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();

    let disparues: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !enumerees.contains(nom))
        .collect();
    let detachees: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| enumerees.contains(nom) && !attachees.contains(nom))
        .collect();

    if disparues.is_empty() && detachees.is_empty() {
        tracing::info!(
            passe,
            count = virtuelles.len(),
            "the N virtual outputs are still there AND attached — the verdicts of this \
             pass are indeed about them"
        );
        return;
    }
    if !disparues.is_empty() {
        tracing::error!(
            passe,
            ?disparues,
            "virtual outputs DISAPPEARED during this pass — their verdicts cannot \
             be blamed on Windows, they no longer existed"
        );
    }
    if !detachees.is_empty() {
        tracing::error!(
            passe,
            ?detachees,
            "virtual outputs were DETACHED from the desktop during this pass — still \
             enumerable, but Windows no longer composes on them: a black image there would be due \
             to the detachment, not to the capture path"
        );
    }
}
