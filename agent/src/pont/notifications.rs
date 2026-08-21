//! Quoi faire de chaque notification ProjFS. **PUR** — aucun `cfg`, aucune
//! dépendance au crate `windows`, entièrement testé sur l'hôte.
//!
//! # ❌ CE MODULE NE TIENT PLUS LA LECTURE SEULE — F2 L'A OUVERT EN ÉCRITURE
//!
//! **C'était le PÉRIMÈTRE de F1** : « toute tentative d'écriture rend
//! `ERROR_WRITE_PROTECT`. C'est un périmètre, pas une lacune » (spec §8). La
//! recette de F1 avait déjà réfuté cette phrase pour **la création** (2
//! exécutions versées sur 2 : elle RÉUSSIT, la notification étant une POST).
//! **F2 réfute l'autre moitié, délibérément** : `PRE_CONVERT_TO_FULL` est
//! désormais AUTORISÉE quand la racine est inscriptible et le canal ouvert, et
//! les octets sont poussés vers le poste local **après coup**.
//!
//! ⚠️ **Ce qui reste refusé, et c'est nommé** : `PRE_RENAME` et `PRE_DELETE`,
//! parce que `Renommer` et `Supprimer` sont des livrables de **F3**. Une
//! application qui emploie l'idiome *écrire-temporaire / renommer / supprimer*
//! échouera donc **bruyamment au renommage**, plutôt que de réussir sur la VM
//! en laissant le poste local sur l'ancien contenu. **Accepter le renommage
//! sans le pousser produirait exactement la perte silencieuse que ce module
//! existe pour interdire.**
//!
//! # 🔴 CE QUE F2 NE PEUT PAS FAIRE, ET QU'IL FAUT SAVOIR AVANT DE LIRE LA SUITE
//!
//! **Le chemin d'écriture n'a AUCUNE contre-pression.** Il n'existe aucune
//! notification par laquelle on *accepte* une écriture : on accepte en **ne
//! refusant pas** `PRE_CONVERT_TO_FULL`, et l'on apprend qu'il y a des octets à
//! pousser par `FILE_HANDLE_CLOSED_FILE_MODIFIED` et `FILE_OVERWRITTEN`, toutes
//! deux **POST** — c'est-à-dire **après** que l'application a refermé son handle
//! et cru avoir enregistré. Si la poussée échoue ensuite — permission révoquée,
//! disque plein, onglet fermé —, **aucun `HRESULT` ne peut plus atteindre
//! personne**.
//!
//! Les deux seuls leviers qui restent sont donc :
//!
//! 1. un refus **EN AMONT**, à `PRE_CONVERT_TO_FULL`, portant sur un **ÉTAT**
//!    (racine en lecture seule, canal fermé) et **jamais sur l'issue** — c'est
//!    la raison d'être du paramètre [`Etat`] de [`decider`] ;
//! 2. une **DÉNONCIATION** après coup : le journal de reprise, le compteur
//!    d'écritures dues de la page-shell, et `beforeunload`.
//!
//! # Pourquoi cette décision est ici et non dans le rappel
//!
//! Laissée dans `pont/projfs/rappels/notification.rs`, elle serait
//! `#[cfg(windows)]`, appelée par le système, et **aucun test ne pourrait
//! l'éprouver** — alors que ce qu'elle fait est un pur appariement d'un code
//! entier et d'un état à une décision. `CLAUDE.md` en fait un critère de revue,
//! pas un souhait : « toute décision qui pourrait vivre dans un module pur DOIT
//! y vivre ».
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
pub const FILE_OVERWRITTEN: i32 = 8; // mod.rs:339
pub const FILE_HANDLE_CLOSED_FILE_MODIFIED: i32 = 1024; // mod.rs:336
// ── LES DEUX DE F3 ────────────────────────────────────────────────────────
// ⚠️ **La VALEUR fait foi, jamais le numéro de ligne.** `windows` et
// `windows-sys` exposent DEUX modules `Win32/Storage/ProjectedFileSystem` aux
// symboles identiques et aux lignes différentes ; l'auteur du plan de F2 a
// déclaré fausses trois citations exactes pour l'avoir oublié. Le crate qui
// fait foi est celui qu'`agent/Cargo.toml` déclare — `windows` (0.62.2).
pub const FILE_RENAMED: i32 = 128; // mod.rs:341
pub const FILE_HANDLE_CLOSED_FILE_DELETED: i32 = 2048; // mod.rs:335

