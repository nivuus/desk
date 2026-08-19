//! Résolution à l'exécution des **treize** entrées de `ProjectedFSLib.dll`, et
//! les treize signatures transcrites à la main.
//!
//! # Pourquoi ce module existe — décision D1 de la spec
//!
//! Les enveloppes `Prj*` du crate `windows` passent par
//! `windows_core::link!`, qui se développe en
//! `#[link(name = …, kind = "raw-dylib", modifiers = "+verbatim")]`
//! (`windows-link-0.2.1/src/lib.rs:22`, la branche `not(target_arch = "x86")`,
//! celle de notre cible x86_64 ; la branche x86 est aux lignes 5-15 et porte en
//! plus `import_name_type = "undecorated"`). En appeler une seule poserait un
//! import **statique** de `ProjectedFSLib.dll` dans le PE.
//!
//! Or `agent.exe` est **un seul binaire pour tous les modes**. Un import non
//! résolu ne tuerait pas « le pont » : il tuerait la capture, la vidéo et
//! l'entrée sur toute VM dépourvue de ProjFS — l'état exact de cette VM avant
//! le sous-bloc F0. D'où `LoadLibraryW` + `GetProcAddress`.
//!
//! **Ce module prend donc les TYPES du crate `windows` et jamais ses
//! ENVELOPPES.** Les types (`PRJ_CALLBACKS`, `PRJ_CALLBACK_DATA`, les huit
//! `PRJ_*_CB`, les constantes) sont des `struct`/`type`/`const` : ils
//! n'émettent aucun symbole importé.
//!
//! # 🔴 Le risque que ce module porte, et que rien ne referme — R7 de la spec
//!
//! **Les treize signatures ci-dessous sont transcrites à la main, et le
//! compilateur ne peut plus rien en dire.** Un paramètre oublié, un type de
//! retour inventé, un `*const` pris pour un `*mut` : aucun de ces défauts
//! n'est détecté avant l'appel, et l'appel corrompt la pile sans diagnostic.
//!
//! Ce qui EXISTE comme garde, et il faut le dire exactement :
//!
//! 1. **Chaque transcription porte, en commentaire, la ligne `link!` dont elle
//!    est la copie, VERBATIM**, avec son numéro de ligne dans
//!    `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`
//!    (relevé par la commande le 19 août 2026, fichier de 621 lignes). La
//!    relecture est donc une comparaison de texte à texte, jamais une
//!    reconstruction de mémoire.
//! 2. **`pont::resolution` vérifie que les treize NOMS existent**, et nomme
//!    celle qui manque — treize chemins balayés à chaque `cargo test`. Il
//!    épingle aussi l'appariement CHAMP ↔ ENTRÉE, et la construction ci-dessous
//!    passe par une macro qui n'écrit chaque nom de champ qu'une fois : deux
//!    entrées interverties sont donc structurellement impossibles ici.
//! 3. Les huit **rappels** que nous fournissons, eux, sont vérifiés par le
//!    compilateur : `pont::projfs` les affecte à leur type `PRJ_*_CB` dans un
//!    `const _`, et une divergence d'ABI ne compile pas.
//!
//! ⚠️ **Ce que ces trois gardes NE couvrent PAS, et c'est le cœur du risque :
//! aucun d'eux ne porte sur l'ABI des treize entrées IMPORTÉES.** Le point 3
//! concerne les huit fonctions que nous ÉCRIVONS, un ensemble **disjoint** des
//! treize que nous APPELONS — il n'existe dans `windows-rs` aucun type auquel
//! comparer `PrjWriteFileData`, ni aucune des douze autres. **Les treize
//! reposent entièrement sur le point 1.** Le dire plutôt que de laisser croire
//! que « huit sur treize sont couvertes ».
//!
//! ⚠️ **Ce n'est pas une crainte, c'est un fait MESURÉ** (20 août 2026, journal
//! `journaux-pont-fichiers/f1-tache12-mutations.txt`) : trois mutations d'ABI
//! jouées sur les déclarations ci-dessous — retirer le cinquième paramètre de
//! `PrjStartVirtualizing`, donner un `HRESULT` de retour à
//! `PrjStopVirtualizing`, changer le retour de `PrjAllocateAlignedBuffer` en
//! `HRESULT` — **ont TOUTES LES TROIS survécu** à `cargo check --target
//! x86_64-pc-windows-gnu` et à la suite d'hôte entière. **Rien, dans ce dépôt,
//! ne peut attraper une transcription fautive.** La relecture texte à texte du
//! point 1 est le seul garde, et l'exécution sur la VM le seul juge.
//!
//! # Le contrôle qui prouve l'absence d'import statique, et sa portée
//!
//! ```text
//! cd agent && cargo build --release --target x86_64-pc-windows-gnu
//! x86_64-w64-mingw32-objdump -x target/x86_64-pc-windows-gnu/release/agent.exe \
//!   | grep -i 'projectedfslib'
//! ```
//!
//! ⚠️ **Il porte sur le binaire `-gnu` de l'hôte, pas sur le binaire `msvc`
//! livré sur la VM.** Il ne prouve donc PAS l'absence d'import dans le
//! livrable. Le contrôle qui porte sur le vrai binaire est en recette (tâche 18)
//! et il est d'une autre nature : renommer `ProjectedFSLib.dll`, et vérifier
//! que le superviseur, le capteur et un enfant démarrent quand même.
//! **L'un ne vaut pas l'autre.**

