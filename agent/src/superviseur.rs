//! Le superviseur : il détecte les fenêtres, leur donne une sortie virtuelle,
//! lance un processus enfant par fenêtre et parle à la page-shell.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Les
//! décisions vivent dans `fenetres` (quelle fenêtre mérite d'exister côté
//! navigateur) et `table` (où en est chacune), tous deux en logique pure et
//! testés sur l'hôte.

pub mod enfants;
pub mod fenetres;
#[cfg(windows)]
pub mod hook;
pub mod placement;
pub mod protocole;
pub mod table;

#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lanceur;
#[cfg(windows)]
pub mod signalisation;

#[cfg(windows)]
pub async fn executer(config: crate::Config) -> anyhow::Result<()> {
    use anyhow::Context;

    let (rx_shell, envoyer) =
        signalisation::connecter(&config.signaling_url, protocole::SESSION_DE_CONTROLE).await?;

    let signaling_url = config.signaling_url.clone();
    let local_ip = config.local_ip.to_string();

    // TOUT le reste court sur un fil bloquant, et pas sur un ouvrier async.
    //
    // Deux raisons, et la seconde est contraignante. D'abord `boucle::tourner`
    // ne rend jamais la main : la tenir sur un ouvrier tokio y gèlerait les
    // deux tâches d'émission et de réception ouvertes ci-dessus, c'est-à-dire
    // le lien avec la page-shell. Ensuite `PiloteParIoctl` (un `HANDLE`) et
    // `Hook` (un `HWINEVENTHOOK`) ne sont pas `Send` : ils ne peuvent pas être
    // créés ici puis déplacés là-bas — ils doivent naître SUR ce fil.
    tokio::task::spawn_blocking(move || {
        // Purger AVANT tout : une exécution précédente tuée net a pu laisser
        // des sorties, et elles occupent le vivier de dix.
        if let Err(erreur) = crate::moniteurs_virtuels::purge::purger() {
            tracing::warn!(%erreur, "purge des sorties orphelines incomplète au démarrage");
        }

        let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()
            .context("ouverture du pilote d'affichage virtuel")?;
        let lanceur = lanceur::LanceurDeProcessus {
            executable: std::env::current_exe().context("chemin de l'exécutable")?,
            signaling_url,
            local_ip,
        };

        let (tx_hook, rx_hook) = std::sync::mpsc::channel();
        // La garde vit jusqu'à la fin de cette fermeture : la lâcher retirerait
        // le hook et arrêterait sa pompe de messages.
        let _garde_hook = hook::poser(tx_hook).context("pose du hook de détection")?;

        boucle::tourner(&pilote, &lanceur, rx_hook, rx_shell, envoyer)
    })
    .await
    .context("le fil du superviseur a paniqué")?
}

#[cfg(not(windows))]
pub async fn executer(_config: crate::Config) -> anyhow::Result<()> {
    anyhow::bail!("le mode superviseur n'existe que sur Windows")
}
