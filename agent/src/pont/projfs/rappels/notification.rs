//! Le rappel de NOTIFICATION, extrait de [`super`] **avant** que F2 ne le
//! fasse grossir, et non après.
//!
//! `rappels.rs` était à **488** lignes pour une marge de **12**, et ce rappel
//! est exactement ce que F2 alourdit : il doit lire `isdirectory`, aiguiller
//! **cinq** notifications de plus, et pousser un événement vers le fil
//! d'écriture. L'extraction vient donc d'abord — geste inventé par D9
//! (`capteur/serveur/instances.rs`) et rejoué trois fois par D10 —, **jamais
//! une compression**, que `CLAUDE.md` interdit nommément.
//!
//! # Ce que cette extraction N'EST PAS
//!
//! **Elle n'ajoute aucun comportement.** La transposition est VERBATIM. Ce qui
//! l'a fait grossir vient de la tâche 12, dans un commit séparé, pour que la
//! revue puisse comparer l'un et l'autre.
//!
//! # ✅ F3 EST ARRIVÉ, ET IL LIT LES DEUX PARAMÈTRES QUE F2 IGNORAIT
//!
//! `_est_repertoire` et `_destination` étaient préfixés d'un souligné parce que
//! F2 refusait renommage et suppression. **Les deux sont désormais lus** —
//! l'un est transporté tel quel dans l'en-tête, l'autre normalisé par
//! `pont::chemins`. `_parametres`, en revanche, **reste `_parametres`** : voir
//! ci-dessous, c'est toujours une union.

use windows::core::HRESULT;
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_NOTIFICATION, PRJ_NOTIFICATION_CB, PRJ_NOTIFICATION_PARAMETERS,
};

use super::{chemins_de, etat, garde};
use crate::pont::ecriture::{fil::Ordre, Evenement};
use crate::pont::notifications;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 LE `const _` PART AVEC SA FONCTION — le seul garde d'ABI de ce dépôt,
// vérifié par la compilation croisée ORDINAIRE et non par un `#[test]` (voir
// l'en-tête de [`super`], qui explique pourquoi un `#[cfg(test)]` sur une cible
// qu'on ne teste jamais n'est compilé par RIEN).
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_NOTIFICATION_CB = Some(notification);

