//! Sondes audio du chantier A : périphérique de rendu par défaut de la
//! session et format de mixage (`AUDIO_PROBE`), et isolation de l'audio
//! d'un seul processus (`PROCESS_LOOPBACK_PROBE`, requis par le chantier D).

use anyhow::{Context, Result};

use crate::wasapi;

pub(super) fn executer_sonde_audio() -> Result<()> {
    let secondes: u64 = std::env::var("AUDIO_PROBE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let mut capture = wasapi::LoopbackCapture::open()?;
    tracing::info!(format = %capture.description(), "loopback ouvert");

    let debut = std::time::Instant::now();
    let mut echantillons = 0u64;
    let mut crete = 0i16;
    let mut lectures_vides = 0u64;
    while debut.elapsed() < std::time::Duration::from_secs(secondes) {
        match capture.read()? {
            Some(bloc) => {
                echantillons += bloc.len() as u64;
                for v in bloc {
                    crete = crete.max(v.saturating_abs());
                }
            }
            None => lectures_vides += 1,
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    tracing::info!(
        echantillons,
        lectures_vides,
        crete,
        silencieux = crete == 0,
        "sonde audio terminée"
    );
    Ok(())
}

pub(super) fn executer_process_loopback(pid_texte: &str) -> Result<()> {
    let pid: u32 = pid_texte
        .parse()
        .context("PROCESS_LOOPBACK_PROBE doit être un identifiant de processus")?;
    match wasapi::process_loopback::probe_process_loopback(pid) {
        Ok(rapport) => tracing::info!(pid, rapport, "sonde process loopback"),
        Err(e) => tracing::warn!(pid, erreur = %e, "sonde process loopback échouée"),
    }
    Ok(())
}

/// `PROCESS_LOOPBACK_CAPTURE=<pid>` — la mesure pivot du sous-bloc D7.
///
/// Va jusqu'où `probe_process_loopback` s'arrête : `Initialize`,
/// `GetService`, `Start`, et une lecture réelle. **Le relevé qui compte n'est
/// pas la crête non nulle** — une capture qui rendrait en réalité le mix global
/// la produirait aussi — **mais la crête NULLE pendant qu'un autre processus
/// joue.** Les deux moitiés se jouent par deux exécutions successives, et le
/// protocole est au §3 de la conception.
pub(super) fn executer_capture_process_loopback(pid_texte: &str) -> Result<()> {
    let pid: u32 = pid_texte
        .parse()
        .context("PROCESS_LOOPBACK_CAPTURE doit être un identifiant de processus")?;
    let secondes: u64 = std::env::var("PROCESS_LOOPBACK_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);

    let mut capture = wasapi::process_loopback::CaptureProcessus::ouvrir(pid)?;
    tracing::info!(pid, format = %capture.description(), "process loopback ouvert");
    capture.demarrer()?;

    let debut = std::time::Instant::now();
    let mut echantillons = 0u64;
    let mut crete = 0i16;
    let mut lectures_vides = 0u64;
    while debut.elapsed() < std::time::Duration::from_secs(secondes) {
        match capture.read()? {
            Some(bloc) => {
                echantillons += bloc.len() as u64;
                for v in bloc {
                    crete = crete.max(v.saturating_abs());
                }
            }
            None => lectures_vides += 1,
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    tracing::info!(
        pid,
        echantillons,
        lectures_vides,
        crete,
        silencieux = crete == 0,
        "sonde de capture process loopback : premiere moitie"
    );

    // Le cycle Stop/Start, dont dépend l'approche retenue au §4.4 de la spec.
    // Un refus ici fait replier sur l'approche B — c'est un relevé, pas un
    // incident.
    capture.arreter()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    match capture.demarrer() {
        Ok(()) => tracing::info!(pid, "cycle Stop puis Start accepte"),
        Err(e) => tracing::warn!(pid, erreur = %e, "cycle Stop puis Start REFUSE"),
    }

    let debut = std::time::Instant::now();
    let mut crete_apres = 0i16;
    while debut.elapsed() < std::time::Duration::from_secs(5) {
        if let Some(bloc) = capture.read()? {
            for v in bloc {
                crete_apres = crete_apres.max(v.saturating_abs());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    tracing::info!(pid, crete_apres, "sonde de capture process loopback terminee");
    Ok(())
}
