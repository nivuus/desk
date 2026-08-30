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
//! type d'entrée disponible (voir `fabrique::log_supported_input_types`, qui
//! journalise
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
//! sortie.** Voir `mft::convertisseur::take_output_sample` pour le mécanisme
//! exact, et `mft::convertisseur::feed_converter` pour le modèle de drainage
//! qui en découle. En résumé :
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
//! qu'un fil de surveillance journalise chaque seconde (voir `diagnostics/capture.rs`,
//! `ENCODER_THROUGHPUT_TEST`). Le débit mesuré est consigné dans le rapport
//! de tâche.

#![cfg(windows)]

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, OnceLock};

use anyhow::{bail, Result};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Media::MediaFoundation::*;

use crate::capture::CapturedFrame;
use crate::h264::{group_access_units, AccessUnit};

mod arret;
// Extraits le 30 août 2026 (lot 31), AVANT toute addition : ce fichier
// pesait 1536 lignes. `fabrique` TROUVE et ACTIVE les MFT, `reglages` POSE
// les réglages sur une MFT obtenue. Enfants ordinaires (un simple `mod`
// chez leur parent gaté) et non `#[path]` : ils n'ont jamais besoin de
// sortir du `#![cfg(windows)]` ci-dessus — voir la convention de module
// enfant dans `CLAUDE.md`.
mod fabrique;
mod reglages;

// Le chemin Media Foundation, extrait le 30 août 2026 (lot 31) AVANT le
// branchement qu'il prépare. `H264Encoder` est RÉEXPORTÉ juste en dessous :
// les ~10 appelants du dépôt continuent d'écrire `crate::encode::H264Encoder`,
// et aucun des huit verbes n'a changé de signature.
mod mft;
mod natif;

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
/// preneur d'une nouvelle entrée (voir `mft::convertisseur::feed_converter`).
/// Strictement borné :
/// ce MFT annonce une sortie prête en permanence, donc une boucle sans borne
/// viderait son pool et bloquerait une seconde entière sur
/// `MF_E_SAMPLEALLOCATOR_EMPTY`. En régime nominal, une itération suffit.
const MAX_CONVERTER_COLLECTS: usize = 4;

/// Nombre d'échantillons NV12 conservés d'avance, prêts à être remis à
/// l'encodeur dès qu'il en réclame un.
///
/// **C'est le correctif du plafond de débit à ~30 i/s** (voir `submit`). La
/// capture (Desktop Duplication) et l'encodeur matériel ont deux rythmes
/// indépendants : garder une image convertie d'avance est ce qui permet de
/// servir une demande d'entrée arrivée à un tour où le bureau, lui, n'a rien
/// changé.
///
/// Pourquoi 1 et pas davantage : chaque image gardée en attente est une image
/// affichée avec un tour de retard. À 60 Hz, 1 suffit pour couvrir le
/// déphasage entre les deux rythmes (au plus un tour d'écart) sans ajouter
/// plus de 16,7 ms au budget de latence. Au-delà, on n'achèterait plus de
/// débit, seulement de la latence.
///
/// **Vérifié le 28/07** : porter cette file à 4 ne rend que ~3 images/s (47,5
/// → 51). Ce n'était donc pas le facteur limitant non plus — la profondeur 1
/// est conservée, car c'est celle qui coûte le moins de latence.
const MAX_PENDING_NV12: usize = 1;

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
/// Hors encodeur : l'appelant est dans l'acquisition d'image. Posée depuis la
/// boucle de diagnostic (`diagnostics/capture.rs`) pour que le fil de surveillance distingue
/// « bloqué dans l'encodeur » de « bloqué dans la capture » — sans quoi les
/// deux se ressemblent : compteurs figés, étape au repos.
pub const PHASE_CAPTURE: u64 = 8;
/// Sous-étapes de `DesktopCapture::next_frame`, publiées depuis `capture.rs`.
///
/// `PHASE_CAPTURE` seule ne suffit pas : elle couvre trois appels Windows aux
/// modes de panne très différents (acquisition DXGI, copie GPU, libération
/// d'image). Sans les distinguer, corriger un blocage dans cette fonction
/// reviendrait à corriger à l'aveugle.
pub const PHASE_CAPTURE_ACQUIRE: u64 = 9;
pub const PHASE_CAPTURE_CROP: u64 = 10;
pub const PHASE_CAPTURE_RELEASE: u64 = 11;

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
        PHASE_CAPTURE => "capture/next_frame",
        PHASE_CAPTURE_ACQUIRE => "capture/AcquireNextFrame",
        PHASE_CAPTURE_CROP => "capture/CopySubresourceRegion",
        PHASE_CAPTURE_RELEASE => "capture/ReleaseFrame",
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
    ///
    /// Derrière un `Arc` propre pour être partageable avec `DesktopCapture`,
    /// qui publie ses propres sous-étapes sans rien connaître du reste de la
    /// télémétrie de l'encodeur.
    pub phase: Arc<AtomicU64>,
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
    /// Échantillons NV12 convertis puis écartés parce qu'une image plus
    /// récente était disponible avant que l'encodeur ne les réclame (voir
    /// `MAX_PENDING_NV12`). Attendu proche de zéro en régime nominal : une
    /// valeur qui monte signifie que l'encodeur ne suit plus la capture.
    pub dropped_stale_nv12: AtomicU64,
}

