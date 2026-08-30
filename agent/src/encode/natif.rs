//! Le dos **NVENC natif** derrière la façade `H264Encoder`.
//!
//! Ce fichier ne parle pas à NVENC : il traduit. D'un côté les huit verbes
//! que le dépôt consomme depuis toujours (`submit`, `poll_output`, …), de
//! l'autre `encode_nvenc::session::SessionNvenc`. Tout ce qui touche à l'ABI
//! vit sous `encode_nvenc`, et **rien de la notice de licence ne déborde
//! ici**.
//!
//! 🟢 **CE CHEMIN A ENCODÉ**, mesuré sur la VM le 30 août 2026 : **1195
//! unités d'accès en 10 s** sur le périphérique de capture réel, porté par
//! l'adaptateur NVIDIA — là où la MFT rendait `0x8000FFFF` et **aucune**
//! unité. (§ 11.1 du document de résultats.)
//!
//! 🟢 **ET SES IMAGES ARRIVENT AU NAVIGATEUR** — `framesDecoded` **+494** et
//! **+484** sur 25 s, deux exécutions, contre **0** sur une source statique
//! dont l'audio coulait pourtant dans le même relevé.
//! 🔴 **CETTE MESURE EST CELLE DU LOT 32, PAS DU LOT 31** : elle a été jouée
//! avec DEUX remèdes en place — ce chemin natif, et la désignation de sortie
//! du lot voisin. Elle établit que ce chemin produit des images qui
//! traversent ; elle n'est pas à porter au crédit de ce fichier seul.
//!
//! ⚠️ **CE QUI N'EST TOUJOURS PAS ÉTABLI**, et qu'il ne faut pas lire dans
//! les lignes ci-dessus : le plafond à **N fenêtres** n'est pas mesuré — le
//! banc n'ouvre qu'un encodeur — et **personne n'a regardé une image**.
//! `framesDecoded` compte des images décodées, il ne dit **rien** de la
//! justesse de ce qui s'affiche.
//!
//! ## Deux différences de fond avec le chemin MFT, et pourquoi elles sont sûres
//!
//! 1. **Pas de convertisseur.** NVENC accepte `NV_ENC_BUFFER_FORMAT_ARGB`,
//!    c'est-à-dire exactement ce que rend la duplication DXGI
//!    (`DXGI_FORMAT_B8G8R8A8_UNORM`). L'étage BGRA→NV12 disparaît du chemin
//!    chaud. ⚠️ **Ce n'est pas un gain mesuré** : personne n'a chiffré ce que
//!    cet étage coûtait, et le lot 31 a seulement établi qu'il est
//!    **logiciel** sur cette machine, faute de MFT matérielle de traitement
//!    vidéo. Le gain est **plausible, pas mesuré**, et ne doit pas être
//!    annoncé autrement.
//! 2. **Pas de file d'événements.** La session est ouverte en mode
//!    synchrone : une image soumise rend sa sortie tout de suite, ou signale
//!    qu'elle a été mise en tampon. `submit` range donc la sortie, et
//!    `poll_output` la rend — ce qui remplit le contrat de la façade sans
//!    qu'aucun appelant ne change.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::capture::CapturedFrame;
use crate::encode::{EncoderTelemetry, PHASE_IDLE};
use crate::encode_nvenc::session::SessionNvenc;
use crate::h264::{group_access_units, AccessUnit};

/// Combien d'unités d'accès on garde entre `submit` et `poll_output`.
///
/// ⚠️ **Une seule**, comme `MAX_PENDING_NV12` du chemin MFT et pour la même
/// raison : servir une image périmée, c'est payer en latence le débit qu'on
/// vient de gagner. Le dépassement est **compté**, pas silencieux.
const MAX_UNITES_EN_ATTENTE: usize = 1;

pub struct EncodeurNatif {
    session: SessionNvenc,
    en_attente: VecDeque<AccessUnit>,
    telemetry: Arc<EncoderTelemetry>,
    fps: u32,
}

impl EncodeurNatif {
    pub fn new(
        peripherique: &ID3D11Device,
        encode: (u32, u32),
        fps: u32,
        bitrate: u32,
    ) -> Result<Self> {
        let session = SessionNvenc::ouvrir(peripherique, encode.0, encode.1, fps, bitrate)?;
        Ok(Self {
            session,
            en_attente: VecDeque::new(),
            telemetry: Arc::new(EncoderTelemetry::default()),
            fps: fps.max(1),
        })
    }

    pub fn telemetry(&self) -> Arc<EncoderTelemetry> {
        Arc::clone(&self.telemetry)
    }

    pub fn encode_size(&self) -> (u32, u32) {
        self.session.taille()
    }

    pub fn request_keyframe(&mut self) -> Result<()> {
        self.session.demander_image_cle();
        Ok(())
    }

    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        if bitrate == self.session.debit() {
            return Ok(());
        }
        self.session.regler_debit(bitrate)
    }

    /// ⚠️ **Sans objet ici, et ce n'est PAS un oubli.** Ce verbe existe pour
    /// la MFT asynchrone, dont les entrées attendent un événement
    /// `METransformNeedInput`. Le mode synchrone n'a rien qui attende : une
    /// image soumise est encodée à l'appel. Rendre `Ok(())` est donc le
    /// comportement JUSTE, pas un silence de complaisance.
    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        Ok(())
    }

    /// ⚠️ **Sans objet ici non plus** : ce verbe bouche la file de travail
    /// que la MFT asynchrone impose, et le mode synchrone n'en a aucune.
    pub fn eprouver_file(&self, _duree: std::time::Duration) {}

    pub fn submit(&mut self, frame: &CapturedFrame, pts_90k: u64) -> Result<()> {
        self.telemetry.submit_calls.fetch_add(1, Ordering::Relaxed);
        // Media Foundation comptait en 100 ns ; NVENC prend ce qu'on lui
        // donne et le rend tel quel. On garde l'unité de la façade — 1/90000 s
        // — pour que `poll_output` n'ait rien à reconvertir.
        let octets = self.session.encoder(&frame.texture, pts_90k)?;
        self.telemetry
            .encoder_inputs
            .fetch_add(1, Ordering::Relaxed);
        crate::encode::ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);

        let Some(octets) = octets else {
            // L'encodeur a mis l'image en tampon : cas normal, pas une perte.
            return Ok(());
        };
        let mut unites = group_access_units(&octets, self.fps);
        if unites.is_empty() {
            return Ok(());
        }
        let mut unite = unites.remove(0);
        unite.pts_90k = pts_90k;
        self.en_attente.push_back(unite);
        self.telemetry
            .encoder_outputs
            .fetch_add(1, Ordering::Relaxed);

        // Ne garder que la plus récente, et COMPTER ce qu'on écarte : un
        // rejet muet ferait lire « l'encodeur suit » à qui regarde les
        // compteurs, alors qu'il décroche.
        while self.en_attente.len() > MAX_UNITES_EN_ATTENTE {
            self.en_attente.pop_front();
            self.telemetry
                .dropped_stale_nv12
                .fetch_add(1, Ordering::Relaxed);
            crate::encode::DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn poll_output(&mut self) -> Result<Option<AccessUnit>> {
        self.telemetry.phase.store(PHASE_IDLE, Ordering::Relaxed);
        Ok(self.en_attente.pop_front())
    }
}
