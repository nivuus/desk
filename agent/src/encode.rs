//! Encodeur H.264 matériel via Media Foundation.
//!
//! Les MFT matérielles sont asynchrones : le pilotage se fait par événements
//! (`METransformNeedInput` / `METransformHaveOutput`) et non par une boucle
//! `ProcessInput`/`ProcessOutput` synchrone. Cette contrainte est imposée par
//! Media Foundation, pas par un choix de conception.
//!
//! **Écart au brief d'origine, vérifié empiriquement sur la VM cible** : la
//! capture (`crate::capture`) fournit des textures `DXGI_FORMAT_B8G8R8A8_UNORM`
//! (BGRA), mais l'encodeur `NVIDIA H.264 Encoder MFT` n'annonce QUE `NV12` en
//! type d'entrée disponible (voir `log_supported_input_types`, qui journalise
//! la liste réelle renvoyée par `GetInputAvailableType` au démarrage — aucune
//! variante RGB/ARGB n'y figure). Envelopper directement la texture BGRA dans
//! un échantillon annoncé NV12 produirait un échec de `ProcessInput` ou, pire,
//! une image corrompue interprétée avec le mauvais plan de couleur. On insère
//! donc un convertisseur GPU (`CLSID_VideoProcessorMFT`, catégorie
//! `MFT_CATEGORY_VIDEO_PROCESSOR`) entre la capture et l'encodeur : c'est une
//! MFT *synchrone* (à la différence de l'encodeur), pilotée par une simple
//! paire `ProcessInput`/`ProcessOutput`, qui convertit BGRA→NV12 sans quitter
//! le GPU (le manager de périphérique D3D est partagé avec elle comme avec
//! l'encodeur).
//!
//! **Correction du 28/07 : le plafond à ~1 image/s, puis l'arrêt total du
//! pipeline, venaient d'une fuite de références COM sur les échantillons de
//! sortie.** Voir `take_output_sample` pour le mécanisme exact, et
//! `feed_converter` pour le modèle de drainage qui en découle. En résumé :
//! `MFT_OUTPUT_DATA_BUFFER::pSample` est un `ManuallyDrop` dont la référence
//! n'était jamais relâchée, si bien que les échantillons du convertisseur ne
//! retournaient jamais à son `IMFVideoSampleAllocator` ; passé les 10
//! échantillons du pool, chaque `ProcessOutput` attendait une seconde entière
//! avant de rendre `MF_E_SAMPLEALLOCATOR_EMPTY`. Les deux « corrections »
//! précédentes (boucle de drainage jusqu'à `MF_E_TRANSFORM_NEED_MORE_INPUT`,
//! puis report du drainage au tour suivant) traitaient les symptômes de cette
//! fuite et l'ont aggravée jusqu'au blocage complet.
//!
//! Ces conclusions ne sont pas déduites du code mais mesurées : le chemin
//! chaud publie son étape courante et ses compteurs dans `EncoderTelemetry`,
//! qu'un fil de surveillance journalise chaque seconde (voir `main.rs`,
//! `ENCODER_THROUGHPUT_TEST`). Le débit mesuré est consigné dans le rapport
//! de tâche.

#![cfg(windows)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use windows::core::{Interface, PWSTR, GUID};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_INPROC_SERVER};
// écart d'API windows-rs 0.62 : `VARIANT_TRUE`/`VARIANT_FALSE` vivent dans
// `Win32::Foundation` (constantes `VARIANT_BOOL`), pas dans
// `Win32::System::Variant` où on les attendrait à côté du reste du type
// `VARIANT` — confirmé en lisant les sources de la crate sur la VM.
use windows::Win32::Foundation::{VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4};

use crate::capture::CapturedFrame;
use crate::h264::{group_access_units, AccessUnit};

/// Identifiants d'événements des MFT asynchrones (repris des constantes
/// fournies par le crate plutôt que dupliqués en dur, comme suggéré par le
/// brief).
const ME_TRANSFORM_NEED_INPUT: u32 = METransformNeedInput.0 as u32;
const ME_TRANSFORM_HAVE_OUTPUT: u32 = METransformHaveOutput.0 as u32;

/// Au-delà de cette durée, un appel Media Foundation du chemin chaud est
/// journalisé : à 60 im/s, une image entière tient dans ~16 ms, donc tout
/// appel qui dépasse ce seuil est déjà un incident, pas du bruit.
const SLOW_CALL: std::time::Duration = std::time::Duration::from_millis(50);

/// Nombre maximal de sorties retirées d'affilée pour rendre le convertisseur
/// preneur d'une nouvelle entrée (voir `feed_converter`). Strictement borné :
/// ce MFT annonce une sortie prête en permanence, donc une boucle sans borne
/// viderait son pool et bloquerait une seconde entière sur
/// `MF_E_SAMPLEALLOCATOR_EMPTY`. En régime nominal, une itération suffit.
const MAX_CONVERTER_COLLECTS: usize = 4;

/// Étapes du chemin chaud, publiées dans `EncoderTelemetry::phase` avant
/// chaque appel Media Foundation et remises à `PHASE_IDLE` juste après.
///
/// Raison d'être : un appel Media Foundation qui ne rend jamais la main est
/// invisible pour toute trace posée *autour* de lui (la ligne « après » n'est
/// jamais atteinte, la ligne « avant » se noie dans le flot). Publier l'étape
/// courante dans un entier atomique permet à un fil de surveillance extérieur
/// de nommer précisément l'appel bloqué pendant qu'il l'est encore.
pub const PHASE_IDLE: u64 = 0;
pub const PHASE_SUBMIT_DRAIN_EVENTS: u64 = 1;
pub const PHASE_CONVERTER_PROCESS_INPUT: u64 = 2;
pub const PHASE_CONVERTER_PROCESS_OUTPUT: u64 = 3;
pub const PHASE_ENCODER_PROCESS_INPUT: u64 = 4;
pub const PHASE_POLL_DRAIN_EVENTS: u64 = 5;
pub const PHASE_ENCODER_PROCESS_OUTPUT: u64 = 6;
pub const PHASE_ENCODER_READ_BUFFER: u64 = 7;

/// Nom lisible d'une étape, pour les journaux de surveillance.
pub fn phase_name(phase: u64) -> &'static str {
    match phase {
        PHASE_IDLE => "repos",
        PHASE_SUBMIT_DRAIN_EVENTS => "submit/GetEvent",
        PHASE_CONVERTER_PROCESS_INPUT => "convertisseur/ProcessInput",
        PHASE_CONVERTER_PROCESS_OUTPUT => "convertisseur/ProcessOutput",
        PHASE_ENCODER_PROCESS_INPUT => "encodeur/ProcessInput",
        PHASE_POLL_DRAIN_EVENTS => "poll_output/GetEvent",
        PHASE_ENCODER_PROCESS_OUTPUT => "encodeur/ProcessOutput",
        PHASE_ENCODER_READ_BUFFER => "encodeur/lecture du tampon",
        _ => "inconnu",
    }
}

