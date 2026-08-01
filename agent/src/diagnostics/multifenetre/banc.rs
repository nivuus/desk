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

use std::time::Instant;

use anyhow::{Context, Result};

use crate::disposition;
use crate::geometry::Rect;
use crate::mire;

use super::compteurs::{self, Compteurs, DUREE_PASSE, PERIODE_JOURNAL};
use super::mires::Mires;
use super::voies::{VoieDeCapture, VoieDuplication, VoiePrintWindow};

/// `sortie` désigne la sortie DXGI à mesurer par son nom (`\\.\DISPLAYn`), ou
/// `None` pour la sortie qui porte le bureau — le comportement d'origine,
/// inchangé.
pub(super) fn executer(nom_voie: &str, nombre: u8, sortie: Option<&str>) -> Result<()> {
    anyhow::ensure!(
        nombre >= 1 && nombre <= mire::MIRES_MAX,
        "MULTIFENETRE_N doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let capture = match sortie {
        Some(nom) => crate::capture::DesktopCapture::sur_sortie(nom)?,
        None => crate::capture::DesktopCapture::new()?,
    };
    let (texture_largeur, texture_hauteur) = capture.desktop_size();

    // Le rectangle où vivent les FENÊTRES : les coordonnées du bureau virtuel,
    // telles que `DXGI_OUTPUT_DESC::DesktopCoordinates` les donne. Sans sortie
    // désignée, c'est le bureau à l'origine — le comportement d'avant.
    let bureau = match sortie {
        Some(nom) => crate::capture::enumerer_sorties()?
            .into_iter()
            .find(|s| s.nom_sortie == nom)
            .map(|s| s.rect)
            .with_context(|| format!("sortie {nom} absente de l'énumération"))?,
        None => Rect { x: 0, y: 0, width: texture_largeur, height: texture_hauteur },
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

    let places = disposition::tuiles(bureau, nombre as u32).with_context(|| {
        format!("{nombre} places sur un bureau {}x{}", bureau.width, bureau.height)
    })?;
    let places_texture: Vec<Rect> = places
        .iter()
        .map(|place| crate::moniteurs_virtuels::vers_texture(*place, bureau, facteur))
        .collect();
    tracing::info!(
        voie = nom_voie,
        nombre,
        ?places,
        ?places_texture,
        "banc : disposition retenue"
    );

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

    // `None` : ce protocole ne détient aucun pilote — il reçoit une sortie
    // déjà là et n'a pas de quoi battre son chien de garde. Comportement
    // INCHANGÉ par la ronde 1, y compris son risque connu : appelé par
    // `capture_virtuelle.rs`, le banc tourne sans un seul ping sur une sortie
    // virtuelle, ce que l'en-tête de ce module-là signale déjà et rattrape
    // après coup par un contrôle de survie.
    compteurs::passe_temoin(&mut mires, None)?;
    let (mut voies, regions) =
        ouvrir_voies(nom_voie, nombre, &mires, &places, &places_texture, sortie)?;
    let compteurs = passe_capture(&mut mires, &mut voies, &regions, false)?;
    compteurs::journaliser("capture", nom_voie, nombre, &compteurs);
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
    compteurs::journaliser("capture+encodage", nom_voie, nombre, &compteurs);
    // Le second suspect, après les encodeurs : la source de duplication que
    // toutes les voies partagent. Tracé séparément pour que le journal
    // distingue « mort aux encodeurs » de « mort à la duplication ».
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}

/// Deux dispositions, et elles ne sont pas interchangeables :
/// `places_fenetres` est en coordonnées du bureau virtuel — c'est là que sont
/// les fenêtres, et `PrintWindow` travaille sur la fenêtre elle-même ;
/// `places_texture` est en coordonnées de la texture dupliquée — c'est là que
/// recadre `CopySubresourceRegion`. Elles ne coïncident que si la sortie n'est
/// pas mise à l'échelle, ce qui est le cas du bureau physique mais pas
/// nécessairement d'une sortie virtuelle (facteur 1,5 relevé par la sonde).
///
/// Rend aussi la région RETENUE par voie, celle dont chaque image portera les
/// dimensions. La passe d'encodage en a besoin telle quelle : dimensionner
/// l'encodeur sur la place de la fenêtre alors que la voie `duplication` rend
/// une image aux dimensions de la texture ferait diverger les deux dès que le
/// facteur d'échelle diffère de 1.
fn ouvrir_voies(
    nom_voie: &str,
    nombre: u8,
    mires: &Mires,
    places_fenetres: &[Rect],
    places_texture: &[Rect],
    sortie: Option<&str>,
) -> Result<(Vec<Box<dyn VoieDeCapture>>, Vec<Rect>)> {
    // Ce que la voie partage entre ses N flux est décidé ICI, une fois : la
    // duplication n'accepte pas d'être ouverte N fois sur la même sortie, et
    // le périphérique D3D11 de la voie printwindow n'a besoin d'exister
    // qu'une fois.
    let mut voies: Vec<Box<dyn VoieDeCapture>> = Vec::new();
    let regions: Vec<Rect> = match nom_voie {
        "duplication" => {
            let partagee = VoieDuplication::partagee_sur(sortie)?;
            for id in 0..nombre {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoieDuplication::nouvelle(partagee.clone()));
                voie.ouvrir(mires.hwnd(id)?, places_texture[id as usize])?;
                voies.push(voie);
            }
            places_texture[..nombre as usize].to_vec()
        }
        "printwindow" => {
            let (device, contexte) = VoiePrintWindow::partagee()?;
            for id in 0..nombre {
                let mut voie: Box<dyn VoieDeCapture> =
                    Box::new(VoiePrintWindow::nouvelle(device.clone(), contexte.clone()));
                voie.ouvrir(mires.hwnd(id)?, places_fenetres[id as usize])?;
                voies.push(voie);
            }
            places_fenetres[..nombre as usize].to_vec()
        }
        autre => {
            anyhow::bail!(
                "voie « {autre} » inconnue du banc — voies câblées : duplication, printwindow"
            )
        }
    };
    Ok((voies, regions))
}

