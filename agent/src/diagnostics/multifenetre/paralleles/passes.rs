//! The pass loop of the multi-output protocol: opening of the N
//! duplications, cadence, encoding.
//!
//! Separated from `paralleles.rs`, which is *the output driver* (creation,
//! designation, survival check), when round 1 brought the
//! single file to 527 lines. The split is not cosmetic: the project's
//! 500-line ceiling required it, and spec §6 provided for it at this
//! precise place.
//!
//! What lives here runs UNDER the `moniteurs_virtuels::Sorties` guard held by
//! the caller: any error that comes out of it goes through the destruction of the
//! outputs, including the refusal of `DuplicateOutput`, which is the expected result
//! of this work stream.

use std::time::Instant;

use anyhow::{Context, Result};

use super::super::compteurs::{self, Compteurs, Garde, DUREE_PASSE, PERIODE_JOURNAL};
use super::super::mires::Mires;
use super::super::voies::{creer_device, VoieDeCapture, VoieDuplication, VoiesOuvertes};
use super::constater_survie;
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// Opens one DXGI duplication per output.
///
/// **A failure here is THE RESULT of this work stream, not a malfunction.** If the Kth
/// `DuplicateOutput` is refused, the rank, the bare HRESULT and the targeted output
/// are logged, then **the original error is propagated**, enriched with the rank
/// reached: it is the answer to the question asked. Never swallow it, never
/// retry it, and never replace it with a reconstructed message — the chain
/// of causes carries the HRESULT, which is the substance of the answer.
///
/// The watchdog is beaten at each rank, **before** the solicitation: at
/// N=8 this loop opens eight duplications, and the time it takes
/// would otherwise add up to the gap of the control pass.
///
/// **What this beat bounds, and what it no longer bounds.** It reasoned about
/// an instantaneous opening; since task 11 bis, `partagee_sur` goes through
/// `DesktopCapture::sur_sortie`, which retries during `DUREE_FENETRE_OUVERTURE`
/// and can therefore **block for up to 3 s** without being able to ping during that
/// time. Beating just before bounds the gap to the duration of ONE rank — not to zero,
/// and 3 s remains of the same order as the driver's `delai = 3`, of unknown unit.
fn ouvrir_duplications(
    garde: &mut Garde<'_>,
    virtuelles: &[SortieDxgi],
    mires: &Mires,
) -> Result<VoiesOuvertes> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        garde.battre()?;
        match VoieDuplication::partagee_sur(Some(&sortie.nom_sortie)) {
            Ok(source) => {
                let dimensions = source.borrow().dimensions_bureau();
                tracing::info!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    texture_largeur = dimensions.0,
                    texture_hauteur = dimensions.1,
                    "duplication ouverte"
                );
                textures.push(dimensions);
                sources.push(source);
            }
            Err(erreur) => {
                // Full chain formatted WITHOUT consuming the error (`{:#}`):
                // the log must carry the HRESULT AND the error must still
                // be propagated as is just below. The helper
                // `multifenetre::causes` does not fit here, it takes its
                // error by value.
                let chaine = format!("{erreur:#}");
                tracing::error!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    causes = %chaine,
                    "duplication REFUSÉE — c'est le résultat de la mesure, pas une panne"
                );
                let nom = sortie.nom_sortie.clone();
                // `Err(erreur).with_context(…)` and not `bail!`: the
                // reconstructed message of `bail!` lost the HRESULT, which is the substance
                // of the answer to the question asked.
                return Err(erreur).with_context(|| {
                    format!(
                        "{rang} duplications DXGI ouvertes de front, la {}ᵉ refusée (sortie {nom})",
                        rang + 1
                    )
                });
            }
        }
    }

    let rects: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let places = crate::moniteurs_virtuels::places_texture_par_sortie(&rects, &textures)?;

    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    for (id, source) in sources.into_iter().enumerate() {
        let mut voie: Box<dyn VoieDeCapture> = Box::new(VoieDuplication::nouvelle(source));
        voie.ouvrir(mires.hwnd(id as u8)?, places[id])?;
        voies.push(voie);
    }
    Ok((voies, places))
}

