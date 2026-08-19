//! Les **huit** rappels que ProjFS appelle, et le seul endroit du pont où du
//! code s'exécute sur un fil que le SYSTÈME possède.
//!
//! ⚠️ **Squelette de la tâche 13 : les cinq rappels obligatoires existent tous,
//! mais aucun ne demande encore quoi que ce soit au navigateur.** Trois d'entre
//! eux — `GetPlaceholderInfo`, `GetFileData`, `GetDirectoryEnumeration` — sont
//! branchés sur la table et le canal par la tâche 14. Ici, ils refusent.
//!
//! # Ce que chaque rappel a le droit de faire, et rien de plus
//!
//! Voir la discipline de fil en tête de [`super`]. En résumé, un rappel :
//!
//! - enveloppe **tout** son corps dans [`std::panic::catch_unwind`] et rend
//!   `E_UNEXPECTED` — une panique Rust qui traverserait une frontière
//!   `extern "system"` est un **abandon de processus**, et c'est exactement
//!   ce que la décision D2 (le pont dans son propre processus) rend
//!   supportable plutôt que fatal ;
//! - n'écrit que dans les états sous verrou de [`super::Etat`], jamais pendant
//!   une E/S ;
//! - **n'appelle JAMAIS `PrjCompleteCommand`** — c'est le fil du pont qui
//!   complète ;
//! - rend la main **immédiatement**.

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_CANCEL_COMMAND_CB, PRJ_DIR_ENTRY_BUFFER_HANDLE,
    PRJ_END_DIRECTORY_ENUMERATION_CB, PRJ_GET_DIRECTORY_ENUMERATION_CB, PRJ_GET_FILE_DATA_CB,
    PRJ_GET_PLACEHOLDER_INFO_CB, PRJ_NOTIFICATION, PRJ_NOTIFICATION_CB,
    PRJ_NOTIFICATION_PARAMETERS, PRJ_QUERY_FILE_NAME_CB, PRJ_START_DIRECTORY_ENUMERATION_CB,
};

use crate::pont::erreurs::{hresult, Erreur};
use crate::pont::notifications;
use crate::pont::projfs::Etat;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 LE SEUL GARDE D'ABI QUE CE DÉPÔT POSSÈDE, et il ne couvre QUE ces huit
// fonctions-ci — celles que nous ÉCRIVONS, un ensemble DISJOINT des treize que
// nous APPELONS (voir l'en-tête de `chargement.rs`).
//
// ⚠️ **Un `const _` et non un `#[test]`, et c'est une divergence DÉLIBÉRÉE
// d'avec le plan de F1** (tâche 12, step 4b), qui écrit ce contrôle sous forme
// de `#[test]` en affirmant qu'« il est donc couvert par `cargo check --target
// x86_64-pc-windows-gnu` ». **C'est faux** : `cargo check` sans `--tests` ne
// compile pas le code de test, et un `#[cfg(test)]` sur une cible qu'on ne
// teste jamais n'est compilé par RIEN. Le contrôle du plan n'aurait pas pu
// échouer. Sous forme de `const _`, il est vérifié par la compilation croisée
// ordinaire, à chaque fois.
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_START_DIRECTORY_ENUMERATION_CB = Some(debut_enumeration);
const _: PRJ_END_DIRECTORY_ENUMERATION_CB = Some(fin_enumeration);
const _: PRJ_GET_DIRECTORY_ENUMERATION_CB = Some(suite_enumeration);
const _: PRJ_GET_PLACEHOLDER_INFO_CB = Some(info_marqueur);
const _: PRJ_GET_FILE_DATA_CB = Some(donnees_fichier);
const _: PRJ_QUERY_FILE_NAME_CB = Some(nom_fichier);
const _: PRJ_NOTIFICATION_CB = Some(notification);
const _: PRJ_CANCEL_COMMAND_CB = Some(annulation);

/// Enveloppe commune : `catch_unwind`, et `E_UNEXPECTED` sur panique.
///
/// **Le message nomme le rappel.** Sans lui, la seule trace d'une panique
/// serait un `E_UNEXPECTED` rendu à une application, c'est-à-dire une erreur
/// d'E/S sans cause lisible — le défaut que `pont::erreurs` existe pour ne pas
/// rejouer.
fn garde(rappel: &'static str, corps: impl FnOnce() -> HRESULT + std::panic::UnwindSafe) -> HRESULT {
    match std::panic::catch_unwind(corps) {
        Ok(resultat) => resultat,
        Err(_) => {
            tracing::error!(
                rappel,
                "panique dans un rappel ProjFS : rattrapée avant la frontière FFI, \
                 E_UNEXPECTED rendu à l'application"
            );
            E_UNEXPECTED
        }
    }
}

/// Récupère l'état du pont depuis `InstanceContext`.
///
/// # Sûreté
///
/// L'appelant garantit que `donnees` est le `PRJ_CALLBACK_DATA` que ProjFS
/// vient de fournir, et que son `InstanceContext` est le pointeur confié à
/// `PrjStartVirtualizing` — un `Arc<Etat>` que [`super::Virtualisation`] tient
/// vivant jusqu'après `PrjStopVirtualizing`. ProjFS garantit qu'aucun rappel ne
/// court après le retour de cet appel, c'est-à-dire avant que l'`Arc` ne soit
/// repris.
unsafe fn etat<'a>(donnees: *const PRJ_CALLBACK_DATA) -> Option<&'a Etat> {
    if donnees.is_null() {
        return None;
    }
    let contexte = unsafe { (*donnees).InstanceContext } as *const Etat;
    if contexte.is_null() {
        return None;
    }
    Some(unsafe { &*contexte })
}

