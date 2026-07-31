//! N sorties virtuelles, une fenêtre et une duplication DXGI chacune.
//!
//! **C'est l'arrangement que la voie recommandée du chantier D propose
//! réellement**, et que rien n'avait exercé : le banc de la sonde et la mesure
//! ③ ont tous deux posé N fenêtres sur UNE sortie. DXGI n'autorisant qu'une
//! duplication par sortie, N duplications de front était une question ouverte,
//! pas un détail d'implémentation.
//!
//! ✅ **Elle ne l'est plus, et c'est ce module qui l'a fermée** (31 juillet
//! 2026) : la voie est **reçue** — 90,1 i/s par fenêtre en capture+encodage
//! jusqu'à N=8, zéro verdict faux, huit duplications ouvertes de front
//! (`docs/superpowers/plans/2026-07-31-duplications-paralleles-resultats.md`).
//! **Une exécution par rang, donc aucun taux**, et rien au-delà de 8 sorties.
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
//!
//! # Piège : un plantage ici laisse jusqu'à HUIT sorties orphelines
//!
//! Cette sonde crée jusqu'à 8 sorties virtuelles sur un vivier qui n'en compte
//! que **10** (plafond mesuré, `montee.rs`). La garde `moniteurs_virtuels::
//! Sorties` les détruit à la sortie de portée, y compris pendant le déroulement
//! d'une panique — mais **pas sur un plantage du processus** : ces API
//! échouent en `0xc0000005`, et la passe d'encodage de la voie `duplication`
//! avait précisément ce défaut jusqu'à sa correction le 31 juillet 2026, par
//! ce chantier même (`capture_virtuelle.rs`, section « Le piège que cette
//! sonde a révélé » ; correctif dans `encode::arret`). Un plantage laisserait
//! **8 des 10 sorties** derrière lui, et l'exécution suivante échouerait à en
//! créer 8 sans que la cause soit lisible.
//!
//! Rattrapage : `MULTIFENETRE_VDD_PURGE=1`, éprouvé sur exactement cet état.
//! Contrôler l'état AVANT de conclure quoi que ce soit d'un refus de création,
//! et depuis un processus neuf (`MULTIFENETRE_DXGI=1`).
//!
//! # Découpage
//!
//! Ce fichier est *le pilote des sorties* : création, désignation, contrôle de
//! survie, restauration de l'état initial. *La boucle de passes* — ouverture
//! des N duplications, cadence, encodage — vit dans `paralleles/passes.rs`.

mod passes;

use std::collections::HashSet;

use anyhow::{Context, Result};

