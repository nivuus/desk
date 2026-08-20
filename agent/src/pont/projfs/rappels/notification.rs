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
use windows::Win32::Foundation::S_OK;
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_NOTIFICATION, PRJ_NOTIFICATION_CB, PRJ_NOTIFICATION_PARAMETERS,
};

use super::garde;
use crate::pont::erreurs::hresult;
use crate::pont::notifications;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 LE `const _` PART AVEC SA FONCTION — le seul garde d'ABI de ce dépôt,
// vérifié par la compilation croisée ORDINAIRE et non par un `#[test]` (voir
// l'en-tête de [`super`], qui explique pourquoi un `#[cfg(test)]` sur une cible
// qu'on ne teste jamais n'est compilé par RIEN).
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_NOTIFICATION_CB = Some(notification);

/// **Le refus d'écriture, et c'est le PÉRIMÈTRE de F1**, pas une lacune.
///
/// ⚠️ **Le naïf ne marche pas, et c'est pour cela que ce rappel existe.** Avec
/// `showDirectoryPicker({ mode: 'read' })`, la File System Access API refuse
/// bien l'écriture côté navigateur — mais **côté VM, l'écriture RÉUSSIT
/// localement** : ProjFS hydrate le fichier et l'application écrit sur le
/// fichier NTFS local. Le fournisseur n'est prévenu qu'APRÈS coup, à la
/// fermeture du handle (spec §6.1). Une application verrait donc son
/// enregistrement réussir, et rien n'arriverait jamais côté poste local.
/// **C'est pire qu'une erreur : c'est une perte silencieuse.**
///
/// Le levier est `PRJ_NOTIFICATION_FILE_PRE_CONVERT_TO_FULL` — « une
/// application est sur le point d'écrire, il faut hydrater complètement ».
/// C'est une notification **`PRE_`, donc REFUSABLE** : rendre
/// `HRESULT_FROM_WIN32(ERROR_WRITE_PROTECT)` fait échouer l'écriture **avant**
/// qu'elle ait commencé.
///
/// Les quatre notifications refusées sont **synchrones et ne consultent jamais
/// le navigateur** (spec §4.3) : leur seul rôle est d'autoriser ou de refuser,
/// et la décision se prend sans quitter le fil. C'est ce qui impose qu'aucune
/// règle d'autorisation ne dépende d'un état distant.
pub(super) unsafe extern "system" fn notification(
    donnees: *const PRJ_CALLBACK_DATA,
    _est_repertoire: bool,
    notification: PRJ_NOTIFICATION,
    _destination: windows::core::PCWSTR,
    _parametres: *mut PRJ_NOTIFICATION_PARAMETERS,
) -> HRESULT {
    garde("Notification", || match notifications::decider(notification.0) {
        notifications::Reponse::Refuser(cause) => HRESULT(hresult(cause)),
        notifications::Reponse::AccepterEnSignalant => {
            let chemin = unsafe { donnees.as_ref() }
                .and_then(|d| unsafe { d.FilePathName.to_string() }.ok())
                .unwrap_or_default();
            tracing::warn!(
                chemin,
                "fichier créé dans la racine du pont : il vit sur la VM et ne sera JAMAIS \
                 poussé vers le poste local (F1 est en lecture seule ; la notification \
                 NEW_FILE_CREATED est une POST, elle ne se refuse pas)"
            );
            S_OK
        }
        // Une notification que le masque n'aurait pas dû livrer. Accepter EN
        // SILENCE ferait qu'un masque élargi par erreur passerait inaperçu.
        notifications::Reponse::AccepterSansAttendre => {
            tracing::warn!(
                code = notification.0,
                "notification ProjFS non attendue par le masque de F1 : acceptée sans effet"
            );
            S_OK
        }
    })
}
