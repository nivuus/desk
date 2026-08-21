//! Les TROIS rappels d'énumération de répertoire, extraits de [`super`].
//!
//! # Pourquoi ce fichier s'appelle `listage` et non `enumeration`
//!
//! La spec §9 écrit : « s'il approche 500, **les rappels d'énumération partent
//! dans `projfs/enumeration.rs`**, jamais par compression ». **Deux choses ont
//! changé depuis** :
//!
//! 1. les rappels ne vivent plus dans `projfs.rs` mais dans
//!    `projfs/rappels.rs` — extrait par la tâche 13 de F1 —, si bien que le
//!    point de chute est `projfs/rappels/…` ;
//! 2. **`agent/src/pont/enumeration.rs` EXISTE DÉJÀ**, et il est **PUR** (la
//!    session d'énumération, 140 lignes). Un `projfs/rappels/enumeration.rs`
//!    créerait deux modules homonymes dont l'un est pur et l'autre
//!    `#[cfg(windows)]` — le genre d'homonymie qu'on ne remarque qu'en
//!    relisant un renvoi, six mois plus tard.
//!
//! D'où **`listage`**. ⚠️ *Le sous-bloc F3 nomme le même fichier dans son plan,
//! sous ce même nom, précisément pour qu'il n'existe jamais un troisième
//! découpage de `rappels.rs`.*
//!
//! # Ce que cette extraction N'EST PAS
//!
//! **Elle n'ajoute aucun comportement.** La transposition est VERBATIM, et
//! elle a été faite en découpant le fichier par NUMÉROS DE LIGNE plutôt qu'en
//! recopiant à la main. Elle vient **AVANT** l'addition de F2 et non après :
//! `rappels.rs` était à **488** lignes, marge **12**, et le rappel de
//! notification est exactement ce que F2 fait grossir. C'est le geste que D9 a
//! inventé (`capteur/serveur/instances.rs`, marge rendue de 10 à 65) et que
//! D10 a joué trois fois — **jamais une compression**, que `CLAUDE.md`
//! interdit nommément.

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACK_DATA, PRJ_DIR_ENTRY_BUFFER_HANDLE, PRJ_END_DIRECTORY_ENUMERATION_CB,
    PRJ_GET_DIRECTORY_ENUMERATION_CB, PRJ_START_DIRECTORY_ENUMERATION_CB,
};

use super::{chemins_de, etat, garde, identifiant};
use crate::pont::erreurs::{Erreur, EN_COURS};
use crate::pont::projfs::{ContexteProjFs, TamponEntrees};
use crate::pont::table::{Attendue, DELAI_LISTER};
use proto::fichiers::entetes;

// ────────────────────────────────────────────────────────────────────────────
// 🔵 LES TROIS `const _` PARTENT AVEC LEURS FONCTIONS, et ce n'est pas un
// rangement : c'est le SEUL garde d'ABI de ce dépôt (voir l'en-tête de
// [`super`]). Le laisser derrière en ferait une déclaration qui pointe
// ailleurs. D9 a posé la règle en extrayant `capteur/serveur/instances.rs` —
// *le commentaire part avec sa constante*, et la revue a comparé mot pour mot.
// ────────────────────────────────────────────────────────────────────────────
const _: PRJ_START_DIRECTORY_ENUMERATION_CB = Some(debut_enumeration);
const _: PRJ_END_DIRECTORY_ENUMERATION_CB = Some(fin_enumeration);
const _: PRJ_GET_DIRECTORY_ENUMERATION_CB = Some(suite_enumeration);

/// Ouvre une session d'énumération. **Synchrone, `S_OK`** — il n'y a rien à
/// demander au navigateur pour ouvrir une session (spec §4.3).
pub(super) unsafe extern "system" fn debut_enumeration(
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
        etat.sessions.lock().expect("verrou des sessions").entry(id).or_default();
        S_OK
    })
}

/// Ferme une session d'énumération. **Synchrone, `S_OK`.**
pub(super) unsafe extern "system" fn fin_enumeration(
    donnees: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
) -> HRESULT {
    garde("EndDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(donnees) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        // La session meurt ici : ses entrées ne survivent PAS à l'énumération.
        // C'est ce qui distingue une session d'un cache d'énumération, qui n'est
        // PAS livré en F1 (voir `pont::enumeration`).
        etat.sessions.lock().expect("verrou des sessions").remove(&id);
        S_OK
    })
}

/// Rend les entrées d'un répertoire.
///
/// ❌ *Annonçait « `S_OK`, tampon vide, racine vide » : l'état de la tâche 13.
/// La tâche 14 de la MÊME branche l'a réfuté — `Lister` part vraiment.*
pub(super) unsafe extern "system" fn suite_enumeration(
    donnees: *const PRJ_CALLBACK_DATA,
    enumeration: *const GUID,
    expression: windows::core::PCWSTR,
    tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    garde("GetDirectoryEnumeration", || {
        let (Some(etat), Some(id)) = (unsafe { etat(donnees) }, unsafe { identifiant(enumeration) })
        else {
            return E_UNEXPECTED;
        };
        let Some((chemin, _)) = (unsafe { chemins_de(donnees) }) else {
            return HRESULT(etat.compteurs.rendre(Erreur::CheminIntrouvable));
        };
        let motif = if expression.is_null() {
            None
        } else {
            // SÛRETÉ : ProjFS garantit un `PCWSTR` terminé par un nul.
            unsafe { expression.to_string() }.ok()
        };
        // ⚠️ `PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN` (`mod.rs:177`, valeur `1i32`)
        // **doit être honoré** : il redémarre l'énumération en cours. L'ignorer
        // ferait rendre un répertoire vide à toute application qui redemande
        // depuis le début, silencieusement.
        let redemarrer = unsafe { (*donnees).Flags }.0 & 1 != 0;

        let mut sessions = match etat.sessions.lock() {
            Ok(sessions) => sessions,
            Err(_) => return E_UNEXPECTED,
        };
        let session = sessions.entry(id).or_default();
        if redemarrer {
            session.redemarrer();
        }
        if session.chargee() {
            // ⚠️ **Chemin SYNCHRONE, et il est le cas nominal.** ProjFS rappelle
            // `GetDirectoryEnumeration` jusqu'à épuisement ; seul le PREMIER
            // appel d'une session consulte le navigateur. Repasser par la table
            // à chaque tour ferait un aller-retour réseau par tampon plein,
            // pour une liste qu'on a déjà.
            return crate::pont::service::remplir_session(etat, session, tampon);
        }
        drop(sessions);

        let entete = match serde_json::to_string(&entetes::Chemin { chemin: chemin.clone() }) {
            Ok(entete) => entete,
            Err(_) => return E_UNEXPECTED,
        };
        let demandee = etat.demander(
            unsafe { (*donnees).CommandId },
            Attendue::Lister { chemin, enumeration: id },
            std::time::Instant::now() + DELAI_LISTER,
            ContexteProjFs::Enumeration {
                tampon: TamponEntrees(tampon),
                expression: motif,
            },
            proto::fichiers::TYPE_LISTER,
            &entete,
        );
        if demandee {
            HRESULT(EN_COURS)
        } else {
            HRESULT(etat.compteurs.rendre(Erreur::CanalFerme))
        }
    })
}
