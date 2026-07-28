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

// SAFETY : `LoopbackCapture` encapsule des interfaces COM (`IAudioClient`,
// `IAudioCaptureClient`) que `windows-core` ne marque `Send` dans aucun cas
// général — un objet COM quelconque peut être lié à un appartement
// mono-thread (STA), auquel cas le faire migrer entre fils serait dangereux.
// Ce n'est pas le cas ici : `wasapi::open()` rejoint explicitement
// l'appartement multi-thread (MTA) via `COINIT_MULTITHREADED`, et le fil de
// capture ci-dessous y rejoint la même MTA avant tout appel COM. Les objets
// créés dans une MTA sont par construction appelables depuis n'importe quel
// fil qui en est membre, sans marshaling — c'est précisément pourquoi ce
// design a été choisi, et le commentaire de `wasapi::open` documente déjà ce
// transfert vers « un fil de capture dédié » comme le fonctionnement prévu de
// cette tâche. Sans cet impl, `LoopbackCapture` ne peut pas être déplacé dans
// la fermeture du fil ci-dessous : le compilateur refuse par défaut, faute de
// pouvoir distinguer ce cas MTA du cas STA général.
unsafe impl Send for LoopbackCapture {}

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
                // appartement. L'omettre « marche » en pratique pour des
                // objets in-process appelés par vtable directe, ce qui rend le
                // manquement parfaitement invisible en test de fumée — et
                // c'est bien ce qui le rend dangereux : rien ne le garantit.
                //
                // Résultat volontairement ignoré, comme dans `wasapi::open` :
                // rejoindre la MTA deux fois depuis deux fils distincts est le
                // fonctionnement normal, et `RPC_E_CHANGED_MODE` signifierait
                // seulement que ce fil appartient déjà à un autre modèle.
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
                            // La vidéo continue.
                            tracing::warn!(erreur = %e, "lecture audio échouée, capture arrêtée");
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
                                tracing::warn!(erreur = %e, "encodage Opus échoué, capture arrêtée");
                                return;
                            }
                        }
                    }

                    if dernier_rapport.elapsed() >= REPORT_INTERVAL {
                        dernier_rapport = Instant::now();
                        tracing::debug!(
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
