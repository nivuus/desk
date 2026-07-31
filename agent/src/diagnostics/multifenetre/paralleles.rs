//! N sorties virtuelles, une fenêtre et une duplication DXGI chacune.
//!
//! **C'est l'arrangement que la voie recommandée du chantier D propose
//! réellement**, et que rien n'avait exercé : le banc de la sonde et la mesure
//! ③ ont tous deux posé N fenêtres sur UNE sortie. DXGI n'autorisant qu'une
//! duplication par sortie, N duplications de front est une question ouverte,
//! pas un détail d'implémentation.
//!
//! Second écart avec tout ce qui précède : chaque sortie fait 1280×720, donc
//! **l'aire totale croît avec N**. Les cadences de la sonde étaient prises à
//! aire totale fixe (`disposition::tuiles` découpe un bureau), où le débit de
//! pixels est quasi constant par construction et où le nombre de fenêtres
//! n'est pas prouvé neutre en soi.
//!
//! Pas de recouvrement ici — une fenêtre par sortie, rien ne peut en cacher
//! une autre — donc pas de porte éliminatoire. Le risque est l'appariement :
//! que la voie *i* capture la sortie *j*, ou du noir. C'est ce que la rotation
//! du contrôle (`mire::voie_controlee`) détecte, pour une lecture par tour.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::compteurs::{self, Compteurs, DUREE_PASSE, PERIODE_JOURNAL};
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use super::moniteurs::PiloteParIoctl;
use super::voies::{creer_device, VoieDeCapture, VoieDuplication};
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// Cadence de ping du chien de garde du pilote pendant les passes.
///
/// Le banc mono-sortie ne pingue pas : il tenait trente secondes sur UNE
/// sortie, et `capture_virtuelle.rs` signale ce point comme un risque assumé,
/// rattrapé après coup par un contrôle de survie. Ici huit sorties sont
/// exposées, sous un chien de garde dont **l'unité reste inconnue — aucune
/// n'est exclue, pas même la seconde**. Une sortie retirée sous la mesure
/// ferait imputer à Windows un défaut du protocole.
const CADENCE_PING: Duration = Duration::from_secs(1);

pub(super) fn mesurer(nombre: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&nombre),
        "MULTIFENETRE_VDD_PARALLELE doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: HashSet<String> =
        avant.iter().map(|sortie| sortie.nom_sortie.clone()).collect();

    let pilote = super::moniteurs::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : les sorties doivent être détruites AVANT
    // le relevé final, sans quoi celui-ci décrirait un état transitoire.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        for rang in 1..=nombre {
            let id = sorties
                .creer(largeur, hauteur, hertz)
                .with_context(|| format!("création de la sortie virtuelle n°{rang}"))?;
            tracing::info!(rang, id, "sortie virtuelle créée");
        }
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("après création")?;
        let virtuelles = designer_sorties_neuves(&apres, &connues, nombre)?;
        pilote.pinguer()?;
        let issue = executer_passes(&pilote, &virtuelles);
        constater_survie(&virtuelles);
        issue
    };

    // Second essai des retraits que la garde n'a pas obtenus : dernière chance
    // de CE processus, au-delà seule la purge inter-processus les atteindra.
    let rejoues = super::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // Une sortie virtuelle survit au processus. Ce contrôle reste celui du
    // processus mesureur, donc juge et partie — le contrôle qui vaut est un
    // relevé `MULTIFENETRE_DXGI=1` depuis un processus neuf, après coup.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let final_ = relever_topologie("après destruction")?;
    let noms_final = noms_attaches(&final_);
    if noms_final == noms_avant {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}