/// Compteurs de diagnostic à l'échelle du processus, doublant deux champs de
/// `EncoderTelemetry` (voir `SOURCE_TRACE` dans `demarrage.rs`).
///
/// Redondants en apparence, mais `EncoderTelemetry` est possédée par
/// l'encodeur, que `resize` remplace : une poignée prise au démarrage cesse
/// d'être alimentée dès la première reconstruction — c'est-à-dire dès le
/// premier redimensionnement demandé par le navigateur, donc dans toutes les
/// sessions réelles. Ces statiques traversent les reconstructions.
pub static NEED_INPUT_EVENTS: AtomicU64 = AtomicU64::new(0);
pub static ENCODER_INPUTS: AtomicU64 = AtomicU64::new(0);
pub static DROPPED_STALE: AtomicU64 = AtomicU64::new(0);

/// Temps cumulé (ns) dans les trois appels Media Foundation du chemin chaud,
/// pour départager le convertisseur BGRA→NV12 de l'encodeur lui-même.
///
/// `submit` englobe la conversion ET la soumission ; sans cette ventilation,
/// un `submit` lent ne dit pas lequel des deux MFT coûte.
pub static CONVERT_NS: AtomicU64 = AtomicU64::new(0);
pub static ENC_IN_NS: AtomicU64 = AtomicU64::new(0);
pub static ENC_OUT_NS: AtomicU64 = AtomicU64::new(0);

/// Bilan matière du convertisseur BGRA→NV12, à l'échelle du processus.
///
/// Sans ces trois-là, le compte des images ne boucle pas : la trace montrait
/// 68,5 images capturées par seconde pour 47,5 remises à l'encodeur et 12,5
/// déclarées périmées, soit 8,5 disparues sans trace. Elles se perdent ici —
/// une entrée refusée (`converter_not_accepting`) ou une sortie non collectée
/// (pool vide, `ConverterPoll::Busy`) fait sortir `mft::convertisseur::feed_converter`
/// sans avoir
/// rien empilé dans `pending_nv12`, et l'image n'est jamais reproposée.
pub static CONVERTER_INPUTS: AtomicU64 = AtomicU64::new(0);
pub static CONVERTER_OUTPUTS: AtomicU64 = AtomicU64::new(0);
pub static CONVERTER_SKIPPED: AtomicU64 = AtomicU64::new(0);



/// Démarre Media Foundation, UNE SEULE FOIS pour la vie du processus, et ne
/// l'arrête JAMAIS.
///
/// **Ce que cela remplace.** Une garde RAII appariait `MFStartup` et
/// `MFShutdown` sur la vie de chaque `H264Encoder` : détruire le dernier
/// encodeur démontait donc toute la plateforme Media Foundation. C'est
/// l'instant où la tâche 2bis avait relevé la faute — fil principal dans
/// `MFShutdown` → `RtwqShutdown` → `CPlatform::FinalShutdown` pendant qu'un
/// élément de travail de la MFT NVIDIA courait encore.
///
/// **`MFShutdown` n'est pourtant PAS nécessaire à la faute, et c'est mesuré
/// ici** : avec cet appel entièrement retiré du chemin, la faute est revenue,
/// pile identique, décalage identique, et cette fois APRÈS la libération
/// complète de l'encodeur (1 récidive sur 5 exécutions).
///
/// Portée exacte de ce relevé, à ne pas dépasser : il établit que la faute
/// **peut survenir sans** `MFShutdown`, donc que cet appel n'en est pas une
/// condition nécessaire. Il n'établit PAS que sa présence dans les deux
/// vidages de 2bis était inerte — deux chemins menant au même symptôme peuvent
/// coexister. La barrière de `arret::FileMft` est ce qui traite la course ; ce
/// changement-ci ne la traite pas.
///
/// **Pourquoi le garder malgré tout.** Aucun processus ne gagne rien à arrêter
/// Media Foundation : `MFShutdown` ne sert qu'à rendre des ressources juste
/// avant de mourir, et le système les reprend de toute façon à la fin du
/// processus. En contrepartie, plus aucune destruction d'encodeur ne démonte
/// puis ne remonte la plateforme — ce qui compte pour le chantier à venir, où
/// fermer une fenêtre détruira son encodeur pendant que d'autres continuent
/// d'encoder.
fn demarrer_media_foundation() -> Result<()> {
    static DEMARRAGE: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    match DEMARRAGE.get_or_init(|| {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }.map_err(|err| err.to_string())
    }) {
        Ok(()) => Ok(()),
        // Un échec est mémorisé : le réessayer à chaque encodeur ne ferait que
        // répéter la même panne, et `MFStartup` n'est pas un appel à retenter.
        Err(message) => bail!("démarrage de Media Foundation : {message}"),
    }
}





