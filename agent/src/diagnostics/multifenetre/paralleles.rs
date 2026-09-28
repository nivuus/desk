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
//! (`docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`).
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

pub(super) fn mesurer(nombre: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&nombre),
        "MULTIFENETRE_VDD_PARALLELE doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: HashSet<String> = avant
        .iter()
        .map(|sortie| sortie.nom_sortie.clone())
        .collect();

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope of the guard: the outputs must be destroyed BEFORE
    // the final survey, otherwise the latter would describe a transient state.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=nombre {
            let id = sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie virtuelle n°{rang}"))?;
            tracing::info!(rang, id, "sortie virtuelle créée");
        }
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("après création")?;
        let virtuelles = designer_sorties_neuves(&apres, &connues, nombre)?;
        // Built right after `attendre_en_pinguant`, hence right after the
        // last known ping. The seam between the two is NOT counted in
        // the maximum interval — `Garde::nouvelle` sets its origin at its own
        // construction —, it is made negligible by the adjacency of the two
        // calls: 66 µs in the survey. See `compteurs::Garde::nouvelle`.
        let mut garde = compteurs::Garde::nouvelle(&pilote);
        garde.battre()?;
        let issue = passes::executer_passes(&mut garde, &virtuelles);
        // The figure that tells whether the watchdog was beaten without a gap over
        // the whole measurement. Logged even when the passes fail: it is
        // precisely when a measurement goes wrong that one must know whether an
        // output could have been taken back under it.
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la mesure"
        );
        // Last finding, that of the ERROR PATH: `executer_passes`
        // already checks survival after each of its passes, but a `?`
        // exits it without going through these checks. This one runs whatever
        // happens, and it is the only one covering an abandonment midway.
        constater_survie("bilan", &virtuelles);
        issue
    };

    // Second attempt at the removals the guard did not obtain: last chance
    // of THIS process, beyond that only the inter-process purge will reach them.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // A virtual output outlives the process. This check remains that of the
    // measuring process, hence judge and party — the check that counts is a
    // `MULTIFENETRE_DXGI=1` survey from a fresh process, afterwards.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("après destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == noms_avant {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}

/// Finds the `nombre` outputs this probe has just created, by
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
    nombre: u8,
) -> Result<Vec<SortieDxgi>> {
    let neuves: Vec<SortieDxgi> = apres
        .iter()
        .filter(|sortie| !connues.contains(&sortie.nom_sortie))
        .cloned()
        .collect();
    let noms: Vec<&str> = neuves.iter().map(|s| s.nom_sortie.as_str()).collect();
    anyhow::ensure!(
        neuves.len() == nombre as usize,
        "{} sorties DXGI neuves après création de {nombre} ({noms:?}) — \
         une addition ou un retrait externe rend la mesure inimputable",
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
        "{} sortie(s) neuve(s) NON attachée(s) au bureau ({detachees:?}) — \
         le pilote a publié la sortie mais Windows n'y compose rien",
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
            "sortie virtuelle retenue"
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
        Err(erreur) => {
            tracing::error!(
                passe,
                causes = %super::causes(erreur),
                "topologie illisible après la passe — survie des sorties inconnue"
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
            nombre = virtuelles.len(),
            "les N sorties virtuelles sont encore là ET attachées — les verdicts de cette \
             passe portent bien sur elles"
        );
        return;
    }
    if !disparues.is_empty() {
        tracing::error!(
            passe,
            ?disparues,
            "des sorties virtuelles ont DISPARU pendant cette passe — leurs verdicts ne sont \
             pas imputables à Windows, elles n'existaient plus"
        );
    }
    if !detachees.is_empty() {
        tracing::error!(
            passe,
            ?detachees,
            "des sorties virtuelles ont été DÉTACHÉES du bureau pendant cette passe — encore \
             énumérables, mais Windows n'y compose plus : une image noire y serait imputable \
             au détachement, pas à la voie de capture"
        );
    }
}
