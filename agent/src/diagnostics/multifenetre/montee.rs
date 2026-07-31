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
//! **Deux sondes, et l'ordre entre elles n'est pas indifférent.**
//!
//! `MULTIFENETRE_VDD_VEILLE` éprouve d'abord le chien de garde. Le pilote
//! annonce `delai = 3` d'unité que l'en-tête amont ne documente PAS. Si cette
//! unité est la seconde, une montée en N qui ne pinguerait pas verrait ses
//! sorties mourir d'elles-mêmes en cours de route, et le plafond publié serait
//! celui du chien de garde, pas celui du pilote — une mesure fausse, et fausse
//! dans le sens qui condamnerait à tort la voie recommandée.
//!
//! `MULTIFENETRE_VDD` prend ensuite la mesure elle-même, chien de garde armé.
//! Elle pingue quoi qu'ait conclu l'épreuve : le ping ne coûte rien, et une
//! mesure qui dépendrait de la bonne interprétation d'un relevé antérieur
//! serait fragile pour rien.
//!
//! **Résultats mesurés le 31 juillet 2026** — journaux
//! `docs/superpowers/plans/journaux-mesures-prealables/moniteurs-chien-de-garde.log`
//! et `moniteurs-montee-en-n.log` :
//!
//! - **Plafond : 10 sorties virtuelles simultanées.** La 11ᵉ création est
//!   refusée par le pilote lui-même, en `0x80070044` (`ERROR_TOO_MANY_NAMES`),
//!   les dix précédentes restant toutes attachées. Aucun remplacement : le
//!   compte de sorties attachées suit exactement, de 1 à 11.
//! - Corroboration indépendante du même chiffre : les `TargetId` rendus par le
//!   pilote parcourent un cycle de dix valeurs (256 à 265) et le rejouent
//!   d'une exécution à l'autre — un vivier fixe de dix cibles, pas un compteur.
//! - Chien de garde : une sortie a survécu **180 s sans un seul ping de notre
//!   part**. Cela n'établit pas qu'un client puisse se taire — Apollo tourne
//!   sur cette VM et pingue le même pilote. La mesure est immunisée autrement :
//!   elle pingue.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::moniteurs::{ouvrir_pilote, PiloteParIoctl};
use crate::moniteurs_virtuels::Sorties;

/// Au-delà, on cesse de chercher : le chantier D vise 8 fenêtres, et la sonde
/// d'encodeurs emploie déjà ce même plafond de recherche.
const PLAFOND_RECHERCHE: usize = 16;

/// Résolution demandée à chaque sortie : celle que le chantier D vise par
/// fenêtre, pas celle du bureau.
const RESOLUTION: (u32, u32, u32) = (1280, 720, 60);

/// Un pilote d'affichage indirect ne publie pas sa sortie dans l'instant :
/// Windows reconfigure sa topologie d'affichage. Interroger DXGI trop tôt
/// ferait conclure à un refus là où il n'y a qu'un délai.
const DELAI_TOPOLOGIE: Duration = Duration::from_secs(3);

/// Cadence de ping du chien de garde.
///
/// Le pilote annonce `delai = 3` d'unité non documentée. Le tiers de ce délai
/// **interprété en secondes** est la cadence du client amont
/// (`sleepInterval = timeout * 1000 / 3` dans son fil de ping), et c'est la
/// seule cadence sûre pour les deux interprétations plausibles : si l'unité
/// est la seconde, une seconde laisse deux tiers de marge ; si elle est plus
/// grosse, pinguer trop souvent ne coûte qu'un IOCTL vide par seconde.
const CADENCE_PING: Duration = Duration::from_secs(1);

/// Durée par défaut de l'épreuve du chien de garde : dix fois le délai annoncé
/// lu en secondes. Une sortie qui survit à cela sans un seul ping n'est pas
/// retirée par un chien de garde de trois secondes.
///
/// La valeur de `MULTIFENETRE_VDD_VEILLE` la remplace quand elle est un entier
/// — l'épreuve est faite pour être rejouée à des horizons différents, et
/// recompiler pour changer une durée d'attente serait absurde.
const DUREE_EPREUVE_PAR_DEFAUT: Duration = Duration::from_secs(30);

