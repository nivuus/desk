//! Les **huit** rappels que ProjFS appelle, et le seul endroit du pont où du
//! code s'exécute sur un fil que le SYSTÈME possède.
//!
//! **TROIS d'entre eux sont ASYNCHRONES** — `GetPlaceholderInfo`,
//! `GetFileData`, `GetDirectoryEnumeration` (plus `QueryFileName`, qui emprunte
//! la même requête que le premier) : ils inscrivent une commande, poussent une
//! requête, rendent `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` et rendent la main.
//!
//! ⚠️ **La spec §8 annonce « les CINQ rappels obligatoires en mode
//! asynchrone » ; c'est un écart d'énoncé, pas de conception, et sa propre
//! table §4.3 le dit** : `StartDirectoryEnumeration` et
//! `EndDirectoryEnumeration` y figurent comme « synchrone, `S_OK` », puisqu'ils
//! ne consultent jamais le navigateur. **Cinq sont implémentés, trois sont
//! asynchrones.**
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

mod listage;
mod notification;

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_CANCEL_COMMAND_CB, PRJ_GET_FILE_DATA_CB, PRJ_GET_PLACEHOLDER_INFO_CB,
    PRJ_QUERY_FILE_NAME_CB,
};

use crate::pont::chemins;
use proto::fichiers::entetes;
use crate::pont::erreurs::{hresult, Erreur, EN_COURS};
use crate::pont::projfs::{ContexteProjFs, Etat, FluxDonnees};
use crate::pont::table::{Attendue, DELAI_ATTRIBUTS};

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
const _: PRJ_GET_PLACEHOLDER_INFO_CB = Some(info_marqueur);
const _: PRJ_GET_FILE_DATA_CB = Some(donnees_fichier);
const _: PRJ_QUERY_FILE_NAME_CB = Some(nom_fichier);
const _: PRJ_CANCEL_COMMAND_CB = Some(annulation);

/// Enveloppe commune : `catch_unwind`, et `E_UNEXPECTED` sur panique.
///
/// **Le message nomme le rappel.** Sans lui, la seule trace d'une panique
/// serait un `E_UNEXPECTED` rendu à une application, c'est-à-dire une erreur
/// d'E/S sans cause lisible — le défaut que `pont::erreurs` existe pour ne pas
/// rejouer.
pub(super) fn garde(rappel: &'static str, corps: impl FnOnce() -> HRESULT + std::panic::UnwindSafe) -> HRESULT {
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
pub(super) unsafe fn etat<'a>(donnees: *const PRJ_CALLBACK_DATA) -> Option<&'a Etat> {
    if donnees.is_null() {
        return None;
    }
    let contexte = unsafe { (*donnees).InstanceContext } as *const Etat;
    if contexte.is_null() {
        return None;
    }
    Some(unsafe { &*contexte })
}

/// Le chemin livré par ProjFS, sous ses DEUX formes : celle que la File System
/// Access API attend (logique, séparée par `/`, **normalisée et vérifiée**), et
/// celle que ProjFS reprendra telle quelle.
///
/// ⚠️ **La normalisation n'est pas un confort : c'est la seule barrière.** La
/// racine de virtualisation est traversée par n'importe quelle application de
/// la session Windows, y compris hostile — remontées `..`, flux alternatifs
/// NTFS, noms de périphérique réservés. `pont::chemins` les refuse, et il est
/// PUR, donc éprouvé sur l'hôte.
pub(super) unsafe fn chemins_de(donnees: *const PRJ_CALLBACK_DATA) -> Option<(String, Vec<u16>)> {
    let brut = unsafe { donnees.as_ref() }?.FilePathName;
    if brut.is_null() {
        // La racine elle-même : chemin vide des deux côtés.
        return Some((String::new(), vec![0u16]));
    }
    // SÛRETÉ : ProjFS garantit un `PCWSTR` terminé par un nul.
    let unites: Vec<u16> = unsafe { brut.as_wide() }.to_vec();
    let logique = match chemins::normaliser_utf16(&unites) {
        Ok(logique) => logique,
        Err(refus) => {
            tracing::warn!(?refus, "chemin ProjFS refusé par la normalisation");
            return None;
        }
    };
    let projfs = unites.into_iter().chain(std::iter::once(0)).collect();
    Some((logique, projfs))
}

/// Le GUID d'une énumération, sous la forme que [`crate::pont::table`] emploie.
///
/// `[u8; 16]` et non `GUID` : la table est **pure** et ne connaît pas
/// `windows`. La conversion passe par `to_u128`, donc elle est totale et
/// réversible — aucune interprétation des champs du GUID n'est faite ici.
pub(super) unsafe fn identifiant(guid: *const GUID) -> Option<[u8; 16]> {
    if guid.is_null() {
        return None;
    }
    Some(unsafe { (*guid).to_u128() }.to_le_bytes())
}

/// Rend les métadonnées d'une entrée.
///
/// ❌ *Annonçait `ERROR_FILE_NOT_FOUND` : état de la tâche 13, réfuté par 14.*
unsafe extern "system" fn info_marqueur(donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("GetPlaceholderInfo", || {
        let Some(etat) = (unsafe { etat(donnees) }) else { return E_UNEXPECTED };
        let Some((chemin, chemin_projfs)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(hresult(Erreur::CheminIntrouvable));
        };
        let entete = match serde_json::to_string(&entetes::Chemin { chemin: chemin.clone() }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Attributs { chemin },
            std::time::Instant::now() + DELAI_ATTRIBUTS,
            ContexteProjFs::Attributs { chemin_projfs },
            proto::fichiers::TYPE_ATTRIBUTS,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(hresult(Erreur::CanalFerme))
        }
    })
}

/// Rend le contenu d'un fichier.
///
/// ❌ *Annonçait `ERROR_FILE_NOT_FOUND` : état de la tâche 13, réfuté par 14.*
unsafe extern "system" fn donnees_fichier(
    donnees: *const PRJ_CALLBACK_DATA,
    position: u64,
    longueur: u32,
) -> HRESULT {
    garde("GetFileData", || {
        let Some(etat) = (unsafe { etat(donnees) }) else { return E_UNEXPECTED };
        let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(hresult(Erreur::CheminIntrouvable));
        };
        // ⚠️ **Le fichier entier n'entre JAMAIS en mémoire** : la plage est
        // découpée par `pont::decoupe`, PUR et testé, et **un seul morceau est
        // en vol à la fois** en F1. Le contrôle de flux par `bufferedAmount`
        // est un livrable de F3 ; l'implémenter à moitié ici serait pire.
        let mut morceaux: std::collections::VecDeque<_> = crate::pont::decoupe::decouper(
            position,
            u64::from(longueur),
            proto::fichiers::TAILLE_TRAME_MAX,
        )
        .into();
        let Some(premier) = morceaux.pop_front() else {
            // Longueur nulle : rien à écrire, et rien à demander. Compléter
            // tout de suite plutôt qu'inscrire une commande qui n'aurait
            // jamais de réponse.
            return S_OK;
        };
        let flux = unsafe { (*donnees).DataStreamId };
        let entete = match serde_json::to_string(&entetes::Lire {
            chemin: chemin.clone(),
            position: premier.position,
            longueur: premier.longueur,
        }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Lire {
                chemin,
                position: premier.position,
                longueur: premier.longueur,
            },
            std::time::Instant::now() + crate::pont::table::DELAI_LIRE,
            ContexteProjFs::Lecture { flux: FluxDonnees(flux), restants: morceaux },
            proto::fichiers::TYPE_LIRE,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(hresult(Erreur::CanalFerme))
        }
    })
}