/// Compteurs du chemin chaud, tous atomiques pour rester lisibles **pendant**
/// qu'un appel Media Foundation est en cours — c'est exactement le cas qu'on
/// cherche à diagnostiquer. Le coût est nul en pratique (écritures `Relaxed`
/// sur des entiers déjà en cache).
#[derive(Default)]
pub struct EncoderTelemetry {
    /// Étape courante (voir les constantes `PHASE_*`).
    pub phase: AtomicU64,
    /// Nombre d'appels à `submit` entrés (pas nécessairement sortis).
    pub submit_calls: AtomicU64,
    /// Images BGRA effectivement remises au convertisseur.
    pub converter_inputs: AtomicU64,
    /// Échantillons NV12 effectivement obtenus du convertisseur.
    pub converter_outputs: AtomicU64,
    /// Images NV12 effectivement remises à l'encodeur.
    pub encoder_inputs: AtomicU64,
    /// Unités d'accès effectivement obtenues de l'encodeur.
    pub encoder_outputs: AtomicU64,
    /// Événements `METransformNeedInput` reçus depuis le démarrage.
    pub need_input_events: AtomicU64,
    /// Événements `METransformHaveOutput` reçus depuis le démarrage.
    pub have_output_events: AtomicU64,
    /// Longueur courante de `pending_nv12`.
    pub queued_nv12: AtomicU64,
    /// Valeur courante de `pending_input_requests`.
    pub pending_input_requests: AtomicU64,
    /// Occurrences de `MF_E_SAMPLEALLOCATOR_EMPTY`.
    pub skipped_busy: AtomicU64,
    /// 1 si une sortie du convertisseur reste à retirer.
    pub awaiting_drain: AtomicU64,
    /// Dernier `GetInputStatus` du convertisseur : bit 0 = `ACCEPT_DATA`,
    /// `u64::MAX` si la méthode n'est pas implémentée par ce MFT.
    pub converter_input_status: AtomicU64,
    /// Dernier `GetOutputStatus` du convertisseur : bit 0 = `SAMPLE_READY`,
    /// `u64::MAX` si la méthode n'est pas implémentée par ce MFT.
    pub converter_output_status: AtomicU64,
    /// Entrées refusées par le convertisseur (`MF_E_NOTACCEPTING`).
    pub converter_not_accepting: AtomicU64,
}

/// Résultat d'un appel à `ProcessOutput` sur le convertisseur BGRA→NV12 (voir
/// `H264Encoder::drain_converter_output`).
enum ConverterPoll {
    /// Un échantillon converti est disponible.
    Sample(IMFSample),
    /// `MF_E_TRANSFORM_NEED_MORE_INPUT` : rien de plus à produire pour
    /// l'instant, le convertisseur est prêt pour une nouvelle entrée.
    NeedMoreInput,
    /// `MF_E_SAMPLEALLOCATOR_EMPTY` : pool de sortie momentanément épuisé.
    /// Signal de contre-pression documenté par Media Foundation, pas une
    /// panne (voir `drain_converter_output`).
    Busy,
}

pub struct H264Encoder {
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    device_manager: IMFDXGIDeviceManager,
    /// Convertisseur BGRA→NV12 (MFT synchrone). Toujours présent : la capture
    /// ne produit que du BGRA, l'encodeur n'accepte que du NV12 (voir le
    /// commentaire de module).
    converter: IMFTransform,
    /// Vrai si le convertisseur alloue lui-même ses échantillons de sortie
    /// (`MFT_OUTPUT_STREAM_PROVIDES_SAMPLES`). Déterminé une fois à la
    /// construction : fournir un échantillon alors que le flag est positionné
    /// (ou l'inverse) est une erreur `ProcessOutput` documentée par MF.
    converter_provides_samples: bool,
    /// Périphérique D3D11 de la capture, conservé pour allouer des textures
    /// NV12 de sortie quand le convertisseur ne s'auto-alloue pas.
    device: ID3D11Device,
    /// Horodatages (temps, durée) des entrées BGRA soumises au convertisseur
    /// mais dont la sortie n'a pas encore été récupérée, dans l'ordre de
    /// soumission. Un convertisseur vidéo ne réordonne jamais les images :
    /// la sortie la plus ancienne pas encore récupérée correspond toujours
    /// à l'entrée la plus ancienne pas encore ressortie (voir la ronde de
    /// correction 1/5 — le convertisseur peut rendre, lors du drainage
    /// d'une entrée, la sortie d'une entrée antérieure encore en attente ;
    /// il faut alors lui associer SON horodatage d'origine, pas celui de
    /// l'entrée qui vient d'être soumise).
    pending_conversion_timestamps: VecDeque<(i64, i64)>,
    /// Échantillons NV12 déjà produits par le convertisseur mais pas encore
    /// soumis à l'encodeur (celui-ci n'en réclamait pas encore).
    pending_nv12: VecDeque<IMFSample>,
    /// Vrai si le convertisseur doit encore rendre la sortie d'une entrée déjà
    /// consommée. Purement diagnostic depuis le 28/07 (publié en
    /// `awaiting_drain`) : le pilotage s'appuie désormais sur `GetInputStatus`,
    /// qui décrit l'état réel du convertisseur au lieu de le déduire.
    converter_output_pending: bool,
    width: u32,
    height: u32,
    fps: u32,
    /// Nombre de demandes d'entrée non encore satisfaites.
    pending_input_requests: u32,
    /// Nombre d'images prêtes à être récupérées.
    pending_outputs: u32,
    /// Compteur diagnostic : occasions où le pool de sortie du convertisseur
    /// était momentanément épuisé (voir `collect_converter_output`). Exposé
    /// pour mesurer l'ampleur réelle de ce contournement, pas consommé par
    /// la logique de pilotage elle-même.
    skipped_busy: u64,
    /// Compteurs et étape courante, lisibles depuis un autre fil (voir
    /// `EncoderTelemetry`).
    telemetry: Arc<EncoderTelemetry>,
    /// Initialisation de Media Foundation, appariée par RAII.
    ///
    /// **Déclaré en dernier volontairement** : les champs sont détruits dans
    /// l'ordre de déclaration, après l'exécution de `Drop for H264Encoder`.
    /// `MFShutdown` doit venir après la libération des MFT et du gestionnaire
    /// DXGI, pas avant.
    _media_foundation: MediaFoundationSession,
}

