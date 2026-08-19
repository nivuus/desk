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
pub mod notifications;
#[cfg(windows)]
pub mod projfs;
pub mod resolution;
pub mod table;
pub mod transport;

/// Point d'entrée du mode pont : charge ProjFS, monte la racine, et la tient.
///
/// ⚠️ **Tâche 13 : la racine est montée et VIDE.** Aucune requête ne part vers
/// le navigateur — les trois rappels asynchrones sont branchés en tâche 14.
/// C'est le premier état observable sur la VM : le dossier
/// `%USERPROFILE%\Mes Fichiers` apparaît, il est vide, et le pont s'arrête
/// proprement.
///
/// ⚠️ **« S'arrête proprement » a une portée exacte** : le `Drop` de
/// [`projfs::Virtualisation`] complète les commandes en vol puis appelle
/// `PrjStopVirtualizing`. Il court sur une sortie NORMALE de cette fonction —
/// jamais sur un `TerminateProcess`, qui est ce que le job object du
/// superviseur inflige à ses enfants. **Une racine peut donc survivre à un
/// arrêt brutal du superviseur**, et rien dans F1 ne la démonte alors : c'est
/// le pendant exact des sorties virtuelles qui survivent à un
/// `Stop-Process -Force` (sous-bloc D5), et ce n'est pas refermé ici.
#[cfg(windows)]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    // Charger AVANT de toucher au système de fichiers : une VM sans ProjFS
    // doit échouer sur le chargement, avec un message qui nomme l'entrée
    // manquante, et non après avoir créé un dossier « Mes Fichiers » vide que
    // rien ne servirait jamais.
    let projfs = projfs::chargement::charger()?;
    let virtualisation = projfs::Virtualisation::demarrer(projfs)?;
    let etat = virtualisation.etat();
    tracing::info!(
        racine = %virtualisation.racine().display(),
        "pont fichiers prêt (tâche 13 : la racine est montée et VIDE, aucune requête \
         ne part vers le navigateur avant la tâche 14)"
    );
    loop {
        tokio::time::sleep(projfs::PERIODE_HYDRATATION).await;
        etat.tracer_hydratation();
    }
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode pont n'existe que sur Windows")
}
