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
//!
//! # Découpage
//!
//! Ce fichier est *le pilote des sorties* : création, désignation, contrôle
//! d'état, restauration — comme `paralleles.rs` l'est du sien, et pour la même
//! raison (plafond de 500 lignes). *Les deux passes et la perturbation* vivent
//! dans `reprise/passes.rs` ; *la sonde post-mortem*, qui n'est pas une mesure
//! mais un diagnostic sur la mesure, dans `reprise/post_mortem.rs`.

mod passes;
mod post_mortem;

use std::collections::HashSet;

use anyhow::{Context, Result};

use super::compteurs;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use super::paralleles::designer_sorties_neuves;
use crate::capture::SortieDxgi;
use crate::mire;

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
        let issue = passes::eprouver(&mut garde, &mut sorties, &virtuelles, &connues);
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la mesure"
        );
        // Constat du CHEMIN D'ERREUR : `eprouver` contrôle déjà l'état après sa
        // perturbation, mais un `?` en sort sans passer par ce contrôle.
        constater_places("bilan", &virtuelles, &connues);
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

/// Dit si les k sorties virtuelles sont encore là, encore attachées, **encore à
/// la même place**, et si une sortie TIERCE est apparue dans la topologie.
///
/// Les deux premiers contrôles sont ceux de `paralleles::constater_survie`. Les
/// deux autres sont propres à ce banc :
///
/// - **la place.** Les mires ont été posées une fois pour toutes aux
///   coordonnées relevées à la création. Si l'arrivée d'une sortie de plus fait
///   glisser les autres dans le bureau virtuel, les mires se retrouvent hors de
///   leur sortie et les captures deviennent noires — et l'on imputerait à un
///   défaut de reprise ce qui n'est qu'un déménagement.
/// - **les tierces**, c'est-à-dire tout ce qui n'est ni connu d'avance
///   (`connues`) ni créé par le montage (`virtuelles`) : au moment « après
///   perturbation », c'est la **sortie perturbatrice**, et c'est la seule preuve
///   que la perturbation a réellement eu lieu. `Sorties::creer` rend `Ok(id)`
///   dès que le pilote accepte l'IOCTL ; rien ne dit alors que Windows a
///   reconfiguré quoi que ce soit. Une perturbatrice créée mais non attachée ne
///   perturberait rien, et « zéro réouverture » se lirait à tort comme un
///   résultat de la reprise. (Une tierce peut aussi être une addition d'Apollo,
///   qui pilote la configuration d'affichage de cette VM : d'où le relevé
///   nominatif plutôt qu'un compte.)
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER les verdicts, pas à les remplacer.
/// Privée : `passes.rs` y accède comme module enfant.
fn constater_places(moment: &str, virtuelles: &[SortieDxgi], connues: &HashSet<String>) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(erreur) => {
            tracing::error!(
                moment,
                causes = %super::causes(erreur),
                "topologie illisible — état des sorties inconnu"
            );
            return;
        }
    };

    constater_tierces(moment, virtuelles, connues, &vivantes);

    let mut disparues: Vec<&str> = Vec::new();
    let mut detachees: Vec<&str> = Vec::new();
    let mut deplacees: Vec<String> = Vec::new();
    for attendue in virtuelles {
        let nom = attendue.nom_sortie.as_str();
        match vivantes.iter().find(|sortie| sortie.nom_sortie == attendue.nom_sortie) {
            None => disparues.push(nom),
            Some(sortie) if !sortie.attachee_au_bureau => detachees.push(nom),
            Some(sortie) if sortie.rect != attendue.rect => {
                deplacees.push(format!("{nom} : {:?} → {:?}", attendue.rect, sortie.rect))
            }
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

/// La preuve que la perturbation a eu lieu : une sortie ni connue d'avance, ni
/// créée par le montage, donc la perturbatrice.
fn constater_tierces(
    moment: &str,
    virtuelles: &[SortieDxgi],
    connues: &HashSet<String>,
    vivantes: &[SortieDxgi],
) {
    let notres: HashSet<&str> =
        virtuelles.iter().map(|sortie| sortie.nom_sortie.as_str()).collect();
    let tierces: Vec<&SortieDxgi> = vivantes
        .iter()
        .filter(|sortie| {
            !notres.contains(sortie.nom_sortie.as_str()) && !connues.contains(&sortie.nom_sortie)
        })
        .collect();

    if tierces.is_empty() {
        tracing::warn!(
            moment,
            "aucune sortie TIERCE dans la topologie — au moment « après perturbation », cela \
             signifie que la perturbatrice ne s'est PAS attachée : le pilote a accepté l'IOCTL \
             mais Windows n'a rien reconfiguré, donc rien n'a pu perturber les duplications, et \
             une absence de réouverture ne dit alors RIEN de la reprise"
        );
        return;
    }
    let attachees: Vec<&str> = tierces
        .iter()
        .filter(|sortie| sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    let detachees: Vec<&str> = tierces
        .iter()
        .filter(|sortie| !sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    tracing::info!(
        moment,
        tierces_attachees = ?attachees,
        tierces_detachees = ?detachees,
        "sorties tierces relevées — au moment « après perturbation », la perturbatrice doit \
         figurer parmi les ATTACHÉES pour que la perturbation ait eu lieu"
    );
}
