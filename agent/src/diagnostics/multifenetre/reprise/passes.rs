//! The two passes of the recovery bench, and the disruption that separates them.
//!
//! Separated from `reprise.rs`, which is *the output driver* — same split as
//! `paralleles`, and for the same reason (the project's 500-line ceiling). The
//! diagnostic that decides between the two ways of dying lives next to it, in
//! `post_mortem.rs`.
//!
//! What lives here runs UNDER the `moniteurs_virtuels::Sorties` guard held by
//! the caller: any error that comes out of it goes through the destruction of the
//! outputs, disruptor included.
//!
//! # Key for reading the log — two words there have a meaning they do not have here
//!
//! `compteurs::journaliser` carries the labels of the SINGLE-OUTPUT protocol, and they
//! are kept as is so that these surveys stay `grep`-able together with those
//! already committed in `docs/`. On this bench:
//!
//! - **`pattern0_before_overlap` is always empty**, and
//!   `mire0_apres_recouvrement` carries **ALL** the verdicts of the rotation, on
//!   all paths — not only those of test pattern 0. No covering is
//!   staged here: one window per output, nothing can hide
//!   another. There is therefore **no elimination gate**, and a wrong verdict does not
//!   disqualify the pass, it qualifies it.
//! - **"before / after" is NOT "before / after covering".** The
//!   `passe` field of these lines is "avant perturbation" / "après
//!   perturbation", and the disruption in question is the **creation of one
//!   more virtual output**. The two vocabularies look alike and do not
//!   speak of the same thing.
//! - **`unites` stays at zero**: this bench does not encode.
//!
//! The path label is `duplication-reprise`, never `duplication`: the
//! series of the fixed-area bench and those of this set-up are comparable
//! on no figure (`CLAUDE.md`), and a single `grep` bucket would invite
//! precisely comparing them. `paralleles` named itself `duplication-parallele`
//! for this exact reason.

use std::collections::HashSet;
use std::time::Instant;

use anyhow::{Context, Result};

use super::super::compteurs::{self, Compteurs, Garde, DUREE_PASSE, PERIODE_JOURNAL};
use super::super::mires::Mires;
use super::super::montee::RESOLUTION;
use super::super::voies::{create_device, VoieDeCapture, VoieDuplication, VoiesOuvertes};
use super::constater_places;
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// What a pass returns: its counters, **and** the list of dead paths.
///
/// Both separately, on purpose. A `Result<Compteurs>` would throw away the
/// counters as soon as a path dies — yet that is precisely the case this bench
/// exists to observe. How many images each path returned before
/// dying, and which one died, IS the result of the measurement.
struct Passe {
    compteurs: Compteurs,
    /// Paths whose capture was permanently lost during this pass,
    /// hence which stopped being solicited. Empty = none died.
    perdues: Vec<usize>,
    /// True if the pass ran in degraded mode (test pattern painting or pixel
    /// reading lost). Carried up to the summary: a summary read alone must say
    /// that it does not describe a nominal pass.
    degradee: bool,
}