// Ce que le MASQUE demande — `PRJ_NOTIFY_TYPES`, `u32`.
pub const NOTIFY_FILE_PRE_CONVERT_TO_FULL: u32 = 4096; // mod.rs:385
pub const NOTIFY_PRE_RENAME: u32 = 32; // mod.rs:391
pub const NOTIFY_PRE_DELETE: u32 = 16; // mod.rs:390
pub const NOTIFY_PRE_SET_HARDLINK: u32 = 64; // mod.rs:392
pub const NOTIFY_NEW_FILE_CREATED: u32 = 4; // mod.rs:388
pub const NOTIFY_FILE_OVERWRITTEN: u32 = 8; // mod.rs:384
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED: u32 = 1024; // mod.rs:381
pub const NOTIFY_FILE_RENAMED: u32 = 128; // mod.rs:386
pub const NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED: u32 = 2048; // mod.rs:380

/// Le masque que la racine demande — **NEUF bits** : cinq en F1, sept après F2,
/// neuf depuis F3.
///
/// - **Les quatre `PRE_` sont REFUSABLES**, et **TROIS** d'entre elles décident
///   réellement de quelque chose depuis F3 : `PRE_CONVERT_TO_FULL` (l'écriture),
///   `PRE_RENAME` et `PRE_DELETE`. *(Ces lignes disaient « une seule », et
///   « les trois autres sont refusées inconditionnellement — renommage et
///   suppression : F3 ». F3 est arrivé.)* La quatrième, `PRE_SET_HARDLINK`,
///   reste refusée sans condition : les liens durs n'ont **aucun** équivalent
///   dans la File System Access API.
/// - **Les cinq POST ne se refusent pas** : elles disent ce qui a DÉJÀ eu lieu
///   sur la VM, et c'est tout ce qu'on peut en tirer.
///
/// 🔵 **CE QUE LES `PRE_` ACHÈTENT À F3, ET QUE F2 N'AVAIT PAS.** F2 déclare
/// que le chemin d'écriture n'a **aucune** contre-pression : il n'apprend une
/// écriture qu'à la fermeture du handle, par une POST, quand l'application a
/// déjà cru enregistrer. `PRE_RENAME` et `PRE_DELETE` sont, elles, des **PRE** :
/// F3 peut refuser un renommage ou une suppression **avant** qu'ils n'aient
/// lieu, et l'application le voit.
///
/// ⚠️ **Mais le refus ne peut porter que sur un ÉTAT, jamais sur une ISSUE.**
/// Les `PRE_` sont **synchrones** et ne consultent jamais le navigateur (spec
/// §4.3). On ne sait donc pas si le poste local acceptera ; on sait seulement
/// si notre côté est en mesure de pousser.
///
/// ⚠️ **`FILE_HANDLE_CLOSED_NO_MODIFICATION` (512, `mod.rs:382`) n'est
/// DÉLIBÉRÉMENT PAS DEMANDÉE.** Elle arriverait à **chaque fermeture de handle
/// en lecture**, c'est-à-dire sur le chemin le plus chaud du pont, pour
/// n'apprendre que ce qu'on sait déjà : qu'il n'y a rien à pousser. Le coût est
/// certain, le gain nul. *Décision, pas oubli.*
///
/// ⚠️ **`HARDLINK_CREATED` (256) reste demandée ET refusée, comme en F1** — mais
/// elle est **POST** : le refus n'empêche rien, il **journalise**. Le dire,
/// plutôt que de laisser croire qu'un lien dur est empêché.
///
/// ⚠️ **Demander MOINS ferait perdre des écritures ; demander PLUS ferait
/// arriver une notification sans décision.** Les deux sont épinglés par
/// `le_masque_demande_exactement_les_sept_notifications_de_f2` et
/// `chaque_bit_du_masque_a_une_decision_nommee`.
pub const MASQUE: u32 = NOTIFY_FILE_PRE_CONVERT_TO_FULL
    | NOTIFY_PRE_RENAME
    | NOTIFY_PRE_DELETE
    | NOTIFY_PRE_SET_HARDLINK
    | NOTIFY_NEW_FILE_CREATED
    | NOTIFY_FILE_OVERWRITTEN
    | NOTIFY_FILE_HANDLE_CLOSED_FILE_MODIFIED
    | NOTIFY_FILE_RENAMED
    | NOTIFY_FILE_HANDLE_CLOSED_FILE_DELETED;

