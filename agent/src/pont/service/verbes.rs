//! Les appels ProjFS que le fil du pont émet : compléter, écrire un marqueur,
//! écrire des données, remplir un tampon d'entrées.
//!
//! Extrait de [`super`] pour la même raison que `projfs/racine.rs` l'a été de
//! `projfs.rs` : **avant** que l'addition ne rende l'extraction nécessaire, et
//! non après. Ce fichier porte les `unsafe`, [`super`] porte la boucle.

use windows::core::{GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS, PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0,
    PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0_1, PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION,
    PRJ_DIR_ENTRY_BUFFER_HANDLE, PRJ_FILE_BASIC_INFO, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    PRJ_PLACEHOLDER_INFO,
};

use crate::pont::entetes::filetime_depuis_ms;
use crate::pont::enumeration::{Entree, Session};
use crate::pont::projfs::{chargement::ProjFs, Contexte, Etat};

/// `FILE_ATTRIBUTE_DIRECTORY` / `FILE_ATTRIBUTE_NORMAL`.
///
/// ⚠️ **`NORMAL` (ou `DIRECTORY`) pour TOUT, et c'est déclaré** : la File
/// System Access API n'expose aucun attribut, il n'y a **rien à transporter**
/// (spec §3.5.2). Ce n'est pas une approximation faute de mieux, c'est
/// l'absence de source.
const ATTRIBUT_REPERTOIRE: u32 = 0x0000_0010; // Storage/FileSystem/mod.rs, FILE_ATTRIBUTE_DIRECTORY
const ATTRIBUT_NORMAL: u32 = 0x0000_0080; // Storage/FileSystem/mod.rs, FILE_ATTRIBUTE_NORMAL

/// `HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER)` — ce que
/// `PrjFillDirEntryBuffer` rend quand le tampon est plein. **Ce n'est pas une
/// erreur** : c'est le signal de s'arrêter là et de compléter, la suite partant
/// au prochain `GetDirectoryEnumeration`.
const TAMPON_PLEIN: HRESULT = HRESULT(0x8007_007Au32 as i32); // Foundation/mod.rs:2813, valeur 122

/// Un tampon aligné, rendu par son `Drop`.
///
/// ⚠️ **`PrjAllocateAlignedBuffer` / `PrjFreeAlignedBuffer` sont la première
/// source de fuite mémoire d'un fournisseur ProjFS** (spec §4.3). Le couple est
/// donc encapsulé ici, **jamais appelé à la main** — et il est rendu par `Drop`,
/// donc y compris quand `PrjWriteFileData` échoue, ce qu'un `free` écrit après
/// l'appel ne ferait pas.
struct TamponAligne<'a> {
    pointeur: *mut core::ffi::c_void,
    projfs: &'a ProjFs,
}

impl<'a> TamponAligne<'a> {
    /// `None` si l'allocation échoue — `PrjAllocateAlignedBuffer` rend un
    /// POINTEUR, et `NULL` est l'échec (piège n°3 des transcriptions).
    fn allouer(
        projfs: &'a ProjFs,
        contexte: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
        taille: usize,
    ) -> Option<Self> {
        // SÛRETÉ : contexte valide, taille non nulle. Transcription du `link!`
        // de `mod.rs:3`.
        let pointeur = unsafe { (projfs.allouer_tampon_aligne)(contexte, taille) };
        if pointeur.is_null() {
            return None;
        }
        Some(Self { pointeur, projfs })
    }
}

impl Drop for TamponAligne<'_> {
    fn drop(&mut self) {
        // SÛRETÉ : `pointeur` vient de `PrjAllocateAlignedBuffer` et n'a été
        // rendu à personne. `PrjFreeAlignedBuffer` ne rend RIEN (`mod.rs:68`).
        unsafe { (self.projfs.rendre_tampon_aligne)(self.pointeur) };
    }
}

