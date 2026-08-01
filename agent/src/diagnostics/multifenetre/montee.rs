//! Mesure ① : combien de sorties virtuelles simultanées ce pilote accepte-t-il ?
//!
//! Sans ce chiffre, la voie « un moniteur virtuel par fenêtre » — celle que la
//! sonde de capture recommandait — n'est pas spécifiable, et l'arbitrage du
//! chantier D bascule.
//!
//! Séparé de `moniteurs.rs`, qui est *le pilote* : ici c'est *la mesure*. Le
//! découpage n'est pas cosmétique — `moniteurs.rs` était déjà à 456 lignes,
//! et le plafond de 500 lignes du projet interdisait de l'y verser.
//!
//! **Deux sondes.** `MULTIFENETRE_VDD_VEILLE` observe le chien de garde du
//! pilote ; `MULTIFENETRE_VDD` prend la mesure du plafond.
//!
//! # Le chien de garde, et pourquoi il ne fausse pas cette mesure
//!
//! Le pilote annonce `delai = 3` d'unité que l'en-tête amont ne documente PAS.
//! Si cette unité était la seconde, une montée en N pourrait voir ses sorties
//! retirées en cours de route, et le plafond publié serait celui du chien de
//! garde, pas celui du pilote — une mesure fausse, et fausse dans le sens qui
//! condamnerait à tort la voie recommandée.
//!
//! **Ce qui écarte ce risque est le journal de la montée lui-même, et rien
//! d'autre.** Au moment où le pilote refuse la 11ᵉ création, les onze sorties
//! déjà obtenues sont toutes présentes, relevées NOMMÉMENT (`\\.\DISPLAY1`,
//! puis `DISPLAY5` à `DISPLAY14`) — trente secondes après la première
//! création. Aucune n'a été retirée pendant la montée. Cet argument ne dépend
//! d'aucune hypothèse sur l'unité de `delai`, ni sur l'efficacité du ping.
//!
//! **Ce que l'épreuve sans ping n'établit PAS.** Elle relève qu'aucune sortie
//! n'a été retirée en 180 s de silence de notre part — mais Apollo tourne sur
//! cette VM et pingue le même pilote pendant tout ce temps. Un chien de garde
//! de trois SECONDES réarmé par autrui serait parfaitement compatible avec ce
//! relevé. L'unité de `delai` reste donc entièrement inconnue, et l'épreuve ne
//! dit rien de ce qu'il adviendrait d'un client seul et muet.
//!
//! **Le ping est prouvé accepté ; qu'il AGISSE n'est qu'indiqué.**
//! `IOCTL_DRIVER_PING` répond sans erreur, ce qui confirme un code IOCTL que la
//! reconnaissance n'avait pas retrouvé en octets. Sur son effet, deux relevés
//! et deux issues : au premier, le décompte valait 3 avant comme après, donc le
//! critère est resté muet ; au second, 2 avant et 3 après, les deux lectures
//! encadrant le ping à moins d'une milliseconde — un indice sérieux, sur UNE
//! occurrence, qu'une coïncidence avec un ping d'Apollo dans cette fenêtre
//! n'exclut pas formellement. La montée pingue donc par précaution, et ne fait
//! reposer sa validité sur rien de tout cela.
//!
//! # Résultats mesurés le 31 juillet 2026
//!
//! Journaux dans `docs/superpowers/plans/journaux-mesures-prealables/` :
//! `moniteurs-montee-en-n.log`, `moniteurs-chien-de-garde.log`,
//! `moniteurs-etat-initial.log`.
//!
//! - **Plafond : 10 sorties virtuelles simultanées.** La 11ᵉ création est
//!   refusée par le pilote lui-même, en `0x80070044` (`ERROR_TOO_MANY_NAMES`),
//!   les dix précédentes restant toutes attachées et toutes nommées au relevé.
//! - Corroboration indépendante du même chiffre : les `TargetId` rendus par le
//!   pilote parcourent un cycle de dix valeurs (256 à 265) et le rejouent —
//!   ce qui ressemble à un vivier fixe de dix cibles plutôt qu'à un compteur.

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl};
use crate::capture::SortieDxgi;
use crate::moniteurs_virtuels::Sorties;

