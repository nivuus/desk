//! P1 of sub-block D8: does a SudoVDA virtual output accept a display mode
//! other than the one it was created with?
//!
//! ⚠️ **The criterion judges on the DXGI READ-BACK, never on the return code.**
//! `mode-sortie-1728x1080.log` shows the `CDS_UPDATEREGISTRY|CDS_NORESET`
//! then `CDS_RESET` idiom announcing `0` on an output that did not move by one pixel:
//! a refusal disguised as a success. And D8's first P1 verdict was `REÇU`
//! returned by a criterion that could not return the other value — the probe
//! asked the output for the size it already had.
//!
//! # Task 1 of sub-block D9 — the bench/product gap, closed
//!
//! D8 tested THREE flag combinations, all `CDS_UPDATEREGISTRY`,
//! and NEVER opened a DXGI duplication — whereas the product, at the time,
//! resized an output whose duplication was open and held for up to
//! 3.1 s (`agent/src/windows_source/redimensionnement/mode_sortie.rs`,
//! **removed by sub-block D9** — see the measurement finding at the head of
//! `capteur/plein_ecran.rs`). This probe closes the gap:
//!
//! 1. a FOURTH combination, dynamic and not persisted
//!    (`combinaisons::combos`);
//! 2. a DXGI duplication opened on the tested output AND HELD for the whole
//!    round (`DesktopCapture::sur_sortie`) — it is the ELIMINATION test itself,
//!    the subject of this probe;
//! 3. a CONTROL: the same arm, replayed on a fresh output with no duplication
//!    open, so as not to blame the duplication for a refusal that would
//!    come from the chosen mode;
//! 4. D8's two side unknowns, surveyed at the same moment as
//!    the elimination test: the access losses inflicted on two NEIGHBOURING outputs
//!    (`mode_sortie::voisines`), and whether the tested output keeps its
//!    `\\.\DISPLAYn` name.
//!
//! **Enumeration is not enough, and that is the whole subject of this probe.** A
//! driver can advertise a mode and refuse it, just as it can accept a mode
//! it does not enumerate. So it does all THREE, in order: it enumerates
//! (`EnumDisplaySettingsExW`), THEN it really changes
//! (`ChangeDisplaySettingsExW`), THEN it reads back through
//! `crate::capture::enumerer_sorties` — the same DXGI read-back
//! (`GetDesc`/`DesktopCoordinates`) this whole module already uses, never
//! WMI, whose resolution field was seen 68 s stale on this ground
//! (see `moniteurs_virtuels.rs`).
//!
//! `MULTIFENETRE_MODE_SORTIE=<W>x<H>`: creates an output at the production
//! resolution (`montee::RESOLUTION`, 1280×720 — the path through which the
//! product makes its outputs appear), tries to switch it to W×H through
//! FOUR increasing `CDS_*` flag combinations, reads back after each one,
//! stops at the first that holds, then returns the output to the driver.
//! `CDS_SET_PRIMARY` is used in NO combination: we do not touch
//! the primary monitor.
//!
//! Refusal and acceptance are two equally valid measurement results —
//! neither is a failure of the probe. A refusal has its fallback already settled
//! elsewhere in the plan (upscaling, documented then accepted).
//!
//! # Defect fixed on 4 August 2026 (F1 replayed) — see `p1-mode-sortie.log`
//!
//! The first run returned "P1 RECU" with no value: it asked
//! the output for the size it **already** had at creation time
//! (`sortie moment="après création" … largeur=1920 hauteur=1080`, whereas
//! the probe believed it had created 1280×720 — probable persistence in the
//! registry of a `CDS_UPDATEREGISTRY` from an earlier run). The criterion
//! `derniere_taille == cible` was therefore true BEFORE any attempt:
//! `ChangeDisplaySettingsExW` never had the chance to change anything
//! at all, and nothing could make the check fail. Exactly the defect
//! this repository calls F1 (a check that cannot return the other
//! verdict), replayed on the instrument meant to avoid it.
//!
//! **The remedy fits in two points, both applied below:**
//! 1. the current size is read through DXGI (`GetDesc`/`DesktopCoordinates`,
//!    never WMI) **before any attempt**, under an unambiguous key
//!    (`taille_avant_tentative_*`), and compared with the target;
//! 2. the target is no longer a fixed pair received as is: it is chosen
//!    dynamically among the modes `EnumDisplaySettingsExW` advertises, **by
//!    excluding the current size** (`choisir_cible`). If no mode
//!    differs from it — a degenerate case, not met in practice with the nine
//!    modes of this VM — the probe REFUSES to measure (`P1 NON MESURABLE`)
//!    rather than return an empty verdict;
//! 3. **the verdict itself is an observed MOVEMENT, not an equality with the
//!    target.** `P1 RECU` ⟺ the size read back by DXGI after an attempt
//!    differs from `avant` — exactly the question the module's title
//!    asks ("a mode OTHER than the one it was created with"), not "THIS precise
//!    mode". Whether or not the driver honours the exact requested value is
//!    logged separately (`cible_atteinte`/`cible_exacte_atteinte`): a
//!    finer question, which can be `false` under a `P1 RECU` verdict
//!    without that being a contradiction.
//!
//! # Task 2bis of sub-block D9 — PERSISTENCE, not only triggering
//!
//! Task 2 established that `CDS_TYPE(0)` (dynamic, not persisted by
//! construction) makes the output move **on the first attempt**, so that the
//! three other arms — including `CDS_UPDATEREGISTRY`, the only one to persist by
//! construction — were never exercised, and the movement obtained does not
//! survive the creation of one more virtual output. This task
//! answers for `CDS_UPDATEREGISTRY`: `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`
//! restricts the round to a single designated arm (`persistance::combinaison_imposee`,
//! `combinaisons::combos_du_tour`), and the survival verdict is logged
//! explicitly (`persistance::journaliser_verdict`) rather than being reconstructible
//! only by cross-checking two topology surveys ten lines apart.
//!
//! ## Review of task 2bis — four defects found on evidence, fixed
//!
//! The first measurement compared `mouvement` with an `avant` value captured
//! several seconds before the round, BEFORE the two neighbours were opened —
//! **the output can move without any API call between these two instants**
//! (residue of registry pollution from an earlier run, reapplied at
//! the creation of a neighbouring virtual output). `essayer_les_modes` now reads
//! the DXGI state back just before the first attempt and compares AGAINST
//! THIS fresh read-back, never against the stale value. Three other
//! defects, more minor but real: `journaliser_verdict` could return
//! `survit=true` on a vanished output (`(0, 0)` fallback on both sides);
//! `mouvement_observe` could be `true` on an EMPTY round (no combo
//! attempted); and only the WINNING arm was logged, never the
//! IMPOSED arm — see the header comments of `eliminatoire.rs` and
//! `persistance.rs` for the detail of each fix.

