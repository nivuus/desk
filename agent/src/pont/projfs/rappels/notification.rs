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
/// ⚠️ **`_destination` non plus** : elle ne porte un nom que pour
/// `PRE_RENAME` / `FILE_RENAMED`, que F2 refuse (F3 les livrera).
pub(super) unsafe extern "system" fn notification(
    donnees: *const PRJ_CALLBACK_DATA,
    est_repertoire: bool,
    notification: PRJ_NOTIFICATION,
    _destination: windows::core::PCWSTR,
    _parametres: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT {
    garde("Notification", || {
        let Some(etat) = (unsafe { etat(donnees) }) else { return E_UNEXPECTED };
        match notifications::decider(notification.0, etat.etat_de_notification()) {
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