/// Au-delà, on cesse de chercher : le chantier D vise 8 fenêtres, et la sonde
/// d'encodeurs emploie déjà ce même plafond de recherche.
///
/// **Ne concerne QUE la mesure.** `purge.rs` employait aussi cette constante
/// pour régénérer les GUID d'une exécution tuée net ; elle a désormais la
/// sienne, `moniteurs_virtuels::numeros::PLAFOND_NUMEROS`, aux côtés du
/// distributeur qui la fait respecter (correctif I1 de la revue finale). Les
/// deux valent seize, et cette coïncidence n'engage rien : celle-ci dit
/// jusqu'où une mesure grimpe, l'autre borne les numéros qu'un moniteur peut
/// porter — la seconde décide de la récupérabilité d'un état système, pas la
/// première.
pub(super) const PLAFOND_RECHERCHE: usize = 16;

/// Résolution demandée à chaque sortie : celle que le chantier D vise par
/// fenêtre, pas celle du bureau.
///
/// `pub(super)` : `capture_virtuelle.rs` crée SA sortie à la même résolution
/// que celle dont la montée en N a mesuré le plafond — deux mesures du même
/// chantier qui divergeraient sur ce point ne seraient plus comparables.
pub(super) const RESOLUTION: (u32, u32, u32) = (1280, 720, 60);

/// Un pilote d'affichage indirect ne publie pas sa sortie dans l'instant :
/// Windows reconfigure sa topologie d'affichage. Interroger DXGI trop tôt
/// ferait conclure à un refus là où il n'y a qu'un délai.
///
/// `pub(super)` : `purge.rs` relève la même topologie, avant et après sa
/// purge, avec le même délai de grâce.
// `pub(crate)` : `moniteurs_virtuels::purge::purger` (production) s'en sert
// pour le même relevé avant/après.
pub(crate) const DELAI_TOPOLOGIE: Duration = Duration::from_secs(3);

/// Cadence de ping du chien de garde, **par précaution et non par remède
/// démontré** : le ping est prouvé accepté du pilote, son effet sur le décompte
/// n'étant qu'indiqué par une occurrence (voir le commentaire de tête).
///
/// Une seconde est le tiers de `delai = 3` lu en secondes, la lecture la plus
/// défavorable de ce champ d'unité inconnue. C'est aussi la cadence du client
/// amont (`sleepInterval = timeout * 1000 / 3` dans son fil de ping). Si
/// l'unité est plus grosse, pinguer trop souvent ne coûte qu'un IOCTL vide par
/// seconde.
const CADENCE_PING: Duration = Duration::from_secs(1);

/// Durée par défaut de l'épreuve du chien de garde : dix fois le délai annoncé
/// lu en secondes, l'horizon le plus court qui vaille la peine d'être observé.
///
/// La valeur de `MULTIFENETRE_VDD_VEILLE` la remplace quand elle est un entier
/// — l'épreuve est faite pour être rejouée à des horizons différents, et
/// recompiler pour changer une durée d'attente serait absurde.
const DUREE_EPREUVE_PAR_DEFAUT: Duration = Duration::from_secs(30);

/// Relève la topologie DXGI et la journalise, sortie par sortie.
///
/// Rend la liste entière et non un cardinal : tout ce que ces deux sondes
/// vérifient repose sur l'IDENTITÉ des sorties (leur `nom_sortie`), pas sur
/// leur nombre. Un cardinal ne distingue pas une addition d'un remplacement
/// compensé.
///
/// `pub(super)` : `purge.rs` s'en sert pour le même relevé avant/après.
// `pub(crate)` : `moniteurs_virtuels::purge::purger` (production) s'en sert
// pour le même relevé avant/après.
pub(crate) fn relever_topologie(moment: &str) -> Result<Vec<SortieDxgi>> {
    let sorties = crate::capture::enumerer_sorties()?;
    let attachees = sorties.iter().filter(|s| s.attachee_au_bureau).count();
    tracing::info!(moment, nombre = sorties.len(), attachees, "topologie relevée");
    for sortie in &sorties {
        tracing::info!(
            moment,
            nom = %sortie.nom_sortie,
            adaptateur = %sortie.adaptateur,
            index_adaptateur = sortie.index_adaptateur,
            index_sortie = sortie.index_sortie,
            attachee = sortie.attachee_au_bureau,
            x = sortie.rect.x,
            y = sortie.rect.y,
            largeur = sortie.rect.width,
            hauteur = sortie.rect.height,
            "sortie"
        );
    }
    Ok(sorties)
}