/// Garde RAII sur l'initialisation de Media Foundation.
///
/// `MFStartup` et `MFShutdown` doivent être appariés. Les placer
/// respectivement en tête de `H264Encoder::new` et dans `Drop for H264Encoder`
/// ne les apparie **que si la construction réussit** : entre les deux, une
/// douzaine de `?` peuvent sortir sans que l'objet n'existe jamais — donc sans
/// que `Drop` ne s'exécute, donc sans `MFShutdown`. Chaque échec de
/// construction fuyait un appel.
///
/// Latent tant qu'aucune reprise n'existe (un échec de construction terminait
/// le processus), mais la tâche 11 en introduira : reconstruction de
/// l'encodeur au redimensionnement, reprise après erreur. Une garde règle le
/// problème pour tous les chemins de sortie, présents et à venir, sans avoir à
/// se souvenir d'en ajouter un à chaque nouveau `?`.
struct MediaFoundationSession;

impl MediaFoundationSession {
    fn start() -> Result<Self> {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }
            .context("démarrage de Media Foundation")?;
        Ok(Self)
    }
}

impl Drop for MediaFoundationSession {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

impl H264Encoder {
    pub fn new(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        // À partir d'ici, tout `?` relâche Media Foundation par la garde.
        let media_foundation = MediaFoundationSession::start()?;

        let transform = find_hardware_encoder()?;
        let attributes = unsafe { transform.GetAttributes() }?;

        // Débloquer le mode asynchrone : obligatoire pour toute MFT matérielle.
        let is_async = unsafe { attributes.GetUINT32(&MF_TRANSFORM_ASYNC) }.unwrap_or(0);
        if is_async == 0 {
            bail!("l'encodeur trouvé n'est pas asynchrone : configuration inattendue");
        }
        unsafe { attributes.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) }?;
        // Mode faible latence : pas de mise en tampon multi-images.
        unsafe { attributes.SetUINT32(&MF_LOW_LATENCY, 1) }?;

        // Preuve empirique (voir commentaire de module) : la liste réelle des
        // types d'entrée annoncés par cet encodeur, avant toute configuration.
        log_supported_input_types(&transform);

        // Partager le périphérique D3D11 pour recevoir des textures GPU.
        let device_manager = share_device(device)?;
        unsafe {
            transform.ProcessMessage(
                MFT_MESSAGE_SET_D3D_MANAGER,
                device_manager.as_raw() as usize,
            )
        }
        .context("partage du périphérique D3D avec l'encodeur")?;

        configure_output(&transform, width, height, fps, bitrate)?;
        configure_input(&transform, width, height, fps)?;
        configure_rate_control(&transform, bitrate)?;

        let events: IMFMediaEventGenerator = transform.cast()?;

        unsafe {
            transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }

        // Convertisseur BGRA→NV12, partageant le même périphérique D3D.
        let converter = create_color_converter(&device_manager, width, height, fps)?;
        let converter_stream_info = unsafe { converter.GetOutputStreamInfo(0) }
            .context("interrogation du flux de sortie du convertisseur")?;
        let converter_provides_samples = converter_stream_info.dwFlags
            & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32
            != 0;
        tracing::info!(
            converter_provides_samples,
            "convertisseur BGRA→NV12 (Video Processor MFT) configuré"
        );
        // Essai mené (investigation débit) : fournir systématiquement notre
        // propre échantillon de sortie, y compris quand
        // `converter_provides_samples` est vrai, pour voir si cela évite
        // l'attente d'~1 s mesurée dans `drain_converter_output`. Rejeté
        // immédiatement par le convertisseur (`Output Sample is Invalid`,
        // `0x80070057`) : le contrat documenté (ne jamais fournir de tampon
        // quand ce drapeau est positionné) doit être respecté, il n'y a pas
        // de contournement possible ici.
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0) }?;
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0) }?;

        Ok(Self {
            transform,
            events,
            device_manager,
            converter,
            converter_provides_samples,
            device: device.clone(),
            pending_conversion_timestamps: VecDeque::new(),
            pending_nv12: VecDeque::new(),
            converter_output_pending: false,
            skipped_busy: 0,
            telemetry: Arc::new(EncoderTelemetry::default()),
            _media_foundation: media_foundation,
            width,
            height,
            fps,
            pending_input_requests: 0,
            pending_outputs: 0,
        })
    }

    /// Poignée de télémétrie, à partager avec un fil de surveillance.
    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        self.telemetry.clone()
    }

    /// Recopie les compteurs « d'état » (longueurs de file) dans la
    /// télémétrie. Appelée aux points de respiration du chemin chaud.
    fn publish_state(&self) {
        self.telemetry
            .queued_nv12
            .store(self.pending_nv12.len() as u64, Ordering::Relaxed);
        self.telemetry
            .pending_input_requests
            .store(self.pending_input_requests as u64, Ordering::Relaxed);
        self.telemetry
            .skipped_busy
            .store(self.skipped_busy, Ordering::Relaxed);
        self.telemetry.awaiting_drain.store(
            self.converter_output_pending as u64,
            Ordering::Relaxed,
        );
    }

    /// Draine les événements disponibles sans bloquer.
    fn drain_events(&mut self) -> Result<()> {
        loop {
            // MF_EVENT_FLAG_NO_WAIT : renvoie immédiatement s'il n'y a rien.
            let event = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(event) => event,
                Err(_) => break, // file vide
            };
            let kind = unsafe { event.GetType() }?;
            match kind {
                ME_TRANSFORM_NEED_INPUT => {
                    self.pending_input_requests += 1;
                    self.telemetry
                        .need_input_events
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::trace!(
                        pending_input_requests = self.pending_input_requests,
                        "événement METransformNeedInput reçu"
                    );
                }
                ME_TRANSFORM_HAVE_OUTPUT => {
                    self.pending_outputs += 1;
                    self.telemetry
                        .have_output_events
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::trace!(
                        pending_outputs = self.pending_outputs,
                        "événement METransformHaveOutput reçu"
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Convertit **une** texture BGRA capturée en **un** échantillon NV12,
    /// empilé dans `pending_nv12`. Contrairement à l'encodeur, ce transform se
    /// pilote par une paire `ProcessInput`/`ProcessOutput` classique : pas
    /// d'événements à suivre.
    ///
    /// **Correction du 28/07 — modèle de drainage revu, mesures à l'appui.**
    /// La ronde précédente avait remplacé un `ProcessOutput` unique par une
    /// boucle « jusqu'à observer réellement `MF_E_TRANSFORM_NEED_MORE_INPUT` »,
    /// en s'appuyant sur le « Basic MFT Processing Model » de Microsoft. La
    /// télémétrie (voir `EncoderTelemetry`) montre que ce MFT-ci **ne renvoie
    /// jamais ce code** : avec seulement 2 entrées soumises, la boucle a tiré
    /// 10 échantillons de sortie, puis 9 de plus par seconde — c'est-à-dire
    /// autant d'échantillons que son `IMFVideoSampleAllocator` en contient,
    /// après quoi `ProcessOutput` bloque une seconde entière avant de rendre
    /// `MF_E_SAMPLEALLOCATOR_EMPTY`. Boucler ne « draine » donc pas ce
    /// convertisseur : ça vide son pool, ça duplique des images qui n'ont
    /// jamais été soumises, et ça impose une seconde d'attente par tour.
    ///
    /// Le pilotage correct pour ce transform 1-entrée/1-sortie est celui
    /// d'origine : un `ProcessOutput` par `ProcessInput`. Ce qui faisait
    /// échouer cette version-là avec `MF_E_NOTACCEPTING` n'était pas le
    /// modèle mais la fuite de références corrigée dans `take_output_sample` —
    /// pool épuisé, donc convertisseur incapable d'accepter une entrée de
    /// plus.
    fn feed_converter(&mut self, frame: &CapturedFrame, sample_time: i64, duration: i64) -> Result<()> {
        // 1. Retirer les sorties en attente jusqu'à ce que le convertisseur se
        //    déclare preneur d'une entrée.
        //
        // C'est `GetInputStatus` — et non `GetOutputStatus` — qui sert de
        // condition d'arrêt, pour une raison mesurée : après avoir consommé
        // une entrée, ce MFT continue d'annoncer « sortie prête » en
        // permanence (chaque `ProcessOutput` supplémentaire réussit en
        // rejouant la dernière image convertie), si bien qu'une boucle
        // « drainer jusqu'à ce qu'il n'annonce plus rien » ne se termine
        // jamais et finit par vider son `IMFVideoSampleAllocator`. En
        // revanche il refuse toute entrée neuve tant qu'on ne lui a pas repris
        // sa sortie : l'appariement strict « une sortie par entrée » bloque
        // donc tout aussi sûrement (mesuré : 1 seule image convertie en 30 s).
        // Retirer des sorties jusqu'à ce qu'il redevienne preneur est le seul
        // des trois pilotages qui fasse réellement passer des images neuves —
        // en régime nominal, une seule itération suffit.
        let mut collected = 0usize;
        while !self.converter_accepts_input() {
            if collected >= MAX_CONVERTER_COLLECTS || !self.collect_converter_output()? {
                // Toujours pas preneur (ou pool momentanément vide) : on saute
                // cette image et on retentera. Une image sautée vaut mieux
                // qu'un pipeline mort — et mieux qu'un `MF_E_NOTACCEPTING`
                // encaissé en erreur.
                self.telemetry
                    .converter_not_accepting
                    .fetch_add(1, Ordering::Relaxed);
                self.publish_state();
                return Ok(());
            }
            collected += 1;
        }

        let bgra_sample = unsafe { MFCreateSample() }?;
        let bgra_buffer = unsafe {
            MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &frame.texture, 0, false)
        }
        .context("enveloppement de la texture BGRA pour le convertisseur")?;
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_CONVERTER_PROCESS_INPUT, Ordering::Relaxed);
        let result = unsafe {
            bgra_sample.AddBuffer(&bgra_buffer)?;
            bgra_sample.SetSampleTime(sample_time)?;
            bgra_sample.SetSampleDuration(duration)?;
            self.converter.ProcessInput(0, &bgra_sample, 0)
        };
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        // Filet de sécurité : `GetInputStatus` vient de dire oui, mais si le
        // convertisseur se ravise, `MF_E_NOTACCEPTING` reste une
        // contre-pression et non une panne — on saute l'image.
        if let Err(e) = &result {
            if e.code() == MF_E_NOTACCEPTING {
                self.telemetry
                    .converter_not_accepting
                    .fetch_add(1, Ordering::Relaxed);
                self.publish_state();
                return Ok(());
            }
        }
        result.context("soumission de l'image au convertisseur BGRA→NV12")?;
        self.telemetry
            .converter_inputs
            .fetch_add(1, Ordering::Relaxed);
        self.converter_output_pending = true;
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessInput du convertisseur lent");
        }
        self.pending_conversion_timestamps.push_back((sample_time, duration));

        self.collect_converter_output()?;
        Ok(())
    }

    /// Le convertisseur se déclare-t-il prêt à accepter une entrée ?
    ///
    /// **Risque de portabilité à connaître** : ce pilotage repose sur une API
    /// documentée, mais son adoption vient de l'observation d'un comportement
    /// anormal constaté sur **un seul pilote (NVIDIA) et une seule machine**.
    /// `GetInputStatus`/`GetOutputStatus` sont optionnelles dans `IMFTransform`
    /// et rien ne garantit qu'un convertisseur Intel ou AMD se comporte de
    /// même. Le repli ci-dessous (tenter `ProcessInput` quand la méthode n'est
    /// pas implémentée) couvre le cas le plus probable, pas tous ; à revalider
    /// sur le premier autre GPU rencontré.
    ///
    /// Publie au passage les deux drapeaux dans la télémétrie. Si le MFT
    /// n'implémente pas `GetInputStatus` (`u64::MAX`), on répond oui : mieux
    /// vaut tenter `ProcessInput` et traiter un éventuel `MF_E_NOTACCEPTING`
    /// que de ne jamais rien soumettre.
    fn converter_accepts_input(&self) -> bool {
        let input = converter_status(&self.converter, true);
        self.telemetry
            .converter_input_status
            .store(input, Ordering::Relaxed);
        self.telemetry.converter_output_status.store(
            converter_status(&self.converter, false),
            Ordering::Relaxed,
        );
        input == u64::MAX || input & MFT_INPUT_STATUS_ACCEPT_DATA.0 as u64 != 0
    }

    /// Retire **un** échantillon converti et l'empile dans `pending_nv12`,
    /// avec l'horodatage de SON entrée d'origine (dépilé de
    /// `pending_conversion_timestamps`, FIFO — voir son commentaire de champ).
    ///
    /// Renvoie `true` si la sortie due a bien été retirée (le convertisseur
    /// accepte à nouveau une entrée), `false` s'il faut réessayer plus tard
    /// (`MF_E_SAMPLEALLOCATOR_EMPTY`, `0xC00D4A3E` — pool momentanément vide,
    /// signal de contre-pression et non une panne).
    fn collect_converter_output(&mut self) -> Result<bool> {
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_CONVERTER_PROCESS_OUTPUT, Ordering::Relaxed);
        let poll = self.drain_converter_output();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        let poll = poll?;
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessOutput du convertisseur lent");
        }
        let ready = match poll {
            ConverterPoll::Sample(sample) => {
                self.telemetry
                    .converter_outputs
                    .fetch_add(1, Ordering::Relaxed);
                let (time, duration) =
                    self.pending_conversion_timestamps.pop_front().unwrap_or((0, 0));
                unsafe {
                    let _ = sample.SetSampleTime(time);
                    let _ = sample.SetSampleDuration(duration);
                }
                self.pending_nv12.push_back(sample);
                self.converter_output_pending = false;
                true
            }
            // Rien de prêt : le convertisseur est disponible pour une entrée.
            ConverterPoll::NeedMoreInput => {
                self.converter_output_pending = false;
                true
            }
            ConverterPoll::Busy => {
                self.converter_output_pending = true;
                self.skipped_busy += 1;
                false
            }
        };
        self.publish_state();
        Ok(ready)
    }

    /// Un appel à `ProcessOutput` sur le convertisseur.
    ///
    /// Piège rencontré à l'essai, au-delà de `MF_E_NOTACCEPTING` documenté
    /// plus haut : même après avoir élargi le pool de sortie via
    /// `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`/`_PROGRESSIVE` (essayé jusqu'à 16),
    /// l'appel « de confirmation » échoue de façon reproductible après
    /// exactement 5 images avec `MF_E_SAMPLEALLOCATOR_EMPTY` — identique
    /// quelle que soit la taille de pool demandée, ce qui prouve que cet
    /// attribut n'est pas honoré par ce MFT pour ce cas d'usage. Le message
    /// d'erreur MF associé à ce code (« vide en raison de demandes non
    /// traitées ») correspond à un signal de contre-pression documenté par
    /// Media Foundation pour `IMFVideoSampleAllocator` — pas à une panne — et
    /// se traite normalement en réessayant plus tard, une fois qu'un
    /// échantillon précédent aura été relâché par l'encodeur en aval.
    fn drain_converter_output(&mut self) -> Result<ConverterPoll> {
        let mut buffer = MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            // Un échantillon neuf à chaque tour dans le cas où le
            // convertisseur ne s'auto-alloue pas : plusieurs sorties peuvent
            // être mises en file simultanément (`pending_nv12`), donc en
            // réutiliser un seul les ferait toutes pointer sur la même texture
            // — chacune écrasant la précédente. Ce chemin n'est pas emprunté
            // sur la VM cible (`converter_provides_samples` y vaut `true`),
            // mais il ne doit pas pour autant être faux.
            pSample: std::mem::ManuallyDrop::new(if self.converter_provides_samples {
                None
            } else {
                Some(create_nv12_sample(&self.device, self.width, self.height)?)
            }),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        };
        let mut status = 0u32;
        let result = unsafe {
            self.converter
                .ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status)
        };
        // `take_output_sample` DOIT être appelé sur tous les chemins, y compris
        // d'erreur : voir son commentaire (la référence déposée dans
        // `pSample` par le MFT n'appartient à personne d'autre que nous).
        let sample = unsafe { take_output_sample(&mut buffer) };
        match result {
            Ok(()) => Ok(sample
                .map(ConverterPoll::Sample)
                .unwrap_or(ConverterPoll::NeedMoreInput)),
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(ConverterPoll::NeedMoreInput),
            Err(e) if e.code() == MF_E_SAMPLEALLOCATOR_EMPTY => Ok(ConverterPoll::Busy),
            Err(e) => Err(e).context("récupération de l'image convertie en NV12"),
        }
    }

    /// Soumet une image capturée si l'encodeur pourrait en avoir besoin.
    ///
    /// Ne nourrit le convertisseur que si `pending_nv12` ne contient pas
    /// déjà de quoi satisfaire toutes les demandes d'entrée en attente —
    /// convertir une image de plus n'ajouterait que de la latence si
    /// l'encodeur est déjà servi. Chaque échantillon NV12 disponible (celui
    /// qui vient d'être produit, ou un plus ancien laissé en file par un
    /// appel précédent où l'encodeur ne réclamait rien) est ensuite transmis
    /// à l'encodeur tant que celui-ci en réclame.
    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        self.telemetry.submit_calls.fetch_add(1, Ordering::Relaxed);
        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_SUBMIT_DRAIN_EVENTS, Ordering::Relaxed);
        let drained = self.drain_events();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        drained?;
        self.publish_state();
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "drainage des événements de l'encodeur lent");
        }

        // Media Foundation compte en unités de 100 ns ; nos horodatages sont
        // en 1/90000 s. 90000 Hz → 10 000 000 Hz : facteur 1000/9.
        let sample_time = (pts_90k as i64) * 1000 / 9;
        let duration = 10_000_000 / self.fps.max(1) as i64;

        if (self.pending_nv12.len() as u32) < self.pending_input_requests {
            self.feed_converter(frame, sample_time, duration)?;
        }

        while self.pending_input_requests > 0 {
            let Some(nv12_sample) = self.pending_nv12.pop_front() else {
                break;
            };
            let t = std::time::Instant::now();
            self.telemetry
                .phase
                .store(PHASE_ENCODER_PROCESS_INPUT, Ordering::Relaxed);
            let fed = unsafe { self.transform.ProcessInput(0, &nv12_sample, 0) };
            self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
            fed.context("soumission de l'image NV12 à l'encodeur")?;
            self.telemetry
                .encoder_inputs
                .fetch_add(1, Ordering::Relaxed);
            let elapsed = t.elapsed();
            if elapsed > SLOW_CALL {
                tracing::warn!(?elapsed, "ProcessInput de l'encodeur lent");
            }
            self.pending_input_requests -= 1;
        }
        self.publish_state();
        Ok(())
    }

    /// Récupère une unité d'accès encodée si elle est disponible.
    ///
    /// **Ronde de correction 1/5** : le relecteur a demandé de vérifier,
    /// plutôt que supposer, que le même raccourci (s'arrêter après un seul
    /// échantillon) n'affecte pas aussi le drainage de l'encodeur. Modèle
    /// async de MF : un événement `METransformHaveOutput` correspond à
    /// exactement un appel à `ProcessOutput` — mais le drapeau
    /// `MFT_OUTPUT_DATA_BUFFER_INCOMPLETE` (posé dans `dwStatus`) signale
    /// explicitement, quand il est présent, qu'il reste de la sortie pour CE
    /// flux sans qu'un nouvel événement ne soit garanti. On le vérifie
    /// désormais explicitement plutôt que de l'ignorer : s'il est posé, on
    /// se replanifie une entrée dans `pending_outputs` pour que la boucle de
    /// l'appelant (`while let Some(unit) = poll_output()?`) redemande
    /// immédiatement, sans attendre un événement qui pourrait ne pas venir.
    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.telemetry
            .phase
            .store(PHASE_POLL_DRAIN_EVENTS, Ordering::Relaxed);
        let drained = self.drain_events();
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        drained?;
        if self.pending_outputs == 0 {
            return Ok(None);
        }
        self.pending_outputs -= 1;

        // Les MFT matérielles allouent elles-mêmes leurs échantillons de sortie.
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: std::mem::ManuallyDrop::new(None),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        }];
        let mut status = 0u32;

        let t = std::time::Instant::now();
        self.telemetry
            .phase
            .store(PHASE_ENCODER_PROCESS_OUTPUT, Ordering::Relaxed);
        let produced = unsafe { self.transform.ProcessOutput(0, &mut buffers, &mut status) };
        self.telemetry
            .phase
            .store(PHASE_ENCODER_READ_BUFFER, Ordering::Relaxed);
        // Reprendre la référence AVANT toute propagation d'erreur, comme
        // l'exige `take_output_sample` et comme le fait déjà
        // `drain_converter_output`. Sortir par `?` d'abord laisserait fuir
        // l'échantillon si le MFT en avait déposé un malgré l'échec — ce
        // pilote ne semble pas le faire, mais c'est exactement la classe de
        // fuite corrigée dans ce fichier, et rien ne la garantit ailleurs.
        // `dwStatus` est lu avant, le tampon ne devant plus l'être après.
        let incomplete = buffers[0].dwStatus & MFT_OUTPUT_DATA_BUFFER_INCOMPLETE.0 as u32 != 0;
        let taken = unsafe { take_output_sample(&mut buffers[0]) };
        produced.context("récupération de l'image encodée")?;
        self.telemetry
            .encoder_outputs
            .fetch_add(1, Ordering::Relaxed);
        let elapsed = t.elapsed();
        if elapsed > SLOW_CALL {
            tracing::warn!(?elapsed, "ProcessOutput de l'encodeur lent");
        }

        if incomplete {
            self.pending_outputs += 1;
        }

        let sample = taken.ok_or_else(|| anyhow!("échantillon de sortie absent"))?;

        let media_buffer = unsafe { sample.ConvertToContiguousBuffer() }?;
        let mut data_ptr: *mut u8 = std::ptr::null_mut();
        let mut length = 0u32;
        unsafe { media_buffer.Lock(&mut data_ptr, None, Some(&mut length))? };
        let bytes = unsafe { std::slice::from_raw_parts(data_ptr, length as usize) }.to_vec();
        unsafe { media_buffer.Unlock()? };

        // La sortie est en Annex-B ; on la repasse par le regroupement pour
        // obtenir l'indicateur d'image clé de façon cohérente avec le reste.
        let mut units = group_access_units(&bytes, self.fps.max(1));
        if units.is_empty() {
            return Ok(None);
        }
        let mut unit = units.remove(0);
        // L'horodatage vient de l'échantillon, pas de la position dans le flux.
        let sample_time = unsafe { sample.GetSampleTime() }.unwrap_or(0);
        unit.pts_90k = (sample_time.max(0) as u64) * 9 / 1000;
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        Ok(Some(unit))
    }

    /// Force la production d'une image clé sur l'image suivante.
    pub fn request_keyframe(&mut self) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let value = variant_bool(true);
        unsafe { codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &value) }?;
        Ok(())
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        if self.skipped_busy > 0 {
            tracing::debug!(
                skipped_busy = self.skipped_busy,
                "images renoncées faute de confirmation du convertisseur (diagnostic)"
            );
        }
        unsafe {
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
        // `MFShutdown` n'est plus appelé ici : il l'est par la destruction de
        // `_media_foundation`, qui a lieu après ce corps ET après celle des
        // MFT (dernier champ déclaré — voir son commentaire).
        let _ = &self.device_manager;
    }
}