/// Dit si un nom existe. Consulté en permanence par Windows pour des chemins
/// qui n'existent pas (`desktop.ini`, `Thumbs.db`, les manifestes
/// d'application) — d'où le cache négatif armé au démarrage.
///
/// ❌ *Annonçait `ERROR_FILE_NOT_FOUND` : état de la tâche 13, réfuté par 14.*
unsafe extern "system" fn nom_fichier(donnees: *const PRJ_CALLBACK_DATA) -> HRESULT {
    garde("QueryFileName", || {
        let Some(etat) = (unsafe { etat(donnees) }) else { return E_UNEXPECTED };
        let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(hresult(Erreur::CheminIntrouvable));
        };
        let entete = match serde_json::to_string(&entetes::Chemin { chemin: chemin.clone() }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        // Même requête que `GetPlaceholderInfo` : « ce nom existe-t-il ? » et
        // « quelles sont ses métadonnées ? » ont la même réponse côté
        // navigateur. Le cache négatif de ProjFS
        // (`PRJ_FLAG_USE_NEGATIVE_PATH_CACHE`) est ce qui empêche que les
        // sondages permanents de Windows — `desktop.ini`, `Thumbs.db`,
        // `folder.jpg`, les manifestes d'application — deviennent chacun un
        // aller-retour navigateur (spec §7.4).
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Attributs { chemin },
            std::time::Instant::now() + DELAI_ATTRIBUTS,
            ContexteProjFs::Existence,
            proto::fichiers::TYPE_ATTRIBUTS,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(hresult(Erreur::CanalFerme))
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
        let correlation = etat.table.lock().expect("verrou de la table").annuler(commande);
        if let Some(correlation) = correlation {
            // ⚠️ **Le contexte ProjFS part AVEC l'entrée de table, sinon il
            // fuit.** `Table::annuler` ne connaît que la table — elle est PURE
            // — et le tampon d'énumération ou le flux de données d'une commande
            // annulée resterait sinon dans `en_attente` pour toute la vie du
            // pont, sans que rien ne le lise jamais.
            if let Ok(mut attente) = etat.en_attente.lock() {
                attente.remove(&correlation);
            }
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
        StartDirectoryEnumerationCallback: Some(listage::debut_enumeration),
        EndDirectoryEnumerationCallback: Some(listage::fin_enumeration),
        GetDirectoryEnumerationCallback: Some(listage::suite_enumeration),
        GetPlaceholderInfoCallback: Some(info_marqueur),
        GetFileDataCallback: Some(donnees_fichier),
        QueryFileNameCallback: Some(nom_fichier),
        NotificationCallback: Some(notification::notification),
        CancelCommandCallback: Some(annulation),
    }
}
