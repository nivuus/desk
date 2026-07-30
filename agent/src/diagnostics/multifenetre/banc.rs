//! Temps 2 : trois passes, de 1 à N fenêtres.
//!
//! 1. TÉMOIN — les mires peignent, rien ne capture. Sans cette passe, un
//!    décrochage à six fenêtres serait indiscernable d'un décrochage de la
//!    mire elle-même : huit swapchains à 60 Hz consomment du GPU, et cette
//!    charge entrerait sinon dans la mesure. Même rôle que le profil `lan`
//!    du banc `netem`.
//! 2. CAPTURE — les mires peignent et la voie capture.
//! 3. CAPTURE + ENCODAGE — un encodeur H.264 par fenêtre.
//!
//! À mi-parcours des passes 2 et 3, une mire vient en recouvrir une autre :
//! la fenêtre recouverte doit continuer de rendre sa mire. Un échec ici
//! élimine la voie SANS mesure de cadence — chiffrer la vitesse d'une image
//! fausse n'apprend rien.
//!
//! Aucune trace par trame : compteurs agrégés, journalisés à la seconde. Au
//! chantier NAT, une trace par paquet écrite sur le partage CIFS a détruit la
//! session qu'elle mesurait.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::mires::Mires;
use super::voies::{VoieDeCapture, VoieDuplication, VoiePrintWindow};

/// Durée de chaque passe.
const DUREE_PASSE: Duration = Duration::from_secs(10);
/// Cadence de journalisation des compteurs.
const PERIODE_JOURNAL: Duration = Duration::from_secs(1);

struct Compteurs {
    images: Vec<u64>,
    unites: Vec<u64>,
    verdicts_faux: u64,
}

pub(super) fn executer(nom_voie: &str, nombre: u8) -> Result<()> {
    anyhow::ensure!(
        nombre >= 1 && nombre <= mire::MIRES_MAX,
        "MULTIFENETRE_N doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let capture = crate::capture::DesktopCapture::new()?;
    let (largeur, hauteur) = capture.desktop_size();
    let places = disposition::tuiles(
        Rect { x: 0, y: 0, width: largeur, height: hauteur },
        nombre as u32,
    )
    .with_context(|| format!("{nombre} places sur un bureau {largeur}x{hauteur}"))?;
    tracing::info!(voie = nom_voie, nombre, ?places, "banc : disposition retenue");

    let mut mires = Mires::ouvrir(capture.device(), &places)?;

    // La duplication DXGI est exclusive à l'échelle du processus : DXGI
    // n'autorise qu'UNE SEULE duplication ouverte à la fois sur une même
    // sortie — mesuré ici, pas supposé : la voie « duplication » (qui ouvre
    // la sienne dans `ouvrir_voies`) échouait avec 0x80070057 (« paramètre
    // incorrect ») tant que celle-ci restait vivante. `capture` n'a servi
    // qu'à donner son périphérique D3D11 à `Mires::ouvrir` (qui en garde son
    // propre clone à comptage de références COM, indépendant) et ses
    // dimensions de bureau, déjà lues ci-dessus : rien ne dépend plus
    // d'elle à partir d'ici, et sa PROPRE duplication doit être relâchée
    // avant que la voie « duplication » n'ouvre la sienne.
    drop(capture);

    passe_temoin(&mut mires)?;
    let mut voies = ouvrir_voies(nom_voie, nombre, &mires, &places)?;
    let compteurs = passe_capture(&mut mires, &mut voies, false)?;
    journaliser("capture", nom_voie, nombre, &compteurs);
    if compteurs.verdicts_faux > 0 {
        tracing::error!(
            voie = nom_voie,
            verdicts_faux = compteurs.verdicts_faux,
            "verdict : ÉLIMINÉE sous recouvrement — la passe d'encodage est sautée"
        );
        return Ok(());
    }
    let compteurs = passe_capture(&mut mires, &mut voies, true)?;
    journaliser("capture+encodage", nom_voie, nombre, &compteurs);
    Ok(())
}

fn ouvrir_voies(
    nom_voie: &str,
    nombre: u8,
    mires: &Mires,
    places: &[Rect],
) -> Result<Vec<Box<dyn VoieDeCapture>>> {
    // Ce que la voie partage entre ses N flux est décidé ICI, une fois : la
    // duplication n'accepte pas d'être ouverte N fois sur la même sortie, et
    // le périphérique D3D11 de la voie printwindow n'a besoin d'exister
    // qu'une fois.
    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    match nom_voie {
        "duplication" => {
            let partagee = VoieDuplication::partagee()?;
            for id in 0..nombre {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoieDuplication::nouvelle(partagee.clone()));
                voie.ouvrir(mires.hwnd(id)?, places[id as usize])?;
                voies.push(voie);
            }
        }
        "printwindow" => {
            let (device, contexte) = VoiePrintWindow::partagee()?;
            for id in 0..nombre {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoiePrintWindow::nouvelle(device.clone(), contexte.clone()));
                voie.ouvrir(mires.hwnd(id)?, places[id as usize])?;
                voies.push(voie);
            }
        }
        autre => {
            anyhow::bail!(
                "voie « {autre} » inconnue du banc — voies câblées : duplication, printwindow"
            )
        }
    }
    Ok(voies)
}

