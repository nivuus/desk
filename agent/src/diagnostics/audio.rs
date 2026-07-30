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
    match wasapi::probe_process_loopback(pid) {
        Ok(rapport) => tracing::info!(pid, rapport, "sonde process loopback"),
        Err(e) => tracing::warn!(pid, erreur = %e, "sonde process loopback échouée"),
    }
    Ok(())
}
