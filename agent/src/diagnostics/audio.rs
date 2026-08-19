//! Sondes audio du chantier A : périphérique de rendu de la session et format
//! de mixage (`AUDIO_PROBE`), et isolation de l'audio d'un seul processus
//! (`PROCESS_LOOPBACK_PROBE`, requis par le chantier D).
//!
//! ⚠️ **`AUDIO_PROBE` ne sonde plus « le périphérique par défaut » mais celui
//! que `LoopbackCapture::open` retient** — c'est-à-dire celui que désigne
//! `AUDIO_PERIPHERIQUE`, ou le défaut de Windows à défaut (correction
//! « A-bis », `wasapi/rendu.rs`). C'est ce qui en fait l'instrument de mesure
//! de cette correction : lancée deux fois, avec et sans la variable, elle
//! rend deux relevés opposés sur la même machine.

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
    // Fenêtre d'analyse spectrale : les DERNIÈRES `FENETRE_ANALYSE` valeurs
    // entrelacées. Bornée à dessein — accumuler dix secondes de son pour n'en
    // analyser que la fin coûterait de la mémoire sans rien apporter, et
    // garder le DÉBUT ferait juger la sonde sur ce qui précède la tonalité
    // qu'on vient de jouer.
    const FENETRE_ANALYSE: usize = 48_000 * 2 * 2; // 2 s de stéréo à 48 kHz
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

    // La CRÊTE distingue « du son » de « rien ». Elle ne distingue pas « MON
    // son » d'un autre — ce dépôt a établi au sous-bloc D7 que l'instrument
    // qui le fait est la FRÉQUENCE DOMINANTE. Les deux sont donc rendues, et
    // c'est la seconde qui juge (correction « A-bis »).
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

    // La seconde moitié journalise les MÊMES quatre champs que la première
    // (echantillons, lectures_vides, crete, silencieux), pas la seule crête.
    // Sans echantillons_apres/lectures_vides_apres, une crete_apres a 0 est
    // ambiguë entre trois causes bien distinctes : le flux a bien repris mais
    // la source est redevenue silencieuse (echantillons_apres > 0) ; le flux a
    // repris mais ne rend jamais rien (echantillons_apres = 0, lectures_vides_apres
    // proche du plafond) ; ou `demarrer()` a été refusé et cette boucle a
    // interrogé un flux resté arrêté pendant 5 s (même signature que le cas
    // précédent, mais pour une tout autre raison). Or c'est précisément cette
    // distinction que la tâche 3 doit trancher pour la décision du §4.4 de la
    // spec : un « `Start` accepté » ne garantit que le HRESULT, pas que
    // l'audio a réellement repris.
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