/// The two passes and the disruption that separates them.
pub(super) fn eprouver(
    garde: &mut Garde<'_>,
    sorties: &mut crate::moniteurs_virtuels::Sorties<'_>,
    virtuelles: &[SortieDxgi],
    connues: &HashSet<String>,
) -> Result<()> {
    // The test patterns' device does NOT come from a provisional
    // `DesktopCapture`: DXGI only allows one duplication per output, and the
    // provisional one would make the real one fail with 0x80070057.
    let (device, _contexte) = create_device()?;
    // The test patterns live in VIRTUAL DESKTOP coordinates — the rectangles
    // announced by DXGI. The paths crop in TEXTURE coordinates, which
    // `ouvrir_duplications` computes. Confusing them would shift everything by a
    // DPI factor.
    let places_bureau: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let mut mires = Mires::ouvrir(&device, &places_bureau)?;

    let (mut voies, places_texture) = ouvrir_duplications(garde, virtuelles, &mires)?;
    let count = voies.len();
    tracing::info!(
        count,
        ?places_bureau,
        ?places_texture,
        "les k duplications sont ouvertes"
    );

    // --- Pass A: the control. It measures what duplications that
    // nothing disturbs return, and it is the only reference that pass B
    // is set against.
    let passe_a = passe(garde, &mut mires, &mut voies, virtuelles, None)?;
    compteurs::journaliser(
        "avant perturbation",
        "duplication-reprise",
        count as u8,
        &passe_a.compteurs,
    );
    // A path already dead BEFORE any disruption invalidates the measurement: what
    // followed would no longer be attributable to the output creation. The refusal is
    // taken here, where it can be read.
    anyhow::ensure!(
        passe_a.perdues.is_empty(),
        "{:?} : ces voies ont perdu leur capture AVANT toute perturbation — rien n'a été \
         perturbé, ce banc n'a rien éprouvé",
        passe_a.perdues
    );

    // --- La perturbation.
    let (largeur, hauteur, hertz) = RESOLUTION;
    tracing::info!(
        duplications_ouvertes = voies.len(),
        "création d'une sortie de PLUS pendant que les duplications tournent — c'est la \
         perturbation mesurée"
    );
    let id_perturbatrice = sorties
        .create(largeur, hauteur, hertz)
        .context("création de la sortie perturbatrice")?;
    let instant_perturbation = Instant::now();
    tracing::info!(id = id_perturbatrice, "sortie perturbatrice créée");

    // --- Pass B: identical to pass A, on a desktop that has just changed.
    let passe_b = passe(
        garde,
        &mut mires,
        &mut voies,
        virtuelles,
        Some(instant_perturbation),
    )?;
    compteurs::journaliser(
        "après perturbation",
        "duplication-reprise",
        count as u8,
        &passe_b.compteurs,
    );
    // Two findings that decide what the summary means: did the
    // disruptor attach (otherwise nothing could have disrupted), and did the k
    // outputs move under their test patterns (otherwise a black image would be
    // attributable to the move and not to recovery).
    constater_places("après perturbation", virtuelles, connues);

    // The duplications are released BEFORE the post-mortem probe: DXGI
    // only allows one duplication per output, and the object of a dead path —
    // invalid but very much alive — would make the reopening the probe
    // attempts be refused. The probe would then conclude "the output would not re-duplicate"
    // for a reason that has nothing to do with recovery.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    // Counting boundary, emitted UNCONDITIONALLY so that the command of
    // reading key no. 1 holds in all cases. The post-mortem probe also captures
    // through `next_frame`, which sets the same reopening trace: without
    // this boundary, a global `grep -c` would mix the recoveries OF THE MEASUREMENT and
    // those of the probe, which come after it and prove nothing about the
    // disruption.
    tracing::info!(
        "borne de comptage des reprises — au-dessus de cette ligne, les lignes de réouverture \
         appartiennent à la MESURE ; en dessous, à la sonde post-mortem et aux clés de lecture"
    );
    if !passe_b.perdues.is_empty() {
        super::post_mortem::sonder(garde, &mut mires, virtuelles, &passe_b.perdues);
    }

    // --- The summary, the only output that counts.
    //
    // The decisive figure is the number of paths that STILL return images
    // after the disruption, path by path: a total would mask a dead
    // path compensated by another.
    let vivantes_apres = passe_b.compteurs.images.iter().filter(|n| **n > 0).count();
    tracing::info!(
        frames_before = ?passe_a.compteurs.images,
        images_apres = ?passe_b.compteurs.images,
        voies_vivantes_apres = vivantes_apres,
        voies_totales = count,
        voies_perdues_apres = ?passe_b.perdues,
        false_verdicts_before = passe_a.compteurs.apres_recouvrement.faux(),
        verdicts_faux_apres = passe_b.compteurs.apres_recouvrement.faux(),
        pass_before_degraded = passe_a.degradee,
        passe_apres_degradee = passe_b.degradee,
        "bilan de la reprise"
    );
    if passe_a.degradee || passe_b.degradee {
        tracing::error!(
            pass_before_degraded = passe_a.degradee,
            passe_apres_degradee = passe_b.degradee,
            "une passe au moins a tourné en mode DÉGRADÉ — dans ce bilan, `images_*`, les \
             cadences et les `verdicts_faux_*` de la ou des passes concernées sont \
             INEXPLOITABLES, et `voies_vivantes_apres` ne se lit qu'avec `voies_perdues_apres`. \
             Voir la ligne d'erreur qui nomme ce qui a été perdu"
        );
    }
    journaliser_cle_de_lecture();
    Ok(())
}

