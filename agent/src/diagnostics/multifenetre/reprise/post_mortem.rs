//! The post-mortem probe of the recovery bench: retry, once, what the
//! dead paths did not manage in a burst.
//!
//! A separate module rather than a function of `passes.rs`: it is not a
//! measurement pass but a **diagnostic on the measurement**, it counts nothing, and it
//! only runs on the path where something died. (The project's 500-line
//! ceiling required it anyway.)

use std::time::{Duration, Instant};

use super::super::compteurs::Garde;
use super::super::mires::Mires;
use super::super::montee::DELAI_TOPOLOGIE;
use crate::capture::{DesktopCapture, SortieDxgi};
use crate::geometry::Rect;

/// Duration during which the probe solicits a reopened duplication before
/// concluding that it returns nothing.
///
/// The test patterns paint meanwhile: Desktop Duplication only emits an image
/// when the desktop changes, and a probe that did not paint would conclude to
/// silence on a perfectly alive duplication.
const DUREE_SONDE_IMAGE: Duration = Duration::from_millis(500);

/// Retries **only once**, with the topology stabilised, what a path did not
/// manage to restore within its recovery window (`FenetreDeReprise`,
/// `DUREE_FENETRE_REPRISE` = 8 s, `PAS_REPRISE` = 150 ms — see
/// `capture::reprise`).
///
/// **Born from a calibration defect, fixed since (task 6 bis) — the probe
/// remains useful.** Before the fix, `next_frame` only granted 3
/// CONSECUTIVE reopenings WITHOUT ANY DELAY (`AcquireNextFrame(0, …)`
/// then immediate `rouvrir()`), exhausted in 14 to 21 ms where the repository
/// elsewhere waits `DELAI_TOPOLOGIE` = 3 s for a topology to stabilise: the
/// work stream's stop point had been triggered by this under-calibration, not
/// by a recovery that was really impossible. The 8 s window covers this
/// duration twice over, but remains an uncalibrated UPPER BOUND (`CLAUDE.md`):
/// nothing excludes that a particularly slow topology also escapes the
/// 8 s.
///
/// This probe therefore remains the line that separates "recovery is impossible on
/// this hardware" from "the recovery window was not enough". The
/// disruption falls just before the first `AcquireNextFrame` of pass
/// B, that is at the most unstable moment, and a path can die within
/// its recovery window without recovery itself being refuted:
///
/// - the 8 s window can expire while the topology has still
///   not stabilised;
/// - a reopening can fail to find `\\.\DISPLAYn` several times in
///   a row during the reshuffle — each failure now consumes an
///   attempt AND writes its own line ("réouverture de la duplication
///   échouée …"), without it being final: only the expiry of the
///   whole window is.
///
/// In both cases the summary shows `voies_vivantes_apres = 0`, whose
/// natural reading is "path refuted". One line must be enough to separate
/// "recovery is impossible on this hardware" from "the window was not
/// enough", and it is the one this probe writes.
///
/// Never fails: it is a diagnostic, not a measurement. It assumes the
/// bench's duplications already released (see its caller).
pub(super) fn sonder(
    garde: &mut Garde<'_>,
    mires: &mut Mires,
    virtuelles: &[SortieDxgi],
    perdues: &[usize],
) {
    tracing::info!(
        voies = ?perdues,
        attente_ms = DELAI_TOPOLOGIE.as_millis() as u64,
        "sonde post-mortem : attente de stabilisation de la topologie avant de retenter"
    );
    let debut = Instant::now();
    while debut.elapsed() < DELAI_TOPOLOGIE {
        std::thread::sleep(Duration::from_millis(100));
        if let Err(erreur) = garde.battre_si_du() {
            tracing::error!(
                causes = %super::super::causes(erreur),
                "sonde post-mortem : chien de garde perdu pendant l'attente — le pilote peut \
                 avoir repris ses sorties, ce qui suit n'est plus imputable"
            );
            return;
        }
    }

    for &id in perdues {
        // Beat BEFORE the solicitation, and not only after.
        //
        // `DesktopCapture::sur_sortie` retries its duplication during
        // `DUREE_FENETRE_OUVERTURE`: it **blocks for up to 3 s** and cannot
        // be pinged during that time. The only beat of this loop
        // was the one following the solicitation, and the failure branch
        // SKIPPED it through its `continue`: with eight lost paths — the nominal case
        // of this probe — that was eight times 3 s end to end, i.e. ~24 s without
        // a ping, where `compteurs::CADENCE_PING` is 1 s precisely because
        // the driver's `delai = 3` is of UNKNOWN unit, the second not
        // excluded. The driver would have taken back its outputs UNDER the probe, and the
        // probe that serves to decide between two refutations would have become
        // unattributable without anything saying so.
        //
        // Resetting the counter just before the block bounds the gap to the
        // duration of ONE `sur_sortie` (~3 s), whatever path is taken afterwards:
        // the `continue` goes back through here. The 3 s themselves remain
        // irreducible without changing the opening semantics — which the final
        // review is not the moment to do.
        if let Err(erreur) = garde.battre_si_du() {
            tracing::error!(
                voie = id,
                causes = %super::super::causes(erreur),
                "sonde post-mortem : chien de garde perdu avant de solliciter cette voie — le \
                 pilote peut avoir repris ses sorties, ce qui suit n'est plus imputable"
            );
            return;
        }
        let nom = virtuelles[id].nom_sortie.as_str();
        let mut capture = match DesktopCapture::sur_sortie(nom) {
            Ok(capture) => capture,
            Err(erreur) => {
                // Beaten here AS WELL, and not only at the next round: the trace
                // below is not free, and the `continue` must
                // leave no path without a beat.
                let _ = garde.battre_si_du();
                tracing::error!(
                    voie = id,
                    nom_sortie = nom,
                    causes = %super::super::causes(erreur),
                    "sonde post-mortem : la sortie ne se redupliquait PAS une fois la topologie \
                     stabilisée — la mort de cette voie n'est pas imputable au seul budget de \
                     reprises"
                );
                continue;
            }
        };
        let (largeur, hauteur) = capture.desktop_size();
        let region = Rect {
            x: 0,
            y: 0,
            width: largeur,
            height: hauteur,
        };

        // The test patterns paint during the solicitation: without a change of the
        // desktop, a live duplication would return nothing and the probe
        // would wrongly conclude to silence.
        //
        // Hence the tracking of painting failure, and not a `let _ =`: a
        // dead painting produces EXACTLY the "no image" symptom, and
        // returning it under a result label would pass off a failure of
        // the instrument as a measurement. It is the class of defect this bench
        // tracks everywhere else.
        let mut issue = Ok(false);
        let mut peinture_perdue = false;
        let debut = Instant::now();
        while debut.elapsed() < DUREE_SONDE_IMAGE {
            if mires.peindre().is_err() {
                peinture_perdue = true;
            }
            mires.pomper();
            match capture.next_frame(region) {
                Ok(Some(_)) => {
                    issue = Ok(true);
                    break;
                }
                Ok(None) => {}
                Err(erreur) => {
                    issue = Err(format!("{erreur}"));
                    break;
                }
            }
        }
        let _ = garde.battre_si_du();

        match issue {
            Ok(true) => tracing::info!(
                voie = id,
                nom_sortie = nom,
                // This message named "3 attempts in a burst and without delay" —
                // the calibration from BEFORE task 6 bis, which the header
                // comment of this file nevertheless declares fixed two screens
                // above. What was not enough, now, is the recovery
                // WINDOW; its duration is therefore logged instead of being stated.
                fenetre_reprise_ms =
                    crate::capture_reprise::DUREE_FENETRE_REPRISE.as_millis() as u64,
                "sonde post-mortem : la sortie se REDUPLIQUAIT et rendait une image une fois la \
                 topologie stabilisée — la mort de cette voie ne réfute PAS la reprise, elle \
                 dit que la fenêtre de reprise n'a pas suffi sur ce remaniement de topologie"
            ),
            // Silence has two possible causes, and only one is a
            // result: naming them separately is the whole point of tracking
            // `peinture_perdue`.
            Ok(false) if peinture_perdue => tracing::error!(
                voie = id,
                nom_sortie = nom,
                duree_ms = DUREE_SONDE_IMAGE.as_millis() as u64,
                "sonde post-mortem SANS VALEUR sur cette voie : la peinture des mires a échoué \
                 pendant la sollicitation, donc le bureau n'a pas changé. « Aucune image » est \
                 ici une panne de l'INSTRUMENT et ne dit rien de la duplication"
            ),
            Ok(false) => tracing::warn!(
                voie = id,
                nom_sortie = nom,
                duree_ms = DUREE_SONDE_IMAGE.as_millis() as u64,
                "sonde post-mortem : duplication rouverte, mires peintes, mais AUCUNE image \
                 dans le délai — ni une réfutation ni une confirmation, la duplication existe \
                 sans rien rendre"
            ),
            Err(causes) => tracing::error!(
                voie = id,
                nom_sortie = nom,
                causes,
                "sonde post-mortem : duplication rouverte, mais l'acquisition échouait ENCORE \
                 une fois la topologie stabilisée — la mort de cette voie n'est pas imputable \
                 au seul budget de reprises"
            ),
        }
    }
}
