//! Phase 2: three passes, from 1 to N windows.
//!
//! 1. CONTROL — the test patterns paint, nothing captures. Without this pass, a
//!    drop at six windows would be indistinguishable from a drop of the
//!    test pattern itself: eight swapchains at 60 Hz consume GPU, and that
//!    load would otherwise enter the measurement. Same role as the `lan` profile
//!    of the `netem` bench.
//! 2. CAPTURE — the test patterns paint and the path captures.
//! 3. CAPTURE + ENCODING — one H.264 encoder per window.
//!
//! Halfway through passes 2 and 3, one test pattern comes to cover another:
//! the covered window must keep rendering its pattern. A failure here
//! eliminates the path WITHOUT a frame rate measurement — putting a figure on the speed of a
//! wrong image teaches nothing.
//!
//! No per-frame trace: aggregated counters, logged every second. In
//! the NAT work stream, a per-packet trace written to the CIFS share destroyed the
//! session it was measuring.

use std::time::Instant;

use anyhow::{Context, Result};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::compteurs::{self, Compteurs, DUREE_PASSE, PERIODE_JOURNAL};
use super::mires::Mires;
use super::voies::{VoieDeCapture, VoieDuplication, VoiePrintWindow, VoiesOuvertes};

/// `sortie` designates the DXGI output to measure by its name (`\\.\DISPLAYn`), or
/// `None` for the output that carries the desktop — the original behaviour,
/// unchanged.
pub(super) fn executer(nom_voie: &str, count: u8, sortie: Option<&str>) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&count),
        "MULTIFENETRE_N doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let capture = match sortie {
        Some(nom) => crate::capture::DesktopCapture::sur_sortie(nom)?,
        None => crate::capture::DesktopCapture::new()?,
    };
    let (texture_largeur, texture_hauteur) = capture.desktop_size();

    // The rectangle where the WINDOWS live: the virtual desktop coordinates,
    // as `DXGI_OUTPUT_DESC::DesktopCoordinates` gives them. Without a designated
    // output, it is the desktop at the origin — the previous behaviour.
    let bureau = match sortie {
        Some(nom) => crate::capture::enumerer_sorties()?
            .into_iter()
            .find(|s| s.nom_sortie == nom)
            .map(|s| s.rect)
            .with_context(|| format!("sortie {nom} absente de l'énumération"))?,
        None => Rect {
            x: 0,
            y: 0,
            width: texture_largeur,
            height: texture_hauteur,
        },
    };
    let facteur = crate::moniteurs_virtuels::facteur_echelle(
        (bureau.width, bureau.height),
        (texture_largeur, texture_hauteur),
    )
    .unwrap_or((1.0, 1.0));
    tracing::info!(
        bureau_x = bureau.x,
        bureau_y = bureau.y,
        bureau_largeur = bureau.width,
        bureau_hauteur = bureau.height,
        texture_largeur,
        texture_hauteur,
        facteur_horizontal = facteur.0,
        facteur_vertical = facteur.1,
        "banc : coordonnées de fenêtre et de texture"
    );

    let places = disposition::tuiles(bureau, count as u32).with_context(|| {
        format!(
            "{count} places sur un bureau {}x{}",
            bureau.width, bureau.height
        )
    })?;
    let places_texture: Vec<Rect> = places
        .iter()
        .map(|place| crate::moniteurs_virtuels::vers_texture(*place, bureau, facteur))
        .collect();
    tracing::info!(
        voie = nom_voie,
        count,
        ?places,
        ?places_texture,
        "banc : disposition retenue"
    );

    let mut mires = Mires::ouvrir(capture.device(), &places)?;

    // DXGI duplication is exclusive at the process level: DXGI
    // only allows ONE open duplication at a time on a given
    // output — measured here, not assumed: the "duplication" path (which opens
    // its own in `ouvrir_voies`) failed with 0x80070057 ("parameter
    // incorrect") as long as this one stayed alive. `capture` only served
    // to give its D3D11 device to `Mires::ouvrir` (which keeps its
    // own COM reference-counted clone, independent) and its
    // desktop dimensions, already read above: nothing depends on
    // it anymore from here on, and its OWN duplication must be released
    // before the "duplication" path opens its own.
    drop(capture);

    // `None`: this protocol holds no driver — it receives an output
    // already there and has nothing to beat its watchdog with. Behaviour
    // UNCHANGED by round 1, including its known risk: called by
    // `capture_virtuelle.rs`, the bench runs without a single ping on a virtual
    // output, which that module's header already flags and makes up for
    // afterwards with a survival check.
    compteurs::passe_temoin(&mut mires, None)?;
    let (mut voies, regions) =
        ouvrir_voies(nom_voie, count, &mires, &places, &places_texture, sortie)?;
    let compteurs = passe_capture(&mut mires, &mut voies, &regions, false)?;
    compteurs::journaliser("capture", nom_voie, count, &compteurs);
    if compteurs.apres_recouvrement.faux() > 0 {
        tracing::error!(
            voie = nom_voie,
            verdicts_faux = compteurs.apres_recouvrement.faux(),
            noires = compteurs.apres_recouvrement.noires,
            voisines = compteurs.apres_recouvrement.voisines,
            inconnues = compteurs.apres_recouvrement.inconnues,
            "verdict : ÉLIMINÉE sous recouvrement — la passe d'encodage est sautée"
        );
        return Ok(());
    }
    let compteurs = passe_capture(&mut mires, &mut voies, &regions, true)?;
    compteurs::journaliser("capture+encodage", nom_voie, count, &compteurs);
    // The second suspect, after the encoders: the duplication source that
    // all paths share. Traced separately so that the log
    // distinguishes "died at the encoders" from "died at the duplication".
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}