/// Ce qu'une poussée transporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poussee {
    /// Les octets d'un fichier refermé après modification.
    Contenu,
    /// Une entrée qui vient d'apparaître. **Un répertoire ne porte aucun
    /// contenu** ; un fichier, lui, sera suivi d'une poussée de contenu à la
    /// fermeture de son handle.
    Creation,
    /// **F3** — l'entrée a été renommée. `de` est `FilePathName`, `vers` est
    /// `destinationFileName` : deux paramètres DIRECTS du rappel, jamais des
    /// membres de l'union `PRJ_NOTIFICATION_PARAMETERS`, que ce pont ne
    /// déréférence toujours pas.
    ///
    /// 🔴 **S'y tromper de sens DÉTRUIT**, et c'est le risque le plus grave de
    /// F3. Le pont refuse donc de pousser un renommage dont la destination est
    /// vide ou égale à la source, avec un `warn!` qui nomme les deux champs
    /// bruts — parade qui ne dépend d'aucune mesure.
    Renommage,
    /// **F3** — l'entrée a été supprimée.
    ///
    /// ⚠️ Le nom de la notification est `FILE_HANDLE_CLOSED_FILE_DELETED` : la
    /// suppression n'est acquise qu'à la fermeture du **dernier** handle, ce
    /// qui est la sémantique de Windows et non une subtilité de ProjFS.
    Suppression,
}

/// L'état dont la décision dépend.
///
/// 🔴 **LA DÉCISION PORTE SUR UN ÉTAT, JAMAIS SUR UNE ISSUE**, et c'est la
/// seule forme de refus qui reste possible : à `PRE_CONVERT_TO_FULL` on ne sait
/// rien de ce que la poussée deviendra, et quand on le saura il sera trop tard
/// pour le dire à qui que ce soit (voir l'en-tête).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etat {
    /// La racine accepte-t-elle l'écriture ? `false` = le comportement de F1.
    pub inscriptible: bool,
    /// Le canal du pont est-il ouvert ? Refuser ici est le seul instant où
    /// l'application peut encore l'apprendre.
    pub canal_ouvert: bool,
    /// **F3** — les mutations sont-elles armées ? `PONT_MUTATION=0` les désarme.
    ///
    /// 🔴 **VARIABLE DE BANC, jamais une configuration livrée.** Elle existe
    /// pour rendre ROUGE les critères ① et ② de la recette : désarmée, le
    /// `PRE_` refuse, l'application voit `ERROR_WRITE_PROTECT`, et **le poste
    /// local est inchangé**. C'est un rouge du MÉCANISME — le refus est
    /// journalisé et le compteur `protege-en-ecriture` monte —, jamais un rouge
    /// vacueux.
    ///
    /// ⚠️ **Elle ne touche PAS l'écriture** : `PONT_ECRITURE` a la sienne. Deux
    /// mécanismes distincts, deux interrupteurs distincts — les confondre
    /// ferait qu'une recette du renommage couperait aussi l'idiome
    /// temp+rename qu'elle veut exercer.
    pub mutations_armees: bool,
}

/// Ce que l'on sait de la DESTINATION d'une notification.
///
/// ⚠️ **Un `enum` et non un `bool`, parce qu'une suppression n'a pas de
/// destination du tout.** Passer `false` y suggérerait « la destination est
/// dans la racine », qui ne veut rien dire, et un test qui l'écrirait
/// n'éprouverait rien.
///
/// ⚠️ **`chemins::normaliser` décide, jamais [`decider`].** La spec §4.3 dit
/// « accepte, sauf si la cible sort de la racine », et `pont::chemins` est déjà
/// le module qui refuse les `..`, les `:` et les noms réservés. Le dupliquer
/// ici en ferait deux vérités.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    /// La notification ne porte aucune destination — tous les codes sauf
    /// `PRE_RENAME`.
    SansObjet,
    /// La destination est recevable : dans la racine, et normalisée.
    DansLaRacine,
    /// La destination sort de la racine, ou `chemins::normaliser` l'a refusée.
    HorsRacine,
}