/// Reprend possession de l'échantillon (et des événements) qu'un MFT vient de
/// déposer dans un `MFT_OUTPUT_DATA_BUFFER`, en laissant la structure vide.
///
/// **C'est la cause racine du blocage du 28/07, corrigée ici** (voir le
/// rapport de tâche). Les deux champs `pSample`/`pEvents` de
/// `MFT_OUTPUT_DATA_BUFFER` sont des `ManuallyDrop<Option<...>>` : windows-rs
/// se refuse délibérément à les libérer tout seul, puisque leur propriété
/// dépend du sens de l'appel. `ProcessOutput` y dépose une référence COM dont
/// **l'appelant devient propriétaire** ; la lire par `.as_ref().cloned()`
/// ajoute une seconde référence sans jamais rendre la première, et le
/// `ManuallyDrop` emporte celle-ci dans la tombe à la fin du bloc. Chaque
/// image encodée fuyait donc une référence.
///
/// Conséquence observée, bien plus grave qu'une simple fuite mémoire : les
/// échantillons de sortie du `Video Processor MFT` proviennent d'un
/// `IMFVideoSampleAllocator` de taille fixe (10 sur cette VM). Un échantillon
/// jamais relâché ne retourne jamais au pool. Après exactement 10 images le
/// pool était définitivement vide, et chaque `ProcessOutput` suivant attendait
/// une seconde entière un échantillon libre avant de rendre
/// `MF_E_SAMPLEALLOCATOR_EMPTY` — d'où le « plafond à ~1 image/s » puis, dès
/// que le drainage a exigé une confirmation par
/// `MF_E_TRANSFORM_NEED_MORE_INPUT` (ronde précédente), l'arrêt total du
/// pipeline. `ManuallyDrop::take` déplace la référence hors de la structure :
/// elle est alors possédée normalement, et relâchée dès que l'appelant en a
/// fini — ce qui rend l'échantillon au pool.
///
/// # Sécurité
///
/// Le tampon ne doit plus être lu après cet appel (ses deux champs COM sont
/// laissés dans un état déplacé). Tous les appelants l'utilisent en variable
/// locale et n'y touchent plus ensuite.
/// Interroge le convertisseur : `true` pour `GetInputStatus` (peut-il accepter
/// une entrée), `false` pour `GetOutputStatus` (une sortie est-elle prête).
///
/// Renvoie les drapeaux bruts, ou `u64::MAX` si la méthode n'est pas
/// implémentée par ce MFT — les deux sont optionnelles dans `IMFTransform`, et
/// la distinction « répond non » / « ne répond pas » est justement ce qu'on a
/// besoin de savoir.
fn converter_status(converter: &IMFTransform, input: bool) -> u64 {
    // écart d'API windows-rs 0.62 : ces deux méthodes rendent les drapeaux par
    // valeur de retour (`Result<u32>`), là où la signature C les écrit dans un
    // paramètre de sortie.
    let result = if input {
        unsafe { converter.GetInputStatus(0) }
    } else {
        unsafe { converter.GetOutputStatus() }
    };
    match result {
        Ok(flags) => flags as u64,
        Err(_) => u64::MAX,
    }
}

