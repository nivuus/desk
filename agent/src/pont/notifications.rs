//! Quoi faire de chaque notification ProjFS. **PUR** — aucun `cfg`, aucune
//! dépendance au crate `windows`, entièrement testé sur l'hôte.
//!
//! # Pourquoi cette décision est ici et non dans le rappel
//!
//! **C'est le PÉRIMÈTRE de F1, et c'est donc la décision la plus importante du
//! sous-bloc** : « toute tentative d'écriture rend `ERROR_WRITE_PROTECT`. C'est
//! un périmètre, pas une lacune » (spec §8).
//!
//! ❌ **CETTE PHRASE DE LA SPEC EST FAUSSE POUR UN FICHIER CRÉÉ DE TOUTES
//! PIÈCES, et la recette de F1 l'a MESURÉ** (2 exécutions versées sur 2 qui
//! atteignent cette phase) : la création RÉUSSIT. **Ce module avait raison
//! contre elle** — son [`decider`] classe `NEW_FILE_CREATED` en POST,
//! irrefusable, cinquante lignes plus bas. C'est donc l'en-tête qui citait un
//! absolu que son propre corps réfutait. La formulation juste : *écrire dans
//! un fichier PROJETÉ rend `ERROR_WRITE_PROTECT` ; un fichier neuf vit sur la
//! VM et n'est jamais poussé.*
//!
//! ⚠️ **Et le levier qui porte la moitié VRAIE de la phrase —
//! `PRE_CONVERT_TO_FULL`, l'écriture d'un fichier EXISTANT — n'a JAMAIS été
//! exercé** : aucune pièce de la recette ne le confirme ni ne l'infirme.
//!
//! La décision est ici et non dans le rappel : laissée dans
//! `pont/projfs/rappels.rs`, elle serait `#[cfg(windows)]`, appelée par le
//! système, et **aucun test ne pourrait l'éprouver** — alors que ce qu'elle
//! fait est un pur appariement d'un code entier à une décision.
//!
//! `CLAUDE.md` en fait un critère de revue, pas un souhait : « toute décision
//! qui pourrait vivre dans un module pur DOIT y vivre ».
//!
//! # Pourquoi le naïf ne marche pas — et c'est la raison d'être du refus
//!
//! Avec `showDirectoryPicker({ mode: 'read' })`, la File System Access API
//! refuse bien l'écriture côté navigateur. Mais **côté VM, l'écriture RÉUSSIT
//! localement** : ProjFS hydrate le fichier et l'application écrit sur le
//! fichier NTFS local. Le fournisseur n'est prévenu qu'**après coup**, à la
//! fermeture du handle (spec §6.1). Une application verrait donc son
//! enregistrement **réussir**, et rien n'arriverait jamais côté poste local.
//! **C'est pire qu'une erreur : c'est une perte silencieuse.**
//!
//! Le levier est `PRJ_NOTIFY_FILE_PRE_CONVERT_TO_FULL` — « une application est
//! sur le point d'écrire, il faut hydrater complètement ». C'est une
//! notification **`PRE_`, donc REFUSABLE**.
//!
//! # Les valeurs sont RECOPIÉES, avec leur ligne source
//!
//! Même doctrine que [`crate::pont::erreurs`] : importer les constantes de
//! `windows::Win32::Storage::ProjectedFileSystem` gaterait ce module en
//! `#[cfg(windows)]` et lui ferait perdre sa testabilité d'hôte, qui est tout
//! son intérêt. Chaque constante porte donc le numéro de ligne de sa source,
//! relevé par la commande le 19 août 2026 dans
//! `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs`.
//!
//! ⚠️ **Les deux familles de constantes de ProjFS ne sont PAS interchangeables,
//! et elles portent des noms qui se ressemblent au point de tromper.**
//! `PRJ_NOTIFICATION_*` (`i32`) est ce que le rappel REÇOIT ;
//! `PRJ_NOTIFY_*` (`u32`) est ce que le MASQUE demande. Elles ont
//! ici les mêmes valeurs numériques, mais ce sont deux types distincts dans
//! windows-rs, et rien ne garantit qu'elles resteront alignées.

use crate::pont::erreurs::Erreur;