/// **La façade.** Un encodeur H.264, quel que soit le dos qui l'exécute.
///
/// 🔴 **LES HUIT VERBES N'ONT PAS CHANGÉ DE SIGNATURE**, et c'est la
/// contrainte qui a gouverné cette conception : les ~10 appelants du dépôt
/// n'ont pas bougé d'une ligne.
///
/// ## L'ordre des dos, et POURQUOI — avec de quoi le refaire
///
/// 1. **NVENC natif** quand un adaptateur NVIDIA est présent.
/// 2. **La MFT**, inchangée, pour tout le reste.
///
/// 🔴 **LA MFT N'EST PAS UN PIS-ALLER, C'EST LE DOS GÉNÉRIQUE.** `MFTEnumEx`
/// n'énumère pas « l'encodeur NVIDIA » : il énumère **les encodeurs H.264
/// matériels**, Intel Quick Sync et AMD VCE compris. Une machine sans NVIDIA
/// n'a aucun NVENC ; la lui retirer la priverait de **tout** encodeur
/// matériel. Trois tests d'hôte figent cette régression
/// (`encode_nvenc::tests`).
///
/// **Pourquoi NVENC d'abord** : sur la VM cible, le 30 août 2026, la MFT
/// `NVIDIA H.264 Encoder MFT` s'active en session 0 et rend `0x8000FFFF` en
/// **session 1** — celle où le produit tourne — sur les quatre arrangements
/// que Media Foundation permet. Deux témoins verts posés dans la même
/// exécution (encodeur H.264 **logiciel**, processeur vidéo **logiciel**)
/// établissent que la machinerie n'est pas en cause. Apollo, sur la même
/// machine et dans la même session, fabrique six encodeurs par la porte
/// native.
///
/// **Refaire la mesure**, plutôt que de me croire :
///
/// ```text
/// (Get-Process sunshine).Modules | ? { $_.ModuleName -match 'mfplat|nvEnc' }
/// Select-String 'NvEnc: created encoder' 'C:\Program Files\Apollo\config\sunshine.log'
/// ```
///
/// Détail, relevés bruts et **trois remèdes réfutés par la mesure** :
/// `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`.
pub enum H264Encoder {
    /// L'API NVENC native — la porte qu'Apollo emprunte.
    Natif(natif::EncodeurNatif),
    /// La MFT Media Foundation — le dos **générique**.
    Mft(mft::EncodeurMft),
}

impl H264Encoder {
    pub fn new(
        device: &ID3D11Device,
        capture: (u32, u32),
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        let adaptateurs = fabrique::adaptateurs_dxgi();
        if let crate::encode_nvenc::Voie::Nvenc(index) =
            crate::encode_nvenc::choisir_voie(&adaptateurs)
        {
            let vu = &adaptateurs[index];
            match natif::EncodeurNatif::new(device, encode, fps, bitrate) {
                Ok(encodeur) => {
                    tracing::info!(
                        adaptateur = %vu.nom,
                        luid = format!("{:08X}:{:08X}", vu.luid.0, vu.luid.1),
                        "encodeur NVENC natif retenu"
                    );
                    return Ok(Self::Natif(encodeur));
                }
                // 🔴 **Le repli est BRUYANT, à dessein.** Retomber en silence
                // sur la MFT ferait lire « NVENC marche » à qui voit une
                // session s'établir, alors que le dos qui tourne est l'autre.
                Err(erreur) => tracing::warn!(
                    %erreur,
                    adaptateur = %vu.nom,
                    "NVENC natif indisponible : repli sur la MFT Media Foundation"
                ),
            }
        }
        Ok(Self::Mft(mft::EncodeurMft::new(
            device, capture, encode, fps, bitrate,
        )?))
    }

    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        match self {
            Self::Natif(e) => e.telemetry(),
            Self::Mft(e) => e.telemetry(),
        }
    }

    pub fn eprouver_file(&self, duree: std::time::Duration) {
        match self {
            Self::Natif(e) => e.eprouver_file(duree),
            Self::Mft(e) => e.eprouver_file(duree),
        }
    }

    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        match self {
            Self::Natif(e) => e.submit(frame, pts_90k),
            Self::Mft(e) => e.submit(frame, pts_90k),
        }
    }

    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        match self {
            Self::Natif(e) => e.poll_output(),
            Self::Mft(e) => e.poll_output(),
        }
    }

    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        match self {
            Self::Natif(e) => e.flush_pending_inputs(),
            Self::Mft(e) => e.flush_pending_inputs(),
        }
    }

    pub fn request_keyframe(&mut self) -> Result<()> {
        match self {
            Self::Natif(e) => e.request_keyframe(),
            Self::Mft(e) => e.request_keyframe(),
        }
    }

    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        match self {
            Self::Natif(e) => e.set_bitrate(bitrate),
            Self::Mft(e) => e.set_bitrate(bitrate),
        }
    }

    pub fn encode_size(&self) -> (u32, u32) {
        match self {
            Self::Natif(e) => e.encode_size(),
            Self::Mft(e) => e.encode_size(),
        }
    }
}
