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
    EnumResourceNamesW, FindResourceW, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
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

/// Le rappel d'énumération : il RETIENT le premier nom et s'arrête.
///
/// SÉCURITÉ : `param` est le `*mut Option<PCWSTR>` que `lire_groupe` passe à
/// `EnumResourceNamesW`, et il vit pour toute la durée de l'appel.
unsafe extern "system" fn premier_nom(
    _module: HMODULE,
    _type_: PCWSTR,
    nom: PCWSTR,
    param: isize,
) -> windows::core::BOOL {
    let sortie = param as *mut Option<PCWSTR>;
    if !sortie.is_null() {
        unsafe { *sortie = Some(nom) };
    }
    // `FALSE` ARRÊTE l'énumération : on ne veut que le premier.
    windows::core::BOOL(0)
}

fn lire_groupe(handle: HMODULE, index: i32) -> Result<Vec<u8>> {
    // 🔴 UN NOM NUL N'EST PAS « LA PREMIÈRE RESSOURCE », ET C'EST LE DÉFAUT
    // QUE LA MESURE DU 21 AOÛT 2026 A TROUVÉ. Une première rédaction passait
    // `PCWSTR(null())` à `FindResourceW` pour un index de 0, en croyant
    // demander le premier groupe : `FindResourceW` cherche alors une ressource
    // dont le NOM est nul, et n'en trouve aucune. **Mesuré : 148 des 154
    // applications rendaient `aucune ressource RT_GROUP_ICON`, y compris des
    // modules qui en portent manifestement — `steam.exe`.** Le catalogue
    // restait juste et les icônes étaient servies ; seule la PROVENANCE
    // tombait à `NonMesuree`, c'est-à-dire très exactement ce que le sous-bloc
    // existe pour mesurer.
    //
    // Le premier groupe s'obtient donc par ÉNUMÉRATION, comme la sonde M1 du
    // plan le faisait et comme la tâche 9 le prescrivait — `EnumResourceNamesW`
    // figurait dans sa liste d'appels, et son omission est ce qui a produit le
    // défaut.
    let mut premier: Option<PCWSTR> = None;
    if index <= 0 {
        // SÉCURITÉ : appel FFI. `handle` est vivant, et le pointeur passé en
        // `param` vise une variable de cette pile, qui survit à l'appel.
        // `EnumResourceNamesW` rend `Err` quand le rappel arrête l'énumération
        // — ce que le nôtre fait toujours —, donc son résultat est ignoré au
        // profit de ce que le rappel a RETENU.
        let _ = unsafe {
            EnumResourceNamesW(
                Some(handle),
                RT_GROUP_ICON,
                Some(premier_nom),
                &mut premier as *mut _ as isize,
            )
        };
    }
    // ⚠️ Un index NÉGATIF désigne une ressource par son identifiant ; un index
    // positif est un RANG. Ce module ne sait suivre que le second cas, et il
    // retombe sur le premier groupe énuméré pour l'autre — voir la réserve en
    // tête de [`grpicondir`].
    let nom = if index > 0 {
        PCWSTR(index as usize as *const u16)
    } else {
        match premier {
            Some(n) => n,
            None => bail!("aucun groupe d'icones enumere dans ce module"),
        }
    };
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