mod combinaisons;
mod eliminatoire;
mod persistance;
mod temoin;
mod voisines;

use std::collections::HashSet;

use anyhow::{Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    EnumDisplaySettingsExW, DEVMODEW, ENUM_DISPLAY_SETTINGS_FLAGS, ENUM_DISPLAY_SETTINGS_MODE,
};

use combinaisons::combo_pour_temoin;
use eliminatoire::essayer_les_modes;
use persistance::journaliser_verdict;
use temoin::{nom_apres_tour, rejouer_temoin};

use super::capture_virtuelle::designer_sortie_neuve;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::capture::DesktopCapture;
use crate::moniteurs_virtuels::pilote::ouvrir_pilote;
use crate::moniteurs_virtuels::Sorties;

/// Modes the output ADVERTISES (`EnumDisplaySettingsExW`, pure enumeration) —
/// says nothing about what it really accepts, that is the whole subject of the
/// module.
///
/// `pub(super)`: `temoin::rejouer_temoin` needs it to guard against the
/// same F1 defect that `choisir_cible` fixes here for the elimination test — see
/// its header comment.
pub(super) fn modes_annonces(nom_sortie: &str) -> Vec<(u32, u32)> {
    let nom: Vec<u16> = nom_sortie
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut modes = Vec::new();
    let mut index = 0u32;
    loop {
        let mut dm = DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        let ok = unsafe {
            EnumDisplaySettingsExW(
                PCWSTR(nom.as_ptr()),
                ENUM_DISPLAY_SETTINGS_MODE(index),
                &mut dm,
                ENUM_DISPLAY_SETTINGS_FLAGS(0),
            )
        };
        if !ok.as_bool() {
            break;
        }
        modes.push((dm.dmPelsWidth, dm.dmPelsHeight));
        index += 1;
    }
    modes.sort_unstable();
    modes.dedup();
    modes
}

