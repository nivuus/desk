//! Le tour ÉLIMINATOIRE (étape 2, brief D9) : essaie les combinaisons connues
//! sur la sortie SOUS TEST, duplication DXGI ouverte et tenue, jusqu'à ce que
//! la relecture DXGI confirme un mouvement, ou jusqu'à épuisement.
//!
//! Extrait de `mode_sortie.rs` à la tâche 2bis du sous-bloc D9, pour le
//! plafond de 500 lignes (`CLAUDE.md`) — même raison que `temoin.rs`,
//! `voisines.rs` et `combinaisons.rs`, extraits à la tâche 1.

use anyhow::Result;
use windows::Win32::Graphics::Gdi::DISP_CHANGE_SUCCESSFUL;

use super::combinaisons::{appliquer_combo, combos_du_tour};
use super::persistance::combinaison_imposee;
use super::voisines::DuplicationVoisine;
use super::{choisir_cible, modes_annonces};
use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;

/// Ce qu'un tour de combinaisons a établi.
pub(super) struct ResultatTour {
    /// Cible numérique effectivement visée pendant le tour, ou `None` si
    /// aucune cible mesurable n'a pu être choisie (`P1 NON MESURABLE`) — le
    /// témoin n'a alors rien à rejouer, voir son appelant.
    pub(super) cible: Option<(u32, u32)>,
    /// Le bras qui a fait bouger la sortie, s'il y en a un.
    pub(super) gagnante: Option<&'static str>,
    /// Nombre de tentatives où au moins une voisine a perdu l'accès à sa
    /// duplication pendant le tour — voir
    /// `voisines::DuplicationVoisine::sonder` pour la granularité exacte.
    pub(super) pertes_voisines: u32,
}