/// **La décision d'écriture, et le seul rappel de F2 qui change ce qu'une
/// application obtient.**
///
/// ⚠️ **Il ne DÉCIDE de rien lui-même** : la décision vit dans
/// [`crate::pont::notifications`], qui est **PUR** et éprouvé sur l'hôte. Ce
/// rappel traduit, il n'arbitre pas.
///
/// # 🔴 Ce qu'il ne fait JAMAIS, et pourquoi
///
/// **Aucune lecture de fichier, aucun verrou tenu, aucune attente.** Il court
/// sur un fil que le SYSTÈME possède : y ouvrir le fichier hydraté ferait une
/// E/S sur ce fil, et la lecture traverserait la racine — donc nos propres
/// rappels. Tout ce qu'il fait est **pousser un événement sur un `mpsc` et
/// rendre `S_OK` immédiatement** ; c'est le fil d'écriture, dédié, qui lit.
///
/// # ⚠️ `PRJ_NOTIFICATION_PARAMETERS` N'EST PAS DÉRÉFÉRENCÉ, ET C'EST DÉLIBÉRÉ
///
/// C'est une **UNION** (`mod.rs:352-356`, membres décrits en `mod.rs:364-376`),
/// et **lire le mauvais membre est un comportement indéfini**. F2 n'a besoin
/// d'aucun des trois : `PostCreate.NotificationMask` et
/// `FileRenamed.NotificationMask` servent à **changer le masque** pour ce
/// fichier, ce que F2 ne fait pas, et `FileDeletedOnHandleClose.IsFileModified`
/// concerne la suppression, qui est **F3**. Le paramètre reste donc
/// `_parametres` — **ne pas la lire du tout est le seul moyen sûr**, et le dire
/// évite qu'un successeur y voie un oubli.
///
/// ✅ **`destination` EST LUE DEPUIS F3, et ce n'est PAS un membre de l'union.**
/// C'est un **paramètre DIRECT** du rappel (`mod.rs:334`,
/// `destinationfilename: PCWSTR`). Le dire évite qu'un successeur aille la
/// chercher dans `PRJ_NOTIFICATION_PARAMETERS.FileRenamed`, qui ne porte qu'un
/// masque de notification.
///
/// ⚠️ **Elle ne porte un nom que pour `PRE_RENAME` et `FILE_RENAMED`.** Pour
/// les sept autres notifications du masque, elle est vide ou nulle — et c'est
/// pourquoi [`destination_de`] rend une [`notifications::Cible`] plutôt qu'un
/// chemin : « il n'y a pas de destination » est un état légitime, distinct de
/// « la destination est irrecevable ».
pub(super) unsafe extern "system" fn notification(
    donnees: *const PRJ_CALLBACK_DATA,
    est_repertoire: bool,
    notification: PRJ_NOTIFICATION,
    destination: windows::core::PCWSTR,
    _parametres: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT {
    garde("Notification", || {
        let Some(etat) = (unsafe { etat(donnees) }) else { return E_UNEXPECTED };
        // SÛRETÉ : `destination` est un `PCWSTR` que ProjFS a fourni ; il est
        // ou bien nul, ou bien terminé par un nul.
        let vers = unsafe { destination_de(destination) };
        let cible = match &vers {
            None => notifications::Cible::SansObjet,
            Some(Ok(_)) => notifications::Cible::DansLaRacine,
            Some(Err(())) => notifications::Cible::HorsRacine,
        };
        match notifications::decider(notification.0, etat.etat_de_notification(), cible) {
            notifications::Reponse::Refuser(cause) => HRESULT(etat.compteurs.rendre(cause)),
            // 🔵 L'écriture est autorisée. **Il n'y a rien de plus à faire
            // ici** : les octets ne nous concernent qu'à la fermeture du
            // handle, par une POST.
            notifications::Reponse::Autoriser => S_OK,
            notifications::Reponse::Pousser(quoi) => {
                // ⚠️ **La normalisation de `pont::chemins` reste la SEULE
                // barrière** contre les remontées `..`, les flux alternatifs
                // NTFS (`:`) et les noms de périphérique réservés. Elle est
                // PURE, donc éprouvée sur l'hôte.
                let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
                    // Un chemin refusé par la normalisation : on ne pousse
                    // RIEN, et `chemins_de` a déjà journalisé le refus. Rendre
                    // S_OK est le seul choix — la notification est une POST,
                    // et refuser n'empêcherait rien.
                    return S_OK;
                };
                let evenement = match quoi {
                    notifications::Poussee::Creation => {
                        Evenement::Cree { chemin, repertoire: est_repertoire }
                    }
                    notifications::Poussee::Contenu => Evenement::Modifie { chemin },
                    // ── LES DEUX POUSSÉES DE F3 ───────────────────────────
                    notifications::Poussee::Renommage => {
                        // 🔴 **DEUX INVARIANTS QUI REFUSENT PLUTÔT QUE DE
                        // DEVINER, et c'est la parade au risque le plus grave
                        // de F3 (R-F3-1).** Se tromper de SENS ne produirait
                        // aucune erreur : le renommage aurait lieu, à l'envers,
                        // et la destination écraserait la source.
                        //
                        // ⚠️ **Cette parade NE DÉPEND D'AUCUNE MESURE.** La
                        // sonde S1 relève sur pièces quel champ ProjFS porte
                        // quoi ; celle-ci tient même si la sonde n'a jamais été
                        // jouée.
                        let Some(Ok(vers)) = vers else {
                            tracing::warn!(
                                de = %chemin,
                                destination_lisible = vers.is_some(),
                                "renommage sans destination utilisable : RIEN n'est pousse"
                            );
                            return S_OK;
                        };
                        if vers.is_empty() || vers == chemin {
                            tracing::warn!(
                                de = %chemin,
                                vers = %vers,
                                "renommage dont la destination est vide ou egale a la source : \
                                 RIEN n'est pousse"
                            );
                            return S_OK;
                        }
                        Evenement::Renomme { de: chemin, vers, repertoire: est_repertoire }
                    }
                    notifications::Poussee::Suppression => {
                        Evenement::Supprime { chemin, repertoire: est_repertoire }
                    }
                };
                if etat.vers_ecriture.send(Ordre::Survenu(evenement)).is_err() {
                    // 🔴 **Le fil d'écriture est parti, et l'application a DÉJÀ
                    // enregistré.** Rien ne peut plus lui être dit : c'est
                    // l'absence de contre-pression que l'en-tête de
                    // `pont::notifications` décrit. Le `warn!` est tout ce qui
                    // reste.
                    tracing::warn!(
                        "fil d'ecriture du pont parti : une ecriture ne sera JAMAIS poussee"
                    );
                }
                S_OK
            }
            // Une notification que le masque n'aurait pas dû livrer. Accepter
            // EN SILENCE ferait qu'un masque élargi par erreur passerait
            // inaperçu.
            notifications::Reponse::AccepterSansAttendre => {
                tracing::warn!(
                    code = notification.0,
                    "notification ProjFS non attendue par le masque de F2 : acceptee sans effet"
                );
                S_OK
            }
        }
    })
}

/// La destination d'une notification, normalisée.
///
/// Trois issues, et **la distinction entre les deux dernières est ce qui rend
/// [`notifications::Cible`] plus honnête qu'un `bool`** :
///
/// - `None` — le paramètre est nul ou vide : **il n'y a pas de destination**,
///   ce qui est le cas des sept notifications du masque autres que
///   `PRE_RENAME` et `FILE_RENAMED` ;
/// - `Some(Ok(chemin))` — une destination recevable, normalisée en chemin
///   logique ;
/// - `Some(Err(()))` — une destination que `pont::chemins` refuse : remontée
///   `..`, flux alternatif NTFS, nom de périphérique réservé, chemin absolu.
///   **C'est aussi ce qu'on obtient d'une cible hors de la racine**, ProjFS ne
///   livrant que des chemins relatifs à celle-ci.
///
/// # Sûreté
///
/// L'appelant garantit que `brut` est le `destinationfilename` que ProjFS vient
/// de fournir : nul, ou terminé par un nul.
unsafe fn destination_de(brut: windows::core::PCWSTR) -> Option<Result<String, ()>> {
    if brut.is_null() {
        return None;
    }
    // SÛRETÉ : garantie de l'appelant.
    let unites = unsafe { brut.as_wide() };
    if unites.is_empty() {
        return None;
    }
    match crate::pont::chemins::normaliser_utf16(unites) {
        Ok(logique) => Some(Ok(logique)),
        Err(refus) => {
            tracing::warn!(?refus, "destination de renommage refusee par la normalisation");
            Some(Err(()))
        }
    }
}
