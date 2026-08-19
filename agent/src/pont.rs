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
pub mod entetes;
pub mod enumeration;
pub mod erreurs;
pub mod notifications;
#[cfg(windows)]
pub mod projfs;
pub mod resolution;
#[cfg(windows)]
pub mod service;
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
pub async fn executer(config: crate::Config) -> anyhow::Result<()> {
    use anyhow::Context;

    // Charger AVANT de toucher au système de fichiers et AVANT le signaling :
    // une VM sans ProjFS doit échouer ici, avec un message qui nomme l'entrée
    // manquante, et non après avoir créé un dossier « Mes Fichiers » vide que
    // rien ne servirait jamais.
    let projfs = projfs::chargement::charger()?;

    // Le socket et le `Rtc` **données seules** : ni piste, ni codec, ni BWE.
    let (socket, mut rtc) = transport::construire_rtc_donnees(config.local_ip)?;

    let crate::signaling::SignalingHandle { mut offers, answers, closed, .. } =
        crate::signaling::run_signaling(
            &config.signaling_url,
            &config.session_id,
            config.jeton.as_deref(),
        )
        .await?;

    let offre = offers.recv().await.context("aucune offre SDP pour le pont fichiers")?;
    let offre = str0m::change::SdpOffer::from_sdp_string(&offre)
        .map_err(|e| anyhow::anyhow!("offre SDP illisible : {e}"))?;
    let reponse = rtc
        .sdp_api()
        .accept_offer(offre)
        .map_err(|e| anyhow::anyhow!("le pont refuse l'offre : {e}"))?;
    answers.send(reponse.to_sdp_string()).await.context("envoi de la réponse SDP du pont")?;
    tracing::info!("réponse SDP du pont envoyée");

    // ⚠️ **Aucun relais TURN pour le pont, et c'est une DIVERGENCE assumée
    // d'avec la session vidéo**, qui en alloue un avant sa réponse
    // (`demarrage.rs`). `pont::transport` — dont la signature est fixée par le
    // plan et livrée depuis la tâche 11 — n'expose aucun chemin d'allocation.
    // Le pont ne traverse donc que ce que les candidats hôtes traversent.
    // **Non couvert par F1**, à rouvrir le jour où la page-shell et la VM ne se
    // voient pas directement.

    // Le canal des requêtes : les rappels y poussent, le transport les émet.
    let (vers_navigateur, requetes) = std::sync::mpsc::channel();
    // Le canal des réponses : le transport y pousse, le fil du pont les lit.
    let (vers_pont, reponses) = std::sync::mpsc::channel();

    let virtualisation = projfs::Virtualisation::demarrer(projfs, vers_navigateur)?;
    let etat = virtualisation.etat();
    tracing::info!(racine = %virtualisation.racine().display(), "racine du pont fichiers montée");

    // Fil 2 — le transport. Il possède le `Rtc` et le socket, et **ne connaît
    // ni ProjFS ni Windows**.
    let transport = std::thread::Builder::new()
        .name("pont-transport".into())
        .spawn(move || {
            if let Err(erreur) = transport::tourner(rtc, socket, requetes, vers_pont) {
                tracing::error!(%erreur, "transport du pont arrêté sur erreur");
            }
        })
        .context("lancement du fil de transport du pont")?;

    // I6, comme pour la session vidéo : une perte du signaling après l'échange
    // initial doit être visible plutôt que silencieuse. Aucune renégociation
    // n'est possible, donc on observe et on journalise — mais on observe.
    let mut closed = closed;
    tokio::spawn(async move {
        if closed.changed().await.is_ok() && *closed.borrow() {
            tracing::warn!("connexion de signaling du pont perdue (aucune renégociation)");
        }
    });

    // Fil 3 — le fil du pont. Il possède la table et complète les commandes.
    // `spawn_blocking` : sa boucle est bloquante et ne doit pas occuper un
    // exécuteur tokio.
    let etat_du_fil = std::sync::Arc::clone(&etat);
    tokio::task::spawn_blocking(move || service::tourner(etat_du_fil, reponses))
        .await
        .context("le fil du pont fichiers a paniqué")?;

    // Le transport s'arrête de lui-même quand `sortant` est lâché, c'est-à-dire
    // quand `Etat` — donc `virtualisation` — est relâché. On l'attend AVANT de
    // rendre la main, sans quoi le `Drop` ci-dessous courrait pendant qu'il
    // émet encore.
    drop(virtualisation);
    let _ = transport.join();
    tracing::info!("pont fichiers arrêté");
    Ok(())
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode pont n'existe que sur Windows")
}