/// Two layouts, and they are not interchangeable:
/// `places_fenetres` is in virtual desktop coordinates — that is where
/// the windows are, and `PrintWindow` works on the window itself;
/// `places_texture` is in duplicated texture coordinates — that is where
/// `CopySubresourceRegion` crops. They only coincide if the output is
/// not scaled, which is the case of the physical desktop but not
/// necessarily of a virtual output (factor 1.5 noted by the probe).
///
/// Also returns the region RETAINED per path, the one whose dimensions each image will
/// carry. The encoding pass needs it as is: sizing
/// the encoder on the window's slot while the `duplication` path returns
/// an image at the texture's dimensions would make the two diverge as soon as the
/// scale factor differs from 1.
fn ouvrir_voies(
    nom_voie: &str,
    count: u8,
    mires: &Mires,
    places_fenetres: &[Rect],
    places_texture: &[Rect],
    sortie: Option<&str>,
) -> Result<VoiesOuvertes> {
    // What the path shares between its N streams is decided HERE, once: the
    // duplication does not accept being opened N times on the same output, and
    // the D3D11 device of the printwindow path only needs to exist
    // once.
    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    let regions: Vec<Rect> = match nom_voie {
        "duplication" => {
            let partagee = VoieDuplication::partagee_sur(sortie)?;
            for id in 0..count {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoieDuplication::new(partagee.clone()));
                voie.ouvrir(mires.hwnd(id)?, places_texture[id as usize])?;
                voies.push(voie);
            }
            places_texture[..count as usize].to_vec()
        }
        "printwindow" => {
            let (device, contexte) = VoiePrintWindow::partagee()?;
            for id in 0..count {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoiePrintWindow::new(device.clone(), contexte.clone()));
                voie.ouvrir(mires.hwnd(id)?, places_fenetres[id as usize])?;
                voies.push(voie);
            }
            places_fenetres[..count as usize].to_vec()
        }
        autre => {
            anyhow::bail!(
                "voie « {autre} » inconnue du banc — voies câblées : duplication, printwindow"
            )
        }
    };
    Ok((voies, regions))
}

