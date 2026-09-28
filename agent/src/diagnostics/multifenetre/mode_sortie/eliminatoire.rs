//! The ELIMINATION round (step 2, D9 brief): tries the known combinations
//! on the output UNDER TEST, DXGI duplication open and held, until
//! the DXGI read-back confirms a movement, or until exhaustion.
//!
//! Extracted from `mode_sortie.rs` at task 2bis of sub-block D9, for the
//! 500-line ceiling (`CLAUDE.md`) — same reason as `temoin.rs`,
//! `voisines.rs` and `combinaisons.rs`, extracted at task 1.

use anyhow::Result;
use windows::Win32::Graphics::Gdi::DISP_CHANGE_SUCCESSFUL;

use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use super::combinaisons::{appliquer_combo, combos_du_tour};
use super::persistance::combinaison_imposee;
use super::voisines::DuplicationVoisine;
use super::{choisir_cible, modes_annonces};
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// What a round of combinations established.
pub(super) struct ResultatTour {
    /// Numeric target actually aimed at during the round, or `None` if
    /// no measurable target could be chosen (`P1 NON MESURABLE`) — the
    /// control then has nothing to replay, see its caller.
    pub(super) cible: Option<(u32, u32)>,
    /// The arm that made the output move, if there is one.
    pub(super) gagnante: Option<&'static str>,
    /// Number of attempts where at least one neighbour lost access to its
    /// duplication during the round — see
    /// `voisines::DuplicationVoisine::sonder` for the exact granularity.
    pub(super) pertes_voisines: u32,
    /// The RAW value of `MULTIFENETRE_MODE_SORTIE_DRAPEAUX` (what
    /// the operator REQUESTED), to be distinguished from `gagnante` (what
    /// REALLY happened) — see `persistance::journaliser_verdict`, which
    /// logs both separately since the review of task 2bis.
    pub(super) combinaison_imposee: Option<String>,
}