unsafe fn take_output_sample(buffer: &mut MFT_OUTPUT_DATA_BUFFER) -> Option<IMFSample> {
    // `pEvents` est presque toujours nul, mais quand un MFT y dépose une file
    // d'événements elle nous appartient exactement au même titre.
    drop(std::mem::ManuallyDrop::take(&mut buffer.pEvents));
    std::mem::ManuallyDrop::take(&mut buffer.pSample)
}

/// Construit une `VARIANT` `VT_UI4` manuellement : cette version de
/// windows-rs ne fournit pas de `From<u32>` pour `VARIANT` (écart au brief,
/// vérifié en lisant les sources de la crate sur la VM — aucun `impl From<`
/// n'existe pour ce type dans `Win32::System::Variant`).
fn variant_u32(value: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: value },
            }),
        },
    }
}

/// Construit une `VARIANT` `VT_BOOL` manuellement (même raison que
/// `variant_u32`).
fn variant_bool(value: bool) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    boolVal: if value { VARIANT_TRUE } else { VARIANT_FALSE },
                },
            }),
        },
    }
}

/// Journalise les types d'entrée réellement annoncés par l'encodeur, avant
/// toute configuration. Sert de preuve empirique à la question BGRA/NV12
/// (voir le commentaire de module) : sur la VM cible, seul NV12 apparaît.
fn log_supported_input_types(transform: &IMFTransform) {
    let mut index = 0u32;
    loop {
        let media_type = match unsafe { transform.GetInputAvailableType(0, index) } {
            Ok(t) => t,
            Err(_) => break, // MF_E_NO_MORE_TYPES : fin de l'énumération.
        };
        let subtype = unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) };
        match subtype {
            Ok(guid) => tracing::info!(
                index,
                subtype = %format_subtype(guid),
                "type d'entrée annoncé par l'encodeur"
            ),
            Err(_) => tracing::info!(index, "type d'entrée annoncé (sous-type illisible)"),
        }
        index += 1;
    }
}

