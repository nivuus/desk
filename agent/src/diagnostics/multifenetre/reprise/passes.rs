//! Les deux passes du banc de reprise, et la perturbation qui les sépare.
//!
//! Séparé de `reprise.rs`, qui est *le pilote des sorties* — même découpage que
//! `paralleles`, et pour la même raison (plafond de 500 lignes du projet). Le
//! diagnostic qui départage les deux façons de mourir vit à côté, dans
//! `post_mortem.rs`.
//!
//! Ce qui vit ici tourne SOUS la garde `moniteurs_virtuels::Sorties` tenue par
//! l'appelant : toute erreur qui en ressort passe par la destruction des
//! sorties, perturbatrice comprise.
//!
//! # Clé de lecture du journal — deux mots y ont un sens qu'ils n'ont pas ici
//!
//! `compteurs::journaliser` porte les libellés du protocole MONO-SORTIE, et ils
//! sont conservés tels quels pour que ces relevés restent `grep`-ables avec ceux
//! déjà versés dans `docs/`. Sur ce banc :
//!
//! - **`mire0_avant_recouvrement` est toujours vide**, et
//!   `mire0_apres_recouvrement` porte **TOUS** les verdicts de la rotation, sur
//!   toutes les voies — pas ceux de la seule mire 0. Aucun recouvrement n'est
//!   mis en scène ici : une fenêtre par sortie, rien ne peut en cacher une
//!   autre. Il n'y a donc **aucune porte éliminatoire**, et un verdict faux ne
//!   disqualifie pas la passe, il la qualifie.
//! - **« avant / après » n'est PAS « avant / après recouvrement ».** Le champ
//!   `passe` de ces lignes vaut « avant perturbation » / « après
//!   perturbation », et la perturbation dont il s'agit est la **création d'une
//!   sortie virtuelle de plus**. Les deux vocabulaires se ressemblent et ne
//!   parlent pas de la même chose.
//! - **`unites` reste à zéro** : ce banc n'encode pas.
//!
//! Le libellé de voie est `duplication-reprise`, jamais `duplication` : les
//! séries du banc à aire fixe et celles de ce montage-ci ne se comparent
//! d'aucun chiffre (`CLAUDE.md`), et un même seau de `grep` inviterait
//! précisément à les comparer. `paralleles` s'est nommé `duplication-parallele`
//! pour cette raison exacte.

use std::collections::HashSet;
use std::time::Instant;

use anyhow::{Context, Result};

use super::super::compteurs::{self, Compteurs, Garde, DUREE_PASSE, PERIODE_JOURNAL};
use super::super::mires::Mires;
use super::super::montee::RESOLUTION;
use super::super::voies::{creer_device, VoieDeCapture, VoieDuplication};
use super::constater_places;
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
    /// Vrai si la passe a tourné en mode dégradé (peinture des mires ou lecture
    /// de pixel perdue). Porté jusqu'au bilan : un bilan lu seul doit dire
    /// qu'il ne décrit pas une passe nominale.
    degradee: bool,
}

/// Les deux passes et la perturbation qui les sépare.
pub(super) fn eprouver(
    garde: &mut Garde<'_>,
    sorties: &mut crate::moniteurs_virtuels::Sorties<'_>,
    virtuelles: &[SortieDxgi],
    connues: &HashSet<String>,
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
    let passe_a = passe(garde, &mut mires, &mut voies, virtuelles, None)?;
    compteurs::journaliser(
        "avant perturbation",
        "duplication-reprise",
        nombre as u8,
        &passe_a.compteurs,
    );
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
    let instant_perturbation = Instant::now();
    tracing::info!(id = id_perturbatrice, "sortie perturbatrice créée");

    // --- Passe B : identique à la passe A, sur un bureau qui vient de changer.
    let passe_b = passe(garde, &mut mires, &mut voies, virtuelles, Some(instant_perturbation))?;
    compteurs::journaliser(
        "après perturbation",
        "duplication-reprise",
        nombre as u8,
        &passe_b.compteurs,
    );
    // Deux constats qui décident de ce que le bilan veut dire : la
    // perturbatrice s'est-elle attachée (sinon rien n'a pu perturber), et les k
    // sorties ont-elles bougé sous leurs mires (sinon une image noire serait
    // imputable au déplacement et non à la reprise).
    constater_places("après perturbation", virtuelles, connues);

    // Les duplications sont relâchées AVANT la sonde post-mortem : DXGI
    // n'autorise qu'une duplication par sortie, et l'objet d'une voie morte —
    // invalide mais bien vivant — ferait refuser la réouverture que la sonde
    // tente. La sonde conclurait alors « la sortie ne se redupliquait pas »
    // pour une raison qui n'a rien à voir avec la reprise.
    tracing::info!("libération des voies de capture : avant");
    drop(voies);
    tracing::info!("libération des voies de capture : après");
    if !passe_b.perdues.is_empty() {
        super::post_mortem::sonder(garde, &mut mires, virtuelles, &passe_b.perdues);
    }

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
        voies_totales = nombre,
        voies_perdues_apres = ?passe_b.perdues,
        verdicts_faux_avant = passe_a.compteurs.apres_recouvrement.faux(),
        verdicts_faux_apres = passe_b.compteurs.apres_recouvrement.faux(),
        passe_avant_degradee = passe_a.degradee,
        passe_apres_degradee = passe_b.degradee,
        "bilan de la reprise"
    );
    journaliser_cle_de_lecture();
    Ok(())
}

