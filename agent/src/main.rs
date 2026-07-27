mod h264;
mod signaling;
mod source;
mod transport;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod window;

use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::source::{FileSource, VideoSource};
use crate::transport::Session;

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

    // Mode diagnostic : CAPTURE_TEST=firefox vérifie le repérage et la capture.
    #[cfg(windows)]
    if let Ok(fragment) = std::env::var("CAPTURE_TEST") {
        let hwnd = window::find_window_by_title(&fragment)?;
        let (w, h) = window::client_size(hwnd)?;
        tracing::info!(largeur = w, hauteur = h, "fenêtre trouvée");

        let mut capture = capture::WindowCapture::new(hwnd)?;
        let mut captured = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Some(frame) = capture.next_texture()? {
                captured += 1;
                if captured == 1 {
                    tracing::info!(frame.width, frame.height, "première image capturée");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        tracing::info!(captured, "images capturées en 3 s");
        return Ok(());
    }

    let source: Box<dyn VideoSource + Send> = match &config.test_file {
        Some(path) => {
            tracing::info!(?path, "source de test");
            Box::new(FileSource::from_path(path, 1280, 720, 60)?)
        }
        None => anyhow::bail!("TEST_FILE non défini ; la capture Windows arrive à la tâche 9"),
    };

    // `receiver_task`/`sender_task` : conservés par `SignalingHandle` pour ne
    // pas être abandonnés silencieusement (I6), mais cette tâche mono-session
    // n'a rien de plus à en faire une fois `closed` observé ci-dessous — on
    // les laisse donc détachés explicitement plutôt que de les ignorer par
    // accident.
    let signaling::SignalingHandle {
        mut offers,
        answers,
        mut closed,
        receiver_task: _,
        sender_task: _,
    } = signaling::run_signaling(&config.signaling_url, &config.session_id).await?;
    let mut session = Session::new(source, config.local_ip)?;

    let offer = offers
        .recv()
        .await
        .context("le signaling s'est fermé avant l'offre")?;
    tracing::info!("offre reçue");
    let answer = session.accept_offer(&offer)?;
    answers.send(answer).await?;
    tracing::info!("réponse envoyée");
    // Note : `AgentControl::ready` n'est plus envoyé ici. À cet instant SCTP
    // n'est pas encore ouvert (le canal de contrôle vaut encore `None`), donc
    // l'envoyer maintenant serait silencieusement perdu (I3 de la revue).
    // `Session` le met en file elle-même dès `Event::ChannelOpen("control")`.

    // I6 : une perte du signaling après l'échange initial doit être visible
    // plutôt que silencieuse. Le transport ne dépend plus du signaling une
    // fois l'offre/réponse échangées (pas de renégociation dans cette
    // tâche), donc on ne fait rien de plus qu'observer et journaliser — mais
    // on l'observe.
    tokio::spawn(async move {
        if closed.changed().await.is_ok() && *closed.borrow() {
            tracing::warn!(
                "connexion de signaling perdue (aucune renégociation possible pour cette session)"
            );
        }
    });

    // I6 : `Session::run` bloque volontairement (lecture UDP synchrone bornée
    // par la cadence vidéo et les échéances str0m). L'exécuter sur un ouvrier
    // async de tokio gèlerait les autres tâches de ce processus — ici, la
    // boucle d'émission du signaling — jusqu'à une seconde par tour, voire
    // beaucoup plus dès que la session n'est plus vivante. On la déplace donc
    // sur le pool de threads bloquants de tokio, dédié à cet usage.
    let transport = tokio::task::spawn_blocking(move || {
        let mut on_input = |message| tracing::debug!(?message, "entrée reçue");
        let mut on_control = |message| tracing::info!(?message, "contrôle reçu");
        session.run(&mut on_input, &mut on_control)
    });

    match transport.await {
        Ok(Ok(())) => tracing::info!("session terminée"),
        Ok(Err(e)) => {
            // `Session::run` ne remonte une erreur que pour un problème jugé
            // irrécupérable au niveau de la session (voir
            // `Session::begin_ending` pour ce qui est au contraire traité
            // comme une fin de session propre, via `Ok(())`).
            tracing::error!(erreur = %e, "erreur fatale dans la boucle de transport");
            return Err(e);
        }
        Err(join_err) => {
            tracing::error!(erreur = %join_err, "la boucle de transport a paniqué");
            return Err(join_err.into());
        }
    }

    Ok(())
}
