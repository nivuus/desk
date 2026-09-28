//! Audio probes of work stream A: the session's render device and mix
//! format (`AUDIO_PROBE`), and isolating the audio of a single process
//! (`PROCESS_LOOPBACK_PROBE`, required by work stream D).
//!
//! ⚠️ **`AUDIO_PROBE` no longer probes "the default device" but the one
//! `LoopbackCapture::open` keeps** — that is, the one designated by
//! `AUDIO_PERIPHERIQUE`, or Windows' default otherwise ("A-bis"
//! fix, `wasapi/rendu.rs`). That is what makes it the measuring instrument
//! of this fix: run twice, with and without the variable, it
//! returns two opposite readings on the same machine.

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
    // Spectral analysis window: the LAST `FENETRE_ANALYSE` interleaved
    // values. Bounded on purpose — accumulating ten seconds of sound to only
    // analyse the end would cost memory without bringing anything, and
    // keeping the BEGINNING would judge the probe on what precedes the tone
    // just played.
    const FENETRE_ANALYSE: usize = 48_000 * 2 * 2; // 2 s of stereo at 48 kHz
    let mut fenetre: std::collections::VecDeque<i16> = std::collections::VecDeque::new();
    while debut.elapsed() < std::time::Duration::from_secs(secondes) {
        match capture.read()? {
            Some(bloc) => {
                echantillons += bloc.len() as u64;
                for v in bloc {
                    crete = crete.max(v.saturating_abs());
                    fenetre.push_back(v);
                    if fenetre.len() > FENETRE_ANALYSE {
                        fenetre.pop_front();
                    }
                }
            }
            None => lectures_vides += 1,
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // The PEAK distinguishes "sound" from "nothing". It does not distinguish "MY
    // sound" from another — this repository established in sub-block D7 that the instrument
    // that does is the DOMINANT FREQUENCY. Both are therefore returned, and
    // it is the second that judges ("A-bis" fix).
    let mono = crate::spectre::mono(&Vec::from(fenetre), 2);
    let dominante = crate::spectre::dominante(&mono, 48_000.0, 100.0, 4_000.0, 1.0);

    tracing::info!(
        echantillons,
        lectures_vides,
        crete,
        silencieux = crete == 0,
        dominante_hz = dominante.map(|d| d.frequence_hz),
        dominante_magnitude = dominante.map(|d| d.magnitude),
        resolution_hz = dominante.map(|d| d.resolution_hz),
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
        Err(e) => tracing::warn!(pid, error = %e, "sonde process loopback échouée"),
    }
    Ok(())
}

/// `PROCESS_LOOPBACK_CAPTURE=<pid>` — the pivotal measurement of sub-block D7.
///
/// Goes as far as where `probe_process_loopback` stops: `Initialize`,
/// `GetService`, `Start`, and a real read. **The reading that matters is
/// not the non-zero peak** — a capture that actually returned the global mix
/// would produce it too — **but the ZERO peak while another process
/// plays.** The two halves are played by two successive runs, and the
/// protocol is in §3 of the design.
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
    capture.start()?;

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

    // The Stop/Start cycle, on which the approach chosen in §4.4 of the spec depends.
    // A refusal here falls back on approach B — it is a reading, not an
    // incident.
    capture.arreter()?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    match capture.start() {
        Ok(()) => tracing::info!(pid, "cycle Stop puis Start accepte"),
        Err(e) => tracing::warn!(pid, error = %e, "cycle Stop puis Start REFUSE"),
    }

    // The second half logs the SAME four fields as the first
    // (samples, empty reads, peak, silent), not the peak alone.
    // Without echantillons_apres/lectures_vides_apres, a crete_apres at 0 is
    // ambiguous between three quite distinct causes: the stream did resume but
    // the source went silent again (echantillons_apres > 0); the stream
    // resumed but never returns anything (echantillons_apres = 0, lectures_vides_apres
    // close to the cap); or `start()` was refused and this loop
    // polled a stream that stayed stopped for 5 s (same signature as the
    // previous case, but for a completely different reason). Yet it is precisely this
    // distinction that task 3 must settle for the decision of §4.4 of the
    // spec: a "`Start` accepted" only guarantees the HRESULT, not that
    // audio has actually resumed.
    let debut = std::time::Instant::now();
    let mut echantillons_apres = 0u64;
    let mut lectures_vides_apres = 0u64;
    let mut crete_apres = 0i16;
    while debut.elapsed() < std::time::Duration::from_secs(5) {
        match capture.read()? {
            Some(bloc) => {
                echantillons_apres += bloc.len() as u64;
                for v in bloc {
                    crete_apres = crete_apres.max(v.saturating_abs());
                }
            }
            None => lectures_vides_apres += 1,
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    tracing::info!(
        pid,
        echantillons_apres,
        lectures_vides_apres,
        crete_apres,
        silencieux_apres = crete_apres == 0,
        "sonde de capture process loopback terminee"
    );
    Ok(())
}
