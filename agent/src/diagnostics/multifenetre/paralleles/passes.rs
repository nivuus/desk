//! La boucle de passes du protocole multi-sorties : ouverture des N
//! duplications, cadence, encodage.
//!
//! Séparée de `paralleles.rs`, qui est *le pilote des sorties* (création,
//! désignation, contrôle de survie), au moment où la ronde 1 a fait passer le
//! fichier unique à 527 lignes. Le découpage n'est pas cosmétique : le plafond
//! de 500 lignes du projet l'imposait, et la spec §6 le prévoyait à cet
//! endroit précis.
//!
//! Ce qui vit ici tourne SOUS la garde `moniteurs_virtuels::Sorties` tenue par
//! l'appelant : toute erreur qui en ressort passe par la destruction des
//! sorties, y compris le refus de `DuplicateOutput` qui est le résultat attendu
//! de ce chantier.

use std::time::Instant;

use anyhow::{Context, Result};

use super::super::compteurs::{self, Compteurs, Garde, DUREE_PASSE, PERIODE_JOURNAL};
use super::super::voies::{creer_device, VoieDeCapture, VoieDuplication};
use super::super::mires::Mires;
use super::constater_survie;
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// Ouvre une duplication DXGI par sortie.
///
/// **Un échec ici est LE RÉSULTAT de ce chantier, pas une panne.** Si la Kᵉ
/// `DuplicateOutput` est refusée, le rang, le HRESULT nu et la sortie visée
/// sont journalisés, puis **l'erreur d'origine est propagée**, enrichie du rang
/// atteint : c'est la réponse à la question posée. Ne jamais l'avaler, ne jamais
/// la retenter, et ne jamais la remplacer par un message reconstruit — la chaîne
/// de causes porte le HRESULT, qui est le fond de la réponse.
///
/// Le chien de garde est battu à chaque rang : à N=8 cette boucle ouvre huit
/// duplications, et le temps qu'elle prend s'ajouterait sinon au trou de la
/// passe témoin.
fn ouvrir_duplications(
    garde: &mut Garde<'_>,
    virtuelles: &[SortieDxgi],
    mires: &Mires,
) -> Result<(Vec<Box<dyn VoieDeCapture>>, Vec<Rect>)> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        garde.battre()?;
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
                // Chaîne complète formatée SANS consommer l'erreur (`{:#}`) :
                // le journal doit porter le HRESULT ET l'erreur doit encore
                // être propagée telle quelle juste en dessous. L'aide
                // `multifenetre::causes` ne convient pas ici, elle prend son
                // erreur par valeur.
                let chaine = format!("{erreur:#}");
                tracing::error!(
                    rang = rang + 1,
                    nom = %sortie.nom_sortie,
                    causes = %chaine,
                    "duplication REFUSÉE — c'est le résultat de la mesure, pas une panne"
                );
                let nom = sortie.nom_sortie.clone();
                // `Err(erreur).with_context(…)` et non `bail!` : le message
                // reconstruit du `bail!` perdait le HRESULT, qui est le fond
                // de la réponse à la question posée.
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

/// `regions` porte, voie par voie, les dimensions que ses images auront —
/// celles de la TEXTURE de sa sortie, et non celles de sa fenêtre. Les deux ne
/// coïncident que si la sortie n'est pas mise à l'échelle : la sonde a relevé
/// un facteur DPI de 1,5 sur une sortie virtuelle, et un encodeur dimensionné
/// sur la fenêtre refuserait alors les images que la voie lui soumet.
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
            // Bâtir huit encodeurs Media Foundation prend un temps non borné,
            // et il court AVANT que la boucle de passe (donc son ping à 1 Hz)
            // ne démarre : sans ce battement, c'est un second trou.
            garde.battre()?;
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

        garde.battre_si_du()?;
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
        // Clé de lecture des `unites`, relevée AVANT la libération. CONSTAT, et
        // rien de plus : le reste des `images` part dans `dropped_stale_nv12`
        // (une image convertie puis écartée parce qu'une plus récente est
        // arrivée avant que l'encodeur ne la réclame). Ce relevé ferme
        // l'arithmétique — `images = unites + nv12_ecartees + au plus 1 en vol`
        // — donc aucune image ne disparaît sans être comptée, et il montre que
        // le rapport est le MÊME aux quatre rangs : il ne vient pas du
        // parallélisme.
        //
        // Ne PAS attribuer ce rapport au ratio entre les 60 i/s de
        // configuration et les ~90 i/s soumis : les journaux le réfutent. Un
        // rapport 90/60 prédirait 600 unités pour 900 images ; les lignes
        // périodiques en montrent 45 par seconde pour 90 images, soit
        // exactement la moitié, linéaire sur les dix intervalles
        // (`paralleles-n1.log:40-49`). La cause de ce rapport d'un demi n'est
        // PAS établie, et cette mesure n'en a pas besoin.
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

/// Les trois passes, dans l'ordre.
///
/// **Pas de porte éliminatoire entre les deux dernières**, contrairement au
/// banc mono-sortie : sans recouvrement, un verdict faux n'invalide pas la
/// mesure de cadence, il la qualifie. Les deux passes tournent toujours, et le
/// rapport lit les verdicts.
pub(super) fn executer_passes(garde: &mut Garde<'_>, virtuelles: &[SortieDxgi]) -> Result<()> {
    // Le périphérique des mires ne vient PAS d'une `DesktopCapture`
    // provisoire : DXGI n'autorise qu'une duplication par sortie, et la
    // provisoire ferait échouer la vraie en 0x80070057.
    let (device, _contexte) = creer_device()?;
    // Les mires vivent en coordonnées du BUREAU VIRTUEL — les rectangles
    // annoncés par DXGI. Les voies recadrent en coordonnées de TEXTURE, que
    // `ouvrir_duplications` calcule. Les confondre décalerait tout d'un
    // facteur DPI.
    let places_bureau: Vec<Rect> = virtuelles.iter().map(|sortie| sortie.rect).collect();
    let mut mires = Mires::ouvrir(&device, &places_bureau)?;

    // La garde est passée : cette passe dure dix secondes, et sans elle elle
    // était un trou sans un seul ping (11,1 s mesurées à la ronde 1, témoin et
    // ouverture des duplications compris).
    compteurs::passe_temoin(&mut mires, Some(garde))?;
    constater_survie("témoin", virtuelles);

    let (mut voies, places_texture) = ouvrir_duplications(garde, virtuelles, &mires)?;
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
    // Un contrôle de survie APRÈS CHAQUE PASSE, et non un seul à la fin : une
    // sortie retirée pendant la passe « capture » ne serait constatée qu'après
    // « capture+encodage », et les deux relevés seraient également suspects
    // sans moyen de dire lequel est atteint.
    let releve = passe_capture(garde, &mut mires, &mut voies, &places_texture, false)?;
    compteurs::journaliser("capture", "duplication-parallele", nombre, &releve);
    constater_survie("capture", virtuelles);

    let releve = passe_capture(garde, &mut mires, &mut voies, &places_texture, true)?;
    compteurs::journaliser("capture+encodage", "duplication-parallele", nombre, &releve);
    constater_survie("capture+encodage", virtuelles);

    // Les duplications étaient le second suspect du défaut hérité, après les
    // encodeurs. Le défaut est depuis désigné par sa pile (un élément de
    // travail de la MFT encore en vol, `encode::arret`) et corrigé : ces deux
    // traces ne cherchent plus un coupable, elles bornent le relâchement — un
    // plantage ici resterait autrement muet.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}
