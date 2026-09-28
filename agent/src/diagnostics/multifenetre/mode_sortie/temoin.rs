//! The CONTROL of step 3 and D8's third side unknown ("does the output
//! keep its `\\.\DISPLAYn` name?").
//!
//! Extracted from `mode_sortie.rs` at task 1 of sub-block D9, for the
//! 500-line ceiling (`CLAUDE.md`). Neither of these two functions touches a
//! private field of `mode_sortie`: one replays an already exposed gesture
//! (`combinaisons::appliquer_combo`), the other reads back an already
//! exposed topology (`montee::relever_topologie`).

use std::collections::HashSet;

use anyhow::Result;
use windows::Win32::Graphics::Gdi::DISP_CHANGE_SUCCESSFUL;

use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use super::combinaisons::{appliquer_combo, Combo};
use super::{choisir_cible, modes_annonces};
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// The CONTROL (step 3, D9 brief): replays `combo` on `nom_sortie` — a
/// NEW output, with no duplication open — and reads back its own movement.
///
/// Without it, a refusal in the ELIMINATION round would be blamed on the duplication
/// held during that round, whereas it could just as well come from the chosen
/// mode itself: the control replays the SAME gesture.
///
/// ⚠️ **Fix (review of task 2, minor point)**: this comment
/// asserted "the only variable that changes being the duplication" — that is
/// false. The control output is a NEW output (not the same one tested
/// by the elimination test), and `drop(voisines)` also releases the two
/// neighbouring duplications before the control is replayed
/// (`mode_sortie.rs::executer`). At least three things differ between
/// the elimination test and the control: the SUT duplication, the two neighbouring
/// duplications, and the identity of the output itself. The control isolates "no
/// DXGI duplication open anywhere", not "only the SUT
/// duplication".
///
/// `cible_eliminatoire` is the target retained for the ELIMINATION test, NOT
/// necessarily the one applied here: see `cible_du_temoin`, which substitutes it
/// in the degenerate case where the control output is already born at that value
/// (registry persistence, doctrine D8) -- exactly the F1 defect that
/// `choisir_cible` already fixes for the elimination test, and which would hit the
/// control identically without this safeguard (Critical 2 of the review of
/// task 1).
pub(super) fn rejouer_temoin(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    before: (u32, u32),
    cible_eliminatoire: (u32, u32),
    combo: &Combo,
) -> Result<()> {
    let Some(cible) = cible_du_temoin(nom_sortie, before, cible_eliminatoire) else {
        tracing::error!(
            verdict = "TEMOIN NON MESURABLE",
            raison = "la sortie temoin nait deja a la cible de l'eliminatoire, et aucun mode \
                      annonce ne differe de sa taille courante",
            width_before_attempt = before.0,
            height_before_attempt = before.1,
            largeur_cible_eliminatoire = cible_eliminatoire.0,
            hauteur_cible_eliminatoire = cible_eliminatoire.1,
            "verdict TEMOIN : mesure impossible, aucune tentative effectuee"
        );
        return Ok(());
    };
    let last_code = appliquer_combo(nom_sortie, cible.0, cible.1, combo);
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let releve = relever_topologie(&format!("après tentative TÉMOIN « {} »", combo.etiquette()))?;
    let read_size = releve
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_sortie)
        .map(|sortie| (sortie.rect.width, sortie.rect.height));
    // ⚠️ **Fix (review of task 2bis, second pass, point 13).**
    // The old `.unwrap_or((0, 0))` fallback could have made "TEMOIN
    // RECU" win on the mere disappearance of the control output (same defect as
    // in `eliminatoire.rs`, same remedy: no verdict is returned on
    // a sentinel, absence is logged separately).
    let Some(last_size) = read_size else {
        tracing::error!(
            etiquette = combo.etiquette(),
            nom_sortie,
            "la sortie TEMOIN est ABSENTE de la relecture apres sa tentative -- aucun verdict \
             ne peut en etre rendu"
        );
        return Ok(());
    };
    let mouvement = last_size != before;
    tracing::info!(
        etiquette = combo.etiquette(),
        code_brut = last_code,
        api_annonce_succes = (last_code == DISP_CHANGE_SUCCESSFUL.0),
        width_before_attempt = before.0,
        height_before_attempt = before.1,
        largeur_relue = last_size.0,
        hauteur_relue = last_size.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        mouvement,
        verdict = if mouvement {
            "TEMOIN RECU"
        } else {
            "TEMOIN REFUSE"
        },
        "verdict TEMOIN : le meme geste, SANS duplication ouverte, sur une sortie neuve -- \
         departage si un refus de l'eliminatoire vient de la duplication tenue ou du mode choisi"
    );
    Ok(())
}

