//! Measurement ③: is a window placed on a virtual monitor captured
//! correctly, even when covered by another?
//!
//! **This is the FOUNDING hypothesis of the "one virtual monitor per
//! window" path**, the one the capture probe recommended — and it had
//! never been verified: the probe only guaranteed it "by construction",
//! without ever capturing on it. That a virtual monitor WITHOUT an attached screen is
//! really composed by Windows, and that Desktop Duplication returns something
//! other than black from it, cannot be deduced from any document — it is measured.
//!
//! A black image here is a RESULT, not a failure of the probe: it would
//! bring down path 2 and change the nature of the next work stream.
//!
//! Module separate from `moniteurs.rs`, which is *the driver*, and from `montee.rs`, which
//! is *the ceiling measurement*: this file is the measurement of CAPTURE. The
//! split is not cosmetic — `moniteurs.rs` is at 493 lines and the
//! project's 500-line ceiling forbade pouring anything into it.
//!
//! # A single process, and it is structural
//!
//! The `moniteurs_virtuels::Sorties` guard destroys the output at the end of the
//! process. A bench launched separately, afterwards, would therefore find nothing
//! left to capture. This probe creates the output **and** runs the bench
//! on it without ever giving control back.
//!
//! # The trap this probe revealed — FIXED on 31 July 2026
//!
//! ✅ **What follows is an account in the PAST tense.** The defect described here was
//! diagnosed and fixed by the parallel duplications work stream
//! (`agent/src/encode/arret.rs`, commits `486e182` / `beb3114`,
//! `docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`
//! §7). It was not deterministic but **intermittent** (2 crashes out of 6
//! runs); its cause: the NVIDIA MFT kept a work item in flight
//! when the encoder was released. Since the fix: **0 recurrence over 20
//! runs** of the comparable case — *which is not a proof of absence*.
//! Do not reread the paragraphs below as the current state of the code.
//!
//! With no covering to stage, the bench's elimination gate lets
//! through and the encoding pass runs — yet, before the fix, **it
//! took down the process**, which left an orphaned virtual output.
//!
//! **Where exactly, because that is what steered the diagnosis: at the EXIT of
//! the loop, not during it.** Both runs of 31 July 2026 wrote
//! their tenth and LAST periodic line at `debut + 10.00 s`, that is
//! the very instant when `while debut.elapsed() < DUREE_PASSE` stopped being true:
//! the loop had run in full, and `journaliser` was never reached. What
//! ran between the two is the destruction of the `Vec<H264Encoder>` local to
//! `passe_capture`. Looking on the side of `submit`/`poll_output` would have been
//! looking in the wrong place — and this bracketing is what won the
//! diagnostic campaign.
//!
//! **It was neither the virtual output, nor the encoder alone — it was the
//! pair.** The same bench on the physical desktop
//! (`MULTIFENETRE_BANC=duplication MULTIFENETRE_N=1`) died at the same place:
//! the virtual output was out of the question. And `printwindow-n4.log` as well as
//! `printwindow-n8.log` carry their `passe terminée
//! passe="capture+encodage"` line: on the `printwindow` path, the encoding pass
//! ran to completion and the process survived. The defect was therefore in the
//! "**duplication** path + H.264 encoder" pair, and in it alone.
//!
//! It had never been seen before, because on this path the elimination
//! gate always cut before the encoding pass: that is why
//! no multi-window encoding measurement through this path existed then.
//! **Some exist since** — the four ranks of the parallel duplications
//! work stream, including N=8 with eight encoders destroyed in a row in 4.0 ms.
//!
//! Practical consequence **at the time**: the guard did not run and the virtual
//! output outlived the process; `MULTIFENETRE_VDD_PURGE=1` recovers that
//! state, tested on exactly it. The purge remains useful — a crash,
//! whatever its cause, always leaves the guard silent. Measurement ③
//! itself is taken at `count = 2`, where the covering verdict cuts before
//! the encoding pass and where the guard runs.

use anyhow::{anyhow, Result};

use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::capture::SortieDxgi;