/// Passe témoin : les mires peignent, rien ne capture.
fn passe_temoin(mires: &mut Mires) -> Result<()> {
    let debut = Instant::now();
    let mut trames = 0u64;
    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        trames += 1;
    }
    let secondes = debut.elapsed().as_secs_f64();
    tracing::info!(
        mires = mires.nombre(),
        trames,
        cadence = trames as f64 / secondes,
        "passe TÉMOIN — cadence de peinture sans capture"
    );
    Ok(())
}

fn passe_capture(
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    avec_encodage: bool,
) -> Result<Compteurs> {
    let nombre = voies.len();
    let mut compteurs = Compteurs {
        images: vec![0; nombre],
        unites: vec![0; nombre],
        verdicts_faux: 0,
    };
    let mut encodeurs: Vec<crate::encode::H264Encoder> = Vec::new();
    if avec_encodage {
        for id in 0..nombre {
            let place = mires.place(id as u8)?;
            // Un encodeur par fenêtre, sur le périphérique de SA voie : une
            // texture ne se soumet pas à un encodeur bâti sur un autre
            // périphérique D3D11.
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
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();

        // Mise en scène de la porte éliminatoire : la dernière mire vient
        // recouvrir la première.
        if !recouvert && Instant::now() >= mi_parcours && nombre >= 2 {
            mires.recouvrir(nombre as u8 - 1, 0)?;
            recouvert = true;
            tracing::info!("recouvrement posé : la mire 0 est sous la mire {}", nombre - 1);
        }

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image()? else {
                continue;
            };
            compteurs.images[id] += 1;

            // La vérification ne porte QUE sur la fenêtre recouverte, et
            // seulement une fois le recouvrement posé : ailleurs elle
            // n'apprendrait rien et coûterait une copie CPU par image.
            if recouvert && id == 0 {
                let appareil = voie.device();
                let (r, g, b, _a) = crate::diagnostics::pixels::read_pixel(
                    &appareil,
                    &image.texture,
                    image.width,
                    image.height,
                    image.width / 2,
                    image.height / 2,
                )?;
                if mire::verdict(0, (r, g, b)) != mire::Verdict::Juste {
                    compteurs.verdicts_faux += 1;
                }
            }

            if avec_encodage {
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
                verdicts_faux = compteurs.verdicts_faux,
                "banc en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }
    Ok(compteurs)
}

fn journaliser(passe: &str, voie: &str, nombre: u8, compteurs: &Compteurs) {
    let secondes = DUREE_PASSE.as_secs_f64();
    let cadences: Vec<f64> = compteurs.images.iter().map(|n| *n as f64 / secondes).collect();
    tracing::info!(
        passe,
        voie,
        nombre,
        ?cadences,
        unites = ?compteurs.unites,
        verdicts_faux = compteurs.verdicts_faux,
        "passe terminée"
    );
}
