//! Le pont fichiers : un seul processus par VM, qui tient la racine de
//! virtualisation ProjFS et la sert depuis le répertoire local que la
//! page-shell a ouvert.
//!
//! Ce fichier reste mince à dessein — **il assemble, il ne décide pas**. Même
//! découpage que `capteur.rs` et `superviseur.rs` : la logique pure (chemins,
//! erreurs, découpe, table, transport) est hors `cfg` et se teste sur l'hôte ;
//! ce qui touche ProjFS est gaté.
//!
//! **Pourquoi un processus séparé** (spec §3.2) : les rappels ProjFS
//! s'exécutent sur des fils que **le système** possède, où une panique Rust
//! devient un `abort` de processus. Les loger dans le superviseur ou dans le
//! capteur ferait du pont fichiers un risque pour la capture entière — c'est
//! la faute de l'ancien pont, transposée. Ici le pire cas est la mort du pont,
//! que le superviseur relance, **sans que le flux vidéo ne bronche** : c'est
//! le principe 4 du cadrage, et `boucle/surveillance_pont.rs` le rend
//! structurel en refusant de traiter un échec de démarrage comme fatal.
//!
//! **Pourquoi les entrées ProjFS sont résolues à l'EXÉCUTION** (décision D1) :
//! les enveloppes du crate `windows` passent par `raw-dylib`, donc par un
//! import statique dans le PE. Or `agent.exe` est **un seul binaire pour tous
//! les modes** : un import non résolu ne tuerait pas « le pont », il tuerait
//! la capture, la vidéo et l'entrée sur toute VM dépourvue de ProjFS. D'où
//! `LoadLibraryW` + `GetProcAddress`, en tâche 12 — et **rien, dans ce
//! fichier ni dans ses enfants purs, ne doit importer quoi que ce soit de
//! `Win32::Storage::ProjectedFileSystem`.**

pub mod chemins;
pub mod decoupe;
pub mod erreurs;
pub mod table;

/// Point d'entrée du mode pont.
///
/// ⚠️ **Squelette : il ne tient encore aucune racine de virtualisation.** Le
/// transport arrive en tâche 11, ProjFS en tâches 12 à 14. Rendre `Ok(())`
/// tout de suite ferait mourir le processus aussitôt lancé, et
/// `surveillance_pont` le relancerait en boucle à la cadence de son
/// espacement minimal — d'où le `bail!` explicite, qui dit ce qu'il en est
/// plutôt que de simuler un succès.
#[cfg(windows)]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le pont fichiers n'est pas encore assemblé (tâches 11 à 14 du sous-bloc F1)")
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode pont n'existe que sur Windows")
}