use anyhow::{Context, Result};
use windows::core::{GUID, PCSTR, PCWSTR};
use windows::Win32::Storage::ProjectedFileSystem::{
    PRJ_CALLBACKS, PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS, PRJ_DIR_ENTRY_BUFFER_HANDLE,
    PRJ_FILE_BASIC_INFO, PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, PRJ_PLACEHOLDER_INFO,
    PRJ_PLACEHOLDER_VERSION_INFO, PRJ_STARTVIRTUALIZING_OPTIONS, PRJ_UPDATE_FAILURE_CAUSES,
    PRJ_UPDATE_TYPES,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::pont::resolution;

// ────────────────────────────────────────────────────────────────────────────
// Les treize signatures. Chacune porte la ligne `link!` dont elle est la copie.
//
// ⚠️ TROIS pièges relevés dans le module, et non supposés :
//
//   1. `PrjStartVirtualizing` a CINQ paramètres dans le `link!` et QUATRE dans
//      l'enveloppe : celle-ci cache le paramètre de SORTIE
//      `namespacevirtualizationcontext` et le rend en `Result`. Transcrire
//      l'enveloppe produirait un appel dont le dernier argument manque.
//   2. `PrjStopVirtualizing` et `PrjFreeAlignedBuffer` NE RENDENT RIEN. Leur
//      donner un `HRESULT` de retour est un défaut d'ABI.
//   3. `PrjAllocateAlignedBuffer` rend un POINTEUR, pas un `HRESULT` : `NULL`
//      est l'échec.
//
// ⚠️ Le type de retour est `windows::core::HRESULT` là où le `link!` l'écrit,
// et jamais `i32` : `HRESULT` est `#[repr(transparent)]` sur un `i32`, donc
// l'ABI est identique, mais écrire `i32` ferait perdre à la relecture la
// comparaison littérale que le point 1 ci-dessus lui promet.
// ────────────────────────────────────────────────────────────────────────────

/// `mod.rs:3` — `fn PrjAllocateAlignedBuffer(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, size : usize) -> *mut core::ffi::c_void`
type AllouerTamponAligne = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    size: usize,
) -> *mut core::ffi::c_void;

/// `mod.rs:8` — `fn PrjClearNegativePathCache(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, totalentrynumber : *mut u32) -> windows_core::HRESULT`
type ViderCacheNegatif = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    totalentrynumber: *mut u32,
) -> windows::core::HRESULT;

/// `mod.rs:13` — `fn PrjCompleteCommand(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, commandid : i32, completionresult : windows_core::HRESULT, extendedparameters : *const PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS) -> windows_core::HRESULT`
type CompleterCommande = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    commandid: i32,
    completionresult: windows::core::HRESULT,
    extendedparameters: *const PRJ_COMPLETE_COMMAND_EXTENDED_PARAMETERS,
) -> windows::core::HRESULT;

