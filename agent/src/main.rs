mod h264;
mod signaling;
mod source;
mod transport;

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use proto::control::AgentControl;

use crate::source::{FileSource, VideoSource};
use crate::transport::{Session, Tick};

/// Configuration de l'agent, lue depuis l'environnement.
struct Config {
    signaling_url: String,
    session_id: String,
    local_ip: IpAddr,
    test_file: Option<PathBuf>,
}

fn config() -> Result<Config> {
    Ok(Config {
        signaling_url: std::env::var("SIGNALING_URL")
            .unwrap_or_else(|_| "ws://127.0.0.1:8080".into()),
        session_id: std::env::var("SESSION_ID").unwrap_or_else(|_| "demo".into()),
        local_ip: std::env::var("LOCAL_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .context("LOCAL_IP n'est pas une adresse IP valide")?,
        test_file: std::env::var("TEST_FILE").ok().map(PathBuf::from),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config()?;
    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => anyhow::bail!("TEST_FILE non défini ; la capture Windows arrive à la tâche 9"),
    };

    let mut handle = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip)?;

    let offer = handle
        .offers
        .recv()
        .await
        .context("le signaling s'est fermé avant l'offre")?;
    tracing::info!("offre reçue");
    let answer = session.accept_offer(&offer)?;
    handle.answers.send(answer).await?;
    tracing::info!("réponse envoyée");

    session.send_control(&AgentControl::ready(1280, 720)).ok();

    // Cadence d'envoi : une image toutes les 16,67 ms.
    let frame_interval = std::time::Duration::from_micros(16_667);
    let mut next_frame = std::time::Instant::now();
    let mut on_input = |message| tracing::debug!(?message, "entrée reçue");
    let mut on_control = |message| tracing::info!(?message, "contrôle reçu");

    loop {
        if let Tick::Disconnected = session.tick(&mut on_input, &mut on_control)? {
            tracing::info!("session terminée");
            break;
        }
        let now = std::time::Instant::now();
        if now >= next_frame {
            session.send_next_frame()?;
            next_frame = now + frame_interval;
        }
    }
    Ok(())
}
