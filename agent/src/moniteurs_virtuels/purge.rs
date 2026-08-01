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
//! identifiant ; `pilote.rs` ne tient ses deux tables (`apparies`,
//! `a_purger`) que dans la mémoire du processus, perdues avec lui. Deux
//! angles existent :
//!
//! 1. **Régénérer les GUID.** Ceux que ce module attribue sont
//!    DÉTERMINISTES (`guid::guid_pour` : gabarit constant | numéro reparti de
//!    zéro à chaque exécution), et la tâche 6 a montré que le pilote réemploie
//!    ses `TargetId` d'une exécution à l'autre — le pilote lui-même est
//!    stable. Une purge peut donc rejouer la même suite de GUID
//!    (1..=`PLAFOND_NUMEROS`) et tenter un retrait sur chacun. **C'est la
//!    stratégie retenue ci-dessous** : aucun état à faire survivre, aucun
//!    fichier à tenir à jour entre deux exécutions — juste rejouer un calcul
//!    pur.
//! 2. **Journaliser les GUID sur disque** pour qu'un processus séparé les
//!    relise. Écartée : un tel journal serait lui-même un état qui peut se
//!    corrompre ou manquer (le processus tué net n'a peut-être pas eu le
//!    temps de l'écrire), pour ne gagner qu'une chose que l'angle 1 a déjà
//!    sans lui.
//!
//! **Un refus est l'issue NORMALE, pas une erreur** : la plupart des GUID
//! régénérés ne désignent RIEN (une exécution précédente en a créé moins de
//! `PLAFOND_NUMEROS`, ou aucune), et un refus sur un GUID libre est
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
//! - **rien, désormais, du côté d'un numéro trop grand** — et c'est le
//!   correctif I1 de la revue finale de branche. La rédaction précédente
//!   justifiait le plafond par « hors d'atteinte tant qu'il dépasse ce que le
//!   pilote accepte (10, mesuré à la tâche 6) », raisonnement qui valait pour
//!   un banc créant au plus dix sorties dans toute sa vie. Le superviseur, lui,
//!   crée une sortie par **ouverture** de fenêtre, sans borne : avec le
//!   compteur monotone d'alors, la 17ᵉ ouverture attribuait un GUID que cette
//!   purge n'aurait jamais balayé, donc un moniteur irrécupérable sans
//!   redémarrage de la VM. Deux choses le ferment maintenant, et il faut les
//!   deux : `numeros::Numeros` **recycle** le numéro d'une sortie réellement
//!   retirée (les numéros en vol sont donc bornés par ce qui est réellement
//!   dû), et il **refuse** de créer au-delà de `PLAFOND_NUMEROS` plutôt que
//!   d'attribuer hors plage. La plage balayée ci-dessous couvre donc, par
//!   construction, tout ce que ce gabarit a pu attribuer ;
//! - deux instances de `PiloteParIoctl` ouvertes en parallèle (deux
//!   processus vivants à la fois, ou un même processus qui rouvrirait le
//!   périphérique) : le distributeur de `numeros` repart de zéro par INSTANCE,
//!   pas par processus ni par pilote physique — les deux attribueraient donc
//!   les mêmes GUID, et cette purge ne retire qu'un seul retrait par numéro,
//!   pas deux. Dette antérieure à cette tâche, mais qui borne ce que la liste
//!   ci-dessus prétend couvrir.

use anyhow::Result;

use crate::moniteurs_virtuels::guid::guid_pour;
use crate::moniteurs_virtuels::numeros::PLAFOND_NUMEROS;
use crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl};
// Ce module reste consommateur de `diagnostics::multifenetre::montee` pour
// deux items de mesure — le relevé de topologie DXGI avant/après et son délai
// d'établissement. Le plafond, lui, ne vient PLUS de là : il vit désormais
// dans `moniteurs_virtuels::numeros`, aux côtés du distributeur qui le fait
// respecter (correctif I1) — c'était le seul des trois emprunts dont la valeur
// engageait la récupérabilité d'un état système, et il n'avait rien à faire
// dans un module de mesure. Voir la réserve de la tâche 4, réécrite à son vrai
// périmètre : `boucle.rs` emprunte lui aussi à ce module.
use crate::diagnostics::multifenetre::montee::{relever_topologie, DELAI_TOPOLOGIE};