/// Le GUID d'une énumération, sous la forme que [`crate::pont::table`] emploie.
///
/// `[u8; 16]` et non `GUID` : la table est **pure** et ne connaît pas
/// `windows`. La conversion passe par `to_u128`, donc elle est totale et
/// réversible — aucune interprétation des champs du GUID n'est faite ici.
unsafe fn identifiant(guid: *const GUID) -> Option<[u8; 16]> {
    if guid.is_null() {
        return None;
    }
    Some(unsafe { (*guid).to_u128() }.to_le_bytes())
}

/// Ouvre une session d'énumération. **Synchrone, `S_OK`** — il n'y a rien à
/// demander au navigateur pour ouvrir une session (spec §4.3).
unsafe extern "system" fn debut_enumeration(
    donnees: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
) -> HRESULT {
    garde("StartDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(donnees) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        // ⚠️ La session est indexée par le GUID d'ÉNUMÉRATION, jamais par le
        // chemin : deux applications qui listent le même répertoire en même
        // temps ouvrent deux sessions distinctes, et indexer par chemin ferait
        // que la seconde écraserait la première — l'une des deux recevrait un
        // répertoire vide (spec §7.2).
        etat.enumerations.lock().expect("verrou des énumérations").insert(id);
        S_OK
    })
}

/// Ferme une session d'énumération. **Synchrone, `S_OK`.**
unsafe extern "system" fn fin_enumeration(
    donnees: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
) -> HRESULT {
    garde("EndDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(donnees) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        etat.enumerations.lock().expect("verrou des énumérations").remove(&id);
        S_OK
    })
}

/// Rend les entrées d'un répertoire.
///
/// ⚠️ **Tâche 13 : `S_OK` avec un tampon vide**, donc la racine paraît vide.
/// C'est l'état VERT que la tâche 13 vise — le dossier apparaît, et le pont
/// s'arrête proprement. La tâche 14 y branche la requête `Lister`.
unsafe extern "system" fn suite_enumeration(
    _donnees: *const PRJ_CALLBACK_DATA,
    _enumeration: *const GUID,
    _expression: windows::core::PCWSTR,
    _tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    garde("GetDirectoryEnumeration", || S_OK)
}

/// Rend les métadonnées d'une entrée.
///
/// ⚠️ **Tâche 13 : `ERROR_FILE_NOT_FOUND`.** Branché en tâche 14.
unsafe extern "system" fn info_marqueur(_donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("GetPlaceholderInfo", || HRESULT(hresult(Erreur::Introuvable)))
}

/// Rend le contenu d'un fichier.
///
/// ⚠️ **Tâche 13 : `ERROR_FILE_NOT_FOUND`.** Branché en tâche 14.
unsafe extern "system" fn donnees_fichier(
    _donnees: *const PRJ_CALLBACK_DATA,
    _position: u64,
    _longueur: u32,
) -> HRESULT {
    garde("GetFileData", || HRESULT(hresult(Erreur::Introuvable)))
}

/// Dit si un nom existe. Consulté en permanence par Windows pour des chemins
/// qui n'existent pas (`desktop.ini`, `Thumbs.db`, les manifestes
/// d'application) — d'où le cache négatif armé au démarrage.
///
/// ⚠️ **Tâche 13 : `ERROR_FILE_NOT_FOUND`.** Branché en tâche 14.
unsafe extern "system" fn nom_fichier(_donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("QueryFileName", || HRESULT(hresult(Erreur::Introuvable)))
}

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
unsafe extern "system" fn notification(
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

/// Retire une commande abandonnée par l'application.
///
/// ⚠️ **Obligatoire dès F1.** `PRJ_CANCEL_COMMAND_CB` n'est optionnel que pour
/// un fournisseur SYNCHRONE ; le nôtre ne l'est pas (spec §4.3). Sans lui, une
/// application qui abandonne son E/S nous laisserait une commande orpheline
/// dans la table, et sa réponse tardive serait appliquée à un tampon que le
/// système a repris.
unsafe extern "system" fn annulation(donnees: *const PRJ_CALLBACK_DATA) {
    // Ce rappel ne rend RIEN (`PRJ_CANCEL_COMMAND_CB`), donc `garde` — qui rend
    // un `HRESULT` — ne s'applique pas tel quel. Le `catch_unwind` est écrit à
    // la main : c'est la même exigence, et l'oublier ici serait exactement
    // aussi fatal.
    let issue = std::panic::catch_unwind(|| {
        let Some(etat) = (unsafe { etat(donnees) }) else { return };
        let commande = unsafe { (*donnees).CommandId };
        if let Some(correlation) = etat.table.lock().expect("verrou de la table").annuler(commande)
        {
            tracing::debug!(commande, correlation, "commande ProjFS annulée par l'application");
        }
    });
    if issue.is_err() {
        tracing::error!(
            rappel = "CancelCommand",
            "panique dans un rappel ProjFS : rattrapée avant la frontière FFI"
        );
    }
}

/// Le bloc des huit rappels, tel que `PrjStartVirtualizing` l'attend.
pub(super) fn bloc() -> windows::Win32::Storage::ProjectedFileSystem::PRJ_CALLBACKS {
    windows::Win32::Storage::ProjectedFileSystem::PRJ_CALLBACKS {
        StartDirectoryEnumerationCallback: Some(debut_enumeration),
        EndDirectoryEnumerationCallback: Some(fin_enumeration),
        GetDirectoryEnumerationCallback: Some(suite_enumeration),
        GetPlaceholderInfoCallback: Some(info_marqueur),
        GetFileDataCallback: Some(donnees_fichier),
        QueryFileNameCallback: Some(nom_fichier),
        NotificationCallback: Some(notification),
        CancelCommandCallback: Some(annulation),
    }
}
