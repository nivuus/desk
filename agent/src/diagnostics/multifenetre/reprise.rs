//! `MULTIFENETRE_REPRISE` — k duplications DXGI qui tournent, une sortie
//! virtuelle créée par-dessus, et la question : reprennent-elles ?
//!
//! **C'est l'épreuve de l'inférence sur laquelle repose tout le sous-bloc D2.**
//! Le sous-bloc D1 a relevé que créer une sortie virtuelle fait abandonner le
//! mutex des duplications déjà ouvertes (`0x887A0026`), et que toutes les
//! sessions de capture meurent avec lui. Le sous-bloc D2 fait le pari que cet
//! échec est **récupérable** — que `DXGI_ERROR_ACCESS_LOST` se rattrape en
//! rouvrant la duplication, comme la documentation Microsoft le dit. Ce banc
//! est ce qui distingue ce pari d'une croyance : il éprouve la reprise sur CE
//! matériel, sans navigateur ni signaling, donc sans rien qui puisse masquer la
//! cause.
//!
//! **C'est un point d'arrêt** (spec §6.1). Si les duplications ne reprennent
//! pas, la voie est réfutée et il faut basculer sur la sérialisation (§8) —
//! avant d'avoir engagé le reste du plan.
//!
//! Le montage est celui de `paralleles.rs` — k sorties virtuelles, une mire et
//! une duplication chacune, contrôle d'image en rotation, restauration de la
//! topologie — **plus une perturbation au milieu**. Il capture par
//! `DesktopCapture::next_frame`, c'est-à-dire par le chemin de PRODUCTION :
//! c'est la raison pour laquelle la reprise a été logée là et non dans
//! `WindowsSource`. Contourner ce chemin viderait la mesure de son objet.
//!
//! # Ce que ce banc ne dit pas
//!
//! - **Une exécution par rang ne donne aucun taux.** Le sous-bloc D1 a
//!   reproduit son défaut trois fois sur trois ; une reprise qui marche une
//!   fois ne prouve pas qu'elle marche toujours.
//! - **Les mires ne sont pas des applications** : D3D11 plein cadre, sans
//!   occlusion ni interaction.
//! - **La justesse est ÉCHANTILLONNÉE** — une voie contrôlée par tour.
//! - **La DESTRUCTION d'une sortie n'est pas exercée ici**, pas plus qu'elle
//!   ne l'a été en D1.
//!
//! # Piège : un plantage ici laisse jusqu'à NEUF sorties orphelines
//!
//! Ce banc crée `k` sorties, puis **une de plus**, sur un vivier qui n'en
//! compte que **10** (plafond mesuré, `montee.rs`). La garde
//! `moniteurs_virtuels::Sorties` les détruit à la sortie de portée, y compris
//! pendant une panique — mais **pas sur un plantage du processus**. Rattrapage :
//! `MULTIFENETRE_VDD_PURGE=1`. Contrôler l'état AVANT de conclure d'un refus de
//! création, et depuis un processus neuf (`MULTIFENETRE_DXGI=1`).

use std::collections::HashSet;
use std::time::Instant;

use anyhow::{Context, Result};

use super::compteurs::{self, Compteurs, Garde, DUREE_PASSE, PERIODE_JOURNAL};
use super::mires::Mires;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use super::paralleles::designer_sorties_neuves;
use super::voies::{creer_device, VoieDeCapture, VoieDuplication};
use crate::capture::SortieDxgi;
use crate::geometry::Rect;
use crate::mire;

/// Ce qu'une passe rend : ses compteurs, **et** la liste des voies mortes.
///
/// Les deux séparément, à dessein. Un `Result<Compteurs>` jetterait les
/// compteurs dès qu'une voie meurt — or c'est précisément le cas que ce banc
/// existe pour observer. Combien d'images chaque voie a rendues avant de
/// mourir, et laquelle est morte, EST le résultat de la mesure.
struct Passe {
    compteurs: Compteurs,
    /// Voies dont la capture s'est perdue définitivement pendant cette passe,
    /// donc qui ont cessé d'être sollicitées. Vide = aucune n'est morte.
    perdues: Vec<usize>,
}

