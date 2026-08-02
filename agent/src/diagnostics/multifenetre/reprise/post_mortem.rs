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

/// Retente **une seule fois**, la topologie stabilisée, ce qu'une voie n'a pas
/// réussi à rétablir dans sa fenêtre de reprise (`FenetreDeReprise`,
/// `DUREE_FENETRE_REPRISE` = 8 s, `PAS_REPRISE` = 150 ms — voir
/// `capture::reprise`).
///
/// **Née d'un défaut de calibrage, corrigé depuis (tâche 6 bis) — la sonde
/// reste utile.** Avant correction, `next_frame` n'accordait que 3
/// réouvertures CONSÉCUTIVES et SANS AUCUN DÉLAI (`AcquireNextFrame(0, …)`
/// puis `rouvrir()` immédiat), épuisées en 14 à 21 ms là où le dépôt attend
/// par ailleurs `DELAI_TOPOLOGIE` = 3 s qu'une topologie se stabilise : le
/// point d'arrêt du chantier avait été déclenché par ce sous-calibrage, pas
/// par une reprise réellement impossible. La fenêtre de 8 s couvre cette
/// durée au double, mais reste une MAJORANTE non calibrée (`CLAUDE.md`) :
/// rien n'exclut qu'une topologie particulièrement lente échappe aussi aux
/// 8 s.
///
/// Cette sonde reste donc la ligne qui sépare « la reprise est impossible sur
/// ce matériel » de « la fenêtre de reprise ne suffisait pas ». La
/// perturbation tombe juste avant le premier `AcquireNextFrame` de la passe
/// B, c'est-à-dire au moment le plus instable, et une voie peut mourir dans
/// sa fenêtre de reprise sans que la reprise elle-même soit réfutée :
///
/// - la fenêtre de 8 s peut expirer alors que la topologie ne s'est toujours
///   pas stabilisée ;
/// - une réouverture peut échouer à retrouver `\\.\DISPLAYn` plusieurs fois de
///   suite le temps du remaniement — chaque échec consomme désormais une
///   tentative ET écrit sa propre ligne (« réouverture de la duplication
///   échouée … »), sans que cela soit définitif : seule l'expiration de la
///   fenêtre entière l'est.
///
/// Dans les deux cas le bilan montre `voies_vivantes_apres = 0`, dont la
/// lecture naturelle est « voie réfutée ». Une ligne doit suffire à séparer
/// « la reprise est impossible sur ce matériel » de « la fenêtre ne
/// suffisait pas », et c'est celle que cette sonde écrit.
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
        // Battre AVANT la sollicitation, et pas seulement après.
        //
        // `DesktopCapture::sur_sortie` retente sa duplication pendant
        // `DUREE_FENETRE_OUVERTURE` : elle **bloque jusqu'à 3 s** et ne peut
        // pas être pinguée pendant ce temps. Le seul battement de cette boucle
        // était celui qui suit la sollicitation, et la branche d'échec le
        // SAUTAIT par son `continue` : sur huit voies perdues — le cas nominal
        // de cette sonde — c'étaient huit fois 3 s bout à bout, soit ~24 s sans
        // un ping, là où `compteurs::CADENCE_PING` vaut 1 s précisément parce
        // que le `delai = 3` du pilote est d'unité INCONNUE, la seconde non
        // exclue. Le pilote aurait repris ses sorties SOUS la sonde, et la
        // sonde qui sert à départager deux réfutations serait devenue
        // inimputable sans que rien ne le dise.
        //
        // Remettre le compteur à zéro juste avant le blocage borne le trou à la
        // durée d'UN `sur_sortie` (~3 s), quel que soit le chemin pris ensuite :
        // le `continue` repasse par ici. Les 3 s elles-mêmes restent
        // irréductibles sans changer la sémantique d'ouverture — ce que la revue
        // finale n'est pas le moment de faire.
        if let Err(erreur) = garde.battre_si_du() {
            tracing::error!(
                voie = id,
                causes = %super::super::causes(erreur),
                "sonde post-mortem : chien de garde perdu avant de solliciter cette voie — le \
                 pilote peut avoir repris ses sorties, ce qui suit n'est plus imputable"
            );
            return;
        }
        let nom = virtuelles[id].nom_sortie.as_str();
        let mut capture = match DesktopCapture::sur_sortie(nom) {
            Ok(capture) => capture,
            Err(erreur) => {
                // Battu ici AUSSI, et pas seulement au tour suivant : la trace
                // ci-dessous n'est pas gratuite, et le `continue` ne doit
                // laisser aucun chemin sans battement.
                let _ = garde.battre_si_du();
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
        //
        // D'où le suivi de l'échec de peinture, et non un `let _ =` : une
        // peinture morte produit EXACTEMENT le symptôme « aucune image », et
        // le rendre sous un libellé de résultat ferait passer une panne de
        // l'instrument pour une mesure. C'est la classe de défaut que ce banc
        // traque partout ailleurs.
        let mut issue = Ok(false);
        let mut peinture_perdue = false;
        let debut = Instant::now();
        while debut.elapsed() < DUREE_SONDE_IMAGE {
            if mires.peindre().is_err() {
                peinture_perdue = true;
            }
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
                // Ce message nommait « 3 tentatives en rafale et sans délai » —
                // le calibrage d'AVANT la tâche 6 bis, que le commentaire de
                // tête de ce fichier déclare pourtant corrigé deux écrans plus
                // haut. Ce qui n'a pas suffi, désormais, c'est la FENÊTRE de
                // reprise ; sa durée est donc journalisée au lieu d'être dite.
                fenetre_reprise_ms =
                    crate::capture_reprise::DUREE_FENETRE_REPRISE.as_millis() as u64,
                "sonde post-mortem : la sortie se REDUPLIQUAIT et rendait une image une fois la \
                 topologie stabilisée — la mort de cette voie ne réfute PAS la reprise, elle \
                 dit que la fenêtre de reprise n'a pas suffi sur ce remaniement de topologie"
            ),
            // Le silence a deux causes possibles, et une seule est un
            // résultat : les nommer séparément est tout l'objet du suivi de
            // `peinture_perdue`.
            Ok(false) if peinture_perdue => tracing::error!(
                voie = id,
                nom_sortie = nom,
                duree_ms = DUREE_SONDE_IMAGE.as_millis() as u64,
                "sonde post-mortem SANS VALEUR sur cette voie : la peinture des mires a échoué \
                 pendant la sollicitation, donc le bureau n'a pas changé. « Aucune image » est \
                 ici une panne de l'INSTRUMENT et ne dit rien de la duplication"
            ),
            Ok(false) => tracing::warn!(
                voie = id,
                nom_sortie = nom,
                duree_ms = DUREE_SONDE_IMAGE.as_millis() as u64,
                "sonde post-mortem : duplication rouverte, mires peintes, mais AUCUNE image \
                 dans le délai — ni une réfutation ni une confirmation, la duplication existe \
                 sans rien rendre"
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
