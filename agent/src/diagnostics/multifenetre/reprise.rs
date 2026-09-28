//! `MULTIFENETRE_REPRISE` — k running DXGI duplications, a virtual
//! output created on top, and the question: do they resume?
//!
//! **This is the test of the inference on which the whole sub-block D2 rests.**
//! Sub-block D1 noted that creating a virtual output makes the mutex of already
//! open duplications be abandoned (`0x887A0026`), and that all
//! capture sessions die with it. Sub-block D2 bets that this
//! failure is **recoverable** — that `DXGI_ERROR_ACCESS_LOST` is caught up by
//! reopening the duplication, as Microsoft's documentation says. This bench
//! is what distinguishes this bet from a belief: it tests recovery on THIS
//! hardware, without browser or signaling, hence without anything that could mask the
//! cause.
//!
//! **This is a stop point** (spec §6.1). If the duplications do not resume,
//! the path is refuted and we must switch to serialisation (§8) —
//! before having committed the rest of the plan.
//!
//! The set-up is that of `paralleles.rs` — k virtual outputs, one test pattern and
//! one duplication each, rotating image check, restoration of the
//! topology — **plus a disruption in the middle**. It captures through
//! `DesktopCapture::next_frame`, that is through the PRODUCTION path:
//! that is the reason why recovery was placed there and not in
//! `WindowsSource`. Bypassing this path would empty the measurement of its purpose.
//!
//! # What this bench does not say
//!
//! - **One run per rank gives no rate.** Sub-block D1
//!   reproduced its defect three times out of three; a recovery that works once
//!   does not prove that it always works.
//! - **The test patterns are not applications**: full-frame D3D11, without
//!   occlusion or interaction.
//! - **Correctness is SAMPLED** — one path checked per round.
//! - **The DESTRUCTION of an output is not exercised here**, any more than it
//!   was in D1.
//!
//! # Trap: a crash here leaves up to NINE orphaned outputs
//!
//! This bench creates `k` outputs, then **one more**, from a pool that only
//! counts **10** (measured ceiling, `montee.rs`). The
//! `moniteurs_virtuels::Sorties` guard destroys them when going out of scope, including
//! during a panic — but **not on a crash of the process**. Recovery:
//! `MULTIFENETRE_VDD_PURGE=1`. Check the state BEFORE concluding from a creation
//! refusal, and from a fresh process (`MULTIFENETRE_DXGI=1`).
//!
//! # Split
//!
//! This file is *the output driver*: creation, designation, state
//! check, restoration — as `paralleles.rs` is of its own, and for the same
//! reason (500-line ceiling). *The two passes and the disruption* live
//! in `reprise/passes.rs`; *the post-mortem probe*, which is not a measurement
//! but a diagnostic on the measurement, in `reprise/post_mortem.rs`.

mod passes;
mod post_mortem;

use std::collections::HashSet;

use anyhow::{Context, Result};

use super::compteurs;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use super::paralleles::designer_sorties_neuves;
use crate::capture::SortieDxgi;
use crate::mire;

pub(super) fn mesurer(count: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&count),
        "MULTIFENETRE_REPRISE doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let before = relever_topologie("avant création")?;
    let names_before = noms_attaches(&before);
    let connues: HashSet<String> = before
        .iter()
        .map(|sortie| sortie.nom_sortie.clone())
        .collect();

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Explicit scope of the guard: the outputs — the k of the set-up AND the
    // disruptor — must be destroyed BEFORE the final survey, otherwise
    // the latter would describe a transient state.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        // A CLOSURE and not a bare block: the `?`s of the preparation must exit
        // from HERE, not from `mesurer`.
        //
        // They used to exit from it, and thereby skipped the purge recovery
        // placed further down — that is, precisely on the paths where the
        // creation or the topology went astray, **those where a removal is most
        // likely to have been refused in the guard's `Drop`**. On a
        // pool of ten outputs, a slot lost that way stays lost until the
        // machine reboots. The RAII guard, for its part, was and remains
        // correct: it runs on all paths, including panic.
        (|| -> Result<()> {
            for rang in 1..=count {
                let id = sorties
                    .create(largeur, hauteur, hertz)
                    .with_context(|| format!("création de la sortie virtuelle n°{rang}"))?;
                tracing::info!(rang, id, "sortie virtuelle créée");
            }
            attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

            let apres = relever_topologie("après création")?;
            let virtuelles = designer_sorties_neuves(&apres, &connues, count)?;
            // Built right after `attendre_en_pinguant`, hence right after the
            // last known ping (see `compteurs::Garde::new`).
            let mut garde = compteurs::Garde::new(&pilote);
            garde.battre()?;
            let issue = passes::eprouver(&mut garde, &mut sorties, &virtuelles, &connues);
            tracing::info!(
                intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
                "chien de garde : plus grand écart entre deux battements sur toute la mesure"
            );
            // UNCONDITIONAL call, on the outcome of `eprouver` whatever it
            // is. It duplicates the check `eprouver` already makes after its
            // disruption, and extends it to the paths where `eprouver` returned an
            // error before getting there.
            //
            // ⚠️ What this implies for reading: if the test failed BEFORE the
            // disruptor was created, the "no third-party
            // output" warning of `constater_tierces` bears on an IOCTL that was never
            // issued. It is labelled at the "après perturbation" moment and
            // therefore does not apply to the "bilan" moment — but a hurried reader
            // could read it there, and it is this paragraph that prevents it.
            constater_places("bilan", &virtuelles, &connues);
            issue
        })()
    };

    // Second attempt at the removals the guard did not obtain: last chance
    // of THIS process, beyond that only the inter-process purge will reach them.
    // Now also runs on ERROR paths (see the closure above).
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
    if noms_final == names_before {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            names_before = ?names_before,
            noms_apres = ?noms_final,
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}

