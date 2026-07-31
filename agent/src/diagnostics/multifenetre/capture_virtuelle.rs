//! Mesure ③ : une fenêtre posée sur un moniteur virtuel est-elle capturée
//! correctement, même recouverte par une autre ?
//!
//! **C'est l'hypothèse FONDATRICE de la voie « un moniteur virtuel par
//! fenêtre »**, celle que la sonde de capture recommandait — et elle n'avait
//! jamais été vérifiée : la sonde ne la garantissait que « par construction »,
//! sans jamais capturer dessus. Qu'un moniteur virtuel SANS écran attaché soit
//! réellement composé par Windows, et que Desktop Duplication en rende autre
//! chose que du noir, ne se déduit d'aucun document — cela se mesure.
//!
//! Une image noire ici est un RÉSULTAT, pas une panne de la sonde : elle ferait
//! tomber la voie 2 et changerait la nature du chantier suivant.
//!
//! Module séparé de `moniteurs.rs`, qui est *le pilote*, et de `montee.rs`, qui
//! est *la mesure du plafond* : ce fichier-ci est la mesure de la CAPTURE. Le
//! découpage n'est pas cosmétique — `moniteurs.rs` est à 493 lignes et le
//! plafond de 500 lignes du projet interdisait d'y verser quoi que ce soit.
//!
//! # Un seul processus, et c'est structurant
//!
//! La garde `moniteurs_virtuels::Sorties` détruit la sortie à la fin du
//! processus. Un banc lancé séparément, après coup, ne trouverait donc plus
//! rien à capturer. Cette sonde crée la sortie **et** fait tourner le banc
//! dessus sans jamais rendre la main.
//!
//! # Piège : à `nombre = 1`, cette sonde LAISSE une sortie orpheline
//!
//! Sans recouvrement à mettre en scène, la porte éliminatoire du banc laisse
//! passer et la passe d'encodage s'exécute — or **elle emporte le processus**.
//!
//! **Où exactement, car cela oriente le diagnostic : à la SORTIE de la boucle,
//! pas pendant.** Les deux exécutions du 31 juillet 2026 écrivent leur dixième
//! et DERNIÈRE ligne périodique à `debut + 10,00 s`, soit l'instant même où
//! `while debut.elapsed() < DUREE_PASSE` cesse d'être vrai : la boucle a tourné
//! entière, et `journaliser` n'est jamais atteint. Ce qui court entre les deux
//! est la destruction du `Vec<H264Encoder>` local à `passe_capture`. Chercher
//! du côté de `submit`/`poll_output` serait chercher au mauvais endroit.
//!
//! **Ce n'est ni la sortie virtuelle, ni l'encodeur seul — c'est le couple.**
//! Le même banc sur le bureau physique
//! (`MULTIFENETRE_BANC=duplication MULTIFENETRE_N=1`) meurt au même endroit :
//! la sortie virtuelle est hors de cause. Et `printwindow-n4.log` comme
//! `printwindow-n8.log` portent leur ligne `passe terminée
//! passe="capture+encodage"` : sur la voie `printwindow`, la passe d'encodage
//! va à son terme et le processus survit. Le défaut est donc dans le couple
//! « voie **duplication** + encodeur H.264 », et dans lui seul.
//!
//! Il n'avait jamais été vu parce que sur cette voie la porte éliminatoire
//! coupait toujours avant la passe d'encodage — voir `CLAUDE.md`, « il n'existe
//! donc aucune mesure d'encodage multi-fenêtres par cette voie, par
//! construction du protocole ».
//!
//! Conséquence pratique : la garde ne court pas, la sortie virtuelle survit au
//! processus. Rattraper avec `MULTIFENETRE_VDD_PURGE=1`, éprouvé sur exactement
//! cet état. La mesure ③ elle-même se prend à `nombre = 2`, où le verdict de
//! recouvrement coupe avant la passe d'encodage et où la garde court.

use anyhow::{anyhow, Result};

use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::capture::SortieDxgi;