/// What must have been read before concluding anything from the summary.
///
/// The bench is a stop point: its two possible wrong readings are costly
/// **in both directions**, and neither shows in the summary's figures
/// alone. This line names them.
///
/// The number of recoveries is NOT counted here, on purpose: it is already in the
/// lines `DesktopCapture::next_frame` sets at each reopening, and a
/// second counter would say the same thing in another way — hence one day something
/// else. The **per-path** breakdown spec §6.1 asks for is done on the
/// `cible` field of these same lines, which carries `Sortie("\\.\DISPLAYn")`, to
/// be matched with the `nom_sortie` of the death lines.
fn journaliser_cle_de_lecture() {
    tracing::info!(
        "clé de lecture n°1 — ce bilan ne vaut QUE si la perturbation a perturbé. Compter les \
         reprises DE LA MESURE, et elles seules : \
         `sed '/borne de comptage des reprises/q' <journal> | grep -c \"perdu, réouverture\"`. \
         Un `grep -c` sur le journal ENTIER serait faux dans le sens dangereux : il compterait \
         aussi les réouvertures de la sonde post-mortem, postérieures à la mesure, plus cette \
         clé de lecture elle-même. ZÉRO ligne signifie qu'aucune duplication n'a perdu son \
         accès, donc que ce banc n'a RIEN éprouvé, et son bilan ne se lit alors PAS comme un \
         succès de la reprise. Contrôler aussi la ligne « sorties tierces relevées » : sans \
         perturbatrice ATTACHÉE, rien n'a pu perturber"
    );
    tracing::info!(
        "clé de lecture n°2 — des voies mortes ne réfutent PAS la reprise à elles seules. La \
         reprise est désormais une FENÊTRE de DUREE_FENETRE_REPRISE (8 s), retentée au plus \
         toutes les PAS_REPRISE (150 ms) ; l'expiration se compte depuis l'OUVERTURE de la \
         fenêtre, pas depuis la dernière tentative, et tout succès d'acquisition — « rien de \
         neuf » compris — la referme. Une réouverture qui échoue à retrouver la sortie n'est PLUS \
         définitive : elle consomme une tentative ET écrit désormais sa propre ligne d'échec, la \
         fenêtre continuant de courir. Sur une fenêtre pleine, jusqu'à 54 tentatives sont \
         possibles (8000 ms / 150 ms), chacune pouvant poser jusqu'à deux lignes (tentative puis \
         échec) : voir plusieurs dizaines de lignes de réouverture pour UNE SEULE voie morte n'est \
         donc PAS un emballement, c'est la fenêtre qui court normalement jusqu'à expiration. C'est \
         ce que départage la « sonde post-mortem », qui retente UNE fois la topologie stabilisée. \
         Ventiler les reprises par voie avec le champ `cible` des lignes de réouverture, à \
         rapprocher du `nom_sortie` des lignes de mort"
    );
}

/// Opens one DXGI duplication per output, and places a test pattern on each.
///
/// Same loop as `paralleles/passes.rs::ouvrir_duplications`, but **not the
/// same reading of a failure**: there, a refusal of the Kth `DuplicateOutput` IS
/// the measured result (the work stream was looking for this ceiling); here it is a bench
/// failure, occurring before any disruption, hence before anything at all
/// had been tested. The two loops would not say the same thing about the same
/// HRESULT: sharing them would force choosing one of the two statements.
///
/// The watchdog is beaten at each rank, **before** the solicitation: the
/// time to open k duplications would otherwise add up to the last ping gap.
///
/// **What this beat bounds, and what it no longer bounds.** It reasoned about
/// an instantaneous opening; since task 11 bis, `partagee_sur` goes through
/// `DesktopCapture::sur_sortie`, which retries during `DUREE_FENETRE_OUVERTURE`
/// and can therefore **block for up to 3 s** without being able to ping during that
/// time. Beating just before resets the counter and bounds the gap to the
/// duration of ONE rank — it does not cancel it, and 3 s remains of the same order as the
/// driver's `delai = 3`, of unknown unit. It is the bound reachable without
/// changing the opening semantics.
fn ouvrir_duplications(
    garde: &mut Garde<'_>,
    virtuelles: &[SortieDxgi],
    mires: &Mires,
) -> Result<VoiesOuvertes> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        garde.battre()?;
        let source =
            VoieDuplication::partagee_sur(Some(&sortie.nom_sortie)).with_context(|| {
                format!(
                    "ouverture de la duplication n°{} (sortie {}) — avant toute perturbation",
                    rang + 1,
                    sortie.nom_sortie
                )
            })?;
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

    let rects: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let places = crate::moniteurs_virtuels::places_texture_par_sortie(&rects, &textures)?;

    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    for (id, source) in sources.into_iter().enumerate() {
        let mut voie: Box<dyn VoieDeCapture> = Box::new(VoieDuplication::new(source));
        voie.ouvrir(mires.hwnd(id as u8)?, places[id])?;
        voies.push(voie);
    }
    Ok((voies, places))
}