/// Tries the known combinations until the DXGI read-back confirms
/// the target, or until exhaustion. Always returns `Ok`: a refusal — or
/// the impossibility of choosing a measurable target — is a measurement, not an
/// error of the probe; see the header comment of `mode_sortie.rs`. Only
/// a topology that became unreadable raises an error.
///
/// `avant_a_la_creation` is the size read by DXGI right after the creation
/// of the output under test, several seconds before this round
/// runs — see the FRESH read-back below, which replaces it as the
/// movement reference.
///
/// `voisines`: probed once PER ATTEMPT (see `DuplicationVoisine::sonder`)
/// — it is D8's side unknown no. 2 surveyed at the same moment as
/// the elimination test, not a separate measurement.
///
/// The round is restricted to ONE arm if the operator imposes it (task 2bis, D9,
/// `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`) — see `persistance::combinaison_imposee`
/// and `combinaisons::combos_du_tour`. Without it, unchanged behaviour: the
/// four combos, in order.
pub(super) fn essayer_les_modes(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    avant_a_la_creation: (u32, u32),
    demande: (u32, u32),
    voisines: &mut [DuplicationVoisine],
) -> Result<ResultatTour> {
    // The RAW value requested by the operator, captured ONCE and
    // reused at both exit points (`P1 NON MESURABLE` included) --
    // see the `combinaison_imposee` field of `ResultatTour`.
    let imposee = combinaison_imposee();

    // ⚠️ **Fix (review of task 2bis, Critical).** `avant_a_la_creation`
    // was captured several seconds earlier, BEFORE the opening of the
    // duplication and the creation of the two neighbours. Between these two instants,
    // the output may have moved WITHOUT ANY API call: `p-persistance-2`
    // and the invalid control of task 2bis both show `DISPLAY5`
    // going from 1280×720 to 2560×1440 before any `ChangeDisplaySettingsExW`
    // of THIS round -- a residue of registry pollution from an earlier
    // run, apparently reapplied at the creation of a neighbouring virtual
    // output (unexplained mechanism, doctrine D1/D2 of
    // abandoning the mutex in another form). Comparing `mouvement` against
    // the STALE value would pass this residue off as an effect OF this round.
    // The fresh read-back now serves as the reference EVERYWHERE in this
    // function (`choisir_cible` included); `avant_a_la_creation` only serves
    // to detect and log the gap.
    //
    // ⚠️ **Fix (review of task 2bis, second pass, point 12).**
    // `.unwrap_or(avant_a_la_creation)` was the same hole as the one
    // this fix closes everywhere else (I4/I13): if the SUT is
    // ABSENT from the fresh read-back, the old fallback returned
    // `avant == avant_a_la_creation` BY CONSTRUCTION, and the gap `WARN`
    // below could then NEVER trigger -- a
    // vanished output read as "no gap detected", exactly
    // the opposite. Absence is now an event logged SEPARATELY
    // (`tracing::error!`, never confused with the "present and
    // unchanged" case), before even falling back to the creation value.
    let releve_frais = relever_topologie("juste avant le premier essai (relecture fraîche)")?;
    let taille_fraiche = releve_frais
        .iter()
        .find(|sortie| sortie.nom_sortie == nom_sortie)
        .map(|sortie| (sortie.rect.width, sortie.rect.height));
    let avant = match taille_fraiche {
        Some(taille) => taille,
        None => {
            tracing::error!(
                nom_sortie,
                largeur_a_la_creation = avant_a_la_creation.0,
                hauteur_a_la_creation = avant_a_la_creation.1,
                "la sortie sous test est ABSENTE de la relecture fraiche -- repli sur la taille \
                 de creation, mais ceci N'EST PAS un 'aucun ecart detecte' : c'est une anomalie \
                 distincte, journalisee ici pour ne jamais se confondre avec elle"
            );
            avant_a_la_creation
        }
    };
    if avant != avant_a_la_creation {
        tracing::warn!(
            nom_sortie,
            largeur_a_la_creation = avant_a_la_creation.0,
            hauteur_a_la_creation = avant_a_la_creation.1,
            largeur_fraiche = avant.0,
            hauteur_fraiche = avant.1,
            "la sortie a bouge SANS appel d'API entre sa creation et ce tour -- residu probable \
             d'une execution anterieure ; la relecture FRAICHE sert desormais de reference"
        );
    }

    let annonces = modes_annonces(nom_sortie);
    tracing::info!(
        nombre = annonces.len(),
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        contient_demande = annonces.contains(&demande),
        demande_egale_avant = demande == avant,
        modes = ?annonces,
        "modes annonces (EnumDisplaySettingsExW) avant tout changement"
    );

    let cible = match choisir_cible(avant, demande, &annonces) {
        Some(cible) => cible,
        None => {
            tracing::error!(
                verdict = "P1 NON MESURABLE",
                raison = "aucun mode annonce ne differe de la taille courante",
                largeur_avant_tentative = avant.0,
                hauteur_avant_tentative = avant.1,
                largeur_demandee = demande.0,
                hauteur_demandee = demande.1,
                modes = ?annonces,
                "verdict P1 : mesure impossible, aucune tentative effectuee"
            );
            return Ok(ResultatTour {
                cible: None,
                gagnante: None,
                pertes_voisines: 0,
                combinaison_imposee: imposee,
            });
        }
    };
    if cible == demande {
        tracing::info!(
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "cible retenue = resolution demandee (differe deja de la taille courante)"
        );
    } else {
        tracing::warn!(
            largeur_demandee = demande.0,
            hauteur_demandee = demande.1,
            largeur_avant_tentative = avant.0,
            hauteur_avant_tentative = avant.1,
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "la resolution demandee egale deja la taille courante (persistance registre probable \
             d'une execution anterieure) -- cible substituee dynamiquement parmi les modes annonces"
        );
    }

    let mut dernier_code = 0i32;
    // ⚠️ **Fix (review of task 2bis, Important I5).** Initialised to
    // `avant` (fresh) and no longer `(0, 0)`: on an EMPTY round (no combo
    // matches `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`, see
    // `combinaisons::combos_du_tour`), `derniere_taille` stayed at `(0, 0)`
    // and `mouvement_observe = derniere_taille != avant` was almost
    // always `true` -- a round that had tried NOTHING displayed a
    // movement. With `avant` as the resting value, the absence of any attempt
    // translates into the absence of movement, without special code.
    let mut derniere_taille = avant;
    let mut gagnante: Option<&'static str> = None;
    let mut cible_exacte_atteinte = false;
    let mut pertes_voisines = 0u32;
    // Counts the attempts REALLY made -- distinct from
    // `gagnante.is_some()`: an EMPTY round (0 attempts) and a round that
    // exhausted its arms without success (N attempts, 0 successes) looked alike
    // until now (`gagnante = None` in both cases). See the
    // "P1 NON TENTE" verdict below.
    let mut tentatives = 0u32;
    for combo in combos_du_tour(imposee.as_deref()) {
        tentatives += 1;
        dernier_code = appliquer_combo(nom_sortie, cible.0, cible.1, &combo);
        // Windows reconfigures its display topology asynchronously —
        // exactly why `montee.rs` observes the same grace delay
        // after a creation. Querying DXGI too early would conclude to a
        // refusal where there is only a delay, and beat the watchdog
        // during the wait as the rest of this module does.
        attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
        // Probed HERE, after the wait: if a loss occurs at any
        // instant of the window that just elapsed, the neighbour's
        // duplication instance still carries it at the moment of this
        // solicitation (see `DuplicationVoisine::sonder`) -- one probe per
        // attempt is enough to detect it.
        for voisine in voisines.iter_mut() {
            if voisine.sonder() {
                pertes_voisines += 1;
            }
        }
        let releve = relever_topologie(&format!("après tentative « {} »", combo.etiquette()))?;
        let taille_lue = releve
            .iter()
            .find(|sortie| sortie.nom_sortie == nom_sortie)
            .map(|sortie| (sortie.rect.width, sortie.rect.height));
        // ⚠️ **Fix (review of task 2bis, second pass, point 13).**
        // The old `.unwrap_or((0, 0))` fallback turned an output ABSENT
        // after an attempt into a FALSE movement in nearly all cases
        // (`(0, 0) != avant` almost always) -- an arm could have
        // "won" (`gagnante = Some(...)`, verdict "P1 RECU") on the
        // mere disappearance of the output, never on a real size
        // change. Absence is now an anomaly logged
        // separately that CANNOT make this arm win: `derniere_taille`
        // keeps the last value REALLY read (the one before this
        // attempt, or `avant` at the first iteration).
        let Some(taille_lue) = taille_lue else {
            tracing::error!(
                etiquette = combo.etiquette(),
                nom_sortie,
                "la sortie sous test est ABSENTE de la relecture apres cette tentative -- \
                 aucun mouvement ne peut en etre conclu, ce bras ne peut pas gagner"
            );
            continue;
        };
        derniere_taille = taille_lue;
        // The criterion that counts is MOVEMENT (`derniere_taille != avant`),
        // not equality with the chosen target — see the header comment of the
        // parent module (F1 defect fixed). `cible_atteinte` remains
        // logged, separately: it documents whether the driver honours the exact
        // requested value, a finer question than P1, never the one that
        // decides the verdict.
        let mouvement = derniere_taille != avant;
        let cible_atteinte = derniere_taille == cible;
        tracing::info!(
            etiquette = combo.etiquette(),
            code_brut = dernier_code,
            api_annonce_succes = (dernier_code == DISP_CHANGE_SUCCESSFUL.0),
            largeur_relue = derniere_taille.0,
            hauteur_relue = derniere_taille.1,
            mouvement,
            cible_atteinte,
            "relecture DXGI (GetDesc/DesktopCoordinates) apres la tentative"
        );
        if mouvement {
            gagnante = Some(combo.etiquette());
            cible_exacte_atteinte = cible_atteinte;
            break;
        }
    }

    // ⚠️ **Fix (review of task 2bis, I5/I6).** An EMPTY round
    // (`tentatives == 0`, the `combos_du_tour` filter having retained nothing)
    // returned "P1 REFUSE" -- an absence of attempt presented as a survey,
    // exactly the confusion that the repository's doctrine (D9, task 2bis I6)
    // forbids. "P1 NON TENTE" distinguishes it from a REAL refusal (N attempts,
    // 0 successes).
    let verdict = if gagnante.is_some() {
        "P1 RECU"
    } else if tentatives == 0 {
        "P1 NON TENTE"
    } else {
        "P1 REFUSE"
    };
    tracing::info!(
        verdict,
        combinaison_imposee = ?imposee,
        combinaison_gagnante = ?gagnante,
        tentatives_effectuees = tentatives,
        code_brut_dernier_essai = dernier_code,
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        largeur_relue = derniere_taille.0,
        hauteur_relue = derniere_taille.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        // The verdict itself: was a movement (A != B) observed?
        // It is THIS field that governs "P1 RECU" above, not an equality with
        // the target (see the header comment of the parent module, F1 defect
        // fixed) -- recomputed here, redundant with `gagnante.is_some()` by
        // construction, so that a reader of the log does not have to deduce it.
        // On an EMPTY round, `derniere_taille == avant` by construction
        // (see its initialisation): `mouvement_observe` is `false`,
        // never `true` by default (fix I5).
        mouvement_observe = derniere_taille != avant,
        // Secondary: did the driver honour the EXACT requested value, or
        // did it stop at an intermediate mode? Can be `false` with
        // a "P1 RECU" verdict -- it is not a contradiction, it is a
        // finer question than P1's.
        cible_exacte_atteinte,
        pertes_acces_voisines_pendant_le_tour = pertes_voisines,
        "verdict P1 : une sortie virtuelle accepte-t-elle un autre mode que celui de sa creation"
    );
    Ok(ResultatTour {
        cible: Some(cible),
        gagnante,
        pertes_voisines,
        combinaison_imposee: imposee,
    })
}
