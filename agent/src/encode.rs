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
//! **Constat de performance sur la VM cible, non résolu** : après une brève
//! rafale initiale (~10 images en <1 s), chaque appel à `ProcessOutput` sur
//! le convertisseur qui doit confirmer l'absence de sortie supplémentaire se
//! met à durer environ 1,0 seconde de façon parfaitement reproductible (voir
//! `drain_converter_output`), plafonnant le débit réel à ~1 image/s au lieu
//! des 60 im/s visées. Cause écartée par des essais empiriques ciblés :
//! bug de notre pilotage (identique en `debug` et en `release`), état GPU
//! contaminé par un run précédent (identique après redémarrage complet de la
//! VM), contention avec `sunshine.exe` (un service de streaming tiers
//! tournant sur cette VM, arrêté sans effet sur la mesure),
//! `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`/`_PROGRESSIVE` trop petit (essayé
//! jusqu'à 16, sans effet), absence de `MF_LOW_LATENCY` sur le convertisseur
//! (ajouté, sans effet). Le comportement pointe vers une caractéristique du
//! pilote/de la virtualisation GPU de cette VM plutôt que vers un défaut du
//! code — voir le rapport de tâche pour le détail de l'investigation.

#![cfg(windows)]

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
    /// Échantillon NV12 réutilisé à chaque image quand le convertisseur ne
    /// s'auto-alloue pas (texture GPU allouée une seule fois : la taille est
    /// fixe pour la durée de vie de l'encodeur).
    nv12_sample: Option<IMFSample>,
    /// Vrai si le convertisseur a produit un échantillon lors du dernier
    /// appel mais n'a pas encore confirmé être drainé (`ProcessOutput` de
    /// confirmation en attente — voir `convert_to_nv12`). Tant que c'est
    /// vrai, un nouveau `ProcessInput` échouerait avec `MF_E_NOTACCEPTING` :
    /// on retente la confirmation au tour suivant plutôt que d'échouer.
    converter_needs_drain_confirmation: bool,
    width: u32,
    height: u32,
    fps: u32,
    /// Nombre de demandes d'entrée non encore satisfaites.
    pending_input_requests: u32,
    /// Nombre d'images prêtes à être récupérées.
    pending_outputs: u32,
    /// Compteur diagnostic : images renoncées faute de confirmation de
    /// drainage du convertisseur (voir `convert_to_nv12`). Exposé pour
    /// mesurer l'ampleur réelle de ce contournement, pas consommé par la
    /// logique de pilotage elle-même.
    skipped_busy: u64,
}