/// Tells whether the k virtual outputs are still there, still attached, **still at
/// the same place**, and whether a THIRD-PARTY output appeared in the topology.
///
/// The first two checks are those of `paralleles::constater_survie`. The
/// two others are specific to this bench:
///
/// - **the place.** The test patterns were placed once and for all at the
///   coordinates surveyed at creation. If the arrival of one more output makes
///   the others slide in the virtual desktop, the test patterns end up outside
///   their output and the captures turn black — and we would blame a
///   recovery defect for what is only a move.
/// - **the third parties**, that is everything that is neither known in advance
///   (`connues`) nor created by the set-up (`virtuelles`): at the "après
///   perturbation" moment, it is the **disrupting output**, and it is the only proof
///   that the disruption really took place. `Sorties::create` returns `Ok(id)`
///   as soon as the driver accepts the IOCTL; nothing then says that Windows
///   reconfigured anything at all. A disruptor created but not attached would
///   disrupt nothing, and "zero reopenings" would wrongly read as a
///   result of recovery. (A third party can also be an addition by Apollo,
///   which drives the display configuration of this VM: hence the survey
///   by name rather than a count.)
///
/// Does not fail: the measurement is done, denying it now would not make it
/// better. This survey serves to INTERPRET the verdicts, not to replace them.
/// Private: `passes.rs` accesses it as a child module.
fn constater_places(moment: &str, virtuelles: &[SortieDxgi], connues: &HashSet<String>) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(error) => {
            tracing::error!(
                moment,
                causes = %super::causes(error),
                "topologie illisible — état des sorties inconnu"
            );
            return;
        }
    };

    constater_tierces(moment, virtuelles, connues, &vivantes);

    let mut disparues: Vec<&str> = Vec::new();
    let mut detachees: Vec<&str> = Vec::new();
    let mut deplacees: Vec<String> = Vec::new();
    for attendue in virtuelles {
        let nom = attendue.nom_sortie.as_str();
        match vivantes
            .iter()
            .find(|sortie| sortie.nom_sortie == attendue.nom_sortie)
        {
            None => disparues.push(nom),
            Some(sortie) if !sortie.attachee_au_bureau => detachees.push(nom),
            Some(sortie) if sortie.rect != attendue.rect => {
                deplacees.push(format!("{nom} : {:?} → {:?}", attendue.rect, sortie.rect))
            }
            Some(_) => {}
        }
    }

    if disparues.is_empty() && detachees.is_empty() && deplacees.is_empty() {
        tracing::info!(
            moment,
            count = virtuelles.len(),
            "les k sorties virtuelles sont là, attachées, et à la même place — les verdicts \
             portent bien sur elles"
        );
        return;
    }
    if !disparues.is_empty() {
        tracing::error!(
            moment,
            ?disparues,
            "des sorties virtuelles ont DISPARU — leurs verdicts ne sont pas imputables à \
             Windows, elles n'existaient plus"
        );
    }
    if !detachees.is_empty() {
        tracing::error!(
            moment,
            ?detachees,
            "des sorties virtuelles ont été DÉTACHÉES du bureau — encore énumérables, mais \
             Windows n'y compose plus : une image noire y serait imputable au détachement"
        );
    }
    if !deplacees.is_empty() {
        tracing::error!(
            moment,
            ?deplacees,
            "des sorties virtuelles ont CHANGÉ DE PLACE dans le bureau virtuel — les mires \
             sont restées où elles étaient, une image noire y serait imputable au déplacement \
             et non à la reprise"
        );
    }
}

/// The proof that the disruption took place: an output neither known in advance, nor
/// created by the set-up, hence the disruptor.
fn constater_tierces(
    moment: &str,
    virtuelles: &[SortieDxgi],
    connues: &HashSet<String>,
    vivantes: &[SortieDxgi],
) {
    let notres: HashSet<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    let tierces: Vec<&SortieDxgi> = vivantes
        .iter()
        .filter(|sortie| {
            !notres.contains(sortie.nom_sortie.as_str()) && !connues.contains(&sortie.nom_sortie)
        })
        .collect();

    if tierces.is_empty() {
        tracing::warn!(
            moment,
            "aucune sortie TIERCE dans la topologie — au moment « après perturbation », cela \
             signifie que la perturbatrice ne s'est PAS attachée : le pilote a accepté l'IOCTL \
             mais Windows n'a rien reconfiguré, donc rien n'a pu perturber les duplications, et \
             une absence de réouverture ne dit alors RIEN de la reprise"
        );
        return;
    }
    let attachees: Vec<&str> = tierces
        .iter()
        .filter(|sortie| sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    let detachees: Vec<&str> = tierces
        .iter()
        .filter(|sortie| !sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    tracing::info!(
        moment,
        tierces_attachees = ?attachees,
        tierces_detachees = ?detachees,
        "sorties tierces relevées — au moment « après perturbation », la perturbatrice doit \
         figurer parmi les ATTACHÉES pour que la perturbation ait eu lieu"
    );
}