/// `mod.rs:21` — `fn PrjDeleteFile(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, destinationfilename : windows_core::PCWSTR, updateflags : PRJ_UPDATE_TYPES, failurereason : *mut PRJ_UPDATE_FAILURE_CAUSES) -> windows_core::HRESULT`
type SupprimerFichier = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    destinationfilename: PCWSTR,
    updateflags: PRJ_UPDATE_TYPES,
    failurereason: *mut PRJ_UPDATE_FAILURE_CAUSES,
) -> windows::core::HRESULT;

/// `mod.rs:38` — `fn PrjFileNameCompare(filename1 : windows_core::PCWSTR, filename2 : windows_core::PCWSTR) -> i32`
type ComparerNoms = unsafe extern "system" fn(filename1: PCWSTR, filename2: PCWSTR) -> i32;

/// `mod.rs:47` — `fn PrjFileNameMatch(filenametocheck : windows_core::PCWSTR, pattern : windows_core::PCWSTR) -> bool`
///
/// ⚠️ Le retour est `bool` **parce que le `link!` l'écrit `bool`**, et non par
/// commodité : la fonction Win32 rend un `BOOLEAN` d'un octet, et `bool` en
/// Rust en fait un aussi. Écrire `i32` ou `BOOL` lirait trois octets de plus
/// que la fonction n'en a écrits.
type ApparierNom = unsafe extern "system" fn(filenametocheck: PCWSTR, pattern: PCWSTR) -> bool;

/// `mod.rs:55` — `fn PrjFillDirEntryBuffer(filename : windows_core::PCWSTR, filebasicinfo : *const PRJ_FILE_BASIC_INFO, direntrybufferhandle : PRJ_DIR_ENTRY_BUFFER_HANDLE) -> windows_core::HRESULT`
type RemplirTamponEntrees = unsafe extern "system" fn(
    filename: PCWSTR,
    filebasicinfo: *const PRJ_FILE_BASIC_INFO,
    direntrybufferhandle: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> windows::core::HRESULT;

/// `mod.rs:68` — `fn PrjFreeAlignedBuffer(buffer : *const core::ffi::c_void)`
///
/// ⚠️ **Aucun retour** (piège 2 ci-dessus).
type RendreTamponAligne = unsafe extern "system" fn(buffer: *const core::ffi::c_void);

/// `mod.rs:93` — `fn PrjMarkDirectoryAsPlaceholder(rootpathname : windows_core::PCWSTR, targetpathname : windows_core::PCWSTR, versioninfo : *const PRJ_PLACEHOLDER_VERSION_INFO, virtualizationinstanceid : *const windows_core::GUID) -> windows_core::HRESULT`
type MarquerRacine = unsafe extern "system" fn(
    rootpathname: PCWSTR,
    targetpathname: PCWSTR,
    versioninfo: *const PRJ_PLACEHOLDER_VERSION_INFO,
    virtualizationinstanceid: *const GUID,
) -> windows::core::HRESULT;

/// `mod.rs:101` — `fn PrjStartVirtualizing(virtualizationrootpath : windows_core::PCWSTR, callbacks : *const PRJ_CALLBACKS, instancecontext : *const core::ffi::c_void, options : *const PRJ_STARTVIRTUALIZING_OPTIONS, namespacevirtualizationcontext : *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT) -> windows_core::HRESULT`
///
/// ⚠️ **CINQ paramètres** (piège 1 ci-dessus). Le cinquième est le paramètre de
/// SORTIE que l'enveloppe cache derrière son `Result`.
type DemarrerVirtualisation = unsafe extern "system" fn(
    virtualizationrootpath: PCWSTR,
    callbacks: *const PRJ_CALLBACKS,
    instancecontext: *const core::ffi::c_void,
    options: *const PRJ_STARTVIRTUALIZING_OPTIONS,
    namespacevirtualizationcontext: *mut PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
) -> windows::core::HRESULT;

/// `mod.rs:109` — `fn PrjStopVirtualizing(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT)`
///
/// ⚠️ **Aucun retour** (piège 2 ci-dessus).
type ArreterVirtualisation =
    unsafe extern "system" fn(namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT);

/// `mod.rs:122` — `fn PrjWriteFileData(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, datastreamid : *const windows_core::GUID, buffer : *const core::ffi::c_void, byteoffset : u64, length : u32) -> windows_core::HRESULT`
type EcrireDonnees = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    datastreamid: *const GUID,
    buffer: *const core::ffi::c_void,
    byteoffset: u64,
    length: u32,
) -> windows::core::HRESULT;

