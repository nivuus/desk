//! Source audio Windows : capture loopback, découpage, encodage Opus.
//!
//! Tout se passe sur un fil dédié, qui dépose dans un tampon circulaire borné.
//! La boucle de transport n'y fait qu'un retrait non bloquant par tour : elle
//! ne doit jamais attendre WASAPI ni l'encodeur.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::audio::{AudioPacket, AudioSource, PacketRing};
use crate::frames::FrameAssembler;
use crate::opus::OpusEncoder;
use crate::wasapi::LoopbackCapture;

/// Profondeur du tampon partagé, en paquets de 10 ms. 10 paquets = 100 ms :
/// assez pour absorber un tour de boucle en retard, trop peu pour que la
/// latence s'installe.
const RING_CAPACITY: usize = 10;

/// Intervalle de sondage du fil de capture. Deux fois plus rapide que la
/// cadence des trames : la capture ne doit jamais être le facteur limitant.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Intervalle entre deux journaux de compteurs agrégés.
const REPORT_INTERVAL: Duration = Duration::from_secs(30);

pub struct WindowsAudioSource {
    ring: PacketRing,
    arret: Arc<AtomicBool>,
    description: String,
}

impl WindowsAudioSource {
    /// Ouvre la capture et démarre le fil de production.
    ///
    /// `origin` est l'origine d'horloge **de la session**, partagée avec la
    /// source vidéo : c'est elle qui rend les deux lignes de temps
    /// comparables, donc la synchro A/V exacte.
    pub fn new(origin: Instant) -> Result<Self> {
        let mut capture = LoopbackCapture::open().context("ouverture du loopback audio")?;
        let description = capture.description();
        let mut encodeur = OpusEncoder::new().context("création de l'encodeur Opus")?;

        let ring = PacketRing::new(RING_CAPACITY);
        let arret = Arc::new(AtomicBool::new(false));

        let ring_fil = ring.clone();
        let arret_fil = Arc::clone(&arret);
        std::thread::Builder::new()
            .name("audio-capture".into())
            .spawn(move || {
                // Ce fil appelle lui-même des méthodes COM — `read()` à chaque
                // tour, et `Stop()` via le `Drop` de `LoopbackCapture` en
                // sortant — alors que `open()` a initialisé COM sur le fil
                // APPELANT, pas sur celui-ci. Microsoft exige que tout fil
                // invoquant des méthodes COM ait d'abord rejoint un
                // appartement.
                //
                // Résultat volontairement ignoré ICI — contrairement à
                // `wasapi::open`, qui lui **vérifie** son `HRESULT` et refuse
                // `RPC_E_CHANGED_MODE` (voir son commentaire, dont dépend
                // `unsafe impl Send for LoopbackCapture`) : ce fil-ci vient
                // d'être créé par `thread::Builder::spawn` juste au-dessus,
                // il n'a donc encore rejoint aucun appartement COM, et
                // `CoInitializeEx` y rend nécessairement `S_OK`. `open()`,
                // lui, s'exécute sur un fil quelconque — potentiellement
                // recyclé, potentiellement déjà lié à une STA — d'où la
                // vérification qui n'a pas lieu d'être répétée ici.
                //
                // Symétriquement, PAS de `CoUninitialize` : voir le motif
                // détaillé dans `wasapi.rs`.
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }

                let mut assembleur = FrameAssembler::new(origin);
                let mut dernier_rapport = Instant::now();

                while !arret_fil.load(Ordering::Relaxed) {
                    match capture.read() {
                        Ok(Some(bloc)) => assembleur.push(&bloc),
                        Ok(None) => {}
                        Err(e) => {
                            // Une erreur de lecture ne doit pas tuer la
                            // session : on journalise et on arrête l'audio.
                            // La vidéo continue. Les compteurs sont inclus
                            // ici parce que c'est la dernière ligne de log de
                            // ce fil : sans eux, une capture morte en cours
                            // de session serait indiscernable d'un simple
                            // silence — `next_packet` continuerait à rendre
                            // `None` comme dans le cas nominal.
                            tracing::warn!(
                                erreur = %e,
                                rejetes = ring_fil.rejetes(),
                                complements = assembleur.complements(),
                                echantillons_jetes = assembleur.echantillons_jetes(),
                                "lecture audio échouée, capture arrêtée définitivement"
                            );
                            return;
                        }
                    }

                    for trame in assembleur.drain_due(Instant::now()) {
                        match encodeur.encode(&trame.pcm) {
                            Ok(data) => ring_fil.push(AudioPacket {
                                data,
                                pts_48k: trame.pts_48k,
                                captured_at: trame.captured_at,
                            }),
                            Err(e) => {
                                // Même raisonnement que pour l'erreur de
                                // lecture ci-dessus : dernière ligne de log
                                // de ce fil, donc dernière chance de rendre
                                // les compteurs accumulés exploitables.
                                tracing::warn!(
                                    erreur = %e,
                                    rejetes = ring_fil.rejetes(),
                                    complements = assembleur.complements(),
                                    echantillons_jetes = assembleur.echantillons_jetes(),
                                    "encodage Opus échoué, capture arrêtée définitivement"
                                );
                                return;
                            }
                        }
                    }

                    if dernier_rapport.elapsed() >= REPORT_INTERVAL {
                        dernier_rapport = Instant::now();
                        // `info!`, pas `debug!` : le filtre par défaut
                        // (`agent/src/main.rs`, `EnvFilter` replié sur
                        // `"info"` en l'absence de `RUST_LOG`) n'émet jamais
                        // les journaux `debug!` en exploitation normale. La
                        // spec (§5, §9) promet des compteurs « journalisés
                        // périodiquement et jamais silencieux » — un
                        // enregistrement toutes les `REPORT_INTERVAL` (30 s)
                        // n'est pas du bruit, et un compteur de rejets muet
                        // est exactement ce qui rendrait une dégradation
                        // audio invisible en recette.
                        tracing::info!(
                            rejetes = ring_fil.rejetes(),
                            complements = assembleur.complements(),
                            echantillons_jetes = assembleur.echantillons_jetes(),
                            "compteurs audio"
                        );
                    }

                    std::thread::sleep(POLL_INTERVAL);
                }
            })
            .context("démarrage du fil de capture audio")?;

        tracing::info!(format = %description, "source audio démarrée");
        Ok(Self {
            ring,
            arret,
            description,
        })
    }

    /// Format de mixage obtenu, pour le journal.
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl AudioSource for WindowsAudioSource {
    fn next_packet(&mut self) -> Option<AudioPacket> {
        self.ring.pop()
    }
}

impl Drop for WindowsAudioSource {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
    }
}
