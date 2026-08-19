//! De la cause d'un échec au `HRESULT` que ProjFS attend. **PUR** : aucun
//! `cfg`, et surtout **aucune dépendance au crate `windows`**.
//!
//! ⚠️ **Pourquoi les douze valeurs sont RECOPIÉES et non importées** : importer
//! `windows::Win32::Foundation::ERROR_FILE_NOT_FOUND` gaterait ce module en
//! `#[cfg(windows)]` et lui ferait perdre sa testabilité d'hôte, qui est tout
//! son intérêt. Chaque constante porte donc **le numéro de ligne de sa source**
//! en regard — c'est le contrôle de revue que la spec §3.1 impose aux
//! transcriptions, appliqué ici aussi. Relevé par la commande le 19 août 2026
//! dans `windows-0.62.2/src/Windows/Win32/Foundation/mod.rs`.
//!
//! **Le contre-exemple que ce module existe pour ne pas rejouer** : l'ancien
//! pont rendait `cb(-1)` — `EPERM` — à **neuf** sites distincts
//! (`src/file.js:189,203,245,267,277,288,299,311,323`). Un fichier absent, un
//! disque plein, un délai dépassé et une erreur interne y étaient
//! indistinguables, côté application Windows comme au journal.

/// `HRESULT_FROM_WIN32(x)` pour un code d'erreur Win32 : le bit de sévérité,
/// l'installation `FACILITY_WIN32` (7), puis le code.
const FACILITE_WIN32: u32 = 0x8007_0000;

// Les TREIZE codes Win32, transcrits avec leur ligne source. (Douze causes
// d'échec, plus `ERROR_IO_PENDING`, qui n'en est pas une — voir `EN_COURS`.)
const ERROR_FILE_NOT_FOUND: u32 = 2; // Foundation/mod.rs:2355
const ERROR_PATH_NOT_FOUND: u32 = 3; // Foundation/mod.rs:3689
const ERROR_ACCESS_DENIED: u32 = 5; // Foundation/mod.rs:1143
const ERROR_WRITE_PROTECT: u32 = 19; // Foundation/mod.rs:4657
const ERROR_GEN_FAILURE: u32 = 31; // Foundation/mod.rs:2435
const ERROR_NOT_SUPPORTED: u32 = 50; // Foundation/mod.rs:3498
const ERROR_FILE_EXISTS: u32 = 80; // Foundation/mod.rs:2347
const ERROR_DISK_FULL: u32 = 112; // Foundation/mod.rs:1781
const ERROR_SEM_TIMEOUT: u32 = 121; // Foundation/mod.rs:3943
const ERROR_DIR_NOT_EMPTY: u32 = 145; // Foundation/mod.rs:1776
const ERROR_OPERATION_ABORTED: u32 = 995; // Foundation/mod.rs:3632
const ERROR_IO_DEVICE: u32 = 1117; // Foundation/mod.rs:3003
const ERROR_IO_PENDING: u32 = 997; // Foundation/mod.rs:3005

/// `HRESULT_FROM_WIN32(ERROR_IO_PENDING)` — **la valeur que rend TOUT rappel
/// asynchrone**, et elle n'est pas une [`Erreur`].
///
/// ⚠️ **Elle n'a délibérément PAS de variante d'[`Erreur`]**, et ce n'est pas
/// un oubli : `Erreur` énumère les causes d'un ÉCHEC, et son `NOMBRE` porte un
/// garde structurel qui oblige à classer toute variante neuve. « L'opération
/// est en cours » n'est pas un échec — lui donner une variante ferait qu'un
/// balayage exhaustif des causes d'erreur inclurait un succès différé, et que
/// `hresult` pourrait rendre `EN_COURS` là où un appelant attend un code de
/// refus.
///
/// C'est la valeur qui fait tenir la discipline de fil : un rappel l'inscrit
/// dans la table, la rend, et **rend la main immédiatement**. Y attendre un
/// aller-retour navigateur figerait l'application qui lit le fichier.
pub const EN_COURS: i32 = (FACILITE_WIN32 | ERROR_IO_PENDING) as i32;