/// Ce qu'il faut avoir lu avant de conclure quoi que ce soit du bilan.
///
/// Le banc est un point d'arrêt : ses deux lectures fausses possibles coûtent
/// cher **dans les deux sens**, et aucune ne se voit sur les seuls chiffres du
/// bilan. Cette ligne les nomme.
///
/// Le nombre de reprises n'est PAS compté ici, à dessein : il est déjà dans les
/// lignes que `DesktopCapture::next_frame` pose à chaque réouverture, et un
/// second compteur dirait la même chose d'une autre façon — donc un jour autre
/// chose. La ventilation **par voie** que demande la spec §6.1 se fait sur le
/// champ `cible` de ces mêmes lignes, qui porte `Sortie("\\.\DISPLAYn")`, à
/// rapprocher du `nom_sortie` des lignes de mort.
fn journaliser_cle_de_lecture() {
    tracing::info!(
        "clé de lecture n°1 — ce bilan ne vaut QUE si la perturbation a perturbé : \
         `grep -c \"accès à la duplication perdu, réouverture\"` sur ce journal. ZÉRO ligne \
         signifie qu'aucune duplication n'a perdu son accès, donc que ce banc n'a RIEN éprouvé, \
         et son bilan ne se lit alors PAS comme un succès de la reprise. Contrôler aussi la \
         ligne « sorties tierces relevées » : sans perturbatrice ATTACHÉE, rien n'a pu perturber"
    );
    tracing::info!(
        "clé de lecture n°2 — des voies mortes ne réfutent PAS la reprise à elles seules. Le \
         budget est de 3 réouvertures CONSÉCUTIVES et SANS DÉLAI, quand le dépôt attend par \
         ailleurs 3 s (DELAI_TOPOLOGIE) qu'une topologie se stabilise : une rafale peut épuiser \
         le budget, et une réouverture peut échouer à retrouver la sortie sans même consommer le \
         budget ni écrire de ligne. C'est ce que départage la « sonde post-mortem », qui retente \
         UNE fois la topologie stabilisée. Ventiler les reprises par voie avec le champ `cible` \
         des lignes de réouverture, à rapprocher du `nom_sortie` des lignes de mort"
    );
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
/// **Rien de ce que la turbulence DXGI peut casser n'interrompt la passe.**
/// C'est le point de méthode de ce banc, et il vaut pour les trois appels qui
/// peuvent échouer sous une topologie qui se remanie :
///
/// - `prochaine_image` — la voie est marquée morte et cesse d'être sollicitée,
///   les autres continuent. Abandonner la passe entière au premier échec
///   tronquerait les compteurs des voies survivantes à l'instant de la mort de
///   celle-là, et le bilan les déclarerait mortes elles aussi : la confusion
///   exacte qu'une lecture voie par voie existe pour empêcher.
/// - `mires.peindre` (`Present` sur une swapchain) et `lire_verdict`
///   (`read_pixel` sur une texture qui vient d'être rouverte) — signalés une
///   fois, la passe continue en mode dégradé. Un `?` là ferait sortir
///   `eprouver` sans jamais émettre le bilan, c'est-à-dire perdrait la mesure
///   au moment précis où elle devient intéressante.
///
/// Seul le chien de garde reste fatal : sans lui le pilote peut reprendre ses
/// sorties sous la mesure, et plus rien de ce qui suivrait ne serait imputable.
///
/// Les mires peignent à chaque tour, dans les deux passes : Desktop Duplication
/// n'émet une image qu'au changement du bureau, et une mire immobile ferait
/// rendre `WAIT_TIMEOUT` à toutes les acquisitions — on mesurerait zéro image
/// et l'on conclurait à une panne.
///
/// `perturbation` date l'instant où la sortie de plus a été créée, pour que
/// chaque mort porte son `ms_depuis_perturbation` : sans lui, distinguer une
/// rafale d'un état durable oblige à recouper des horodatages à la main.
/// `None` pour la passe témoin, où le champ n'a pas de sens.
fn passe(
    garde: &mut Garde<'_>,
    mires: &mut Mires,
    voies: &mut [Box<dyn VoieDeCapture>],
    virtuelles: &[SortieDxgi],
    perturbation: Option<Instant>,
) -> Result<Passe> {
    let nombre = voies.len();
    let mut compteurs = Compteurs::nouveaux(nombre);
    let mut vivantes = vec![true; nombre];
    let mut peinture_signalee = false;
    let mut lecture_signalee = false;

    let debut = Instant::now();
    let mut prochain_journal = debut + PERIODE_JOURNAL;

    while debut.elapsed() < DUREE_PASSE {
        if let Err(erreur) = mires.peindre() {
            if !peinture_signalee {
                peinture_signalee = true;
                tracing::error!(
                    causes = %super::super::causes(erreur),
                    "peinture des mires perdue — la passe continue en mode DÉGRADÉ : le numéro \
                     de trame n'avance plus, donc les acquisitions ne trouveront plus rien de \
                     neuf. Les images comptées à partir d'ici ne mesurent plus la capture"
                );
            }
        }
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
                        nom_sortie = %virtuelles[id].nom_sortie,
                        ms_depuis_perturbation = ?perturbation.map(|t| t.elapsed().as_millis() as u64),
                        images_avant_la_mort = compteurs.images[id],
                        causes = %super::super::causes(erreur),
                        "capture définitivement perdue sur cette voie — elle cesse d'être \
                         sollicitée, les autres continuent. Ne PAS en conclure que la reprise \
                         est impossible avant d'avoir lu la sonde post-mortem"
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
                let verdict = match compteurs::lire_verdict(voie.as_mut(), &image, id as u8) {
                    Ok(verdict) => verdict,
                    Err(erreur) => {
                        if !lecture_signalee {
                            lecture_signalee = true;
                            tracing::error!(
                                voie = id,
                                causes = %super::super::causes(erreur),
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
    Ok(Passe { compteurs, perdues, degradee: peinture_signalee || lecture_signalee })
}