/// `regions` carries, path by path, the dimensions its images will have —
/// those retained by `ouvrir_voies`, and not those of the windows: on a
/// scaled output, the `duplication` path returns images at the
/// dimensions of the TEXTURE, which an encoder sized on the window
/// would refuse.
fn passe_capture(
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    regions: &[Rect],
    with_encoding: bool,
) -> Result<Compteurs> {
    let count = voies.len();
    let mut compteurs = Compteurs::nouveaux(count);
    let mut encodeurs: Vec<crate::encode::H264Encoder> = Vec::new();
    if with_encoding {
        for id in 0..count {
            let place = regions[id];
            // One encoder per window, on the device of ITS path: a
            // texture cannot be submitted to an encoder built on another
            // D3D11 device.
            let appareil = voies[id].device();
            encodeurs.push(crate::encode::H264Encoder::new(
                &appareil,
                (place.width, place.height),
                (place.width, place.height),
                60,
                8_000_000,
            )?);
        }
    }

    let debut = Instant::now();
    let mi_parcours = debut + DUREE_PASSE / 2;
    let mut recouvert = false;
    let epreuve_file_ms: Option<u64> = std::env::var("MULTIFENETRE_EPREUVE_FILE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|ms| *ms > 0);
    let mut eprouve = false;
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut pts = vec![0u64; count];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        // Identifies this tick for shared-source paths
        // (`VoieDuplication`/`SourceDuplication`): lets them
        // acquire that source only once per round, whatever the
        // number of paths that crop it afterwards — see the header
        // comment of `SourceDuplication` (`voies.rs`).
        let tour = mires.trame();

        // Test of the queue imposed on the MFT (`MULTIFENETRE_EPREUVE_FILE_MS`):
        // we clog the queue in the middle of the pass and watch whether the
        // `unites` of the periodic log collapse. It is the only measurement
        // that tells whether the MFT's work goes through this queue — hence whether
        // the idle barrier bears on anything at all. Without the
        // variable, this block does not exist at runtime.
        if !eprouve && Instant::now() >= mi_parcours {
            if let (Some(ms), Some(encodeur)) = (epreuve_file_ms, encodeurs.first()) {
                encodeur.eprouver_file(std::time::Duration::from_millis(ms));
                eprouve = true;
            }
        }

        // Staging of the elimination gate: the last test pattern comes to
        // cover the first.
        if !recouvert && Instant::now() >= mi_parcours && count >= 2 {
            mires.recouvrir(count as u8 - 1, 0)?;
            recouvert = true;
            tracing::info!(
                "recouvrement posé : la mire 0 est sous la mire {}",
                count - 1
            );
        }

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image(tour)? else {
                continue;
            };
            compteurs.images[id] += 1;

            // The check ONLY bears on test pattern 0 — elsewhere it
            // would cost one CPU copy per image without teaching anything — but it
            // bears on the WHOLE pass, before as well as after the covering.
            //
            // It used to run only once the covering was in place:
            // on the physical desktop, that an unobstructed window is captured
            // correctly went without saying, and only the covering was in question.
            // Measurement ③ reverses that — on a virtual output WITHOUT a screen
            // attached, whether Windows composes anything at all is
            // the hypothesis to test. Without the reading from before the covering, an
            // output that rendered only black would give the same "eliminated
            // under covering" as a perfectly composed output, and we would
            // conclude on the wrong defect.
            if id == 0 {
                let verdict = compteurs::lire_verdict(voie.as_mut(), &image, 0)?;
                if recouvert {
                    compteurs.apres_recouvrement.compter(verdict);
                } else {
                    compteurs.before_overlap.compter(verdict);
                }
            }

            if with_encoding {
                encodeurs[id].submit(&image, pts[id])?;
                pts[id] += 90_000 / 60;
                while let Some(_unite) = encodeurs[id].poll_output()? {
                    compteurs.unites[id] += 1;
                }
            }
        }

        if Instant::now() >= prochain_journal {
            tracing::info!(
                images = ?compteurs.images,
                unites = ?compteurs.unites,
                verdicts_faux = compteurs.apres_recouvrement.faux(),
                before = ?compteurs.before_overlap,
                apres = ?compteurs.apres_recouvrement,
                "banc en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }

    // EXPLICIT and traced release, one by one. The inherited defect killed the
    // process here — at release, not at submission — and a `Vec`
    // destroyed implicitly would not have said which of its elements had killed it.
    // ✅ This defect has been diagnosed and fixed since 31 July 2026
    // (`encode::arret`, 0 recurrence over 20 runs of the comparable case); these
    // traces stay, because it is through them that it was seen and nothing
    // proves its absence.
    // These traces are rare by construction (one per encoder, once per
    // pass): they do not violate the "no per-frame trace" rule.
    if !encodeurs.is_empty() {
        tracing::info!(count = encodeurs.len(), "libération des encodeurs : début");
        for (id, encodeur) in encodeurs.drain(..).enumerate() {
            tracing::info!(id, "libération d'un encodeur : avant");
            drop(encodeur);
            tracing::info!(id, "libération d'un encodeur : après");
        }
        tracing::info!("libération des encodeurs : terminée");
    }
    Ok(compteurs)
}
