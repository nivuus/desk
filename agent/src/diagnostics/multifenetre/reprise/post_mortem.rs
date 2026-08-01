//! La sonde post-mortem du banc de reprise : retenter, une fois, ce que les
//! voies mortes n'ont pas réussi en rafale.
//!
//! Module à part plutôt que fonction de `passes.rs` : ce n'est pas une passe de
//! mesure mais un **diagnostic sur la mesure**, il ne compte rien, et il ne
//! tourne que sur le chemin où quelque chose est mort. (Le plafond de 500
//! lignes du projet l'imposait de toute façon.)

use std::time::{Duration, Instant};

use super::super::compteurs::Garde;
use super::super::mires::Mires;
use super::super::montee::DELAI_TOPOLOGIE;
use crate::capture::{DesktopCapture, SortieDxgi};
use crate::geometry::Rect;

/// Durée pendant laquelle la sonde sollicite une duplication rouverte avant de
/// conclure qu'elle ne rend rien.
///
/// Les mires peignent pendant ce temps : Desktop Duplication n'émet une image
/// qu'au changement du bureau, et une sonde qui ne peindrait pas conclurait au
/// silence sur une duplication parfaitement vivante.
const DUREE_SONDE_IMAGE: Duration = Duration::from_millis(500);

/// Retente **une seule fois**, la topologie stabilisée, ce que les voies mortes
/// n'ont pas réussi en rafale.
///
/// **Sans elle, le point d'arrêt du chantier peut être déclenché par un défaut
/// de calibrage.** `next_frame` accorde 3 réouvertures CONSÉCUTIVES et SANS
/// AUCUN DÉLAI (`AcquireNextFrame(0, …)` puis `rouvrir()` immédiat), là où le
/// dépôt attend par ailleurs `DELAI_TOPOLOGIE` = 3 s qu'une topologie se
/// stabilise. La perturbation tombe juste avant le premier `AcquireNextFrame`
/// de la passe B, c'est-à-dire au moment le plus instable. Deux chemins tuent
/// alors une voie en quelques millisecondes sans rien réfuter :
///
/// - trois `DXGI_ERROR_ACCESS_LOST` d'affilée pendant le remaniement ;
/// - une réouverture qui ne retrouve pas `\\.\DISPLAYn` le temps du remaniement
///   — et là **le budget n'est même pas consommé, et aucune ligne de
///   réouverture n'est écrite**.
///
/// Dans les deux cas le bilan montre `voies_vivantes_apres = 0`, dont la
/// lecture naturelle est « voie A réfutée ». Une ligne doit suffire à séparer
/// « la reprise est impossible sur ce matériel » de « trois tentatives en
/// rafale ne suffisaient pas », et c'est celle que cette sonde écrit.
///
/// N'échoue jamais : c'est un diagnostic, pas une mesure. Elle suppose les
/// duplications du banc déjà relâchées (voir son appelant).
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
        let nom = virtuelles[id].nom_sortie.as_str();
        let mut capture = match DesktopCapture::sur_sortie(nom) {
            Ok(capture) => capture,
            Err(erreur) => {
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
        let region = Rect { x: 0, y: 0, width: largeur, height: hauteur };

        // Les mires peignent pendant la sollicitation : sans changement du
        // bureau, une duplication vivante ne rendrait rien et la sonde
        // conclurait au silence à tort.
        let mut issue = Ok(false);
        let debut = Instant::now();
        while debut.elapsed() < DUREE_SONDE_IMAGE {
            let _ = mires.peindre();
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
                "sonde post-mortem : la sortie se REDUPLIQUAIT et rendait une image une fois la \
                 topologie stabilisée — la mort de cette voie ne réfute PAS la reprise, elle \
                 dit que 3 tentatives en rafale et sans délai ne suffisaient pas"
            ),
            Ok(false) => tracing::warn!(
                voie = id,
                nom_sortie = nom,
                duree_ms = DUREE_SONDE_IMAGE.as_millis() as u64,
                "sonde post-mortem : duplication rouverte, mais AUCUNE image dans le délai — \
                 ni une réfutation ni une confirmation, la duplication existe sans rien rendre"
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