/// Retrouve les `nombre` sorties que cette sonde vient de créer, par
/// DIFFÉRENCE D'ENSEMBLES DE NOMS.
///
/// Pas par index : DXGI renumérote ses sorties à chaque reconfiguration de
/// topologie. Pas par cardinal : Apollo pilote la configuration d'affichage de
/// cette VM et peut ajouter une sortie à tout instant — une addition externe
/// compenserait exactement un retrait, et un contrôle par nombre passerait
/// alors qu'une sortie a disparu. Le cardinal n'est éprouvé qu'APRÈS la
/// différence de noms, jamais à sa place.
fn designer_sorties_neuves(
    apres: &[SortieDxgi],
    connues: &HashSet<String>,
    nombre: u8,
) -> Result<Vec<SortieDxgi>> {
    let neuves: Vec<SortieDxgi> = apres
        .iter()
        .filter(|sortie| !connues.contains(&sortie.nom_sortie))
        .cloned()
        .collect();
    let noms: Vec<&str> = neuves.iter().map(|s| s.nom_sortie.as_str()).collect();
    anyhow::ensure!(
        neuves.len() == nombre as usize,
        "{} sorties DXGI neuves après création de {nombre} ({noms:?}) — \
         une addition ou un retrait externe rend la mesure inimputable",
        neuves.len()
    );
    for sortie in &neuves {
        tracing::info!(
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur_annoncee = sortie.rect.width,
            hauteur_annoncee = sortie.rect.height,
            "sortie virtuelle retenue"
        );
    }
    Ok(neuves)
}

/// Ouvre une duplication DXGI par sortie.
///
/// **Un échec ici est LE RÉSULTAT de ce chantier, pas une panne.** Si la Kᵉ
/// `DuplicateOutput` est refusée, le rang, le HRESULT nu et la sortie visée
/// sont journalisés, et l'erreur ressort telle quelle : c'est la réponse à la
/// question posée. Ne jamais l'avaler ni la retenter.
fn ouvrir_duplications(
    virtuelles: &[SortieDxgi],
    mires: &super::mires::Mires,
) -> Result<(Vec<Box<dyn VoieDeCapture>>, Vec<Rect>)> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        let designation = (sortie.index_adaptateur, sortie.index_sortie);
        match VoieDuplication::partagee_sur(Some(designation)) {
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
                tracing::error!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    causes = %super::causes(erreur),
                    "duplication REFUSÉE — c'est le résultat de la mesure, pas une panne"
                );
                anyhow::bail!(
                    "{} duplications DXGI ouvertes de front, la {}ᵉ refusée",
                    rang,
                    rang + 1
                );
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

