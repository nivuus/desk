//! Le chemin **Media Foundation** : l'objet `EncodeurMft`, sa construction et
//! sa destruction.
//!
//! Extrait d'`encode.rs` le 30 août 2026 (lot 31), dans une tâche DÉDIÉE et
//! **avant** le branchement qu'il prépare — la doctrine du dépôt : *extraire,
//! jamais comprimer*, et *l'extraction jouée AVANT celle qui ajoute*.
//!
//! 🔴 **POURQUOI TROIS FICHIERS ET NON DEUX.** Les seules méthodes de la
//! pompe pèsent **487 lignes** (mesuré, pas estimé) : elles ne tiennent pas
//! sous le plafond de 500 avec leur en-tête et leurs imports. Et la règle de
//! visibilité de Rust interdit de les loger ailleurs que **sous** le module
//! qui définit la structure — un frère ne voit pas les champs privés. La
//! pompe est donc scindée **par responsabilité** — `convertisseur` (BGRA→NV12)
//! et `encodeur` (la MFT asynchrone) — et non tranchée au hasard.
//!
//! ⚠️ **Ce module n'est PAS la façade.** `encode.rs` réexporte
//! `EncodeurMft` : les ~10 appelants du dépôt n'ont pas bougé d'une ligne, et
//! aucun des huit verbes n'a changé de signature.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Media::MediaFoundation::*;

// Même raison que chez ses enfants : ce fichier est la continuation
// d'`encode.rs`, pas un module indépendant qui en consommerait l'interface.
use crate::encode::*;
use crate::encode::{arret, fabrique, reglages};

mod convertisseur;
mod encodeur;

pub struct EncodeurMft {
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
    /// Taille des textures BGRA remises par la capture — l'entrée du
    /// convertisseur.
    capture: (u32, u32),
    /// Taille réellement encodée et transportée — la sortie du convertisseur
    /// et l'entrée de l'encodeur. Peut être plus petite que `capture` : c'est
    /// le levier de résolution adaptative, et il ne touche pas à la fenêtre
    /// Windows (contrairement à `WindowsSource::resize`).
    encode: (u32, u32),
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
    /// File de travail sérialisée imposée à la MFT encodeur, et barrière de sa
    /// mise au repos (voir `arret::FileMft` ; le convertisseur n'en a pas, et
    /// `arret::mettre_au_repos` dit pourquoi).
    ///
    /// **Déclarée en dernier volontairement** : les champs sont détruits dans
    /// l'ordre de déclaration, après l'exécution de `Drop for EncodeurMft`.
    /// La file ne doit être rendue qu'une fois relâchée la MFT qui la détient.
    file_encodeur: arret::FileMft,
}

impl EncodeurMft {
    pub fn new(
        device: &ID3D11Device,
        capture: (u32, u32),
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        demarrer_media_foundation()?;

        // Allouée ICI, avant toute MFT : les locales sont détruites dans
        // l'ordre INVERSE de déclaration, donc celle-ci l'est en dernier si un
        // `?` plus bas interrompt la construction. Une file rendue avant la MFT
        // qui la détient serait exactement l'inversion que l'ordre des champs
        // ci-dessus évite. Voir `arret::FileMft::allouer`.
        let mut file_encodeur = arret::FileMft::allouer();

        let transform = fabrique::find_hardware_encoder()?;
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
        fabrique::log_supported_input_types(&transform);

        // Partager le périphérique D3D11 pour recevoir des textures GPU.
        let device_manager = fabrique::share_device(device)?;
        unsafe {
            transform.ProcessMessage(
                MFT_MESSAGE_SET_D3D_MANAGER,
                device_manager.as_raw() as usize,
            )
        }
        .context("partage du périphérique D3D avec l'encodeur")?;

        reglages::configure_output(&transform, encode.0, encode.1, fps, bitrate)?;
        reglages::configure_input(&transform, encode.0, encode.1, fps)?;
        reglages::configure_rate_control(&transform, bitrate)?;

        let events: IMFMediaEventGenerator = transform.cast()?;

        // AVANT tout démarrage de flux : imposer à la MFT la file sur laquelle
        // elle déposera son travail asynchrone, seul moyen d'obtenir plus tard
        // une barrière sur ce travail (voir `arret::FileMft`).
        file_encodeur.confier(&transform, "encodeur");

        // `NOTIFY_BEGIN_STREAMING` est le point où une MFT matérielle réserve
        // ses ressources de session GPU : candidat au refus quand plusieurs
        // encodeurs coexistent, à ne pas confondre avec les deux autres.
        unsafe {
            transform
                .ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)
                .context("purge initiale de l'encodeur H.264 (transform matériel)")?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .context("démarrage du flux de l'encodeur H.264 (transform matériel)")?;
            transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .context("début de flux de l'encodeur H.264 (transform matériel)")?;
        }