/// Noms des sorties attachées au bureau, triés — donc comparables comme des
/// ensembles, l'ordre d'énumération de DXGI n'ayant aucune signification.
///
/// `pub(crate)` : `capture_virtuelle.rs` compare les mêmes ensembles avant et
/// après sa mesure, et `superviseur::boucle` (production) n'apparie une sortie
/// fraîchement créée que parmi les noms APPARUS — sans quoi il poserait la
/// fenêtre sur un moniteur préexistant de mêmes dimensions.
pub(crate) fn noms_attaches(sorties: &[SortieDxgi]) -> Vec<String> {
    let mut noms: Vec<String> = sorties
        .iter()
        .filter(|s| s.attachee_au_bureau)
        .map(|s| s.nom_sortie.clone())
        .collect();
    noms.sort();
    noms
}

/// Noms présents dans `reference` et absents de `observes`.
fn manquants(reference: &[String], observes: &[String]) -> Vec<String> {
    reference.iter().filter(|nom| !observes.contains(nom)).cloned().collect()
}

/// Attend `duree` en battant le chien de garde du pilote.
///
/// Un `sleep` nu laisserait le pilote libre de retirer nos sorties pendant
/// l'attente de reconfiguration. Que ce battement l'en empêche réellement n'est
/// pas établi — c'est une précaution, dont le coût est nul.
pub(super) fn attendre_en_pinguant(pilote: &PiloteParIoctl, duree: Duration) -> Result<()> {
    let debut = Instant::now();
    loop {
        pilote.pinguer()?;
        let restant = duree.saturating_sub(debut.elapsed());
        if restant.is_zero() {
            return Ok(());
        }
        std::thread::sleep(restant.min(CADENCE_PING));
    }
}