/// `regions` porte, voie par voie, les dimensions que ses images auront —
/// celles de la TEXTURE de sa sortie, et non celles de sa fenêtre. Les deux ne
/// coïncident que si la sortie n'est pas mise à l'échelle : la sonde a relevé
/// un facteur DPI de 1,5 sur une sortie virtuelle, et un encodeur dimensionné
/// sur la fenêtre refuserait alors les images que la voie lui soumet.
fn passe_capture(
    pilote: &PiloteParIoctl,
    mires: &mut super::mires::Mires,
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
            // périphérique D3D11. Ici chaque voie a le sien, une duplication
            // par sortie créant un périphérique par sortie.
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
    let mut prochain_ping = debut + CADENCE_PING;
    let mut pts = vec![0u64; nombre];

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        let tour = mires.trame();
        // La voie contrôlée à ce tour, et elle seule : une lecture par tour
        // quel que soit N (voir `mire::voie_controlee`).
        let controlee = mire::voie_controlee(tour, nombre);

        for (id, voie) in voies.iter_mut().enumerate() {
            let Some(image) = voie.prochaine_image(tour)? else {
                continue;
            };
            compteurs.images[id] += 1;

            if controlee == Some(id) {
                // L'identité ATTENDUE est celle de la mire posée sur CETTE
                // sortie : un verdict `Voisine(j)` dit que la voie i a capturé
                // la sortie j, l'appariement croisé que ce montage doit
                // détecter.
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

        if Instant::now() >= prochain_ping {
            pilote.pinguer()?;
            prochain_ping += CADENCE_PING;
        }
        // Journalisation périodique, à la seconde : aucune trace par trame.
        // Écrite ici plutôt que partagée avec `banc.rs` — les deux protocoles
        // n'observent pas la même chose (celui-ci n'a ni recouvrement ni
        // porte éliminatoire, donc ni « avant » ni « après » à distinguer),
        // et les factoriser imposerait de journaliser des champs vides d'un
        // côté ou de l'autre.
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

    // Libération EXPLICITE et tracée, une par une : un `Vec` détruit
    // implicitement ne dirait pas lequel de ses éléments a tué le processus.
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

/// Les trois passes, dans l'ordre.
///
/// **Pas de porte éliminatoire entre les deux dernières**, contrairement au
/// banc mono-sortie : sans recouvrement, un verdict faux n'invalide pas la
/// mesure de cadence, il la qualifie. Les deux passes tournent toujours, et le
/// rapport lit les verdicts.
fn executer_passes(pilote: &PiloteParIoctl, virtuelles: &[SortieDxgi]) -> Result<()> {
    // Le périphérique des mires ne vient PAS d'une `DesktopCapture`
    // provisoire : DXGI n'autorise qu'une duplication par sortie, et la
    // provisoire ferait échouer la vraie en 0x80070057.
    let (device, _contexte) = creer_device()?;
    // Les mires vivent en coordonnées du BUREAU VIRTUEL — les rectangles
    // annoncés par DXGI. Les voies recadrent en coordonnées de TEXTURE, que
    // `ouvrir_duplications` calcule. Les confondre décalerait tout d'un
    // facteur DPI.
    let places_bureau: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let mut mires = super::mires::Mires::ouvrir(&device, &places_bureau)?;

    compteurs::passe_temoin(&mut mires)?;

    let (mut voies, places_texture) = ouvrir_duplications(virtuelles, &mires)?;
    tracing::info!(
        nombre = voies.len(),
        ?places_bureau,
        ?places_texture,
        "les N duplications sont ouvertes de front"
    );

    // Clé de lecture du journal. `compteurs::journaliser` porte les libellés du
    // protocole mono-sortie, et ils sont conservés tels quels pour que ces
    // relevés restent `grep`-ables avec ceux déjà versés dans `docs/`. Ici :
    // `mire0_avant_recouvrement` est toujours vide (aucun recouvrement n'est
    // mis en scène), et `mire0_apres_recouvrement` porte TOUS les verdicts de
    // la rotation, sur toutes les voies — pas ceux de la seule mire 0.
    let nombre = voies.len() as u8;
    let releve = passe_capture(pilote, &mut mires, &mut voies, &places_texture, false)?;
    compteurs::journaliser("capture", "duplication-parallele", nombre, &releve);

    let releve = passe_capture(pilote, &mut mires, &mut voies, &places_texture, true)?;
    compteurs::journaliser("capture+encodage", "duplication-parallele", nombre, &releve);

    // Second suspect du défaut hérité, après les encodeurs : les duplications.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}

/// Dit si les N sorties virtuelles sont encore là après le passage du banc.
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER les verdicts, pas à les remplacer —
/// une sortie retirée par le chien de garde en cours de route rendrait du noir,
/// et l'on imputerait à Windows un défaut du protocole de mesure.
fn constater_survie(virtuelles: &[SortieDxgi]) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(erreur) => {
            tracing::error!(
                causes = %super::causes(erreur),
                "topologie illisible après le banc — survie des sorties inconnue"
            );
            return;
        }
    };
    let presentes: HashSet<&str> =
        vivantes.iter().map(|sortie| sortie.nom_sortie.as_str()).collect();
    let disparues: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !presentes.contains(nom))
        .collect();
    if disparues.is_empty() {
        tracing::info!(
            nombre = virtuelles.len(),
            "les N sorties virtuelles ont survécu au banc — les verdicts portent bien sur elles"
        );
    } else {
        tracing::error!(
            ?disparues,
            "des sorties virtuelles ont DISPARU pendant le banc — leurs verdicts ne sont pas \
             imputables à Windows, elles n'existaient plus"
        );
    }
}