use super::compteurs;
use super::montee::{
    attendre_en_pinguant, noms_attaches, relever_topologie, DELAI_TOPOLOGIE, RESOLUTION,
};
use crate::capture::SortieDxgi;
use crate::mire;

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

    let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()?;
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
        // Construite juste après `attendre_en_pinguant`, donc juste après le
        // dernier ping connu. La couture entre les deux n'est PAS comptée dans
        // l'intervalle maximal — `Garde::nouvelle` pose son origine à sa propre
        // construction —, elle est rendue négligeable par l'adjacence des deux
        // appels : 66 µs au relevé. Voir `compteurs::Garde::nouvelle`.
        let mut garde = compteurs::Garde::nouvelle(&pilote);
        garde.battre()?;
        let issue = passes::executer_passes(&mut garde, &virtuelles);
        // Le chiffre qui dit si le chien de garde a été battu sans trou sur
        // toute la mesure. Journalisé même en cas d'échec des passes : c'est
        // justement quand une mesure tourne mal qu'il faut savoir si une
        // sortie a pu être reprise sous elle.
        tracing::info!(
            intervalle_ping_max_ms = garde.intervalle_max().as_millis() as u64,
            "chien de garde : plus grand écart entre deux battements sur toute la mesure"
        );
        // Dernier constat, celui du CHEMIN D'ERREUR : `executer_passes`
        // contrôle déjà la survie après chacune de ses passes, mais un `?` en
        // sort sans passer par ces contrôles. Celui-ci court quoi qu'il
        // arrive, et c'est le seul qui couvre un abandon en cours de route.
        constater_survie("bilan", &virtuelles);
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
    // Une sortie énumérée mais NON attachée au bureau n'est pas capturable :
    // Windows ne compose rien dessus, DXGI l'annonce en 0×0, et le défaut ne
    // se manifesterait que bien plus loin — sur `facteur_echelle` (dimension
    // nulle) ou sur la swapchain d'une mire 0×0, loin de sa cause. Le refus se
    // prend ici, où il se lit.
    let detachees: Vec<&str> = neuves
        .iter()
        .filter(|sortie| !sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();
    anyhow::ensure!(
        detachees.is_empty(),
        "{} sortie(s) neuve(s) NON attachée(s) au bureau ({detachees:?}) — \
         le pilote a publié la sortie mais Windows n'y compose rien",
        detachees.len()
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

/// Dit si les N sorties virtuelles sont encore là, et encore ATTACHÉES, après
/// la passe nommée par `passe`.
///
/// N'échoue pas : la mesure est faite, la nier maintenant ne la rendrait pas
/// meilleure. Ce relevé sert à INTERPRÉTER les verdicts, pas à les remplacer —
/// une sortie retirée par le chien de garde en cours de route rendrait du noir,
/// et l'on imputerait à Windows un défaut du protocole de mesure.
///
/// **Présente ne suffit pas : il faut attachée.** Une sortie que le pilote
/// détacherait du bureau en cours de passe reste parfaitement énumérable par
/// DXGI — Windows cesse simplement d'y composer, et la capture devient noire.
/// Un contrôle qui ne regarderait que l'énumération déclarerait cette sortie
/// « survivante » sur une image devenue noire : exactement la mésattribution
/// que cette fonction existe pour empêcher. Le reste du module compare déjà des
/// ensembles d'attachées (`montee::noms_attaches`), pas d'énumérées.
///
/// Les deux défauts sont journalisés SÉPARÉMENT parce qu'ils ne disent pas la
/// même chose : `disparues` est un retrait, `detachees` une sortie que le
/// pilote garde mais que Windows n'affiche plus.
/// Privée : `passes.rs` y accède comme module enfant (`super::`), sans que ce
/// contrôle devienne une surface offerte au reste de `multifenetre`.
fn constater_survie(passe: &str, virtuelles: &[SortieDxgi]) {
    let vivantes = match crate::capture::enumerer_sorties() {
        Ok(sorties) => sorties,
        Err(erreur) => {
            tracing::error!(
                passe,
                causes = %super::causes(erreur),
                "topologie illisible après la passe — survie des sorties inconnue"
            );
            return;
        }
    };
    let enumerees: HashSet<&str> =
        vivantes.iter().map(|sortie| sortie.nom_sortie.as_str()).collect();
    let attachees: HashSet<&str> = vivantes
        .iter()
        .filter(|sortie| sortie.attachee_au_bureau)
        .map(|sortie| sortie.nom_sortie.as_str())
        .collect();

    let disparues: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| !enumerees.contains(nom))
        .collect();
    let detachees: Vec<&str> = virtuelles
        .iter()
        .map(|sortie| sortie.nom_sortie.as_str())
        .filter(|nom| enumerees.contains(nom) && !attachees.contains(nom))
        .collect();

    if disparues.is_empty() && detachees.is_empty() {
        tracing::info!(
            passe,
            nombre = virtuelles.len(),
            "les N sorties virtuelles sont encore là ET attachées — les verdicts de cette \
             passe portent bien sur elles"
        );
        return;
    }
    if !disparues.is_empty() {
        tracing::error!(
            passe,
            ?disparues,
            "des sorties virtuelles ont DISPARU pendant cette passe — leurs verdicts ne sont \
             pas imputables à Windows, elles n'existaient plus"
        );
    }
    if !detachees.is_empty() {
        tracing::error!(
            passe,
            ?detachees,
            "des sorties virtuelles ont été DÉTACHÉES du bureau pendant cette passe — encore \
             énumérables, mais Windows n'y compose plus : une image noire y serait imputable \
             au détachement, pas à la voie de capture"
        );
    }
}