/// Essaie les combinaisons connues jusqu'à ce que la relecture DXGI confirme
/// la cible, ou jusqu'à épuisement. Rend toujours `Ok` : un refus — ou
/// l'impossibilité de choisir une cible mesurable — est une mesure, pas une
/// erreur de la sonde ; voir le commentaire de tête de `mode_sortie.rs`. Seule
/// une topologie devenue illisible fait remonter une erreur.
///
/// `avant` est la taille lue par DXGI juste après la création, PAS supposée
/// être la résolution de création : c'est exactement la valeur dont l'absence
/// a rendu le premier relevé vide de sens (voir le commentaire de tête du
/// module parent).
///
/// `voisines` : sondées une fois PAR TENTATIVE (voir `DuplicationVoisine::sonder`)
/// — c'est l'inconnue annexe n°2 de D8 relevée au même moment que
/// l'éliminatoire, pas une mesure séparée.
///
/// Le tour est restreint à UN bras si l'opérateur l'impose (tâche 2bis, D9,
/// `MULTIFENETRE_MODE_SORTIE_DRAPEAUX`) — voir `persistance::combinaison_imposee`
/// et `combinaisons::combos_du_tour`. Sans elle, comportement inchangé : les
/// quatre combos, dans l'ordre.
pub(super) fn essayer_les_modes(
    pilote: &PiloteParIoctl,
    nom_sortie: &str,
    avant: (u32, u32),
    demande: (u32, u32),
    voisines: &mut [DuplicationVoisine],
) -> Result<ResultatTour> {
    let annonces = modes_annonces(nom_sortie);
    tracing::info!(
        nombre = annonces.len(),
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        contient_demande = annonces.contains(&demande),
        demande_egale_avant = demande == avant,
        modes = ?annonces,
        "modes annonces (EnumDisplaySettingsExW) avant tout changement"
    );

    let cible = match choisir_cible(avant, demande, &annonces) {
        Some(cible) => cible,
        None => {
            tracing::error!(
                verdict = "P1 NON MESURABLE",
                raison = "aucun mode annonce ne differe de la taille courante",
                largeur_avant_tentative = avant.0,
                hauteur_avant_tentative = avant.1,
                largeur_demandee = demande.0,
                hauteur_demandee = demande.1,
                modes = ?annonces,
                "verdict P1 : mesure impossible, aucune tentative effectuee"
            );
            return Ok(ResultatTour { cible: None, gagnante: None, pertes_voisines: 0 });
        }
    };
    if cible == demande {
        tracing::info!(
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "cible retenue = resolution demandee (differe deja de la taille courante)"
        );
    } else {
        tracing::warn!(
            largeur_demandee = demande.0,
            hauteur_demandee = demande.1,
            largeur_avant_tentative = avant.0,
            hauteur_avant_tentative = avant.1,
            largeur_cible = cible.0,
            hauteur_cible = cible.1,
            "la resolution demandee egale deja la taille courante (persistance registre probable \
             d'une execution anterieure) -- cible substituee dynamiquement parmi les modes annonces"
        );
    }

    let mut dernier_code = 0i32;
    let mut derniere_taille = (0u32, 0u32);
    let mut gagnante: Option<&'static str> = None;
    let mut cible_exacte_atteinte = false;
    let mut pertes_voisines = 0u32;
    let imposee = combinaison_imposee();
    for combo in combos_du_tour(imposee.as_deref()) {
        dernier_code = appliquer_combo(nom_sortie, cible.0, cible.1, &combo);
        // Windows reconfigure sa topologie d'affichage de façon asynchrone —
        // exactement pourquoi `montee.rs` observe le même délai de grâce
        // après une création. Interroger DXGI trop tôt ferait conclure à un
        // refus là où il n'y a qu'un délai, et battre le chien de garde
        // pendant l'attente comme le fait le reste de ce module.
        attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
        // Sondées ICI, après l'attente : si une perte survient à n'importe
        // quel instant de la fenêtre qui vient de s'écouler, l'instance de
        // duplication de la voisine la porte encore au moment de cette
        // sollicitation (voir `DuplicationVoisine::sonder`) -- une sonde par
        // tentative suffit à la détecter.
        for voisine in voisines.iter_mut() {
            if voisine.sonder() {
                pertes_voisines += 1;
            }
        }
        let releve = relever_topologie(&format!("après tentative « {} »", combo.etiquette()))?;
        derniere_taille = releve
            .iter()
            .find(|sortie| sortie.nom_sortie == nom_sortie)
            .map(|sortie| (sortie.rect.width, sortie.rect.height))
            .unwrap_or((0, 0));
        // Le critère qui compte est le MOUVEMENT (`derniere_taille != avant`),
        // pas l'égalité à la cible choisie — voir le commentaire de tête du
        // module parent (défaut F1 corrigé). `cible_atteinte` reste
        // journalisé, séparément : il documente si le pilote honore la valeur
        // exacte demandée, une question plus fine que P1, jamais celle qui
        // décide du verdict.
        let mouvement = derniere_taille != avant;
        let cible_atteinte = derniere_taille == cible;
        tracing::info!(
            etiquette = combo.etiquette(),
            code_brut = dernier_code,
            api_annonce_succes = (dernier_code == DISP_CHANGE_SUCCESSFUL.0),
            largeur_relue = derniere_taille.0,
            hauteur_relue = derniere_taille.1,
            mouvement,
            cible_atteinte,
            "relecture DXGI (GetDesc/DesktopCoordinates) apres la tentative"
        );
        if mouvement {
            gagnante = Some(combo.etiquette());
            cible_exacte_atteinte = cible_atteinte;
            break;
        }
    }

    tracing::info!(
        verdict = if gagnante.is_some() { "P1 RECU" } else { "P1 REFUSE" },
        combinaison_gagnante = ?gagnante,
        code_brut_dernier_essai = dernier_code,
        largeur_avant_tentative = avant.0,
        hauteur_avant_tentative = avant.1,
        largeur_relue = derniere_taille.0,
        hauteur_relue = derniere_taille.1,
        largeur_cible = cible.0,
        hauteur_cible = cible.1,
        // Le verdict lui-même : un mouvement (A != B) a-t-il été observé ?
        // C'est CE champ qui gouverne "P1 RECU" ci-dessus, pas une égalité à
        // la cible (voir le commentaire de tête du module parent, défaut F1
        // corrigé) -- recalculé ici, redondant avec `gagnante.is_some()` par
        // construction, pour qu'un lecteur du journal n'ait pas à le déduire.
        mouvement_observe = derniere_taille != avant,
        // Secondaire : le pilote a-t-il honoré la valeur EXACTE demandée, ou
        // s'est-il arrêté à un mode intermédiaire ? Peut valoir `false` avec
        // un verdict "P1 RECU" -- ce n'est pas une contradiction, c'est une
        // question plus fine que celle de P1.
        cible_exacte_atteinte,
        pertes_acces_voisines_pendant_le_tour = pertes_voisines,
        "verdict P1 : une sortie virtuelle accepte-t-elle un autre mode que celui de sa creation"
    );
    Ok(ResultatTour { cible: Some(cible), gagnante, pertes_voisines })
}
