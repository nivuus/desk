//! Filet de rattrapage : détruit les sorties virtuelles laissées par une
//! exécution précédente que la garde RAII n'a pas pu couvrir.
//!
//! **Pourquoi la garde ne suffit pas.** `moniteurs_virtuels::Sorties`
//! détruit tout ce qu'elle a créé, y compris pendant une panique — mais
//! `Drop::drop` ne court jamais pour un processus tué net (`Stop-Process
//! -Force`, une VM qui gèle, un plantage À L'INTÉRIEUR du pilote lui-même).
//! Un moniteur virtuel est un état global du système, pas un état du
//! processus : il survit à sa disparition. Sans cette purge, le seul
//! rattrapage serait de redémarrer la VM.
//!
//! **Comment retrouver les GUID d'un processus mort — la question que le
//! plan initial n'avait pas anticipée.** Le pilote retire par GUID, pas par
//! identifiant ; `moniteurs.rs` ne tient ses deux tables (`apparies`,
//! `a_purger`) que dans la mémoire du processus, perdues avec lui. Deux
//! angles existent :
//!
//! 1. **Régénérer les GUID.** Ceux que ce module attribue sont
//!    DÉTERMINISTES (`moniteurs::guid_pour` : gabarit constant | compteur
//!    reparti de zéro à chaque exécution), et la tâche 6 a montré que le
//!    pilote réemploie ses `TargetId` d'une exécution à l'autre — le pilote
//!    lui-même est stable. Une purge peut donc rejouer la même suite de GUID
//!    (1..=`PLAFOND_RECHERCHE`, le même plafond que la montée en N) et tenter
//!    un retrait sur chacun. **C'est la stratégie retenue ci-dessous** :
//!    aucun état à faire survivre, aucun fichier à tenir à jour entre deux
//!    exécutions — juste rejouer un calcul pur.
//! 2. **Journaliser les GUID sur disque** pour qu'un processus séparé les
//!    relise. Écartée : un tel journal serait lui-même un état qui peut se
//!    corrompre ou manquer (le processus tué net n'a peut-être pas eu le
//!    temps de l'écrire), pour ne gagner qu'une chose que l'angle 1 a déjà
//!    sans lui.
//!
//! **Un refus est l'issue NORMALE, pas une erreur** : la plupart des GUID
//! régénérés ne désignent RIEN (une exécution précédente en a créé moins de
//! `PLAFOND_RECHERCHE`, ou aucune), et un refus sur un GUID libre est
//! indiscernable d'un refus sur un GUID tenu par un processus encore vivant
//! — cette purge ne fait donc aucune différence entre les deux, par
//! construction.
//!
//! **Ce que cette purge NE rattrape PAS**, à dire explicitement plutôt qu'à
//! laisser deviner :
//! - toute sortie créée par un AUTRE logiciel que ce module (Apollo, par
//!   exemple, attribue ses propres GUID à ses propres sorties) ;
//! - toute sortie créée par une version ANTÉRIEURE de `GABARIT_GUID_MONITEUR`
//!   — le gabarit est une constante arbitraire choisie ici ; s'il change un
//!   jour, les sorties de l'ancien gabarit deviennent invisibles à cette
//!   purge, exactement comme elles le seraient à l'ancien code lui-même ;
//!   - un compteur qui aurait dépassé `PLAFOND_RECHERCHE` avant de mourir —
//!   hors d'atteinte tant que ce plafond dépasse ce que le pilote accepte
//!   (10, mesuré à la tâche 6), mais pas une garantie au-delà.

use anyhow::Result;

use super::moniteurs::{guid_pour, ouvrir_pilote, PiloteParIoctl};
use super::montee::{relever_topologie, DELAI_TOPOLOGIE, PLAFOND_RECHERCHE};

/// Sonde `MULTIFENETRE_VDD_PURGE`.
pub(super) fn purger() -> Result<()> {
    let avant = relever_topologie("avant purge")?;
    tracing::info!(nombre = avant.len(), "topologie avant purge");

    let pilote = ouvrir_pilote()?;
    let mut retirees = 0usize;
    for numero in 1..=PLAFOND_RECHERCHE as u16 {
        let guid = guid_pour(numero);
        if pilote.retirer_par_guid(guid, "retrait déterministe (purge inter-processus)").is_ok() {
            retirees += 1;
            tracing::info!(numero, guid = ?guid, "sortie virtuelle retirée par la purge");
        }
        // Un refus n'est PAS journalisé individuellement : sur
        // `PLAFOND_RECHERCHE` essais, la grande majorité vise un GUID que
        // personne n'a jamais attribué, et c'est l'issue attendue — un
        // journal d'erreurs par essai noierait le signal utile.
    }

    // Ferme la même dette que `montee::monter_en_n` : si CE processus de
    // purge a lui-même laissé un retrait dû (son propre `retirer_par_guid`
    // a échoué au lieu de simplement refuser), on le rejoue avant de
    // conclure plutôt que de le laisser filer avec le processus.
    let rejoues = rejouer_purge_due(&pilote);

    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après purge")?;
    tracing::info!(
        retirees,
        rejoues,
        avant = avant.len(),
        apres = apres.len(),
        "purge terminée"
    );
    Ok(())
}

/// Retente le retrait de toute sortie que CE `pilote` sait due mais n'a pas
/// réussi à retirer plus tôt — la dette laissée par la tâche 5 : `a_purger`
/// n'était relue par personne, un retrait raté restait donc irrécupérable
/// pour le reste de l'exécution alors même que son GUID était connu.
///
/// Distincte de `purger()` ci-dessus, qui régénère des GUID pour un
/// processus qui n'existe PLUS, mais assez proche pour partager le même
/// geste — tenter `retirer_par_guid`, traiter un refus comme une issue
/// possible — d'où sa place ici plutôt que dans `moniteurs.rs`, qui n'a plus
/// la place pour l'accueillir sous le plafond de 500 lignes. Appelée aussi
/// par `montee::monter_en_n`, où elle rejoue les retraits que la garde
/// `Sorties` a laissés dus AVANT que `pilote` ne parte à son tour.
pub(super) fn rejouer_purge_due(pilote: &PiloteParIoctl) -> usize {
    let mut reussis = 0usize;
    for guid_moniteur in pilote.a_purger() {
        match pilote.retirer_par_guid(guid_moniteur, "retrait rejoué d'un retrait dû") {
            Ok(()) => {
                pilote.oublier(guid_moniteur);
                reussis += 1;
                tracing::info!(guid = ?guid_moniteur, "retrait dû rejoué avec succès");
            }
            Err(erreur) => {
                tracing::warn!(guid = ?guid_moniteur, %erreur, "retrait dû toujours refusé");
            }
        }
    }
    reussis
}