/// Complète une commande ProjFS, sans paramètres étendus.
///
/// 🔴 **UN `command_id` ABSENT N'EST PAS UNE ERREUR : c'est une ÉCRITURE.**
/// Elle naît d'une notification POST, qui a déjà rendu la main à
/// l'application — il n'y a donc **aucun rappel à compléter**. Appeler
/// `PrjCompleteCommand(0)` compléterait une commande qui appartient à
/// quelqu'un d'autre.
///
/// ⚠️ **Le cas est journalisé à `debug!`, jamais tu.** Le silence ferait qu'un
/// `None` inattendu — venu d'une lecture dont on aurait perdu l'identifiant —
/// serait indiscernable du cas nominal.
pub(super) fn completer(etat: &Etat, commande: Option<i32>, resultat: HRESULT) {
    let Some(commande) = commande else {
        tracing::debug!(%resultat, "aucune commande ProjFS a completer : c'est une ecriture");
        return;
    };
    let Some(Contexte(contexte)) = etat.contexte() else {
        tracing::warn!(commande, "complétion impossible : aucun contexte de virtualisation");
        return;
    };
    // SÛRETÉ : contexte valide tant que la virtualisation tourne — le fil du
    // pont s'arrête avant le `Drop` de `Virtualisation`. Le quatrième
    // paramètre nul vaut « aucun paramètre étendu » (`mod.rs:14`).
    let issue =
        unsafe { (etat.projfs.completer_commande)(contexte, commande, resultat, std::ptr::null()) };
    if issue.is_err() {
        tracing::warn!(commande, %issue, %resultat, "PrjCompleteCommand refusée");
    }
}

/// Complète une **énumération**, qui exige des paramètres étendus.
///
/// ⚠️ Une énumération complétée sans `PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION` et
/// sans son `DirEntryBufferHandle` rendrait un répertoire vide (spec §4.3) :
/// ProjFS ne saurait pas quel tampon relire.
pub(super) fn completer_enumeration(
    etat: &Etat,
    commande: i32,
    tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE,
    resultat: HRESULT,
) {
    let Some(Contexte(contexte)) = etat.contexte() else {
        tracing::warn!(commande, "complétion d'énumération impossible : aucun contexte");
        return;
    };
    let parametres = PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS {
        CommandType: PRJ_COMPLETE_COMMAND_TYPE_ENUMERATION,
        Anonymous: PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0 {
            Enumeration: PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS_0_1 {
                DirEntryBufferHandle: tampon,
            },
        },
    };
    // SÛRETÉ : `parametres` vit jusqu'à la fin de l'expression, donc au-delà
    // de l'appel.
    let issue = unsafe {
        (etat.projfs.completer_commande)(contexte, commande, resultat, &parametres)
    };
    if issue.is_err() {
        tracing::warn!(commande, %issue, "PrjCompleteCommand (énumération) refusée");
    }
}

/// Le bloc d'informations de base d'une entrée.
fn info_de_base(repertoire: bool, taille: u64, modifie_ms: i64) -> PRJ_FILE_BASIC_INFO {
    let horodatage = filetime_depuis_ms(modifie_ms);
    PRJ_FILE_BASIC_INFO {
        IsDirectory: repertoire,
        // `i64` côté ProjFS, `u64` côté protocole : une taille supérieure à
        // 8 Eio n'existe pas, mais la saturer vaut mieux qu'un négatif, que
        // ProjFS lirait comme une taille absurde.
        FileSize: taille.min(i64::MAX as u64) as i64,
        // ⚠️ **Les quatre champs portent le MÊME horodatage, et c'est
        // déclaré** : la File System Access API n'expose que
        // `File.lastModified` (spec §3.5.2). Inventer une date de création
        // distincte serait une donnée fabriquée.
        CreationTime: horodatage,
        LastAccessTime: horodatage,
        LastWriteTime: horodatage,
        ChangeTime: horodatage,
        FileAttributes: if repertoire { ATTRIBUT_REPERTOIRE } else { ATTRIBUT_NORMAL },
    }
}

/// Écrit le marqueur d'une entrée — la réponse à `GetPlaceholderInfo`.
pub(super) fn ecrire_marqueur(
    etat: &Etat,
    chemin: &[u16],
    repertoire: bool,
    taille: u64,
    modifie_ms: i64,
) -> HRESULT {
    let Some(Contexte(contexte)) = etat.contexte() else {
        return HRESULT(etat.compteurs.rendre(crate::pont::erreurs::Erreur::Inattendue));
    };
    let info = PRJ_PLACEHOLDER_INFO {
        FileBasicInfo: info_de_base(repertoire, taille, modifie_ms),
        ..Default::default()
    };
    // SÛRETÉ : `chemin` est terminé par un nul (posé par le rappel depuis le
    // `FilePathName` de ProjFS), `info` vit jusqu'à la fin de la fonction.
    // `PRJ_PLACEHOLDER_INFO` se termine par un `VariableData: [u8; 1]` de
    // taille flexible : la taille annoncée est celle de la structure, puisque
    // nous n'écrivons aucune donnée variable.
    unsafe {
        (etat.projfs.ecrire_info_marqueur)(
            contexte,
            PCWSTR(chemin.as_ptr()),
            &info,
            std::mem::size_of::<PRJ_PLACEHOLDER_INFO>() as u32,
        )
    }
}

