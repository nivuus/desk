//! Le superviseur : il détecte les fenêtres, leur donne une sortie virtuelle,
//! lance un processus enfant par fenêtre et parle à la page-shell.
//!
//! Ce fichier reste mince à dessein — il assemble, il ne décide pas. Les
//! décisions vivent dans `fenetres` (quelle fenêtre mérite d'exister côté
//! navigateur) et `table` (où en est chacune), tous deux en logique pure et
//! testés sur l'hôte.

pub mod enfants;
pub mod designation;
pub mod fenetres;
#[cfg(windows)]
pub mod hook;
pub mod placement;
pub mod reprise;
// La décision PURE de reconnexion de la session de contrôle. Enfant
// ORDINAIRE — pas de `#[path]` : ce fichier-ci n'est pas `#[cfg(windows)]`,
// donc `cargo test --workspace` compile et exécute ses tests sur l'hôte,
// là où `signalisation.rs` (son seul appelant) reste hors de portée.
pub mod reprise_controle;
pub mod protocole;
pub mod table;

#[cfg(windows)]
pub mod boucle;
#[cfg(windows)]
pub mod lanceur;
#[cfg(windows)]
pub mod signalisation;

#[cfg(windows)]
pub async fn executer(
    config: crate::Config,
    identite: Option<tokio::sync::watch::Receiver<Option<crate::plateforme::Identite>>>,
) -> anyhow::Result<()> {
    use anyhow::Context;

    // La session de contrôle porte le préfixe de la VM depuis le sous-bloc
    // P3 : sans lui, deux VMs ouvriraient toutes deux `bureau` et la seconde
    // serait refusée en « un agent est déjà connecté à la session ».
    let session_de_controle = protocole::session_de_controle(&config.prefixe);
    // 🔴 CETTE SESSION VIT SUR LE MÊME RELAIS QUE `demarrage.rs` ET
    // `pont.rs` — celui que le 21 août 2026 a déplacé de la racine vers
    // `/signal` (voir `crate::signaling::url_du_relais`). `config.signaling_url`
    // reste la BASE du service ; sans cette dérivation, le superviseur
    // continuerait de frapper l'ancienne racine, désormais fermée, et la
    // page-shell — qui, elle, a suivi le déplacement via
    // `client/src/adresse-plateforme.ts` — ne trouverait jamais personne en
    // face sur la session de contrôle.
    // 🔴 LA VEILLE D'IDENTITÉ PART AVEC, ET PAS SEULEMENT `config.jeton`.
    // Le socket de contrôle SE REPREND depuis ce lot ; une reconnexion
    // survenue une heure plus tard présenterait l'instantané du démarrage,
    // c'est-à-dire un jeton mort que la garde refuse (« jeton refusé
    // (expire) », ligne réellement observée en production). C'est le même
    // argument, mot pour mot, que `main.rs` porte déjà pour le LANCEUR —
    // « un superviseur vit des heures ; le jeton d'agent dure dix minutes ».
    // `config.jeton` reste la valeur de la PREMIÈRE ouverture, et le repli
    // quand aucun canal `/agent` n'a été ouvert.
    let (rx_shell, envoyer) = signalisation::connecter(
        &crate::signaling::url_du_relais(&config.signaling_url),
        &session_de_controle,
        config.jeton.as_deref(),
        identite.clone(),
    )
    .await?;

    let signaling_url = config.signaling_url.clone();
    let prefixe = config.prefixe.clone();
    let local_ip = config.local_ip.to_string();

    // TOUT le reste court sur un fil bloquant, et pas sur un ouvrier async.
    //
    // Une seule raison, et elle suffit : `boucle::tourner` ne rend JAMAIS la
    // main. La tenir sur un ouvrier tokio y gèlerait les deux tâches
    // d'émission et de réception ouvertes ci-dessus, c'est-à-dire le lien avec
    // la page-shell.
    //
    // Ce n'est PAS une contrainte de compilation : `PiloteParIoctl` (un
    // `HANDLE`) et `Hook` (un `HWINEVENTHOOK`) ne sont certes pas `Send`, mais
    // les créer dans le contexte async compilerait — ils naîtraient après le
    // dernier `.await`, et le futur de `main` sous `block_on` ne porte aucune
    // borne `Send`. C'est un choix d'exécution, pas une obligation du type
    // système ; qu'ils naissent ici les fait simplement vivre sur le fil même
    // qui les emploie.
    tokio::task::spawn_blocking(move || {
        // Purger AVANT tout : une exécution précédente tuée net a pu laisser
        // des sorties, et elles occupent le vivier de dix.
        if let Err(erreur) = crate::moniteurs_virtuels::purge::purger() {
            tracing::warn!(%erreur, "purge des sorties orphelines incomplète au démarrage");
        }

        let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()
            .context("ouverture du pilote d'affichage virtuel")?;
        let lanceur = lanceur::LanceurDeProcessus::nouveau(
            std::env::current_exe().context("chemin de l'exécutable")?,
            signaling_url,
            local_ip,
            prefixe.clone(),
            identite,
        )?;

        let (tx_hook, rx_hook) = std::sync::mpsc::channel();
        // La garde vit jusqu'à la fin de cette fermeture : la lâcher retirerait
        // le hook et arrêterait sa pompe de messages.
        let _garde_hook = hook::poser(tx_hook).context("pose du hook de détection")?;

        boucle::tourner(&pilote, &lanceur, rx_hook, rx_shell, envoyer, prefixe)
    })
    .await
    .context("le fil du superviseur a paniqué")?
}

#[cfg(not(windows))]
pub async fn executer(
    _config: crate::Config,
    _identite: Option<tokio::sync::watch::Receiver<Option<crate::plateforme::Identite>>>,
) -> anyhow::Result<()> {
    anyhow::bail!("le mode superviseur n'existe que sur Windows")
}