/// One capture pass of `DUREE_PASSE`, with a rotating check.
///
/// **Nothing that DXGI turbulence can break interrupts the pass.**
/// It is the methodological point of this bench, and it holds for the three calls that
/// can fail under a topology being reshuffled:
///
/// - `prochaine_image` — the path is marked dead and stops being solicited,
///   the others continue. Abandoning the whole pass at the first failure
///   would truncate the counters of the surviving paths at the instant of the death of
///   that one, and the summary would declare them dead too: the exact
///   confusion that a path-by-path reading exists to prevent.
/// - `mires.peindre` (`Present` on a swapchain) and `lire_verdict`
///   (`read_pixel` on a texture that has just been reopened) — reported
///   once, the pass continues in degraded mode. A `?` there would make
///   `eprouver` exit without ever emitting the summary, that is it would lose the measurement
///   at the precise moment it becomes interesting. **What degraded mode
///   costs is named, not minimised**: lost painting makes the image count
///   and the frame rate unusable (the desktop no longer changes); lost reading
///   makes the verdicts unusable. In both cases the PATH DEATHS
///   remain valid, and that is what the stop point depends on.
///
/// Only the watchdog remains fatal: without it the driver can take back its
/// outputs under the measurement, and nothing that followed would be attributable.
///
/// The test patterns paint at each round, in both passes: Desktop Duplication
/// only emits an image when the desktop changes, and a still test pattern would make
/// all acquisitions return `WAIT_TIMEOUT` — we would measure zero images
/// and conclude to a failure.
///
/// `perturbation` dates the instant the extra output was created, so that
/// each death carries its `ms_depuis_perturbation`: without it, distinguishing a
/// burst from a lasting state requires cross-checking timestamps by hand.
/// `None` for the control pass, where the field has no meaning.
fn passe(
    garde: &mut Garde<'_>,
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    virtuelles: &[SortieDxgi],
    perturbation: Option<Instant>,
) -> Result<Passe> {
    let count = voies.len();
    let mut compteurs = Compteurs::nouveaux(count);
    let mut vivantes = vec![true; count];
    let mut peinture_signalee = false;
    let mut lecture_signalee = false;
    // The round number is kept HERE, and not read from `Mires::trame`.
    //
    // **This is not a detail of style, it is what prevents a false
    // POSITIVE on `voies_vivantes_apres`.** `Mires::peindre` only increments its
    // frame after having presented all its windows: a failure leaves it
    // FROZEN. A pass that read `mires.trame()` would then call
    // `prochaine_image` again with the same `tour`, where `SourceDuplication::amorcer`
    // short-circuits — no more calls to `next_frame`. Two consequences,
    // both wrong in the dangerous direction: the last primed image
    // would be recounted at each iteration of the tight loop (`images`
    // races instead of freezing), and above all **a really dead path
    // could no longer learn it** — never marked lost, counted alive
    // in the summary, and never probed by the post-mortem probe. A local counter
    // advances whatever happens: `next_frame` keeps being called, so
    // deaths stay detected, and a desktop that no longer changes simply
    // returns `Ok(None)`.
    //
    // Nothing depends on this counter being equal to the painted frame:
    // `amorcer` only uses it as a cache key, `mire::voie_controlee` as a
    // rotation, and `mire::verdict` accepts both parities of green.
    let mut tour: u64 = 0;

    let debut = Instant::now();
    let mut prochain_journal = debut + PERIODE_JOURNAL;

    while debut.elapsed() < DUREE_PASSE {
        tour += 1;
        if let Err(error) = mires.peindre() {
            if !peinture_signalee {
                peinture_signalee = true;
                tracing::error!(
                    causes = %super::super::causes(error),
                    // This message asserted "the test patterns no longer change, THEREFORE the
                    // desktop does not either, so the image count stops
                    // advancing" — beyond its survey. `Mires::peindre`
                    // fails at the first faulty `Present`, AFTER having presented
                    // the previous ones: nothing guarantees that the duplication
                    // stops emitting for those paths. Painting is
                    // partial OR nil, and that is all we know about it.
                    "peinture des mires perdue — la passe continue en mode DÉGRADÉ. La peinture \
                     est PARTIELLE ou NULLE à partir d'ici : `peindre` échoue au premier \
                     `Present` fautif, après avoir présenté les mires précédentes, et rien ne dit \
                     lesquelles changent encore. La CADENCE et le compte d'images de cette passe \
                     ne mesurent donc plus la capture — ni justes, ni « sous-estimés » d'un \
                     facteur connu. Ce qui reste valable : les morts de voies, `next_frame` \
                     continuant d'être appelée à chaque tour"
                );
            }
        }
        mires.pomper();
        // The path checked at this round, and it alone: one reading per round
        // whatever k (see `mire::voie_controlee`).
        let controlee = mire::voie_controlee(tour, count);

        for (id, voie) in voies.iter_mut().enumerate() {
            if !vivantes[id] {
                continue;
            }
            let image = match voie.prochaine_image(tour) {
                Ok(Some(image)) => image,
                // Nothing new on this desktop at this instant: the common case,
                // not an error.
                Ok(None) => continue,
                Err(error) => {
                    vivantes[id] = false;
                    // One line per dead path, at most k for the whole pass:
                    // it is not a per-image trace.
                    tracing::error!(
                        voie = id,
                        nom_sortie = %virtuelles[id].nom_sortie,
                        ms_depuis_perturbation = ?perturbation.map(|t| t.elapsed().as_millis() as u64),
                        frames_before_death = compteurs.images[id],
                        causes = %super::super::causes(error),
                        "capture définitivement perdue sur cette voie — elle cesse d'être \
                         sollicitée, les autres continuent. Ne PAS en conclure que la reprise \
                         est impossible avant d'avoir lu la sonde post-mortem"
                    );
                    continue;
                }
            };
            compteurs.images[id] += 1;

            if controlee == Some(id) {
                // The EXPECTED identity is that of the test pattern placed on THIS
                // output: a `Voisine(j)` verdict says that path i captured
                // output j. After a reopening, it is the check that tells
                // whether the path came back to ITS output and not to another —
                // the risk specific to recovery in a topology that has just
                // changed.
                let verdict = match compteurs::lire_verdict(voie.as_mut(), &image, id as u8) {
                    Ok(verdict) => verdict,
                    Err(error) => {
                        if !lecture_signalee {
                            lecture_signalee = true;
                            tracing::error!(
                                voie = id,
                                causes = %super::super::causes(error),
                                "lecture de pixel perdue — comptée « Inconnue » et la passe \
                                 continue : les verdicts de cette passe sont DÉGRADÉS, leur \
                                 nombre de faux ne dit plus rien de la justesse des images"
                            );
                        }
                        mire::Verdict::Inconnue
                    }
                };
                compteurs.apres_recouvrement.compter(verdict);
            }
        }

        garde.battre_si_du()?;
        // Periodic logging, every second: no per-frame trace.
        if Instant::now() >= prochain_journal {
            tracing::info!(
                images = ?compteurs.images,
                verdicts = ?compteurs.apres_recouvrement,
                "banc de reprise en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }

    let perdues: Vec<usize> = vivantes
        .iter()
        .enumerate()
        .filter(|(_, vivante)| !**vivante)
        .map(|(id, _)| id)
        .collect();
    Ok(Passe {
        compteurs,
        perdues,
        degradee: peinture_signalee || lecture_signalee,
    })
}