/// Relève la topologie DXGI et la journalise, sortie par sortie.
///
/// Rend `(nombre total, nombre attachées au bureau)`. Les deux comptent : une
/// sortie créée mais non attachée est un cas distinct d'une sortie qui n'a pas
/// paru du tout, et la distinction est précisément ce que cette mesure doit
/// établir.
fn relever_topologie(moment: &str) -> Result<(usize, usize)> {
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
    Ok((sorties.len(), attachees))
}

/// Attend `duree` en battant le chien de garde du pilote.
///
/// Un `sleep` nu ferait exactement ce que cette mesure doit éviter : laisser le
/// pilote retirer nos sorties pendant l'attente de reconfiguration, et nous
/// faire lire un plafond qui serait le sien.
fn attendre_en_pinguant(pilote: &PiloteParIoctl, duree: Duration) -> Result<()> {
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

/// Sonde `MULTIFENETRE_VDD_VEILLE` : une sortie créée survit-elle sans ping, et
/// que vaut l'unité du délai annoncé ?
///
/// Deux observations indépendantes, faites une fois par seconde et sans jamais
/// pinguer :
///
/// - **le décompte du pilote lui-même** (`IOCTL_GET_WATCHDOG`), qui est la
///   seule fenêtre sur l'unité : un décompte qui perd une unité par seconde
///   *est* la réponse, là où le nom des champs et l'en-tête amont se taisent ;
/// - **la présence de la sortie dans DXGI**, qui dit ce que le chien de garde
///   FAIT, indépendamment de ce qu'il compte.
///
/// Les deux sont relevées parce qu'elles peuvent diverger : un décompte qui
/// tombe à zéro sans que rien ne disparaisse serait un chien de garde qui aboie
/// sans mordre, et c'est un résultat en soi.
pub(super) fn eprouver_chien_de_garde() -> Result<()> {
    let duree = std::env::var("MULTIFENETRE_VDD_VEILLE")
        .ok()
        .and_then(|valeur| valeur.parse().ok())
        .map_or(DUREE_EPREUVE_PAR_DEFAUT, Duration::from_secs);
    let (_, attachees_avant) = relever_topologie("avant création")?;

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

    let mut disparue_a = None;
    let mut decomptes = Vec::new();
    while debut.elapsed() < duree {
        std::thread::sleep(Duration::from_secs(1));
        let seconde = debut.elapsed().as_secs();
        let (veille, _) = pilote.veille()?;
        let attachees = match crate::capture::enumerer_sorties() {
            Ok(liste) => liste.iter().filter(|s| s.attachee_au_bureau).count(),
            Err(erreur) => {
                tracing::warn!(seconde, %erreur, "énumération DXGI en échec pendant l'épreuve");
                continue;
            }
        };
        decomptes.push(veille.decompte);
        tracing::info!(
            seconde,
            id,
            delai = veille.delai,
            decompte = veille.decompte,
            attachees,
            "épreuve sans ping"
        );
        if attachees <= attachees_avant && disparue_a.is_none() {
            disparue_a = Some(seconde);
        }
    }

    match disparue_a {
        Some(seconde) => tracing::error!(
            disparue_apres_s = seconde,
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "SANS PING, la sortie est retirée — toute mesure de plafond doit pinguer"
        ),
        None => tracing::info!(
            duree_epreuve_s = duree.as_secs(),
            delai_annonce = veille_initiale.delai,
            decomptes = ?decomptes,
            "sans un seul ping DE NOTRE PART, la sortie a survécu à toute l'épreuve \
             — ce qui n'établit pas qu'un client puisse se taire : un autre client \
             du même pilote (Apollo) peut le tenir éveillé"
        ),
    }
    // Un ping, UN SEUL, et à la toute fin : c'est le seul moyen d'établir que
    // `IOCTL_DRIVER_PING` — non confirmé par octets, contrairement à deux des
    // six codes — est bien le bon code, AVANT que la montée en N ne fasse
    // reposer sa validité dessus. Un code faux rendrait
    // `ERROR_INVALID_FUNCTION` plutôt que de réussir en silence.
    //
    // Ce que le décompte relevé juste après vaut en plus : s'il remonte, le
    // ping n'est pas seulement accepté, il AGIT sur le compteur observé.
    match pilote.pinguer() {
        Ok(()) => {
            let apres_ping = pilote.veille().map(|(veille, _)| veille.decompte);
            tracing::info!(
                decompte_apres_ping = ?apres_ping.as_ref().ok(),
                "IOCTL_DRIVER_PING accepté par le pilote — le code non confirmé par octets \
                 est le bon"
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
    Ok(())
}

/// Sonde `MULTIFENETRE_VDD` : la montée en N, chien de garde armé.
pub(super) fn monter_en_n() -> Result<()> {
    // Relevé AVANT toute création : sans lui, une restauration manuelle après
    // plantage se ferait à l'aveugle (spec §6.3).
    let (total_avant, attachees_avant) = relever_topologie("avant toute création")?;

    let pilote = ouvrir_pilote()?;
    let (veille, _) = pilote.veille()?;
    tracing::info!(
        delai = veille.delai,
        decompte = veille.decompte,
        cadence_ping_s = CADENCE_PING.as_secs(),
        "chien de garde armé pour toute la durée de la montée"
    );
    let (largeur, hauteur, hertz) = RESOLUTION;

    // Portée explicite : la garde doit avoir détruit AVANT le relevé final,
    // sans quoi celui-ci décrirait un état transitoire.
    let mut arret = None;
    {
        let mut sorties = Sorties::nouvelles(&pilote);
        for rang in 1..=PLAFOND_RECHERCHE {
            pilote.pinguer()?;
            match sorties.creer(largeur, hauteur, hertz) {
                Err(erreur) => {
                    tracing::info!(
                        plafond = rang - 1,
                        causes = %super::causes(erreur),
                        "plafond de sorties virtuelles atteint — le pilote refuse la suivante"
                    );
                    arret = Some("refus du pilote");
                    break;
                }
                Ok(id) => {
                    attendre_en_pinguant(&pilote, DELAI_TOPOLOGIE)?;
                    let (total, attachees) = relever_topologie(&format!("après création {rang}"))?;
                    tracing::info!(rang, id, sorties_dxgi = total, attachees, "sortie virtuelle créée");

                    // Les deux cas qui feraient basculer tout l'arbitrage du
                    // chantier D, et qu'il ne faut surtout pas confondre avec un
                    // refus. Le pilote peut accepter la demande sans que le
                    // compte suive : soit parce que la sortie n'a pas paru, soit
                    // parce qu'il REMPLACE au lieu d'ajouter — c'est ce qu'a fait
                    // Apollo pendant la sonde, par son réglage
                    // `ensure_only_display`. On vérifie ici que le pilote nu ne
                    // le fait pas.
                    if attachees < attachees_avant + rang {
                        tracing::error!(
                            rang,
                            attachees,
                            attendu = attachees_avant + rang,
                            sorties_dxgi = total,
                            "le pilote a accepté la demande mais le compte de sorties attachées \
                             ne suit pas — sortie non parue, ou remplacement au lieu d'addition"
                        );
                        arret = Some("compte de sorties non suivi");
                        break;
                    }
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

    // Une sortie virtuelle survit au processus : ne pas vérifier le retour à
    // l'état initial laisserait la VM polluée pour toutes les mesures
    // suivantes, sans que personne ne le sache.
    std::thread::sleep(DELAI_TOPOLOGIE);
    let (total_apres, attachees_apres) = relever_topologie("après destruction")?;
    if (total_apres, attachees_apres) == (total_avant, attachees_avant) {
        tracing::info!(
            arret = arret.unwrap_or("plafond de recherche épuisé"),
            total = total_apres,
            attachees = attachees_apres,
            "état initial restauré"
        );
    } else {
        tracing::error!(
            total_avant,
            attachees_avant,
            total_apres,
            attachees_apres,
            "la topologie n'est PAS revenue à son état initial — purge requise"
        );
    }
    Ok(())
}