/// Sonde `MULTIFENETRE_VDD_PURGE`.
pub(crate) fn purger() -> Result<()> {
    // `relever_topologie` journalise déjà `topologie relevée moment="avant
    // purge" nombre=…` — un second message ici ferait doublon.
    let avant = relever_topologie("avant purge")?;

    let pilote = ouvrir_pilote()?;
    let mut retirees = 0usize;
    for numero in 1..=PLAFOND_NUMEROS {
        let guid = guid_pour(numero);
        match pilote.retirer_par_guid(guid, "retrait déterministe (purge inter-processus)") {
            Ok(()) => {
                retirees += 1;
                tracing::info!(numero, guid = ?guid, "sortie virtuelle retirée par la purge");
            }
            Err(erreur) => {
                // En `debug`, pas silencieux : sur `PLAFOND_NUMEROS`
                // essais, la grande majorité vise un GUID que personne n'a
                // jamais attribué, et c'est l'issue attendue — un `info` ou
                // `warn` par essai noierait le signal utile. Mais l'absence
                // TOTALE de trace est ce que I1 reprochait : sans elle,
                // l'affirmation du commentaire de tête (« indiscernable
                // d'un GUID tenu par un processus vivant ») n'était vérifiable
                // par personne, y compris nous. Le code d'erreur reste ici,
                // consultable a posteriori.
                tracing::debug!(numero, guid = ?guid, %erreur, "retrait refusé (GUID jamais attribué, ou échec réel — indiscernable côté code de retour)");
            }
        }
    }

    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après purge")?;

    // Verdict explicite : sans lui, un handle ou un IOCTL cassé produirait
    // silencieusement `retirees=0` et un `Ok(())` — la sonde dont le métier
    // EST de restaurer serait alors la seule à ne rien juger, alors que
    // `monter_en_n` (montee.rs) en émet un dans le cas symétrique.
    let attendu = avant.len().saturating_sub(retirees);
    if apres.len() == attendu {
        tracing::info!(retirees, avant = avant.len(), apres = apres.len(), "purge terminée");
    } else {
        tracing::error!(
            retirees,
            avant = avant.len(),
            apres = apres.len(),
            attendu,
            "purge terminée SANS retrouver le compte attendu — topologie non \
             conforme à ce que la purge a retiré"
        );
    }
    Ok(())
}

/// Retente le retrait de toute sortie que CE `pilote` sait due mais n'a pas
/// réussi à retirer plus tôt — la dette laissée par la tâche 5 : `a_purger`
/// n'était relue par personne, un retrait raté restait donc irrécupérable
/// pour le reste de l'exécution alors même que son GUID était connu.
///
/// **N'est PAS appelée par `purger()` ci-dessus.** `a_purger` n'est
/// alimentée que par `creer` et `detruire` (`moniteurs.rs`) ; `purger()`
/// n'appelle ni l'un ni l'autre — elle passe exclusivement par
/// `retirer_par_guid`, qui ne touche à aucune table. `pilote.a_purger()` y
/// serait donc TOUJOURS vide : l'appeler là n'aurait rien fermé, seulement
/// simulé une fermeture. La seule utilisatrice réelle est
/// `montee::monter_en_n`, où elle rejoue, juste après que la garde `Sorties`
/// a fini de détruire et AVANT que `pilote` ne parte à son tour, les
/// retraits qu'elle a laissés dus. Distincte de `purger()` par ce qu'elle
/// vise — un état encore vivant en mémoire, pas un état régénéré par calcul
/// — mais assez proche pour partager le même geste (tenter, traiter un refus
/// comme une issue possible), d'où sa place ici plutôt que dans
/// `moniteurs.rs`, qui n'a plus la place sous le plafond de 500 lignes.
pub(crate) fn rejouer_purge_due(pilote: &PiloteParIoctl) -> usize {
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