// Ce que le rappel REÇOIT — `PRJ_NOTIFICATION`, `i32`.
pub const PRE_CONVERT_TO_FULL: i32 = 4096; // mod.rs:340
pub const PRE_RENAME: i32 = 32; // mod.rs:378
pub const PRE_DELETE: i32 = 16; // mod.rs:377
pub const PRE_SET_HARDLINK: i32 = 64; // mod.rs:379
pub const HARDLINK_CREATED: i32 = 256; // mod.rs:342
pub const NEW_FILE_CREATED: i32 = 4; // mod.rs:349

// Ce que le MASQUE demande — `PRJ_NOTIFY_TYPES`, `u32`.
pub const NOTIFY_FILE_PRE_CONVERT_TO_FULL: u32 = 4096; // mod.rs:385
pub const NOTIFY_PRE_RENAME: u32 = 32; // mod.rs:391
pub const NOTIFY_PRE_DELETE: u32 = 16; // mod.rs:390
pub const NOTIFY_PRE_SET_HARDLINK: u32 = 64; // mod.rs:392
pub const NOTIFY_NEW_FILE_CREATED: u32 = 4; // mod.rs:388

/// Le masque que la racine demande.
///
/// **Les quatre premières sont des `PRE_`, donc REFUSABLES** — c'est ce qui
/// fait tenir la lecture seule. La cinquième est une POST : elle ne se refuse
/// pas, et elle n'est demandée que pour être **journalisée**.
///
/// ⚠️ **Demander MOINS ouvrirait un chemin d'écriture ; demander PLUS ferait
/// arriver une notification sans décision.** Les deux sont épinglés par
/// `le_masque_demande_exactement_les_cinq_notifications_de_f1` et
/// `chaque_bit_du_masque_a_une_decision_nommee`.
pub const MASQUE: u32 = NOTIFY_FILE_PRE_CONVERT_TO_FULL
    | NOTIFY_PRE_RENAME
    | NOTIFY_PRE_DELETE
    | NOTIFY_PRE_SET_HARDLINK
    | NOTIFY_NEW_FILE_CREATED;

/// Ce que le rappel de notification doit faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reponse {
    /// Refuser, avec la cause. Le rappel rend `hresult(cause)`.
    Refuser(Erreur),
    /// Accepter — on ne peut pas faire autrement, c'est une POST — **et le
    /// signaler au journal, avec le chemin**.
    AccepterEnSignalant,
    /// Accepter sans rien dire de plus qu'un `warn!` de masque inattendu.
    AccepterSansAttendre,
}

/// La décision, pour un code de notification `PRJ_NOTIFICATION`.
///
/// Le `match` n'est **pas** exhaustif au sens du compilateur — `PRJ_NOTIFICATION`
/// est un entier, pas une énumération Rust —, d'où le garde de test
/// `chaque_bit_du_masque_a_une_decision_nommee`, qui balaie les 32 bits et
/// vérifie qu'aucun bit DEMANDÉ ne retombe dans le bras fourre-tout.
pub fn decider(code: i32) -> Reponse {
    match code {
        // Les trois chemins d'écriture, refusés AVANT d'avoir commencé.
        PRE_CONVERT_TO_FULL | PRE_RENAME | PRE_DELETE => Reponse::Refuser(Erreur::ProtegeEnEcriture),
        // Les liens durs n'ont aucun équivalent dans la File System Access
        // API : ce n'est pas un refus de lecture seule, c'est une opération qui
        // n'existe pas de l'autre côté (spec §3.5.2). La distinction est
        // visible côté application ET au journal — c'est tout l'objet de
        // `pont::erreurs`, dont le contre-exemple est l'ancien pont, qui
        // rendait `EPERM` à neuf sites distincts.
        PRE_SET_HARDLINK | HARDLINK_CREATED => Reponse::Refuser(Erreur::NonSupporte),
        // ⚠️ **POST : elle ne se refuse pas.** Un fichier créé de toutes pièces
        // dans la racine existe donc localement et n'est JAMAIS poussé en F1.
        // Ce n'est pas une perte de donnée de l'utilisateur — son poste local
        // ne l'a jamais eu — mais c'est une divergence. Refermer ce cas est du
        // ressort de F2.
        NEW_FILE_CREATED => Reponse::AccepterEnSignalant,
        _ => Reponse::AccepterSansAttendre,
    }
}

#[cfg(test)]
mod tests;
