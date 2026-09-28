//! The supervisor: it detects windows, gives them a virtual output,
//! launches a child process per window and talks to the shell page.
//!
//! This file stays thin on purpose — it assembles, it does not decide. The
//! decisions live in `fenetres` (which window deserves to exist on the
//! browser side) and `table` (where each one stands), both pure logic and
//! tested on the host.

pub mod designation;
pub mod enfants;
pub mod fenetres;
#[cfg(windows)]
pub mod hook;
pub mod placement;
pub mod reprise;
pub mod sursis;
// The PURE reconnection decision of the control session. ORDINARY
// child — no `#[path]`: this file is not `#[cfg(windows)]`,
// so `cargo test --workspace` compiles and runs its tests on the host,
// where `signalisation.rs` (its only caller) stays out of reach.
pub mod protocole;
pub mod reprise_controle;
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

    // The control session carries the VM's prefix since sub-block
    // P3: without it, two VMs would both open `bureau` and the second
    // would be refused with "an agent is already connected to the session".
    let session_de_controle = protocole::session_de_controle(&config.prefixe);
    // 🔴 THIS SESSION LIVES ON THE SAME RELAY AS `demarrage.rs` AND
    // `pont.rs` — the one that August 21st, 2026 moved from the root to
    // `/signal` (see `crate::signaling::url_du_relais`). `config.signaling_url`
    // stays the service BASE; without this derivation, the supervisor
    // would keep hitting the old root, now closed, and the
    // shell page — which, for its part, followed the move via
    // `client/src/adresse-plateforme.ts` — would never find anyone
    // facing it on the control session.
    // 🔴 THE IDENTITY WATCH GOES ALONG, AND NOT ONLY `config.jeton`.
    // The control socket RESUMES since this batch; a reconnection
    // occurring an hour later would present the startup snapshot,
    // that is, a dead token the guard refuses ("token refused
    // (expired)", a line really observed in production). It is the same
    // argument, word for word, that `main.rs` already carries for the LAUNCHER —
    // "a supervisor lives for hours; the agent token lasts ten minutes".
    // `config.jeton` stays the value of the FIRST opening, and the fallback
    // when no `/agent` channel has been opened.
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

    // EVERYTHING else runs on a blocking thread, and not on an async worker.
    //
    // A single reason, and it is enough: `boucle::tourner` NEVER returns
    // control. Holding it on a tokio worker would freeze the two send
    // and receive tasks opened above there, that is, the link with
    // the shell page.
    //
    // It is NOT a compilation constraint: `PiloteParIoctl` (a
    // `HANDLE`) and `Hook` (an `HWINEVENTHOOK`) are indeed not `Send`, but
    // creating them in the async context would compile — they would be born after the
    // last `.await`, and `main`'s future under `block_on` carries no
    // `Send` bound. It is an execution choice, not a type system
    // obligation; having them born here simply makes them live on the very thread
    // that uses them.
    tokio::task::spawn_blocking(move || {
        // Purge BEFORE anything: a previous run killed outright may have left
        // outputs, and they occupy the pool of ten.
        if let Err(error) = crate::moniteurs_virtuels::purge::purger() {
            tracing::warn!(%error, "purge of orphan outputs incomplete at startup");
        }

        let pilote = crate::moniteurs_virtuels::pilote::ouvrir_pilote()
            .context("opening the virtual display driver")?;
        let lanceur = lanceur::LanceurDeProcessus::new(
            std::env::current_exe().context("executable path")?,
            signaling_url,
            local_ip,
            prefixe.clone(),
            identite,
        )?;

        let (tx_hook, rx_hook) = std::sync::mpsc::channel();
        // The guard lives until the end of this closure: dropping it would remove
        // the hook and stop its message pump.
        let _garde_hook = hook::poser(tx_hook).context("installing the detection hook")?;

        boucle::tourner(&pilote, &lanceur, rx_hook, rx_shell, envoyer, prefixe)
    })
    .await
    .context("the supervisor thread panicked")?
}

#[cfg(not(windows))]
pub async fn executer(
    _config: crate::Config,
    _identite: Option<tokio::sync::watch::Receiver<Option<crate::plateforme::Identite>>>,
) -> anyhow::Result<()> {
    anyhow::bail!("supervisor mode only exists on Windows")
}