/// Ce que le rappel de notification doit faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reponse {
    /// Refuser, avec la cause. Le rappel rend `hresult(cause)`.
    Refuser(Erreur),
    /// **Autoriser une écriture.** C'est la SEULE acceptation explicite de ce
    /// module, et la seule ligne de F2 qui change ce qu'une application obtient.
    ///
    /// 🔴 **ELLE EST DISTINCTE DU FOURRE-TOUT, ET CE N'EST PAS UN LUXE.** Le
    /// plan de F2 prescrivait `AccepterSansAttendre` pour ce cas ; cela aurait
    /// fait retomber un bit **DEMANDÉ par le masque** dans le bras fourre-tout,
    /// donc fait échouer `chaque_bit_du_masque_a_une_decision_nommee` — le
    /// garde d'exhaustivité de ce module. Le remède évident aurait été
    /// d'exclure `PRE_CONVERT_TO_FULL` du balayage, ce qui aurait **vidé le
    /// garde** au lieu de le satisfaire. *Un plan n'immunise pas contre le
    /// contrôle vacueux : il en est une source.*
    Autoriser,
    /// La notification déclenche un **write-back**.
    Pousser(Poussee),
    /// Accepter sans rien dire de plus qu'un `warn!` de masque inattendu.
    ///
    /// ⚠️ **C'est le bras FOURRE-TOUT, et il est nommé comme tel.** Aucun bit
    /// demandé par [`MASQUE`] ne doit y retomber.
    AccepterSansAttendre,
}

// ⚠️ **`AccepterEnSignalant` A DISPARU, et c'est une divergence déclarée avec le
// plan de F2**, qui la conservait dans son énumération. Elle n'avait plus qu'un
// producteur en F1 — `NEW_FILE_CREATED` —, que F2 fait passer à
// `Pousser(Creation)` : la garder en ferait une variante sans aucun site de
// construction, c'est-à-dire du code mort dans un module dont tout l'intérêt
// est d'être exhaustivement balayé.