/// Écrit un morceau de fichier — la réponse à `GetFileData`.
///
/// ⚠️ **Le fichier entier n'entre JAMAIS en mémoire.** C'est l'inverse exact de
/// l'ancien pont, dont chaque lecture faisait `getFile()` + `arrayBuffer()` +
/// `.slice(...)` (`web/index.js:562-564`) : une lecture séquentielle d'un
/// fichier de 100 Mio par blocs de 128 Kio y relisait 100 Mio depuis le disque,
/// **huit cents fois**.
///
/// ⚠️ **Hypothèse d'alignement, déclarée et NON vérifiée** :
/// `PrjGetVirtualizationInstanceInfo` rend un `WriteAlignment` que ce pont ne
/// lit pas — l'entrée n'est pas chargée (les treize de la tâche 12 ne
/// l'incluent pas). Les morceaux font `TAILLE_TRAME_MAX` (64 Kio), multiple de
/// toute taille de secteur plausible, et leur position dérive de celle que
/// ProjFS a demandée. **Cela n'est pas une preuve** : si un
/// `PrjWriteFileData` était refusé pour alignement, c'est ici qu'il faudrait
/// charger `PrjGetVirtualizationInstanceInfo` et arrondir. Legs déclaré.
pub(super) fn ecrire_donnees(etat: &Etat, flux: GUID, position: u64, charge: &[u8]) -> HRESULT {
    let Some(Contexte(contexte)) = etat.contexte() else {
        return HRESULT(etat.compteurs.rendre(crate::pont::erreurs::Erreur::Inattendue));
    };
    let Some(tampon) = TamponAligne::allouer(&etat.projfs, contexte, charge.len()) else {
        tracing::warn!(octets = charge.len(), "PrjAllocateAlignedBuffer a rendu NULL");
        return HRESULT(etat.compteurs.rendre(crate::pont::erreurs::Erreur::Inattendue));
    };
    // SÛRETÉ : `tampon.pointeur` est non nul et fait au moins `charge.len()`
    // octets ; les deux régions ne se recouvrent pas.
    unsafe {
        std::ptr::copy_nonoverlapping(charge.as_ptr(), tampon.pointeur as *mut u8, charge.len())
    };
    // SÛRETÉ : transcription du `link!` de `mod.rs:122`. Le tampon est rendu
    // par le `Drop` de `TamponAligne`, y compris si cet appel échoue.
    unsafe {
        (etat.projfs.ecrire_donnees)(
            contexte,
            &flux,
            tampon.pointeur,
            position,
            charge.len() as u32,
        )
    }
}

/// Remplit le tampon d'entrées depuis la session, jusqu'à ce qu'il soit plein
/// ou la session épuisée.
///
/// Rend le `HRESULT` de complétion : `S_OK` dans les deux cas. **Un tampon
/// plein n'est pas une erreur** — la suite part au prochain
/// `GetDirectoryEnumeration`, sur la même session.
pub(super) fn remplir(etat: &Etat, session: &mut Session, tampon: PRJ_DIR_ENTRY_BUFFER_HANDLE) -> HRESULT {
    while let Some(entree) = session.prochaine() {
        let nom: Vec<u16> = entree.nom.encode_utf16().chain(std::iter::once(0)).collect();
        let info = info_de_base(entree.repertoire, entree.taille, entree.modifie_ms);
        // SÛRETÉ : `nom` est terminé par un nul et vit jusqu'à la fin du tour ;
        // `info` de même. Transcription du `link!` de `mod.rs:55`.
        let issue = unsafe {
            (etat.projfs.remplir_tampon_entrees)(PCWSTR(nom.as_ptr()), &info, tampon)
        };
        if issue == TAMPON_PLEIN {
            // ⚠️ **Ne PAS avancer** : l'entrée n'a pas été écrite, et avancer
            // la perdrait pour toujours — silencieusement, puisque
            // l'énumération se terminerait normalement avec un fichier de
            // moins.
            return S_OK;
        }
        if issue.is_err() {
            tracing::warn!(nom = %entree.nom, %issue, "PrjFillDirEntryBuffer refusée");
            return issue;
        }
        session.avancer();
    }
    S_OK
}

/// Convertit les entrées du protocole en entrées d'énumération.
pub(super) fn entrees_depuis(json: Vec<proto::fichiers::entetes::EntreeJson>) -> Vec<Entree> {
    json.into_iter()
        .map(|e| Entree {
            nom: e.nom,
            repertoire: e.repertoire,
            taille: e.taille,
            modifie_ms: e.modifie,
        })
        .collect()
}