/// `regions` porte, voie par voie, les dimensions que ses images auront —
/// celles retenues par `ouvrir_voies`, et non celles des fenêtres : sur une
/// sortie mise à l'échelle, la voie `duplication` rend des images aux
/// dimensions de la TEXTURE, qu'un encodeur dimensionné sur la fenêtre
/// refuserait.
fn passe_capture(
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
            let place = regions[id];
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
    let epreuve_file_ms: Option<u64> = std::env::var("MULTIFENETRE_EPREUVE_FILE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|ms| *ms > 0);
    let mut eprouve = false;
    let mut prochain_journal = debut + PERIODE_JOURNAL;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        // Identifie ce tick pour les voies à source partagée
        // (`VoieDuplication`/`SourceDuplication`) : leur permet de
        // n'acquérir cette source qu'une fois par tour, quel que soit le
        // nombre de voies qui la recadrent ensuite — voir le commentaire de
        // tête de `SourceDuplication` (`voies.rs`).
        let tour = mires.trame();

        // Épreuve de la file imposée à la MFT (`MULTIFENETRE_EPREUVE_FILE_MS`) :
        // on bouche la file au milieu de la passe et l'on regarde si les
        // `unites` du journal périodique s'effondrent. C'est la seule mesure
        // qui dise si le travail de la MFT transite par cette file — donc si
        // la barrière de mise au repos porte sur quoi que ce soit. Hors
        // variable, ce bloc n'existe pas à l'exécution.
        if !eprouve && Instant::now() >= mi_parcours {
            if let (Some(ms), Some(encodeur)) = (epreuve_file_ms, encodeurs.first()) {
                encodeur.eprouver_file(std::time::Duration::from_millis(ms));
                eprouve = true;
            }
        }

        // Mise en scène de la porte éliminatoire : la dernière mire vient
        // recouvrir la première.
        if !recouvert && Instant::now() >= mi_parcours && nombre >= 2 {
            mires.recouvrir(nombre as u8 - 1, 0)?;
            recouvert = true;
            tracing::info!("recouvrement posé : la mire 0 est sous la mire {}", nombre - 1);
        }

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image(tour)? else {
                continue;
            };
            compteurs.images[id] += 1;

            // La vérification ne porte QUE sur la mire 0 — ailleurs elle
            // coûterait une copie CPU par image sans rien apprendre — mais elle
            // porte sur TOUTE la passe, avant comme après le recouvrement.
            //
            // Elle ne courait auparavant qu'une fois le recouvrement posé :
            // sur le bureau physique, qu'une fenêtre dégagée soit capturée
            // juste allait de soi, et seul le recouvrement était en question.
            // La mesure ③ renverse cela — sur une sortie virtuelle SANS écran
            // attaché, que Windows compose seulement quelque chose est
            // l'hypothèse à éprouver. Sans le relevé d'avant recouvrement, une
            // sortie qui ne rendrait que du noir donnerait le même « éliminée
            // sous recouvrement » qu'une sortie parfaitement composée, et l'on
            // conclurait au mauvais défaut.
            if id == 0 {
                let verdict = compteurs::lire_verdict(voie.as_mut(), &image, 0)?;
                if recouvert {
                    compteurs.apres_recouvrement.compter(verdict);
                } else {
                    compteurs.avant_recouvrement.compter(verdict);
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
                verdicts_faux = compteurs.apres_recouvrement.faux(),
                avant = ?compteurs.avant_recouvrement,
                apres = ?compteurs.apres_recouvrement,
                "banc en cours"
            );
            prochain_journal += PERIODE_JOURNAL;
        }
    }

    // Libération EXPLICITE et tracée, une par une. Le défaut hérité tuait le
    // processus ici — au relâchement, pas à la soumission — et un `Vec`
    // détruit implicitement n'aurait pas dit lequel de ses éléments avait tué.
    // ✅ Ce défaut est diagnostiqué et corrigé depuis le 31 juillet 2026
    // (`encode::arret`, 0 récidive sur 20 exécutions du cas comparable) ; ces
    // traces restent, parce que c'est par elles qu'on l'a vu et que rien ne
    // prouve son absence.
    // Ces traces sont rares par construction (une par encodeur, une fois par
    // passe) : elles ne violent pas la règle « aucune trace par trame ».
    if !encodeurs.is_empty() {
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