/// La décision, pour un code de notification `PRJ_NOTIFICATION` et un [`Etat`].
///
/// Le `match` n'est **pas** exhaustif au sens du compilateur — `PRJ_NOTIFICATION`
/// est un entier, pas une énumération Rust —, d'où le garde de test
/// `chaque_bit_du_masque_a_une_decision_nommee`, qui balaie les 32 bits et
/// vérifie qu'aucun bit DEMANDÉ ne retombe dans le bras fourre-tout.
pub fn decider(code: i32, etat: Etat, cible: Cible) -> Reponse {
    match code {
        // 🔵 **L'UNIQUE PORTE DE REFUS D'UNE ÉCRITURE.** Au-delà, plus rien ne
        // peut être dit à l'application : elle refermera son handle en croyant
        // avoir enregistré.
        //
        // ⚠️ **DEUX CAUSES, DEUX CODES**, et la spec §5.1 l'exige : une racine
        // en lecture seule rend `ERROR_WRITE_PROTECT`, un canal fermé rend
        // `ERROR_IO_DEVICE`. Les faire partager un code rendrait
        // indistinguables « ce partage est en lecture seule » et « l'onglet est
        // fermé » — deux situations qui n'appellent pas le même geste.
        PRE_CONVERT_TO_FULL if !etat.inscriptible => Reponse::Refuser(Erreur::ProtegeEnEcriture),
        PRE_CONVERT_TO_FULL if !etat.canal_ouvert => Reponse::Refuser(Erreur::CanalFerme),
        PRE_CONVERT_TO_FULL => Reponse::Autoriser,
        // ❌ **CES DEUX-LÀ N'ÉTAIENT PAS REFUSÉS PARCE QU'ILS DEVAIENT L'ÊTRE,
        // MAIS PARCE QUE F3 N'EXISTAIT PAS ENCORE.** *(Ce bras disait :
        // « REFUSÉS INCONDITIONNELLEMENT, ET C'EST DÉLIBÉRÉ. `Renommer` et
        // `Supprimer` sont des livrables de F3 : les accepter sans pouvoir les
        // pousser laisserait le poste local sur l'ancien contenu. » La raison
        // était juste, et elle a cessé de l'être : F3 sait les pousser.)*
        //
        // 🔴 **QUATRE ÉTATS REFUSENT, ET AUCUNE ISSUE NE LE FAIT.** Un `PRE_`
        // est synchrone : on ne peut pas demander au navigateur ce qu'il
        // pense de l'opération, seulement constater que notre côté n'est pas en
        // mesure de la pousser.
        //
        // ⚠️ **L'ORDRE EST CELUI DE `PRE_CONVERT_TO_FULL`, par symétrie.** Il
        // ne départage que le cas d'un renommage hors racine sur une racine
        // déjà en lecture seule, qui n'arrive pas — mais le laisser au hasard
        // ferait diverger les deux portes de refus du module.
        PRE_RENAME | PRE_DELETE if !etat.mutations_armees => {
            Reponse::Refuser(Erreur::ProtegeEnEcriture)
        }
        PRE_RENAME | PRE_DELETE if !etat.inscriptible => {
            Reponse::Refuser(Erreur::ProtegeEnEcriture)
        }
        PRE_RENAME | PRE_DELETE if !etat.canal_ouvert => Reponse::Refuser(Erreur::CanalFerme),
        // ⚠️ **`NonSupporte` ET NON `ProtegeEnEcriture`** : sortir de la racine
        // n'est pas un refus de droit, c'est une opération que l'autre bout ne
        // sait pas faire — il n'a aucune poignée hors du répertoire que
        // l'utilisateur a choisi. Les faire partager un code violerait le §5.1
        // de la spec, et ferait chercher une permission là où il n'y en a pas.
        PRE_RENAME if cible == Cible::HorsRacine => Reponse::Refuser(Erreur::NonSupporte),
        PRE_RENAME | PRE_DELETE => Reponse::Autoriser,
        // Les liens durs n'ont aucun équivalent dans la File System Access
        // API : ce n'est pas un refus de lecture seule, c'est une opération qui
        // n'existe pas de l'autre côté (spec §3.5.2). La distinction est
        // visible côté application ET au journal — c'est tout l'objet de
        // `pont::erreurs`, dont le contre-exemple est l'ancien pont, qui
        // rendait `EPERM` à neuf sites distincts.
        PRE_SET_HARDLINK | HARDLINK_CREATED => Reponse::Refuser(Erreur::NonSupporte),
        // ⚠️ **POST : elle ne se refuse pas** — mais F2 la POUSSE, ce qui
        // referme la divergence que F1 déclarait sienne (« un fichier créé de
        // toutes pièces vit sur la VM et n'est JAMAIS poussé »).
        NEW_FILE_CREATED => Reponse::Pousser(Poussee::Creation),
        // Les deux POST de contenu. **`FILE_OVERWRITTEN` n'est pas redondante
        // avec la fermeture de handle** : elle signale une troncature à
        // l'ouverture (`CREATE_ALWAYS`, `TRUNCATE_EXISTING`), qu'un
        // enregistrement « en place » produit couramment.
        FILE_OVERWRITTEN | FILE_HANDLE_CLOSED_FILE_MODIFIED => {
            Reponse::Pousser(Poussee::Contenu)
        }
        // ── LES DEUX POST DE F3 ───────────────────────────────────────────
        // ⚠️ **Elles ne se refusent PAS**, comme toutes les POST : le geste a
        // déjà eu lieu dans la VM. Ce qui les a autorisées est le `PRE_`
        // correspondant, quelques microsecondes plus tôt.
        //
        // 🔴 **ET L'ÉTAT NE LES CHANGE PAS NON PLUS.** Une poussée part même
        // canal fermé : le fil qui la sert la journalisera et la retiendra.
        // Les subordonner à l'état ferait perdre en silence exactement ce que
        // le refus au `PRE_` avait laissé passer.
        FILE_RENAMED => Reponse::Pousser(Poussee::Renommage),
        FILE_HANDLE_CLOSED_FILE_DELETED => Reponse::Pousser(Poussee::Suppression),
        _ => Reponse::AccepterSansAttendre,
    }
}

#[cfg(test)]
mod tests;