/// Sonde `MULTIFENETRE_VDD_VEILLE` : que fait le chien de garde d'une sortie
/// dont le créateur se tait ?
///
/// Deux observations, faites une fois par seconde et sans jamais pinguer :
///
/// - **le décompte du pilote lui-même** (`IOCTL_GET_WATCHDOG`), seule fenêtre
///   sur l'unité de `delai` ;
/// - **la présence de LA sortie créée**, repérée par son nom DXGI et suivie
///   nommément — pas un cardinal, qu'une addition externe compenserait.
///
/// **Cette sonde ne peut pas conclure seule, et c'est su d'avance.** Apollo
/// tourne sur cette VM et pingue le même pilote : tout ce qu'elle relève est
/// compatible avec un chien de garde de trois secondes réarmé par autrui. Elle
/// est ici pour dire ce qui se passe dans les conditions réelles de la VM, et
/// pour éprouver le code de `IOCTL_DRIVER_PING` — pas pour établir une unité.
pub(super) fn eprouver_chien_de_garde() -> Result<()> {
    let duree = std::env::var("MULTIFENETRE_VDD_VEILLE")
        .ok()
        .and_then(|valeur| valeur.parse().ok())
        .map_or(DUREE_EPREUVE_PAR_DEFAUT, Duration::from_secs);
    let noms_avant = noms_attaches(&relever_topologie("avant création")?);

    let pilote = ouvrir_pilote()?;
    let (veille_initiale, _) = pilote.veille()?;
    tracing::info!(
        delai = veille_initiale.delai,
        decompte = veille_initiale.decompte,
        duree_epreuve_s = duree.as_secs(),
        "chien de garde avant création — l'épreuve qui suit ne pingue JAMAIS"
    );

    let (largeur, hauteur, hertz) = RESOLUTION;
    let mut sorties = Sorties::nouvelles(&pilote);
    let id = sorties.creer(largeur, hauteur, hertz)?;
    let debut = Instant::now();

    let mut nom_cree: Option<String> = None;
    let mut disparue_a = None;
    let mut decomptes = Vec::new();
    while debut.elapsed() < duree {
        std::thread::sleep(Duration::from_secs(1));
        let seconde = debut.elapsed().as_secs();
        let (veille, _) = pilote.veille()?;
        let noms = match crate::capture::enumerer_sorties() {
            Ok(liste) => noms_attaches(&liste),
            Err(erreur) => {
                tracing::warn!(seconde, %erreur, "énumération DXGI en échec pendant l'épreuve");
                continue;
            }
        };
        if nom_cree.is_none() {
            if let Some(nouveau) = noms.iter().find(|nom| !noms_avant.contains(nom)) {
                tracing::info!(
                    seconde,
                    id,
                    nom = %nouveau,
                    "la sortie créée est repérée par son nom — c'est SA présence qui est \
                     suivie ensuite, pas un cardinal"
                );
                nom_cree = Some(nouveau.clone());
            }
        }
        let presente = nom_cree.as_ref().map(|nom| noms.contains(nom));
        decomptes.push(veille.decompte);
        tracing::info!(
            seconde,
            id,
            delai = veille.delai,
            decompte = veille.decompte,
            attachees = noms.len(),
            presente = ?presente,
            "épreuve sans ping"
        );
        if presente == Some(false) && disparue_a.is_none() {
            disparue_a = Some(seconde);
        }
    }

    match (&nom_cree, disparue_a) {
        (None, _) => tracing::error!(
            id,
            duree_epreuve_s = duree.as_secs(),
            "la sortie créée n'a JAMAIS paru dans DXGI — cas distinct d'un retrait"
        ),
        (Some(nom), Some(seconde)) => tracing::error!(
            nom = %nom,
            disparue_apres_s = seconde,
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "SANS PING, la sortie est retirée — toute mesure de plafond doit pinguer"
        ),
        (Some(nom), None) => tracing::info!(
            nom = %nom,
            duree_epreuve_s = duree.as_secs(),
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "rien n'a été retiré pendant l'épreuve, sans un seul ping DE NOTRE PART \
             — Apollo pinguant le même pilote pendant tout ce temps, cela n'établit \
             ni l'unité de « delai », ni ce qu'il adviendrait d'un client seul et muet"
        ),
    }

    // Un ping, UN SEUL, et à la toute fin : c'est le seul moyen d'établir que
    // `IOCTL_DRIVER_PING` — non confirmé par octets, contrairement à deux des
    // six codes — est bien le bon code, sans quoi la montée en N ferait reposer
    // sa précaution sur un appel qui n'existe pas.
    //
    // Le décompte est relu AVANT et APRÈS, pour que le journal porte la
    // COMPARAISON plutôt qu'une valeur isolée : lire 3 après le ping ne dit
    // rien s'il valait déjà 3 avant, et c'est ce qui s'est produit au premier
    // relevé de cette sonde — le critère y est resté muet. Au second, 2 avant
    // et 3 après, les deux lectures encadrant le ping à moins d'une
    // milliseconde : indice sérieux que le ping AGIT, sur une seule occurrence,
    // qu'une coïncidence avec un ping d'Apollo n'exclut pas formellement.
    let avant_ping = pilote.veille().map(|(veille, _)| veille.decompte).ok();
    match pilote.pinguer() {
        Ok(()) => {
            let apres_ping = pilote.veille().map(|(veille, _)| veille.decompte).ok();
            tracing::info!(
                decompte_avant_ping = ?avant_ping,
                decompte_apres_ping = ?apres_ping,
                "IOCTL_DRIVER_PING accepté par le pilote — le code non confirmé par octets \
                 est le bon. ACCEPTÉ n'est pas AGISSANT : seul un décompte qui REMONTE \
                 prouverait un réarmement"
            );
        }
        Err(erreur) => tracing::error!(
            causes = %super::causes(erreur),
            "IOCTL_DRIVER_PING REFUSÉ — la montée en N ne pourra pas s'en servir"
        ),
    }

    // La garde détruit ici. Si le chien de garde a déjà retiré la sortie, le
    // retrait échouera et sera journalisé en erreur : c'est attendu dans ce
    // cas-là, et ce n'est pas une fuite — la sortie n'existe plus.
    //
    // Ce GUID rejoint alors `a_purger`, et volontairement PAS rejoué par
    // `purge::rejouer_purge_due` ici (contrairement à `monter_en_n`) : un
    // rejeu échouerait pour la même raison que le premier essai — la sortie
    // n'existe déjà plus, ce n'est pas un retrait qu'un second essai
    // sauverait. Le laisser dans `a_purger` documente le fait sans promettre
    // une guérison qu'aucun rejeu ne peut apporter.
    Ok(())
}

