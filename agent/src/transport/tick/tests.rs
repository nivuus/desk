//! Tests du tick de transport, répartis par famille de branche.
//!
//! Découpé à la tâche 2 du sous-bloc D10 : le fichier était à 489 lignes
//! (marge 11) et la tâche 11 y ajoute la couverture de la reconstruction
//! audio. L'extraction précède l'addition — voir `CLAUDE.md`.
//!
//! Les aides partagées restent ici ; les `use super::*;` des deux enfants les
//! atteignent.

mod audio;
mod reste;

use super::*;
use crate::audio::{AudioPacket, AudioSource};
use crate::h264::AccessUnit;
use crate::source::VideoSource;

/// Session minimale pour les tests qui n'exercent qu'un sous-système isolé
/// (ici, la reconstruction audio) : vidéo de test, IP locale, horloge et
/// plafond arbitraires — aucun de ces choix n'est inspecté par les tests qui
/// l'emploient.
pub(super) fn session_d_essai() -> Session {
    let source = Box::new(crate::transport::fixtures::video_test_source());
    Session::new(
        source,
        crate::transport::fixtures::local_ip(),
        Instant::now(),
        12_000_000,
    )
    .expect("session")
}

/// Paquet Opus minimal, horodaté à l'instant présent — suffisant pour les
/// tests qui ne portent que sur le CHEMIN emprunté par un paquet, jamais sur
/// son contenu.
pub(super) fn paquet_d_essai() -> AudioPacket {
    AudioPacket {
        data: vec![0xAA],
        pts_48k: 0,
        captured_at: Instant::now(),
    }
}

/// Source factice qui enregistre les appels à `set_awake` et rend une
/// annonce de sommeil préparée une seule fois — comme le fait réellement
/// `SourceDistante` (`Option` consommé par `take()`), sans dépendre du
/// capteur : ce test vérifie le câblage des branches a1bis/a1ter
/// d'`act_on_timeout`, pas la logique de `SourceDistante` elle-même
/// (couverte par `capteur/distante/tests.rs`).
struct SourceAvecSommeil {
    inner: crate::source::FileSource,
    awake_recus: std::sync::Arc<std::sync::Mutex<Vec<(bool, bool)>>>,
    sommeil_prepare: Option<(bool, String)>,
}

impl VideoSource for SourceAvecSommeil {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn set_awake(&mut self, visible: bool, focalisee: bool) -> anyhow::Result<()> {
        self.awake_recus.lock().unwrap().push((visible, focalisee));
        Ok(())
    }
    fn sommeil_a_annoncer(&mut self) -> Option<(bool, String)> {
        self.sommeil_prepare.take()
    }
}

/// Source factice qui rend une part de budget préparée une seule fois —
/// comme le fait réellement `SourceDistante` (`Option` consommé par
/// `take()`), sans dépendre du capteur : ce test vérifie le CÂBLAGE de la
/// branche a1quater d'`act_on_timeout`, pas la logique de `SourceDistante`
/// elle-même (couverte par `capteur/distante/tests.rs`) ni celle de
/// `Session::appliquer_part` (couverte par `transport/part.rs`).
struct SourceAvecPart {
    inner: crate::source::FileSource,
    part_preparee: Option<u32>,
}

impl VideoSource for SourceAvecPart {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn part_a_appliquer(&mut self) -> Option<u32> {
        self.part_preparee.take()
    }
}

/// Source audio factice pilotable de l'extérieur (`Arc<AtomicBool>`) : rend
/// `capture_morte()` sur commande, sans jamais produire de paquet — ce test
/// n'exerce que la DÉTECTION (branche a1sexies), pas l'émission audio.
struct AudioSourceMortelle {
    morte: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AudioSource for AudioSourceMortelle {
    fn next_packet(&mut self) -> Option<AudioPacket> {
        None
    }
    fn capture_morte(&self) -> bool {
        self.morte.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Source vidéo factice qui compte les appels à `signaler_audio_mort` et
/// rend un rattachement piloté de l'extérieur — même patron que
/// `SourceAvecSommeil`/`SourceAvecPart` ci-dessus : ce test vérifie le
/// CÂBLAGE de la branche a1sexies d'`act_on_timeout` (et sa remise à zéro
/// par a1sexies elle-même), pas la logique de `SourceDistante`, couverte par
/// `capteur/distante/tests.rs`.
struct SourceAvecAudioMort {
    inner: crate::source::FileSource,
    signalements: std::sync::Arc<std::sync::Mutex<u32>>,
    rattachement_prepare: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl VideoSource for SourceAvecAudioMort {
    fn next_frame(&mut self) -> Option<AccessUnit> {
        self.inner.next_frame()
    }
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }
    fn signaler_audio_mort(&mut self) {
        *self.signalements.lock().unwrap() += 1;
    }
    fn rattachement_survenu(&mut self) -> bool {
        // `swap`, pas une simple lecture : CONSOMMÉ, sur le même régime que
        // `Option::take()` dans `SourceAvecSommeil`/`SourceAvecPart` — sans
        // quoi ce drapeau resterait vrai indéfiniment et masquerait le
        // défaut que ce test existe pour attraper (un verrou qui ne se
        // remettrait jamais à zéro serait, à l'identique, invisible).
        self.rattachement_prepare.swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}