pub(super) fn mesurer(nombre: u8) -> Result<()> {
    anyhow::ensure!(
        (1..=mire::MIRES_MAX).contains(&nombre),
        "MULTIFENETRE_REPRISE doit valoir 1 à {}",
        mire::MIRES_MAX
    );

    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: HashSet<String> =
        avant.iter().map(|sortie| sortie.nom_sortie.clone()).collect();

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : les sorties — les k du montage ET la
    // perturbatrice — doivent être détruites AVANT le relevé final, sans quoi
    // celui-ci décrirait un état transitoire.
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
        // Construite juste après `attendre_en_pinguant`, donc juste après le
        // dernier ping connu (voir `compteurs::Garde::nouvelle`).
        let mut garde = compteurs::Garde::nouvelle(&pilote);
        garde.battre()?;
        let issue = eprouver(&mut garde, &mut sorties, &virtuelles);
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la mesure"
        );
        // Constat du CHEMIN D'ERREUR : `eprouver` contrôle déjà la survie après
        // sa perturbation, mais un `?` en sort sans passer par ce contrôle.
        constater_places("bilan", &virtuelles);
        issue
    };

    // Second essai des retraits que la garde n'a pas obtenus : dernière chance
    // de CE processus, au-delà seule la purge inter-processus les atteindra.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
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

/// Les deux passes et la perturbation qui les sépare.
fn eprouver(
    garde: &mut Garde<'_>,
    sorties: &mut crate::moniteurs_virtuels::Sorties<'_>,
    virtuelles: &[SortieDxgi],
) -> Result<()> {
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

    let (mut voies, places_texture) = ouvrir_duplications(garde, virtuelles, &mires)?;
    let nombre = voies.len();
    tracing::info!(nombre, ?places_bureau, ?places_texture, "les k duplications sont ouvertes");

    // --- Passe A : le témoin. Elle mesure ce que rendent des duplications que
    // rien ne dérange, et c'est la seule référence à laquelle la passe B
    // s'oppose.
    let passe_a = passe(garde, &mut mires, &mut voies)?;
    compteurs::journaliser("avant perturbation", "duplication", nombre as u8, &passe_a.compteurs);
    // Une voie déjà morte AVANT toute perturbation invalide la mesure : ce qui
    // suivrait ne serait plus imputable à la création de sortie. Le refus se
    // prend ici, où il se lit.
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
        .creer(largeur, hauteur, hertz)
        .context("création de la sortie perturbatrice")?;
    tracing::info!(id = id_perturbatrice, "sortie perturbatrice créée");

    // --- Passe B : identique à la passe A, sur un bureau qui vient de changer.
    let passe_b = passe(garde, &mut mires, &mut voies)?;
    compteurs::journaliser("après perturbation", "duplication", nombre as u8, &passe_b.compteurs);
    // Une sortie qui aurait déménagé sous ses mires rendrait du noir, et l'on
    // imputerait à un défaut de reprise ce qui n'est qu'un déplacement.
    constater_places("après perturbation", virtuelles);

    // --- Le bilan, la seule sortie qui compte.
    //
    // Le chiffre décisif est le nombre de voies qui rendent ENCORE des images
    // après la perturbation, voie par voie : un total masquerait une voie
    // morte compensée par une autre.
    let vivantes_apres = passe_b.compteurs.images.iter().filter(|n| **n > 0).count();
    tracing::info!(
        images_avant = ?passe_a.compteurs.images,
        images_apres = ?passe_b.compteurs.images,
        voies_vivantes_apres = vivantes_apres,
        voies_totales = voies.len(),
        voies_perdues_apres = ?passe_b.perdues,
        verdicts_faux_avant = passe_a.compteurs.apres_recouvrement.faux(),
        verdicts_faux_apres = passe_b.compteurs.apres_recouvrement.faux(),
        "bilan de la reprise"
    );
    // Clé de lecture, et garde-fou contre la conclusion la plus tentante. Le
    // nombre de reprises n'est PAS compté ici, à dessein : il est déjà dans les
    // lignes que `DesktopCapture::next_frame` pose à chaque réouverture, et un
    // second compteur dirait la même chose d'une autre façon — donc un jour
    // autre chose.
    tracing::info!(
        "clé de lecture — ce bilan ne vaut QUE si la perturbation a perturbé : \
         `grep -c \"accès à la duplication perdu, réouverture\"` sur ce journal. ZÉRO ligne \
         signifie qu'aucune duplication n'a perdu son accès, donc que ce banc n'a RIEN éprouvé, \
         et son bilan ne se lit alors PAS comme un succès de la reprise"
    );

    // Le relâchement est tracé de part et d'autre : un plantage ici resterait
    // autrement muet (défaut de libération déjà payé au chantier des
    // duplications parallèles).
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    Ok(())
}