/// `mod.rs:130` — `fn PrjWritePlaceholderInfo(namespacevirtualizationcontext : PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT, destinationfilename : windows_core::PCWSTR, placeholderinfo : *const PRJ_PLACEHOLDER_INFO, placeholderinfosize : u32) -> windows_core::HRESULT`
type EcrireInfoMarqueur = unsafe extern "system" fn(
    namespacevirtualizationcontext: PRJ_NAMESPACE_VIRTUALIZATION_CONTEXT,
    destinationfilename: PCWSTR,
    placeholderinfo: *const PRJ_PLACEHOLDER_INFO,
    placeholderinfosize: u32,
) -> windows::core::HRESULT;

/// Convertit une adresse en pointeur de fonction typé.
///
/// # Sûreté
///
/// L'appelant garantit qu'`adresse` est l'adresse réellement exportée pour
/// l'entrée dont `T` est la transcription. **C'est là que le risque R7 est
/// pris** : ni cette fonction ni le compilateur ne peuvent vérifier que `T`
/// décrit la signature de la fonction qui vit à cette adresse.
///
/// L'assertion de taille n'y change rien, et elle n'est pas décorative pour
/// autant : elle attrape un `T` qui ne serait **pas** un pointeur de fonction
/// nu — un `Option<fn>` habitant une niche, une référence grasse —, cas que
/// `transmute_copy` accepterait en silence **en lisant au-delà de la variable
/// source**.
unsafe fn depuis_adresse<T: Copy>(adresse: usize) -> T {
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<usize>(),
        "la cible d'une transcription ProjFS n'est pas un pointeur de fonction nu"
    );
    // SÛRETÉ : les tailles sont égales (assertion ci-dessus), et l'appelant
    // garantit la correspondance de signature.
    unsafe { std::mem::transmute_copy(&adresse) }
}

/// Les treize entrées, résolues une seule fois.
///
/// **Aucune n'est `Option`** : [`charger`] échoue si une seule manque, donc un
/// `ProjFs` qui existe les a toutes. Les rendre optionnelles ferait porter à
/// chaque site d'appel une décision qui appartient au chargement.
pub struct ProjFs {
    pub allouer_tampon_aligne: AllouerTamponAligne,
    /// ⚠️ **Chargée sans appelant, DÉLIBÉRÉMENT.** Elle vide le cache négatif
    /// de ProjFS, ce qu'aucun chemin de F1 ne demande : le seul moyen de le
    /// solliciter est `Rafraichir`, un livrable de **F5**. Elle est résolue
    /// dès maintenant pour que F5 n'ait pas à rouvrir cette couche — et parce
    /// qu'une entrée absente doit être découverte au CHARGEMENT, avec un
    /// message qui la nomme, jamais au premier appel.
    #[allow(dead_code)]
    pub vider_cache_negatif: ViderCacheNegatif,
    pub completer_commande: CompleterCommande,
    /// ⚠️ **Chargée sans appelant, DÉLIBÉRÉMENT**, pour la même raison :
    /// **aucune politique d'éviction en F1**. Chaque fichier lu est hydraté sur
    /// le disque de la VM et il y reste (spec §6.4, cas 4). Poser une politique
    /// sans mesure serait exactement le geste que ce dépôt reproche à ses
    /// constantes non calibrées ; la mesure appartient à F5, et l'entrée est
    /// prête pour elle.
    #[allow(dead_code)]
    pub supprimer_fichier: SupprimerFichier,
    pub comparer_noms: ComparerNoms,
    pub apparier_nom: ApparierNom,
    pub remplir_tampon_entrees: RemplirTamponEntrees,
    pub rendre_tampon_aligne: RendreTamponAligne,
    pub marquer_racine: MarquerRacine,
    pub demarrer_virtualisation: DemarrerVirtualisation,
    pub arreter_virtualisation: ArreterVirtualisation,
    pub ecrire_donnees: EcrireDonnees,
    pub ecrire_info_marqueur: EcrireInfoMarqueur,
}

