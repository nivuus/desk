//! Obtenir les OCTETS BRUTS d'un répertoire d'icônes — et rien d'autre.
//!
//! 🔴 CE MODULE NE DÉCIDE RIEN. Il rend un `Vec<u8>` que
//! `apps::icone::ressource`, qui est PUR, analyse. C'est la coupure que la
//! spécification §6 désigne comme « le point le plus important de cette
//! liste », et sans elle le critère ② de recette n'aurait aucun test d'hôte :
//! sa seule preuve serait un argument de flot de contrôle — la situation
//! exacte que le défaut F1 du sous-bloc D7 a payée.
//!
//! ⚠️ IL N'EST VÉRIFIÉ QUE PAR `cargo check --target x86_64-pc-windows-gnu`,
//! qui couvre types, emprunts, visibilités et durées de vie — et PAS l'édition
//! de liens, la cible réelle étant `msvc`. Aucun test d'hôte ne peut le
//! couvrir, et c'est dit plutôt que dissimulé.

use std::path::Path;

use anyhow::{bail, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    FindResourceW, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
    LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
};

use super::super::lecture::vers_utf16;

/// `RT_GROUP_ICON` — la ressource qui porte le `GRPICONDIR`.
///
/// ⚠️ C'EST UN ENTIER DÉGUISÉ EN POINTEUR, et c'est le contrat de l'API : les
/// types de ressource prédéfinis se passent comme des `PCWSTR` dont la valeur
/// numérique est l'identifiant. `RT_ICON` vaut 3, `RT_GROUP_ICON` vaut
/// `3 + 11 = 14`.
const RT_GROUP_ICON: PCWSTR = PCWSTR(14 as *const u16);

/// Les octets du `GRPICONDIR` du PREMIER groupe d'icônes d'un module PE.
///
/// ⚠️ **LE PREMIER GROUPE, ET C'EST UNE APPROXIMATION DÉCLARÉE.** Le Shell,
/// lui, choisit le groupe que l'`IconLocation` désigne par son index, et un
/// index négatif désigne une ressource par son IDENTIFIANT. Ce module prend
/// l'index quand il est positif et retombe sur le premier groupe sinon. **La
/// conséquence est bornée et nommée** : `source_max` peut alors décrire un
/// groupe voisin de celui que l'image montre — jamais une taille inventée, et
/// jamais un `256` de complaisance.
///
/// 🔴 `LOAD_LIBRARY_AS_IMAGE_RESOURCE` S'AJOUTE À `AS_DATAFILE` ET N'EST PAS
/// FACULTATIF : sans lui, `FindResourceW` sur un module chargé en pur fichier
/// de données ne trouve pas toujours ses ressources. La sonde M1 du plan les a
/// employés tous les deux, et elle a lu 110 sources sur 110 tentées.
///
/// ⚠️ **AUCUN CODE DU MODULE CHARGÉ N'EST EXÉCUTÉ** : c'est tout l'objet de
/// `AS_DATAFILE`, et c'est ce qui rend acceptable d'ouvrir des `.exe`
/// arbitraires du disque de la VM.
pub fn grpicondir(module: &Path, index: i32) -> Result<Vec<u8>> {
    let large = vers_utf16(&module.to_string_lossy());
    // SÉCURITÉ : appel FFI. Le chemin est un tampon UTF-16 terminé par un nul
    // que nous possédons, et les deux drapeaux interdisent toute exécution de
    // code du module.
    let handle = unsafe {
        LoadLibraryExW(
            PCWSTR(large.as_ptr()),
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .with_context(|| format!("chargement du module {} en fichier de donnees", module.display()))?;

    let resultat = lire_groupe(handle, index);

    // 🔴 `FreeLibrary` SUR TOUS LES CHEMINS DE SORTIE, y compris d'erreur. Une
    // fuite ici serait invisible pendant des heures, sur un processus qui
    // réconcilie toutes les trente secondes et ouvre jusqu'à 110 modules par
    // tour — c'est-à-dire 13 200 handles par heure.
    // SÉCURITÉ : appel FFI. Le handle vient de `LoadLibraryExW` juste
    // au-dessus et n'a pas été relâché ailleurs.
    let _ = unsafe { FreeLibrary(handle) };
    resultat
}

fn lire_groupe(handle: HMODULE, index: i32) -> Result<Vec<u8>> {
    // ⚠️ Un index NÉGATIF désigne une ressource par son identifiant ; un index
    // positif est un RANG. Ce module ne sait suivre que le second cas, et il
    // retombe sur le premier groupe pour l'autre — voir la réserve en tête de
    // [`grpicondir`].
    let nom = PCWSTR(if index > 0 { index as usize as *const u16 } else { std::ptr::null() });
    // SÉCURITÉ : appel FFI. `handle` est vivant (son `FreeLibrary` est dans
    // l'appelant), et `nom` est soit un identifiant entier déguisé, soit nul.
    let bloc = unsafe { FindResourceW(Some(handle), nom, RT_GROUP_ICON) };
    if bloc.is_invalid() {
        bail!("aucune ressource RT_GROUP_ICON dans ce module");
    }
    // SÉCURITÉ : appel FFI. `bloc` vient d'être validé.
    let taille = unsafe { SizeofResource(Some(handle), bloc) } as usize;
    if taille == 0 {
        bail!("ressource RT_GROUP_ICON de taille nulle");
    }
    // SÉCURITÉ : appel FFI.
    let charge = unsafe { LoadResource(Some(handle), bloc) }.context("LoadResource")?;
    // SÉCURITÉ : appel FFI. `LockResource` rend un pointeur sur la ressource
    // mappée, valide tant que le module est chargé — donc jusqu'au
    // `FreeLibrary` de l'appelant, qui court APRÈS cette fonction.
    let debut = unsafe { LockResource(charge) } as *const u8;
    if debut.is_null() {
        bail!("LockResource a rendu un pointeur nul");
    }
    // SÉCURITÉ : la ressource est mappée sur `taille` octets, que
    // `SizeofResource` vient de rendre, et la copie est faite AVANT le
    // `FreeLibrary` de l'appelant. Le `Vec` qui en sort ne dépend plus du
    // module.
    Ok(unsafe { std::slice::from_raw_parts(debut, taille) }.to_vec())
}