/// `MULTIFENETRE_VDD_CAPTURE` probe: creates ONE virtual output, places
/// `count` test patterns on it, and runs the `duplication` path bench on it.
pub(super) fn capturer_sur_virtuelle(count: u8) -> Result<()> {
    let before = relever_topologie("avant création")?;
    let names_before = noms_attaches(&before);
    let connues: std::collections::HashSet<String> = before
        .iter()
        .map(|sortie| sortie.nom_sortie.clone())
        .collect();

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope of the guard: the output must be destroyed BEFORE the
    // final survey, otherwise the latter would describe a transient state.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        let id = sorties.create(largeur, hauteur, hertz)?;
        // Beat the watchdog while waiting for reconfiguration, as
        // the scale-up in N does: its unit remains unknown, and a bare `sleep`
        // would leave the driver free to remove the output under the measurement.
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("après création")?;
        let virtuelle = designer_sortie_neuve(&apres, &connues, id)?;
        let nom_virtuelle = virtuelle.nom_sortie.clone();

        // The scale factor is read and applied by the bench itself
        // (trace "banc : coordonnées de fenêtre et de texture"): measuring it
        // a second time HERE would require opening a duplication on this
        // output, yet DXGI only allows ONE — the bench's would then fail
        // with 0x80070057.
        pilote.pinguer()?;
        let issue = super::banc::executer("duplication", count, Some(&nom_virtuelle));

        // The bench does not ping: it runs thirty seconds in a tight loop.
        // If the watchdog had removed the output along the way, the
        // capture would have returned black or nothing at all, and we would have blamed
        // Windows for a defect of the measurement protocol. This check settles
        // between the two.
        constater_survie(&nom_virtuelle);
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
    if noms_final == names_before && final_.len() == before.len() {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            names_before = ?names_before,
            noms_apres = ?noms_final,
            total_before = before.len(),
            total_apres = final_.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}

/// Finds, in a topology surveyed after creation, THE output that was not
/// there before.
///
/// By NAME and not by index: DXGI renumbers its outputs at each
/// topology reconfiguration, and an index retained before creation may
/// designate another output afterwards. A set of names does not suffer from this
/// drift.
///
/// Refuses if several outputs are new: someone else would have added
/// one during the measurement (Apollo drives the display configuration of this
/// VM), and nothing would tell which one is ours anymore.
///
/// `pub(super)`: `mode_sortie.rs` (task 3 of sub-block D8) also creates
/// ONE output and must find its DXGI name before attempting a mode
/// change — exactly the same need as this measurement, hence the same function rather
/// than a duplication of the name-matching logic.
pub(super) fn designer_sortie_neuve<'s>(
    apres: &'s [SortieDxgi],
    connues: &std::collections::HashSet<String>,
    id: crate::moniteurs_virtuels::IdSortie,
) -> Result<&'s SortieDxgi> {
    let neuves: Vec<&SortieDxgi> = apres
        .iter()
        .filter(|sortie| !connues.contains(&sortie.nom_sortie))
        .collect();
    let virtuelle = match neuves.as_slice() {
        [] => {
            return Err(anyhow!(
                "aucune sortie DXGI neuve après création de la sortie {id} — \
                 le pilote a accepté la demande mais Windows n'a rien publié"
            ))
        }
        [seule] => *seule,
        several => {
            let noms: Vec<&str> = several.iter().map(|s| s.nom_sortie.as_str()).collect();
            return Err(anyhow!(
                "{} sorties DXGI neuves après création de la sortie {id} ({noms:?}) — \
                 une addition externe rend la mesure inimputable",
                several.len()
            ));
        }
    };
    tracing::info!(
        id,
        nom = %virtuelle.nom_sortie,
        adaptateur = %virtuelle.adaptateur,
        index_adaptateur = virtuelle.index_adaptateur,
        index_sortie = virtuelle.index_sortie,
        attachee = virtuelle.attachee_au_bureau,
        x = virtuelle.rect.x,
        y = virtuelle.rect.y,
        largeur_annoncee = virtuelle.rect.width,
        hauteur_annoncee = virtuelle.rect.height,
        "sortie virtuelle retenue pour la capture"
    );
    Ok(virtuelle)
}

/// Tells whether the virtual output is still there after the bench has run.
///
/// Does not fail: the measurement is done, denying it now would not make it
/// better. This survey serves to INTERPRET the bench's verdict, not to
/// replace it.
fn constater_survie(nom_virtuelle: &str) {
    match crate::capture::enumerer_sorties() {
        Ok(sorties) => {
            let presente = sorties
                .iter()
                .any(|sortie| sortie.nom_sortie == nom_virtuelle);
            if presente {
                tracing::info!(
                    nom = %nom_virtuelle,
                    "la sortie virtuelle a survécu au banc — le verdict porte bien sur elle"
                );
            } else {
                tracing::error!(
                    nom = %nom_virtuelle,
                    "la sortie virtuelle a DISPARU pendant le banc — le verdict de capture \
                     n'est pas imputable à Windows, la sortie n'existait plus"
                );
            }
        }
        Err(error) => tracing::error!(
            causes = %super::causes(error),
            "topologie illisible après le banc — survie de la sortie virtuelle inconnue"
        ),
    }
}