/// Le nom de la bibliothèque, en UTF-16 terminé par un nul.
///
/// Écrit en toutes lettres plutôt que par la macro `w!` pour que le nom soit
/// `grep`-able tel quel : c'est le motif exact que le contrôle `objdump` et la
/// recette de renommage (tâche 18) cherchent tous les deux.
const BIBLIOTHEQUE: &str = "ProjectedFSLib.dll";

/// Charge `ProjectedFSLib.dll` et résout les treize entrées.
///
/// **Une seule entrée absente fait échouer le chargement, et l'erreur la
/// nomme** — voir [`crate::pont::resolution`], où cette règle vit et est testée
/// sur l'hôte. Ici ne reste que ce qu'aucun test d'hôte ne peut atteindre.
pub fn charger() -> Result<ProjFs> {
    let nom: Vec<u16> = BIBLIOTHEQUE.encode_utf16().chain(std::iter::once(0)).collect();
    // `LoadLibraryW` et non `LoadLibraryA` : le nom est du texte, et rien ne
    // garantit la page de code ANSI de la session Windows.
    let module = unsafe { LoadLibraryW(PCWSTR(nom.as_ptr())) }
        .with_context(|| format!("chargement de {BIBLIOTHEQUE}"))?;

    let adresses = resolution::resoudre(|entree| {
        // `GetProcAddress` prend un nom ANSI terminé par un nul. Les treize
        // noms sont de l'ASCII pur (`resolution::NOMS`), donc la conversion ne
        // peut pas perdre de caractère ; le `ok()?` couvre le seul cas
        // restant, un nul interne, qui ne peut venir que d'une édition
        // fautive de `NOMS`.
        let c = std::ffi::CString::new(entree).ok()?;
        // SÛRETÉ : `module` est un handle valide rendu par `LoadLibraryW`, et
        // `c` vit jusqu'à la fin de cette expression, donc au-delà de l'appel.
        unsafe { GetProcAddress(module, PCSTR(c.as_ptr() as *const u8)) }.map(|f| f as usize)
    })
    .with_context(|| {
        format!(
            "{BIBLIOTHEQUE} est chargée mais n'exporte pas les treize entrées que le pont \
             fichiers appelle : cette VM porte probablement une génération antérieure de ProjFS"
        )
    })?;

    // SÛRETÉ : chaque adresse vient de `GetProcAddress` sur le nom que
    // `resolution::Adresses` associe à ce champ — appariement épinglé par
    // `resolution::chaque_champ_recoit_l_adresse_de_son_entree` — et chaque
    // type ci-dessus est la copie littérale du `link!` cité en regard de sa
    // déclaration. **C'est ici, et nulle part ailleurs, que le risque R7 est
    // pris** : si une transcription diverge, ce `transmute` est valide pour le
    // compilateur et faux pour la machine.
    //
    // ⚠️ **Le nom de champ n'est écrit QU'UNE FOIS par entrée**, par la macro
    // ci-dessous, et il est le MÊME dans `ProjFs` et dans
    // `resolution::Adresses`. C'est ce qui rend l'interversion de deux entrées
    // structurellement impossible ici : la première rédaction indexait un
    // `[usize; 13]` par des constantes de rang, et une mutation qui échangeait
    // deux de ces rangs SURVIVAIT à toute la suite de tests.
    macro_rules! transcrire {
        ($($champ:ident),+ $(,)?) => {
            ProjFs { $( $champ: unsafe { depuis_adresse(adresses.$champ) }, )+ }
        };
    }
    Ok(transcrire!(
            allouer_tampon_aligne,
            vider_cache_negatif,
            completer_commande,
            supprimer_fichier,
            comparer_noms,
            apparier_nom,
            remplir_tampon_entrees,
            rendre_tampon_aligne,
            marquer_racine,
            demarrer_virtualisation,
            arreter_virtualisation,
            ecrire_donnees,
            ecrire_info_marqueur,
    ))
}