/// Sonde `MULTIFENETRE_VDD` : la montée en N.
pub(super) fn monter_en_n() -> Result<()> {
    // Relevé AVANT toute création : sans lui, une restauration manuelle après
    // plantage se ferait à l'aveugle (spec §6.3).
    let avant = relever_topologie("avant toute création")?;
    let noms_avant = noms_attaches(&avant);

    let pilote = ouvrir_pilote()?;
    let (veille, _) = pilote.veille()?;
    tracing::info!(
        delai = veille.delai,
        decompte = veille.decompte,
        cadence_ping_s = CADENCE_PING.as_secs(),
        "chien de garde pingué par précaution pendant toute la montée — effet non établi"
    );
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite : la garde doit avoir détruit AVANT le relevé final,
    // sans quoi celui-ci décrirait un état transitoire.
    let mut arret = None;
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        let mut noms_connus = noms_avant.clone();
        for rang in 1..=PLAFOND_RECHERCHE {
            pilote.pinguer()?;
            match sorties.creer(largeur, hauteur, hertz) {
                Err(erreur) => {
                    tracing::info!(
                        plafond = rang - 1,
                        causes = %super::causes(erreur),
                        presentes = ?noms_connus,
                        "plafond de sorties virtuelles atteint — le pilote refuse la suivante, \
                         et toutes les précédentes sont encore là, nommément"
                    );
                    arret = Some("refus du pilote");
                    break;
                }
                Ok(id) => {
                    attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;
                    let apres = relever_topologie(&format!("après création {rang}"))?;
                    let noms = noms_attaches(&apres);

                    // Le contrôle porte sur des NOMS, pas sur un cardinal.
                    // Comparer `attachees` à `attachees_avant + rang` laisserait
                    // passer le cas où une sortie disparaît pendant qu'une autre
                    // paraît — or Apollo pilote la configuration d'affichage de
                    // cette VM et peut en ajouter une à tout instant, ce qui
                    // compenserait exactement un retrait par le chien de garde.
                    // Trois défauts distincts se lisent ici :
                    //   - `disparues` non vide : une sortie déjà obtenue a été
                    //     retirée — c'est le remplacement qu'Apollo faisait par
                    //     son réglage `ensure_only_display` ;
                    //   - `parues` vide : le pilote a accepté la demande mais la
                    //     sortie n'a pas paru ;
                    //   - `parues` de plus d'un nom : quelqu'un d'autre a ajouté
                    //     une sortie pendant la mesure, qui n'est donc plus
                    //     imputable au seul pilote.
                    let disparues = manquants(&noms_connus, &noms);
                    let parues = manquants(&noms, &noms_connus);
                    tracing::info!(
                        rang,
                        id,
                        sorties_dxgi = apres.len(),
                        attachees = noms.len(),
                        parues = ?parues,
                        "sortie virtuelle créée"
                    );
                    if !disparues.is_empty() || parues.len() != 1 {
                        tracing::error!(
                            rang,
                            disparues = ?disparues,
                            parues = ?parues,
                            attachees = noms.len(),
                            attendu = noms_connus.len() + 1,
                            "le pilote a accepté la demande mais la topologie ne suit pas \
                             — sortie non parue, retrait d'une précédente, ou addition externe"
                        );
                        arret = Some("topologie non suivie");
                        break;
                    }
                    noms_connus = noms;
                }
            }
        }
        if arret.is_none() {
            tracing::info!(
                plafond_recherche = PLAFOND_RECHERCHE,
                "aucun plafond atteint sous {PLAFOND_RECHERCHE} sorties virtuelles"
            );
        }
        // Destruction par la garde, ici, à la sortie de portée.
    }

    // Second essai, avant que `pilote` lui-même ne parte : si la garde a
    // laissé un retrait dû (le pilote l'a refusé une première fois), c'est
    // ici la dernière chance de ce PROCESSUS de le rejouer — au-delà, seule
    // la purge inter-processus de `purge.rs` pourra encore l'atteindre.
    let rejoues = crate::moniteurs_virtuels::purge::rejouer_purge_due(&pilote);
    if rejoues > 0 {
        tracing::info!(rejoues, "retraits dus rejoués avec succès après la garde");
    }

    // Une sortie virtuelle survit au processus : ne pas vérifier le retour à
    // l'état initial laisserait la VM polluée pour toutes les mesures
    // suivantes, sans que personne ne le sache. Ce contrôle-ci reste celui du
    // processus mesureur, donc juge et partie — le contrôle qui vaut est un
    // relevé `MULTIFENETRE_DXGI=1` depuis un processus neuf, après coup.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après destruction")?;
    let noms_apres = noms_attaches(&apres);
    if noms_apres == noms_avant && apres.len() == avant.len() {
        tracing::info!(
            arret = arret.unwrap_or("plafond de recherche épuisé"),
            noms = ?noms_apres,
            "état initial restauré — mêmes sorties, nommément"
        );
    } else {
        tracing::error!(
            noms_avant = ?noms_avant,
            noms_apres = ?noms_apres,
            total_avant = avant.len(),
            total_apres = apres.len(),
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }
    Ok(())
}