/// Chooses the target ACTUALLY applied by the control.
///
/// Generally the SAME as the elimination test's (`cible_eliminatoire`) — that is the
/// very meaning of the word "control": replaying the gesture identically. But if
/// this NEW output is already born at `cible_eliminatoire`, applying THIS combo
/// on THIS target could NEVER observe a movement, whatever the
/// driver's real verdict: a driver that accepts and a driver that refuses
/// would both return `last_size == before`. It is the F1 defect
/// replayed — the same one `choisir_cible` fixes for the elimination test.
///
/// **This case is not an accident of chance.** Doctrine D8 is that an
/// output is born at the LAST size left in the registry by an earlier
/// `CDS_UPDATEREGISTRY`, and `CLAUDE.md` reports that
/// "`CDS_UPDATEREGISTRY` alone was always enough when something
/// moved": IF the winning arm of the elimination test writes the registry (3 of the
/// 4 arms of `combinaisons::combos` do), the control output, created
/// right AFTER, is then born precisely at `cible_eliminatoire`. It is therefore the
/// EXPECTED case on a persistent winning arm, not a rare exception.
///
/// The safeguard is the SAME as for the elimination test: substitute a measurable
/// target, chosen among what THIS output advertises, excluding its
/// current size. `None` if no advertised mode differs from it — a
/// degenerate case, see `choisir_cible`.
fn cible_du_temoin(
    nom_sortie: &str,
    before: (u32, u32),
    cible_eliminatoire: (u32, u32),
) -> Option<(u32, u32)> {
    if before != cible_eliminatoire {
        return Some(cible_eliminatoire);
    }
    let annonces = modes_annonces(nom_sortie);
    let substituee = choisir_cible(before, cible_eliminatoire, &annonces);
    if let Some(cible) = substituee {
        tracing::warn!(
            width_before_attempt = before.0,
            height_before_attempt = before.1,
            largeur_cible_eliminatoire = cible_eliminatoire.0,
            hauteur_cible_eliminatoire = cible_eliminatoire.1,
            largeur_cible_temoin = cible.0,
            hauteur_cible_temoin = cible.1,
            "la sortie temoin nait deja a la cible de l'eliminatoire (persistance registre \
             probable) -- cible substituee pour rester mesurable, meme parade que choisir_cible \
             pour l'eliminatoire (defaut F1)"
        );
    }
    substituee
}

/// The name under which the tested output ends up after the elimination round
/// — D8's side unknown no. 3 ("does the output keep its
/// `\\.\DISPLAYn` name?"), and its size at that same instant — the
/// `before_creation` point of the PERSISTENCE check (`persistance::journaliser_verdict`,
/// task 2bis of D9), so as not to read back a second time a topology already
/// in hand.
///
/// First looks for the UNCHANGED name. Failing that, looks for a successor among the
/// names that appeared since the very start of the probe and that are neither the tested
/// output itself nor one of the two neighbours: a single candidate settles it,
/// several or none leave the question open (`<disparue>`, logged
/// separately rather than guessed).
///
/// ⚠️ **Fix (review of task 2bis, I4)**: the size now returns
/// `Option<(u32, u32)>`, NOT `(0, 0)` as a fallback for the `<disparue>` case. A
/// `(0, 0)` fallback made `journaliser_verdict` say that a vanished
/// output had "survived" whenever the other end of the calculation was
/// also `(0, 0)` — `None` makes this confusion impossible by construction.
pub(super) fn nom_apres_tour(
    nom_sortie: &str,
    known_before_all: &HashSet<String>,
    autres_noms_a_nous: &HashSet<String>,
) -> Result<(String, Option<(u32, u32)>)> {
    let releve = relever_topologie("après le tour (inconnues annexes)")?;
    if let Some(sortie) = releve.iter().find(|sortie| sortie.nom_sortie == nom_sortie) {
        return Ok((
            nom_sortie.to_string(),
            Some((sortie.rect.width, sortie.rect.height)),
        ));
    }
    let candidats: Vec<&str> = releve
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !known_before_all.contains(*nom) && !autres_noms_a_nous.contains(*nom))
        .collect();
    if let [seul] = candidats.as_slice() {
        let size = releve
            .iter()
            .find(|sortie| sortie.nom_sortie == *seul)
            .map(|sortie| (sortie.rect.width, sortie.rect.height));
        return Ok((seul.to_string(), size));
    }
    tracing::warn!(
        nom_sortie,
        ?candidats,
        "la sortie testée n'apparaît plus sous son nom d'origine, et aucun successeur univoque \
         ne se dégage -- nom_apres = <disparue>"
    );
    Ok(("<disparue>".to_string(), None))
}