/// Ouvre une duplication DXGI par sortie, et pose une mire sur chacune.
///
/// Même boucle que `paralleles/passes.rs::ouvrir_duplications`, mais **pas la
/// même lecture d'un échec** : là-bas, un refus de la Kᵉ `DuplicateOutput` EST
/// le résultat mesuré (le chantier cherchait ce plafond) ; ici c'est une panne
/// du banc, survenue avant toute perturbation, donc avant que quoi que ce soit
/// n'ait été éprouvé. Les deux boucles ne diraient pas la même chose du même
/// HRESULT : les partager forcerait à choisir un des deux énoncés.
///
/// Le chien de garde est battu à chaque rang : ouvrir k duplications prend un
/// temps qui s'ajouterait sinon au dernier trou de ping.
fn ouvrir_duplications(
    garde: &mut Garde<'_>,
    virtuelles: &[SortieDxgi],
    mires: &Mires,
) -> Result<(Vec<Box<dyn VoieDeCapture>>, Vec<Rect>)> {
    let mut sources = Vec::new();
    let mut textures = Vec::new();
    for (rang, sortie) in virtuelles.iter().enumerate() {
        garde.battre()?;
        let source = VoieDuplication::partagee_sur(Some(&sortie.nom_sortie)).with_context(|| {
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
        let mut voie: Box<dyn VoieDeCapture> = Box::new(VoieDuplication::nouvelle(source));
        voie.ouvrir(mires.hwnd(id as u8)?, places[id])?;
        voies.push(voie);
    }
    Ok((voies, places))
}

/// Une passe de capture de `DUREE_PASSE`, en rotation de contrôle.
///
/// **Une voie qui meurt cesse d'être sollicitée, et les autres continuent.**
/// C'est le point de méthode de ce banc : abandonner la passe entière au
/// premier échec tronquerait les compteurs des voies survivantes à l'instant de
/// la mort de celle-là, et le bilan les déclarerait mortes elles aussi — la
/// confusion exacte qu'une lecture voie par voie existe pour empêcher.
///
/// Les mires peignent à chaque tour, dans les deux passes : Desktop Duplication
/// n'émet une image qu'au changement du bureau, et une mire immobile ferait
/// rendre `WAIT_TIMEOUT` à toutes les acquisitions — on mesurerait zéro image
/// et l'on conclurait à une panne.
fn passe(
    garde: &mut Garde<'_>,
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
) -> Result<Passe> {
    let nombre = voies.len();
    let mut compteurs = Compteurs::nouveaux(nombre);
    let mut vivantes = vec![true; nombre];

    let debut = Instant::now();
    let mut prochain_journal = debut + PERIODE_JOURNAL;

    while debut.elapsed() < DUREE_PASSE {
        mires.peindre()?;
        mires.pomper();
        let tour = mires.trame();
        // La voie contrôlée à ce tour, et elle seule : une lecture par tour
        // quel que soit k (voir `mire::voie_controlee`).
        let controlee = mire::voie_controlee(tour, nombre);

        for (id, voie) in voies.iter_mut().enumerate() {
            if !vivantes[id] {
                continue;
            }
            let image = match voie.prochaine_image(tour) {
                Ok(Some(image)) => image,
                // Rien de neuf sur ce bureau à cet instant : le cas courant,
                // pas une erreur.
                Ok(None) => continue,
                Err(erreur) => {
                    vivantes[id] = false;
                    // Une ligne par voie morte, au plus k pour toute la passe :
                    // ce n'est pas une trace par image.
                    tracing::error!(
                        voie = id,
                        images_avant_la_mort = compteurs.images[id],
                        causes = %super::causes(erreur),
                        "capture définitivement perdue sur cette voie — elle cesse d'être \
                         sollicitée, les autres continuent"
                    );
                    continue;
                }
            };
            compteurs.images[id] += 1;

            if controlee == Some(id) {
                // L'identité ATTENDUE est celle de la mire posée sur CETTE
                // sortie : un verdict `Voisine(j)` dit que la voie i a capturé
                // la sortie j. Après une réouverture, c'est le contrôle qui dit
                // si la voie est revenue sur SA sortie et non sur une autre —
                // le risque propre à une reprise dans une topologie qui vient
                // de changer.
                let verdict = compteurs::lire_verdict(voie.as_mut(), &image, id as u8)?;
                compteurs.apres_recouvrement.compter(verdict);
            }
        }

        garde.battre_si_du()?;
        // Journalisation périodique, à la seconde : aucune trace par trame.
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
    Ok(Passe { compteurs, perdues })
}

/// Dit si les k sorties virtuelles sont encore là, encore attachées, et
/// **encore à la même place**.
///
/// Les deux premiers contrôles sont ceux de `paralleles::constater_survie` ; le
/// troisième est propre à ce banc. Les mires ont été posées une fois pour
/// toutes aux coordonnées relevées à la création. Si l'arrivée d'une sortie de
/// plus fait glisser les autres dans le bureau virtuel, les mires se retrouvent
/// hors de leur sortie et les captures deviennent noires — et l'on imputerait à
/// un défaut de reprise ce qui n'est qu'un déménagement.
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER les verdicts, pas à les remplacer.
fn constater_places(moment: &str, virtuelles: &[SortieDxgi]) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(erreur) => {
            tracing::error!(
                moment,
                causes = %super::causes(erreur),
                "topologie illisible — survie et places des sorties inconnues"
            );
            return;
        }
    };

    let mut disparues: Vec<&str> = Vec::new();
    let mut detachees: Vec<&str> = Vec::new();
    let mut deplacees: Vec<String> = Vec::new();
    for attendue in virtuelles {
        let nom = attendue.nom_sortie.as_str();
        match vivantes.iter().find(|sortie| sortie.nom_sortie == attendue.nom_sortie) {
            None => disparues.push(nom),
            Some(sortie) if !sortie.attachee_au_bureau => detachees.push(nom),
            Some(sortie) if sortie.rect != attendue.rect => deplacees.push(format!(
                "{nom} : {:?} → {:?}",
                attendue.rect, sortie.rect
            )),
            Some(_) => {}
        }
    }

    if disparues.is_empty() && detachees.is_empty() && deplacees.is_empty() {
        tracing::info!(
            moment,
            nombre = virtuelles.len(),
            "les k sorties virtuelles sont là, attachées, et à la même place — les verdicts \
             portent bien sur elles"
        );
        return;
    }
    if !disparues.is_empty() {
        tracing::error!(
            moment,
            ?disparues,
            "des sorties virtuelles ont DISPARU — leurs verdicts ne sont pas imputables à \
             Windows, elles n'existaient plus"
        );
    }
    if !detachees.is_empty() {
        tracing::error!(
            moment,
            ?detachees,
            "des sorties virtuelles ont été DÉTACHÉES du bureau — encore énumérables, mais \
             Windows n'y compose plus : une image noire y serait imputable au détachement"
        );
    }
    if !deplacees.is_empty() {
        tracing::error!(
            moment,
            ?deplacees,
            "des sorties virtuelles ont CHANGÉ DE PLACE dans le bureau virtuel — les mires \
             sont restées où elles étaient, une image noire y serait imputable au déplacement \
             et non à la reprise"
        );
    }
}