impl H264Encoder {
    pub fn new(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).context("démarrage de Media Foundation")?;
        }

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
        let nv12_sample = if converter_provides_samples {
            None
        } else {
            Some(create_nv12_sample(device, width, height)?)
        };
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0) }?;
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0) }?;

        Ok(Self {
            transform,
            events,
            device_manager,
            converter,
            converter_provides_samples,
            nv12_sample,
            converter_needs_drain_confirmation: false,
            skipped_busy: 0,
            width,
            height,
            fps,
            pending_input_requests: 0,
            pending_outputs: 0,
        })
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
                    tracing::trace!(
                        pending_input_requests = self.pending_input_requests,
                        "événement METransformNeedInput reçu"
                    );
                }
                ME_TRANSFORM_HAVE_OUTPUT => {
                    self.pending_outputs += 1;
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

    /// Convertit une texture BGRA capturée en un échantillon NV12, via le
    /// convertisseur GPU synchrone. Contrairement à l'encodeur, ce transform
    /// se pilote par une paire `ProcessInput`/`ProcessOutput` classique : pas
    /// d'événements à suivre.
    ///
    /// Piège rencontré à l'essai : un seul appel à `ProcessOutput` après
    /// `ProcessInput` ne suffit pas. Le modèle documenté des MFT synchrones
    /// (« Basic MFT Processing Model ») exige d'appeler `ProcessOutput` en
    /// boucle jusqu'à ce qu'il renvoie `MF_E_TRANSFORM_NEED_MORE_INPUT` — ce
    /// code signale que le transform est redevenu prêt à accepter une
    /// nouvelle entrée. S'arrêter dès le premier échantillon obtenu laisse le
    /// convertisseur dans un état interne non drainé : le `ProcessInput` de
    /// l'image suivante échoue alors avec `MF_E_NOTACCEPTING`
    /// (`0xC00D36B5`), observé tel quel lors du premier essai sur la VM.
    ///
    /// `Ok(None)` signifie qu'il n'y a rien à transmettre à l'encodeur pour
    /// cette image : soit la confirmation de drainage d'un tour précédent
    /// n'a pas encore abouti (voir le champ
    /// `converter_needs_drain_confirmation`), soit le pool de sortie du
    /// convertisseur est momentanément épuisé
    /// (`MF_E_SAMPLEALLOCATOR_EMPTY`, `0xC00D4A3E` — rencontré de façon
    /// reproductible après quelques images, malgré l'agrandissement du pool
    /// via `MF_SA_MINIMUM_OUTPUT_SAMPLE_COUNT`/`_PROGRESSIVE`). Dans les deux
    /// cas ce n'est pas une erreur fatale : l'appelant réessaiera à l'image
    /// suivante (voir `submit`).
    ///
    /// Piège rencontré à l'essai, résolu par le champ
    /// `converter_needs_drain_confirmation` : un premier correctif qui
    /// abandonnait purement et simplement la confirmation en cas d'échec
    /// laissait le convertisseur dans un état où `ProcessInput` échouait
    /// ensuite systématiquement avec `MF_E_NOTACCEPTING` sur TOUTES les
    /// images suivantes (le convertisseur exige d'avoir confirmé le drainage
    /// de sa sortie précédente avant d'accepter une entrée nouvelle — voir
    /// plus haut). La confirmation manquée est donc retentée aux tours
    /// suivants plutôt qu'abandonnée : la sortie réelle de chaque image,
    /// elle, est systématiquement obtenue dès le premier appel à
    /// `drain_converter_output` — seule la confirmation « plus rien à
    /// sortir » échoue parfois faute de place dans le pool.
    fn convert_to_nv12(
        &mut self,
        frame: &CapturedFrame,
        sample_time: i64,
        duration: i64,
    ) -> Result<Option<IMFSample>> {
        if self.converter_needs_drain_confirmation {
            let t = std::time::Instant::now();
            let poll = self.drain_converter_output()?;
            let elapsed = t.elapsed();
            if elapsed > std::time::Duration::from_millis(20) {
                tracing::debug!(?elapsed, "confirmation différée du convertisseur lente (voir commentaire de module)");
            }
            match poll {
                ConverterPoll::NeedMoreInput => {
                    self.converter_needs_drain_confirmation = false;
                }
                ConverterPoll::Sample(_) => {
                    // Ne devrait pas se produire (conversion de format pure,
                    // 1 entrée → 1 sortie) ; on l'écarte et on considère la
                    // confirmation obtenue plutôt que de bloquer dessus.
                    self.converter_needs_drain_confirmation = false;
                }
                ConverterPoll::Busy => {
                    // Toujours pas confirmé : on ne peut pas soumettre de
                    // nouvelle entrée ce tour-ci sans risquer
                    // `MF_E_NOTACCEPTING`.
                    self.skipped_busy += 1;
                    return Ok(None);
                }
            }
        }

        let bgra_sample = unsafe { MFCreateSample() }?;
        let bgra_buffer = unsafe {
            MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &frame.texture, 0, false)
        }
        .context("enveloppement de la texture BGRA pour le convertisseur")?;
        let t = std::time::Instant::now();
        unsafe {
            bgra_sample.AddBuffer(&bgra_buffer)?;
            bgra_sample.SetSampleTime(sample_time)?;
            bgra_sample.SetSampleDuration(duration)?;
            self.converter.ProcessInput(0, &bgra_sample, 0)
        }
        .context("soumission de l'image au convertisseur BGRA→NV12")?;
        let elapsed = t.elapsed();
        if elapsed > std::time::Duration::from_millis(20) {
            tracing::debug!(?elapsed, "ProcessInput du convertisseur lent");
        }

        // La sortie réelle de cette image : observée fiable dès ce premier
        // appel dans tous les essais menés (contrairement à l'appel de
        // confirmation qui suit).
        let t = std::time::Instant::now();
        let first_poll = self.drain_converter_output()?;
        let elapsed = t.elapsed();
        if elapsed > std::time::Duration::from_millis(20) {
            tracing::debug!(?elapsed, "récupération de la sortie du convertisseur lente");
        }
        let sample = match first_poll {
            ConverterPoll::Sample(sample) => sample,
            ConverterPoll::NeedMoreInput => {
                return Ok(None);
            }
            ConverterPoll::Busy => {
                // Jamais observé à ce point précis en pratique, mais géré par
                // prudence : la confirmation restera à faire au tour suivant.
                self.converter_needs_drain_confirmation = true;
                return Ok(None);
            }
        };

        // Tentative de confirmation immédiate (cas courant : réussit tout de
        // suite). Si elle échoue faute de place dans le pool, on la reporte
        // au tour suivant plutôt que de perdre l'échantillon qu'on a déjà.
        let t = std::time::Instant::now();
        let confirm_poll = self.drain_converter_output()?;
        let elapsed = t.elapsed();
        if elapsed > std::time::Duration::from_millis(20) {
            tracing::debug!(?elapsed, "confirmation immédiate du convertisseur lente (voir commentaire de module)");
        }
        match confirm_poll {
            ConverterPoll::NeedMoreInput => {}
            ConverterPoll::Sample(_) => {}
            ConverterPoll::Busy => {
                self.converter_needs_drain_confirmation = true;
            }
        }

        unsafe {
            sample.SetSampleTime(sample_time)?;
            sample.SetSampleDuration(duration)?;
        }
        Ok(Some(sample))
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
            pSample: std::mem::ManuallyDrop::new(if self.converter_provides_samples {
                None
            } else {
                self.nv12_sample.clone()
            }),
            dwStatus: 0,
            pEvents: std::mem::ManuallyDrop::new(None),
        };
        let mut status = 0u32;
        match unsafe {
            self.converter
                .ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status)
        } {
            Ok(()) => Ok(buffer
                .pSample
                .as_ref()
                .cloned()
                .map(ConverterPoll::Sample)
                .unwrap_or(ConverterPoll::NeedMoreInput)),
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(ConverterPoll::NeedMoreInput),
            Err(e) if e.code() == MF_E_SAMPLEALLOCATOR_EMPTY => Ok(ConverterPoll::Busy),
            Err(e) => Err(e).context("récupération de l'image convertie en NV12"),
        }
    }

    /// Soumet une image capturée si l'encodeur en réclame une.
    ///
    /// Si aucune demande n'est en attente, l'image est ignorée : l'encodeur est
    /// saturé et une image de plus ne ferait qu'ajouter de la latence. La
    /// conversion BGRA→NV12 n'est donc effectuée que lorsqu'elle sera
    /// réellement consommée.
    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        let t = std::time::Instant::now();
        self.drain_events()?;
        let elapsed = t.elapsed();
        if elapsed > std::time::Duration::from_millis(20) {
            tracing::debug!(?elapsed, "drainage des événements de l'encodeur lent");
        }
        if self.pending_input_requests == 0 {
            return Ok(());
        }

        // Media Foundation compte en unités de 100 ns ; nos horodatages sont
        // en 1/90000 s. 90000 Hz → 10 000 000 Hz : facteur 1000/9.
        let sample_time = (pts_90k as i64) * 1000 / 9;
        let duration = 10_000_000 / self.fps.max(1) as i64;

        let Some(nv12_sample) = self.convert_to_nv12(frame, sample_time, duration)? else {
            // Pool du convertisseur momentanément épuisé (voir
            // `drain_converter_output`) : on renonce à cette image sans
            // toucher `pending_input_requests`, l'encodeur redemandera une
            // entrée à la prochaine image capturée.
            return Ok(());
        };
        let t = std::time::Instant::now();
        unsafe { self.transform.ProcessInput(0, &nv12_sample, 0) }
            .context("soumission de l'image NV12 à l'encodeur")?;
        let elapsed = t.elapsed();
        if elapsed > std::time::Duration::from_millis(20) {
            tracing::debug!(?elapsed, "ProcessInput de l'encodeur lent");
        }
        self.pending_input_requests -= 1;
        Ok(())
    }

    /// Récupère une unité d'accès encodée si elle est disponible.
    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.drain_events()?;
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

        unsafe { self.transform.ProcessOutput(0, &mut buffers, &mut status) }
            .context("récupération de l'image encodée")?;

        let sample = buffers[0]
            .pSample
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("échantillon de sortie absent"))?;

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
            let _ = MFShutdown();
        }
        let _ = &self.device_manager;
    }
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
    let converter: IMFTransform =
        unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER) }
            .context("création du convertisseur vidéo (Video Processor MFT)")?;

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
        bail!(
            "aucun encodeur H.264 matériel trouvé sur cette machine. \
             Vérifier le pilote GPU ; le jalon 1 n'a pas de repli logiciel."
        );
    }

    // Récupérer les objets AVANT de libérer le tableau alloué par CoTaskMemAlloc.
    let slice = unsafe { std::slice::from_raw_parts(activates, count as usize) };
    let first = slice
        .first()
        .and_then(|a| a.clone())
        .ok_or_else(|| anyhow!("activateur d'encodeur absent"))?;

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