/// Traduit les GUID de sous-type vidéo les plus courants en texte lisible,
/// pour les journaux. Sans rapport avec la logique de conversion elle-même.
fn format_subtype(guid: GUID) -> String {
    if guid == MFVideoFormat_NV12 {
        "NV12".to_string()
    } else if guid == MFVideoFormat_ARGB32 {
        "ARGB32 (BGRA)".to_string()
    } else if guid == MFVideoFormat_RGB32 {
        "RGB32 (BGRX)".to_string()
    } else if guid == MFVideoFormat_YUY2 {
        "YUY2".to_string()
    } else if guid == MFVideoFormat_YV12 {
        "YV12".to_string()
    } else if guid == MFVideoFormat_IYUV {
        "IYUV".to_string()
    } else {
        format!("{guid:?}")
    }
}

/// Crée le convertisseur GPU BGRA→NV12 (Video Processor MFT de Media
/// Foundation, `CLSID_VideoProcessorMFT`). Contrairement à l'encodeur, cette
/// MFT est synchrone : pas d'événements à suivre, `ProcessInput` suivi de
/// `ProcessOutput` suffit.
fn create_color_converter(
    device_manager: &IMFDXGIDeviceManager,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<IMFTransform> {
    // Essai (ronde de correction 1/5, investigation du débit) :
    // `CoCreateInstance(CLSID_VideoProcessorMFT)` instancie l'implémentation
    // par défaut de ce CLSID, qui pourrait être un chemin logiciel/mixte
    // plutôt qu'une implémentation matérielle. On tente d'abord de trouver
    // un convertisseur explicitement enregistré comme matériel via
    // `MFTEnumEx`, comme pour l'encodeur — repli sur `CoCreateInstance` si
    // rien n'est trouvé.
    let converter: IMFTransform = match find_hardware_video_processor() {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!(erreur = %e, "aucun convertisseur vidéo matériel énuméré, repli sur CLSID_VideoProcessorMFT");
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER) }
                .context("création du convertisseur vidéo (Video Processor MFT)")?
        }
    };

    // Essai : le mode faible latence n'était appliqué qu'à l'encodeur, pas au
    // convertisseur — potentiellement lié à l'attente d'~1 s observée dans
    // `drain_converter_output` (voir son commentaire). `GetAttributes` peut
    // échouer si le convertisseur n'expose pas d'attributs modifiables ; dans
    // ce cas on continue sans bloquer la construction.
    if let Ok(converter_attributes) = unsafe { converter.GetAttributes() } {
        let _ = unsafe { converter_attributes.SetUINT32(&MF_LOW_LATENCY, 1) };
    }

    unsafe {
        converter.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, device_manager.as_raw() as usize)
    }
    .context("partage du périphérique D3D avec le convertisseur")?;

    // Piège rencontré à l'essai : avec le pool par défaut, la séquence
    // documentée « ProcessOutput jusqu'à MF_E_TRANSFORM_NEED_MORE_INPUT »
    // échoue avec `MF_E_SAMPLEALLOCATOR_EMPTY` (0xC00D4A3E) après exactement 5
    // images — l'encodeur matériel en garde plusieurs « en vol » avant d'en
    // libérer, et le pool par défaut n'a pas cette marge.
    //
    // Premier correctif tenté, insuffisant : `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`
    // seul, avec les valeurs 4 puis 16 — dans les deux cas, échec au exactement
    // le même 5e appel, preuve que cet attribut seul n'a aucun effet ici.
    // Cause : notre flux est **progressif**
    // (`MF_MT_INTERLACE_MODE` = `MFVideoInterlace_Progressive`), et ce MFT
    // distingue apparemment deux attributs de taille de pool — un pour le
    // contenu entrelacé, un pour le progressif
    // (`MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE`) — seul ce second
    // attribut est honoré pour du contenu progressif. On positionne les deux
    // par prudence (documentation MF ambiguë sur ce point).
    let output_stream_attributes = unsafe { converter.GetOutputStreamAttributes(0) }
        .context("attributs du flux de sortie du convertisseur")?;
    unsafe {
        output_stream_attributes.SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT, 16)?;
        output_stream_attributes
            .SetUINT32(&MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT_PROGRESSIVE, 16)?;
    }

    let input_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        input_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        // Format d'entrée = ce que produit la capture (BGRA, avec alpha) ;
        // `MFVideoFormat_ARGB32` correspond à `DXGI_FORMAT_B8G8R8A8_UNORM`.
        input_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_ARGB32)?;
        input_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        input_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        input_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter.SetInputType(0, &input_type, 0)?;
    }

    let output_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        output_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        output_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        output_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        output_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        converter.SetOutputType(0, &output_type, 0)?;
    }

    Ok(converter)
}