/// Ce qui a empêché une opération d'aboutir.
///
/// **Une variante par CAUSE**, jamais par commodité de code : c'est ce qui
/// permet à l'application Windows de distinguer « ce fichier n'existe pas » de
/// « le navigateur a fermé l'onglet », et à l'exploitant de le lire au journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Erreur {
    /// Le fichier demandé n'existe pas dans le répertoire partagé.
    Introuvable,
    /// Un composant intermédiaire du chemin n'existe pas.
    CheminIntrouvable,
    /// La File System Access API a refusé l'accès (permission révoquée).
    AccesRefuse,
    /// Le canal de données est fermé : onglet fermé, page rechargée, WebRTC
    /// tombé. C'est « l'erreur d'E/S standard » du cadrage §7.
    CanalFerme,
    /// Le navigateur n'a pas répondu dans le budget imparti.
    DelaiDepasse,
    /// La commande a été annulée, par ProjFS ou par l'arrêt du pont.
    Abandonnee,
    /// Le disque du poste local est plein (F2 et au-delà).
    DisquePlein,
    /// L'opération n'a pas d'équivalent dans la File System Access API.
    NonSupporte,
    /// Suppression d'un répertoire non vide (F3).
    RepertoireNonVide,
    /// Création d'une entrée qui existe déjà (F2).
    DejaPresent,
    /// **F1 vit tout entier dans cet état** : le lecteur est en lecture seule.
    ProtegeEnEcriture,
    /// Tout le reste. Une seule variante fourre-tout, et elle est nommée comme
    /// telle — c'est ce qui empêche qu'elle avale les onze autres.
    Inattendue,
}

/// Le nombre de variantes d'[`Erreur`].
///
/// ⚠️ **Ce n'est pas une commodité : c'est le second étage du garde
/// structurel.** Ajouter une variante casse d'abord la compilation d'[`index`]
/// et de [`hresult`] (deux `match` exhaustifs), ce qui oblige à lui donner un
/// indice ; l'indice suivant impose de porter `NOMBRE` à 13, ce qui fait
/// échouer la compilation de [`Erreur::TOUTES`], typé `[Erreur; NOMBRE]`, tant
/// que la variante n'y est pas inscrite. **Le balayage ne peut donc pas devenir
/// décoratif en silence** — même intention que le critère (4) de F3.
pub const NOMBRE: usize = 12;

impl Erreur {
    /// Toutes les variantes. Le balayage exhaustif des tests s'appuie dessus.
    pub const TOUTES: [Erreur; NOMBRE] = [
        Erreur::Introuvable,
        Erreur::CheminIntrouvable,
        Erreur::AccesRefuse,
        Erreur::CanalFerme,
        Erreur::DelaiDepasse,
        Erreur::Abandonnee,
        Erreur::DisquePlein,
        Erreur::NonSupporte,
        Erreur::RepertoireNonVide,
        Erreur::DejaPresent,
        Erreur::ProtegeEnEcriture,
        Erreur::Inattendue,
    ];
}

/// Le rang d'une variante. `match` **exhaustif** : c'est lui qui rend
/// impossible d'ajouter une variante sans être forcé de la classer.
const fn index(e: Erreur) -> usize {
    match e {
        Erreur::Introuvable => 0,
        Erreur::CheminIntrouvable => 1,
        Erreur::AccesRefuse => 2,
        Erreur::CanalFerme => 3,
        Erreur::DelaiDepasse => 4,
        Erreur::Abandonnee => 5,
        Erreur::DisquePlein => 6,
        Erreur::NonSupporte => 7,
        Erreur::RepertoireNonVide => 8,
        Erreur::DejaPresent => 9,
        Erreur::ProtegeEnEcriture => 10,
        Erreur::Inattendue => 11,
    }
}

/// `HRESULT_FROM_WIN32(x) = 0x8007_0000 | x`.
///
/// Rend un `i32` : c'est ce que `windows_core::HRESULT` enveloppe, et le module
/// reste PUR. Le `match` est **exhaustif** — une variante neuve ne peut pas
/// tomber dans un bras fourre-tout et hériter en silence du code d'une autre.
pub fn hresult(e: Erreur) -> i32 {
    let code = match e {
        Erreur::Introuvable => ERROR_FILE_NOT_FOUND,
        Erreur::CheminIntrouvable => ERROR_PATH_NOT_FOUND,
        Erreur::AccesRefuse => ERROR_ACCESS_DENIED,
        Erreur::CanalFerme => ERROR_IO_DEVICE,
        Erreur::DelaiDepasse => ERROR_SEM_TIMEOUT,
        Erreur::Abandonnee => ERROR_OPERATION_ABORTED,
        Erreur::DisquePlein => ERROR_DISK_FULL,
        Erreur::NonSupporte => ERROR_NOT_SUPPORTED,
        Erreur::RepertoireNonVide => ERROR_DIR_NOT_EMPTY,
        Erreur::DejaPresent => ERROR_FILE_EXISTS,
        Erreur::ProtegeEnEcriture => ERROR_WRITE_PROTECT,
        Erreur::Inattendue => ERROR_GEN_FAILURE,
    };
    (FACILITE_WIN32 | code) as i32
}

#[cfg(test)]
mod tests;