/// Chooses the target to attempt: the requested resolution if it already differs from
/// the current size and is among the advertised modes, otherwise a target
/// taken dynamically from the advertised modes, **always excluding
/// the current size** (`avant`) — this is what makes it impossible to replay the
/// F1 defect documented at the head of the module: whatever state the
/// registry persistence left the output in, the retained target differs from it
/// by construction, except for the degenerate case returned as `None`.
///
/// Modes sorted by `modes_annonces` (ascending, deduplicated): the fallback
/// picks the largest mode distinct from `avant`, for a wide movement and
/// hence one without measurement ambiguity.
///
/// `pub(super)`: `temoin::rejouer_temoin` reuses this SAME function
/// rather than rewriting a variant of it, for the same reason it exists
/// here -- see Critical 2 of the review of task 1.
pub(super) fn choisir_cible(
    avant: (u32, u32),
    demande: (u32, u32),
    annonces: &[(u32, u32)],
) -> Option<(u32, u32)> {
    if demande != avant && annonces.contains(&demande) {
        return Some(demande);
    }
    annonces.iter().copied().rev().find(|&mode| mode != avant)
}

/// Sonde `MULTIFENETRE_MODE_SORTIE=<L>x<H>`.
pub(super) fn executer(consigne: &str) -> Result<()> {
    let (l, h) = consigne.split_once('x').with_context(|| {
        format!(
            "MULTIFENETRE_MODE_SORTIE='{consigne}' invalide, attendu <largeur>x<hauteur> \
             (par exemple 1920x1080)"
        )
    })?;
    // `demande`: the operator's preference, NOT necessarily the retained
    // target — see `choisir_cible`. Keeping it lets an operator who
    // already knows the current size aim directly at a useful target,
    // without changing anything in the documented call format.
    let demande: (u32, u32) = (
        l.trim()
            .parse()
            .context("largeur invalide dans MULTIFENETRE_MODE_SORTIE")?,
        h.trim()
            .parse()
            .context("hauteur invalide dans MULTIFENETRE_MODE_SORTIE")?,
    );

    // Surveyed BEFORE any creation, like the neighbouring probes: without it, a
    // manual restoration after a crash would be done blindly.
    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues_avant_tout: HashSet<String> = avant
        .iter()
        .map(|sortie| sortie.nom_sortie.clone())
        .collect();

    let pilote = ouvrir_pilote()?;
    let (largeur_creation, hauteur_creation, hertz) = RESOLUTION;

    // Explicit scope: the `Sorties` guard must have destroyed BEFORE the
    // final survey, otherwise the latter would describe a transient state — same
    // discipline as `montee.rs` and `capture_virtuelle.rs`.
    let issue = {
        let mut sorties = Sorties::nouvelles(&pilote);

        // --- The output UNDER TEST ---
        let id = sorties.creer(largeur_creation, hauteur_creation, hertz)?;
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;
        let apres_creation = relever_topologie("après création (sortie testée)")?;
        let virtuelle = designer_sortie_neuve(&apres_creation, &connues_avant_tout, id)?;
        let nom_sortie = virtuelle.nom_sortie.clone();
        // `taille_avant_tentative`: the size ACTUALLY read by DXGI right
        // after creation — NOT assumed to be
        // `largeur_creation`×`hauteur_creation`. It is exactly the survey
        // whose absence made the first run of this probe
        // meaningless (see the module's header comment): the output can
        // be born at a size different from the one requested from the driver, through
        // registry persistence of an earlier `CDS_UPDATEREGISTRY`.
        let taille_avant_tentative = (virtuelle.rect.width, virtuelle.rect.height);
        tracing::info!(
            nom = %nom_sortie,
            largeur_demandee_a_la_creation = largeur_creation,
            hauteur_demandee_a_la_creation = hauteur_creation,
            largeur_avant_tentative = taille_avant_tentative.0,
            hauteur_avant_tentative = taille_avant_tentative.1,
            "sortie de sonde créée -- taille relue par DXGI avant toute tentative de changement"
        );
        let mut connues_a_ce_point = connues_avant_tout.clone();
        connues_a_ce_point.insert(nom_sortie.clone());

        // THE BENCH/PRODUCT GAP that D8 left wide open, and the very subject of
        // this probe: production then resized an output WHOSE
        // DUPLICATION IS OPEN and held for up to 3.1 s. P1 never opened
        // one. The mechanism measured here has since been removed by
        // sub-block D9 (see `capteur/plein_ecran.rs`); this probe keeps its
        // measurement value.
        let duplication = DesktopCapture::sur_sortie(&nom_sortie)
            .context("ouverture de la duplication sur la sortie virtuelle neuve")?;
        tracing::info!(sortie = %nom_sortie, "duplication ouverte et TENUE pendant les tentatives");

        // --- Two NEIGHBOURS, for side unknown no. 2 (D8): how many
        // access losses does a mode change inflict on them? The
        // creation, designation AND opening of each are
        // ORCHESTRATED by `voisines::creer_deux` (and not chained here):
        // it is this strict order, a positional index resolved and consumed
        // before any following creation, that avoids the trap documented in
        // doctrine D1 -- see its header comment.
        let (voisine1, voisine2, nom_v1, nom_v2) = voisines::creer_deux(
            &pilote,
            &mut sorties,
            &mut connues_a_ce_point,
            largeur_creation,
            hauteur_creation,
            hertz,
        )?;
        let mut voisines = vec![voisine1, voisine2];

        // --- THE ELIMINATION TEST ---
        let resultat = essayer_les_modes(
            &pilote,
            &nom_sortie,
            taille_avant_tentative,
            demande,
            &mut voisines,
        )?;

        // --- The two side unknowns (step 5), surveyed at the same moment
        // as the elimination test -- before releasing anything.
        let autres_noms_a_nous: HashSet<String> = [nom_v1, nom_v2].into_iter().collect();
        let (nom_apres, taille_apres_tour) =
            nom_apres_tour(&nom_sortie, &connues_avant_tout, &autres_noms_a_nous)?;
        let pertes_acces_voisines = resultat.pertes_voisines;
        tracing::info!(
            pertes_acces_voisines,
            nom_avant = %nom_sortie,
            nom_apres = %nom_apres,
            nom_conserve = nom_sortie == nom_apres,
            "inconnues annexes relevées au même moment que l'éliminatoire"
        );
        // The name that replaces `nom_sortie` (if it changed -- which the line
        // above just measured) is not yet known to ANYONE: neither to
        // `connues_avant_tout`, nor to `connues_a_ce_point` (which only carries
        // the OLD name). Without this addition, the creation of the control would see TWO
        // new entries -- the renamed name AND the control -- and would fail with
        // "external addition" (Important 2, review of task 1)
        // precisely when the renaming is the phenomenon under study.
        if nom_apres != "<disparue>" {
            connues_a_ce_point.insert(nom_apres.clone());
        }

        // CONTROL. Without it, a refusal would be blamed on the duplication while
        // it could come from the chosen mode. The control replays the SAME
        // gesture on a fresh output, duplication closed.
        drop(duplication);
        tracing::info!("duplication relâchée — début du témoin sans duplication");
        // The neighbours have nothing left to probe: the control bears on the
        // duplication of the TESTED output, not on that of the neighbours.
        drop(voisines);

        match resultat.cible {
            Some(cible) => {
                let combo_temoin = combo_pour_temoin(resultat.gagnante);
                let id_temoin = sorties.creer(largeur_creation, hauteur_creation, hertz)?;
                attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;
                let apres_temoin = relever_topologie("après création (témoin)")?;
                // PERSISTENCE (task 2bis, D9): did the output under test
                // keep, at the moment when ONE MORE virtual output has just
                // been created, the size the round had just given it?
                // See `persistance.rs` -- same survey as `designer_sortie_neuve`
                // below, looked up under `nom_apres` rather than by
                // position. The first two arguments are DISTINCT since
                // the review of task 2bis: `combinaison_imposee` (what was
                // REQUESTED) and `resultat.gagnante` (what
                // REALLY happened) can diverge (empty round, mojibake).
                journaliser_verdict(
                    resultat.combinaison_imposee.as_deref(),
                    resultat.gagnante,
                    &nom_apres,
                    taille_apres_tour,
                    &apres_temoin,
                );
                let sortie_temoin =
                    designer_sortie_neuve(&apres_temoin, &connues_a_ce_point, id_temoin)?;
                let nom_temoin = sortie_temoin.nom_sortie.clone();
                let avant_temoin = (sortie_temoin.rect.width, sortie_temoin.rect.height);
                rejouer_temoin(&pilote, &nom_temoin, avant_temoin, cible, &combo_temoin)?;
            }
            None => tracing::info!(
                "témoin non joué : le tour éliminatoire n'a désigné aucune cible mesurable \
                 (P1 NON MESURABLE)"
            ),
        }

        Ok(())
        // The `sorties` guard returns the outputs to the driver here, when going out of
        // scope.
    };

    // Second attempt at the removals the guard did not obtain: last
    // chance of THIS process, beyond that only the inter-process purge will
    // reach them.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // A virtual output outlives the process. This check remains that of the
    // measuring process, hence judge and party — the check that counts is a
    // `MULTIFENETRE_DXGI=1` survey from a FRESH process, afterwards, by
    // comparing SETS OF NAMES and never cardinalities.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("après destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == noms_avant && final_.len() == avant.len() {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            total_avant = avant.len(),
            total_apres = final_.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}