        // Convertisseur BGRA→NV12, partageant le même périphérique D3D.
        let converter = fabrique::create_color_converter(&device_manager, capture, encode, fps)?;
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
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0) }
            .context("démarrage du flux du convertisseur de couleur (Video Processor MFT)")?;
        unsafe { converter.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0) }
            .context("début de flux du convertisseur de couleur (Video Processor MFT)")?;

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
            file_encodeur,
            capture,
            encode,
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

    /// Occupe la file de travail imposée à la MFT encodeur pendant `duree`.
    ///
    /// **Sonde de mesure, jamais appelée en exploitation** : elle éprouve si le
    /// travail asynchrone de la MFT transite réellement par la file qu'on lui
    /// impose. Si oui, la boucher doit arrêter l'encodeur ; si l'encodeur
    /// continue, la barrière de `arret::FileMft` ne barre rien et il faut le
    /// savoir. Voir le mode d'échec résiduel documenté sur `arret::FileMft`.
    pub fn eprouver_file(&self, duree: std::time::Duration) {
        self.file_encodeur.bloquer(duree);
    }

    /// Force la production d'une image clé sur l'image suivante.
    pub fn request_keyframe(&mut self) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let value = reglages::variant_bool(true);
        unsafe { codec.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &value) }?;
        Ok(())
    }

    /// Change le débit cible sans reconstruire l'encodeur.
    ///
    /// `ICodecAPI::SetValue` à chaud est déjà éprouvé sur ce pilote par
    /// `request_keyframe`, qui écrit `AVEncVideoForceKeyFrame` en cours de
    /// session sur ce même objet.
    ///
    /// Un refus du pilote est rendu à l'appelant plutôt que journalisé ici :
    /// c'est `WindowsSource` qui sait s'il doit continuer par la résolution
    /// (voir tâche 7).
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        let codec: ICodecAPI = self.transform.cast()?;
        let rate = reglages::variant_u32(bitrate);
        unsafe { codec.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &rate) }
            .context("réglage à chaud du débit d'encodage")?;
        Ok(())
    }

    /// Taille réellement encodée. Distincte de la taille capturée depuis que
    /// la résolution s'adapte au lien.
    pub fn encode_size(&self) -> (u32, u32) {
        self.encode
    }
}

impl Drop for EncodeurMft {
    fn drop(&mut self) {
        if self.skipped_busy > 0 {
            tracing::debug!(
                skipped_busy = self.skipped_busy,
                "images renoncées faute de confirmation du convertisseur (diagnostic)"
            );
        }
        // Mise au repos AVANT le relâchement des références COM : rien
        // ne demandait jamais à la MFT matérielle de cesser ses traitements
        // asynchrones, et c'est la course que la tâche 2bis a relevée. Voir
        // `arret::mettre_au_repos` pour le détail et le relevé qui le motive.
        arret::mettre_au_repos(&self.converter, &self.transform, &self.file_encodeur);

        // `MFShutdown` n'est appelé nulle part, et c'est délibéré : voir
        // `demarrer_media_foundation`. La ligne ci-dessous est INERTE (emprunt
        // aussitôt jeté, zéro code machine) : à retirer hors branche de mesure.
        let _ = &self.device_manager;
    }
}