/// Énumère les convertisseurs vidéo (BGRA→NV12) explicitement enregistrés
/// comme matériels, et active le premier — même logique que
/// `find_hardware_encoder`, avec les mêmes précautions de libération
/// mémoire (voir son commentaire).
fn find_hardware_video_processor() -> Result<IMFTransform> {
    let input_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_ARGB32,
    };
    let output_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;

    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_PROCESSOR,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input_info),
            Some(&output_info),
            &mut activates,
            &mut count,
        )
        .context("énumération des convertisseurs vidéo matériels")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!("aucun convertisseur vidéo matériel enregistré");
    }

    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
    }
    let first = first.ok_or_else(|| anyhow!("activateur de convertisseur absent"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    if unsafe { first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len) }
        .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(convertisseur = %name, "convertisseur vidéo matériel retenu");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let transform: IMFTransform = unsafe { first.ActivateObject() }?;
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    Ok(transform)
}

/// Alloue une texture NV12 GPU et l'enveloppe dans un échantillon Media
/// Foundation réutilisable, pour les cas où le convertisseur ne s'auto-alloue
/// pas (`MFT_OUTPUT_STREAM_PROVIDES_SAMPLES` absent — voir
/// `H264Encoder::new`).
fn create_nv12_sample(device: &ID3D11Device, width: u32, height: u32) -> Result<IMFSample> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_NV12,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("allocation de la texture NV12 intermédiaire")?;
    let texture = texture.ok_or_else(|| anyhow!("texture NV12 absente"))?;

    let sample = unsafe { MFCreateSample() }?;
    let buffer = unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &texture, 0, false) }
        .context("enveloppement de la texture NV12")?;
    unsafe { sample.AddBuffer(&buffer) }?;
    Ok(sample)
}

/// Énumère les encodeurs H.264 matériels et active le premier.
fn find_hardware_encoder() -> Result<IMFTransform> {
    let input_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let output_info = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;

    unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input_info),
            Some(&output_info),
            &mut activates,
            &mut count,
        )
        .context("énumération des encodeurs H.264 matériels")?;
    }

    if count == 0 {
        unsafe { CoTaskMemFree(Some(activates as *const _)) };
        bail!(
            "aucun encodeur H.264 matériel trouvé sur cette machine. \
             Vérifier le pilote GPU ; le jalon 1 n'a pas de repli logiciel."
        );
    }

    // Récupérer les objets AVANT de libérer le tableau alloué par CoTaskMemAlloc.
    //
    // Ronde de correction 1/5 — fuite corrigée ici : la version précédente
    // ne relâchait que le premier `IMFActivate` (par `.clone()`, qui ajoute
    // une référence sans jamais libérer celle que `MFTEnumEx` a placée dans
    // la case du tableau). `CoTaskMemFree` ne libère que la mémoire brute du
    // tableau, pas les références COM qu'il contient : chaque entrée, y
    // compris la première, fuyait donc une référence. `slot.take()` déplace
    // chaque entrée hors du tableau (remplacée par `None`) ; les entrées
    // qu'on ne garde pas sont droppées immédiatement (donc relâchées), la
    // première est conservée dans `first` sans référence supplémentaire.
    // Latent tant qu'un seul encodeur est présent, mais réel dès qu'il y en
    // aurait plusieurs.
    let slice = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut first: Option<IMFActivate> = None;
    for (index, slot) in slice.iter_mut().enumerate() {
        let activate = slot.take();
        if index == 0 {
            first = activate;
        }
        // Sinon : `activate` est droppé ici, relâchant sa référence COM.
    }
    let first = first.ok_or_else(|| anyhow!("activateur d'encodeur absent"))?;

    let mut name_ptr = PWSTR::null();
    let mut name_len = 0u32;
    // écart d'API windows-rs 0.62 : `GetStringAlloc` n'existe pas sur
    // `IMFAttributes` dans cette version ; la méthode s'appelle
    // `GetAllocatedString` (mémoire allouée par `CoTaskMemAlloc`, à libérer
    // explicitement après usage).
    if unsafe { first.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut name_ptr, &mut name_len) }
        .is_ok()
    {
        let name = unsafe { name_ptr.to_string() }.unwrap_or_default();
        tracing::info!(encodeur = %name, "encodeur matériel retenu");
        unsafe { CoTaskMemFree(Some(name_ptr.0 as *const _)) };
    }

    let transform: IMFTransform = unsafe { first.ActivateObject() }?;
    unsafe { CoTaskMemFree(Some(activates as *const _)) };
    Ok(transform)
}

fn configure_output(
    transform: &IMFTransform,
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        media_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_u64(1, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        // Baseline évite les images B : ordre de décodage = ordre d'affichage.
        media_type.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
        transform.SetOutputType(0, &media_type, 0)?;
    }
    Ok(())
}

fn configure_input(transform: &IMFTransform, width: u32, height: u32, fps: u32) -> Result<()> {
    let media_type = unsafe { MFCreateMediaType() }?;
    unsafe {
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, pack_u64(width, height))?;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, pack_u64(fps, 1))?;
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        transform.SetInputType(0, &media_type, 0)?;
    }
    Ok(())
}

fn configure_rate_control(transform: &IMFTransform, bitrate: u32) -> Result<()> {
    let codec: ICodecAPI = transform.cast()?;
    unsafe {
        // Débit constant : latence prévisible, indispensable en interactif.
        let mode = variant_u32(eAVEncCommonRateControlMode_CBR.0 as u32);
        codec.SetValue(&CODECAPI_AVEncCommonRateControlMode, &mode)?;
        let rate = variant_u32(bitrate);
        codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate)?;
        // Pas de groupe d'images fermé : on demande les images clés à la volée.
        let gop = variant_u32(0);
        let _ = codec.SetValue(&CODECAPI_AVEncMPVGOPSize, &gop);
        let low_latency = variant_bool(true);
        let _ = codec.SetValue(&CODECAPI_AVLowLatencyMode, &low_latency);
    }
    Ok(())
}

/// Empaquette deux entiers 32 bits dans l'attribut 64 bits attendu par MF.
fn pack_u64(high: u32, low: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

fn share_device(device: &ID3D11Device) -> Result<IMFDXGIDeviceManager> {
    let mut token = 0u32;
    let mut manager: Option<IMFDXGIDeviceManager> = None;
    unsafe {
        MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
    }
    let manager = manager.ok_or_else(|| anyhow!("gestionnaire DXGI absent"))?;
    unsafe { manager.ResetDevice(device, token)? };
    Ok(manager)
}