/// Sonde `MULTIFENETRE_VDD_CAPTURE` : crée UNE sortie virtuelle, y pose
/// `nombre` mires, et fait tourner le banc de la voie `duplication` dessus.
pub(super) fn capturer_sur_virtuelle(nombre: u8) -> Result<()> {
    let avant = relever_topologie("avant création")?;
    let noms_avant = noms_attaches(&avant);
    let connues: std::collections::HashSet<String> =
        avant.iter().map(|sortie| sortie.nom_sortie.clone()).collect();

    let pilote = super::moniteurs::ouvrir_pilote()?;
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite de la garde : la sortie doit être détruite AVANT le
    // relevé final, sans quoi celui-ci décrirait un état transitoire.
    let issue = {
        let mut sorties = crate::moniteurs_virtuels::Sorties::nouvelles(&pilote);
        let id = sorties.creer(largeur, hauteur, hertz)?;
        // Battre le chien de garde pendant l'attente de reconfiguration, comme
        // le fait la montée en N : son unité reste inconnue, et un `sleep` nu
        // laisserait le pilote libre de retirer la sortie sous la mesure.
        attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;

        let apres = relever_topologie("après création")?;
        let virtuelle = designer_sortie_neuve(&apres, &connues, id)?;
        let designation = (virtuelle.index_adaptateur, virtuelle.index_sortie);
        let nom_virtuelle = virtuelle.nom_sortie.clone();

        // Le facteur d'échelle est relevé et appliqué par le banc lui-même
        // (trace « banc : coordonnées de fenêtre et de texture ») : le mesurer
        // une seconde fois ICI obligerait à ouvrir une duplication sur cette
        // sortie, or DXGI n'en autorise qu'UNE — celle du banc échouerait
        // alors en 0x80070057.
        pilote.pinguer()?;
        let issue = super::banc::executer("duplication", nombre, Some(designation));

        // Le banc ne pingue pas : il tourne trente secondes en boucle serrée.
        // Si le chien de garde avait retiré la sortie en cours de route, la
        // capture aurait rendu du noir ou rien du tout, et l'on aurait imputé
        // à Windows un défaut du protocole de mesure. Ce contrôle-ci tranche
        // entre les deux.
        constater_survie(&nom_virtuelle);
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
    if noms_final == noms_avant && final_.len() == avant.len() {
        tracing::info!(noms = ?noms_final, "état initial restauré — mêmes sorties, nommément");
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_final,
            total_avant = avant.len(),
            total_apres = final_.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }

    issue
}

/// Retrouve, dans une topologie relevée après création, LA sortie qui n'y
/// était pas avant.
///
/// Par le NOM et non par l'index : DXGI renumérote ses sorties à chaque
/// reconfiguration de topologie, et un index retenu avant la création peut
/// désigner une autre sortie après. Un ensemble de noms ne souffre pas de cette
/// dérive.
///
/// Refuse si plusieurs sorties sont neuves : quelqu'un d'autre en aurait ajouté
/// une pendant la mesure (Apollo pilote la configuration d'affichage de cette
/// VM), et rien ne dirait plus laquelle est la nôtre.
fn designer_sortie_neuve<'s>(
    apres: &'s [SortieDxgi],
    connues: &std::collections::HashSet<String>,
    id: crate::moniteurs_virtuels::IdSortie,
) -> Result<&'s SortieDxgi> {
    let neuves: Vec<&SortieDxgi> =
        apres.iter().filter(|sortie| !connues.contains(&sortie.nom_sortie)).collect();
    let virtuelle = match neuves.as_slice() {
        [] => {
            return Err(anyhow!(
                "aucune sortie DXGI neuve après création de la sortie {id} — \
                 le pilote a accepté la demande mais Windows n'a rien publié"
            ))
        }
        [seule] => *seule,
        plusieurs => {
            let noms: Vec<&str> = plusieurs.iter().map(|s| s.nom_sortie.as_str()).collect();
            return Err(anyhow!(
                "{} sorties DXGI neuves après création de la sortie {id} ({noms:?}) — \
                 une addition externe rend la mesure inimputable",
                plusieurs.len()
            ));
        }
    };
    tracing::info!(
        id,
        nom = %virtuelle.nom_sortie,
        adaptateur = %virtuelle.adaptateur,
        index_adaptateur = virtuelle.index_adaptateur,
        index_sortie = virtuelle.index_sortie,
        attachee = virtuelle.attachee_au_bureau,
        x = virtuelle.rect.x,
        y = virtuelle.rect.y,
        largeur_annoncee = virtuelle.rect.width,
        hauteur_annoncee = virtuelle.rect.height,
        "sortie virtuelle retenue pour la capture"
    );
    Ok(virtuelle)
}

/// Dit si la sortie virtuelle est encore là après le passage du banc.
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER le verdict du banc, pas à le
/// remplacer.
fn constater_survie(nom_virtuelle: &str) {
    match crate::capture::enumerer_sorties() {
        Ok(sorties) => {
            let presente = sorties.iter().any(|sortie| sortie.nom_sortie == nom_virtuelle);
            if presente {
                tracing::info!(
                    nom = %nom_virtuelle,
                    "la sortie virtuelle a survécu au banc — le verdict porte bien sur elle"
                );
            } else {
                tracing::error!(
                    nom = %nom_virtuelle,
                    "la sortie virtuelle a DISPARU pendant le banc — le verdict de capture \
                     n'est pas imputable à Windows, la sortie n'existait plus"
                );
            }
        }
        Err(erreur) => tracing::error!(
            causes = %super::causes(erreur),
            "topologie illisible après le banc — survie de la sortie virtuelle inconnue"
        ),
    }
}