/// `regions` carries, path by path, the dimensions its images will have —
/// those of its output's TEXTURE, and not those of its window. The two
/// only coincide if the output is not scaled: the probe noted
/// a DPI factor of 1.5 on a virtual output, and an encoder sized
/// on the window would then refuse the images the path submits to it.
fn passe_capture(
    garde: &mut Garde<'_>,
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    regions: &[Rect],
    avec_encodage: bool,
) -> Result<Compteurs> {
    let nombre = voies.len();
    let mut compteurs = Compteurs::nouveaux(nombre);
    let mut encodeurs: Vec<crate::encode::H264Encoder> = Vec::new();
    if avec_encodage {
        for id in 0..nombre {
            // Building eight Media Foundation encoders takes an unbounded time,
            // and it runs BEFORE the pass loop (hence its 1 Hz ping)
            // starts: without this beat, it is a second gap.
            garde.battre()?;
            let place = regions[id];
            // One encoder per window, on the device of ITS path: a
            // texture cannot be submitted to an encoder built on another
            // D3D11 device. Here each path has its own, one duplication
            // per output creating one device per output.
            let appareil = voies[id].device();
            encodeurs.push(
                crate::encode::H264Encoder::new(
                    &appareil,
                    (place.width, place.height),
                    (place.width, place.height),
                    60,
                    8_000_000,
                )
                .with_context(|| format!("encodeur n°{}", id + 1))?,
            );
        }
    }

    let debut = Instant::now();
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        let tour = mires.trame();
        // The path checked at this round, and it alone: one reading per round
        // whatever N (see `mire::voie_controlee`).
        let controlee = mire::voie_controlee(tour, nombre);

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image(tour)? else {
                continue;
            };
            compteurs.images[id] += 1;

            if controlee == Some(id) {
                // The EXPECTED identity is that of the test pattern placed on THIS
                // output: a `Voisine(j)` verdict says that path i captured
                // output j, the cross-pairing this set-up must
                // detect.
                let verdict = compteurs::lire_verdict(voie.as_mut(), &image, id as u8)?;
                compteurs.apres_recouvrement.compter(verdict);
            }

            if avec_encodage {
                encodeurs[id].submit(&image, pts[id])?;
                pts[id] += 90_000 / 60;
                while let Some(_unite) = encodeurs[id].poll_output()? {
                    compteurs.unites[id] += 1;
                }
            }
        }

        garde.battre_si_du()?;
        // Periodic logging, every second: no per-frame trace.
        // Written here rather than shared with `banc.rs` — the two protocols
        // do not observe the same thing (this one has neither covering nor
        // elimination gate, hence no "before" or "after" to distinguish),
        // and factoring them would require logging empty fields on one
        // side or the other.
        if Instant::now() >= prochain_journal {
            tracing::info!(
                images = ?compteurs.images,
                unites = ?compteurs.unites,
                verdicts = ?compteurs.apres_recouvrement,
                "banc parallèle en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }

    // EXPLICIT and traced release, one by one: a `Vec` destroyed
    // implicitly would not say which of its elements killed the process.
    // These traces are rare by construction (one per encoder, once per
    // pass): they do not violate the "no per-frame trace" rule.
    if !encodeurs.is_empty() {
        // Key for reading the `unites`, surveyed BEFORE the release. A FINDING, and
        // nothing more: the rest of the `images` go into `dropped_stale_nv12`
        // (an image converted then discarded because a more recent one
        // arrived before the encoder claimed it). This survey closes
        // the arithmetic — `images = unites + nv12_ecartees + at most 1 in flight`
        // — so no image disappears without being counted, and it shows that
        // the ratio is the SAME at all four ranks: it does not come from
        // parallelism.
        //
        // Do NOT attribute this ratio to the ratio between the configured 60 fps
        // and the ~90 fps submitted: the logs refute it. A
        // 90/60 ratio would predict 600 units for 900 images; the periodic
        // lines show 45 per second for 90 images, that is
        // exactly half, linear over the ten intervals
        // (`paralleles-n1.log:40-49`). The cause of this one-half ratio is
        // NOT established, and this measurement does not need it.
        let ecartees: Vec<u64> = encodeurs
            .iter()
            .map(|encodeur| {
                encodeur
                    .telemetry()
                    .dropped_stale_nv12
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .collect();
        tracing::info!(
            nv12_ecartees = ?ecartees,
            "images converties puis écartées, encodeur par encodeur — l'écart entre \
             « images » et « unites » se lit ici, pas dans le parallélisme"
        );
        tracing::info!(nombre = encodeurs.len(), "libération des encodeurs : début");
        for (id, encodeur) in encodeurs.drain(..).enumerate() {
            tracing::info!(id, "libération d'un encodeur : avant");
            drop(encodeur);
            tracing::info!(id, "libération d'un encodeur : après");
        }
        tracing::info!("libération des encodeurs : terminée");
    }
    Ok(compteurs)
}

/// The three passes, in order.
///
/// **No elimination gate between the last two**, unlike the
/// single-output bench: without covering, a wrong verdict does not invalidate the
/// frame rate measurement, it qualifies it. Both passes always run, and the
/// report reads the verdicts.
pub(super) fn executer_passes(garde: &mut Garde<'_>, virtuelles: &[SortieDxgi]) -> Result<()> {
    // The test patterns' device does NOT come from a provisional
    // `DesktopCapture`: DXGI only allows one duplication per output, and the
    // provisional one would make the real one fail with 0x80070057.
    let (device, _contexte) = creer_device()?;
    // The test patterns live in VIRTUAL DESKTOP coordinates — the rectangles
    // announced by DXGI. The paths crop in TEXTURE coordinates, which
    // `ouvrir_duplications` computes. Confusing them would shift everything by a
    // DPI factor.
    let places_bureau: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let mut mires = Mires::ouvrir(&device, &places_bureau)?;

    // The guard is passed: this pass lasts ten seconds, and without it it
    // was a gap without a single ping (11.1 s measured in round 1, control and
    // opening of the duplications included).
    compteurs::passe_temoin(&mut mires, Some(garde))?;
    constater_survie("témoin", virtuelles);

    let (mut voies, places_texture) = ouvrir_duplications(garde, virtuelles, &mires)?;
    tracing::info!(
        nombre = voies.len(),
        ?places_bureau,
        ?places_texture,
        "les N duplications sont ouvertes de front"
    );

    // Key for reading the log. `compteurs::journaliser` carries the labels of the
    // single-output protocol, and they are kept as is so that these
    // surveys stay `grep`-able together with those already committed in `docs/`. Here:
    // `mire0_avant_recouvrement` is always empty (no covering is
    // staged), and `mire0_apres_recouvrement` carries ALL the verdicts of
    // the rotation, on all paths — not only those of test pattern 0.
    let nombre = voies.len() as u8;
    // A survival check AFTER EACH PASS, and not a single one at the end: an
    // output removed during the "capture" pass would only be noticed after
    // "capture+encodage", and both surveys would be equally suspect
    // with no way of saying which one is affected.
    let releve = passe_capture(garde, &mut mires, &mut voies, &places_texture, false)?;
    compteurs::journaliser("capture", "duplication-parallele", nombre, &releve);
    constater_survie("capture", virtuelles);

    let releve = passe_capture(garde, &mut mires, &mut voies, &places_texture, true)?;
    compteurs::journaliser("capture+encodage", "duplication-parallele", nombre, &releve);
    constater_survie("capture+encodage", virtuelles);

    // The duplications were the second suspect of the inherited defect, after the
    // encoders. The defect has since been identified by its stack (a MFT work
    // item still in flight, `encode::arret`) and fixed: these two
    // traces no longer look for a culprit, they bracket the release — a
    // crash here would otherwise stay silent.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}
