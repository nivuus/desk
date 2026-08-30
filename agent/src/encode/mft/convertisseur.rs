//! La moitié **convertisseur** de la pompe : BGRA → NV12, par une MFT
//! *synchrone* pilotée par une paire `ProcessInput`/`ProcessOutput`.
//!
//! Extrait d'`encode.rs` avec le reste du chemin MFT (lot 31). Son pendant est
//! `super::encodeur`, qui pilote la MFT *asynchrone*. La frontière entre les
//! deux est celle des deux transforms, pas une coupe arbitraire.
//!
//! ⚠️ **`feed_converter` et `take_output_sample` sont `pub(super)`**, et eux
//! seuls : ce sont les deux seuls éléments que `super::encodeur` appelle —
//! établi par relevé des sites d'appel, pas par supposition. Tout le reste
//! demeure privé à ce fichier.

use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use windows::core::Interface;
use windows::Win32::Media::MediaFoundation::*;

// ⚠️ **Importation globale, et c'est un choix motivé.** Ce fichier est la
// CONTINUATION d'`encode.rs` : phases, compteurs publics et constantes de
// réglage y vivent, et les énumérer ici en donnerait une seconde liste à
// tenir à jour — celle qui se désynchronise. La règle du dépôt vise les
// affirmations recopiées ; une importation globale n'en recopie aucune.
use crate::encode::*;
// Le type du dos MFT vient du PARENT, pas du glob : `crate::encode`
// exporte la FAÇADE, qui n'est pas ce qu'on implémente ici.
use super::EncodeurMft;
use crate::capture::CapturedFrame;

/// Résultat d'un appel à `ProcessOutput` sur le convertisseur BGRA→NV12 (voir
/// `EncodeurMft::drain_converter_output`).
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

impl EncodeurMft {
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
    pub(super) fn feed_converter(&mut self, frame: &CapturedFrame, sample_time: i64, duration: i64) -> Result<()> {
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
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
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
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
                self.publish_state();
                return Ok(());
            }
        }
        result.context("soumission de l'image au convertisseur BGRA→NV12")?;
        self.telemetry
            .converter_inputs
            .fetch_add(1, Ordering::Relaxed);
        CONVERTER_INPUTS.fetch_add(1, Ordering::Relaxed);
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
                CONVERTER_OUTPUTS.fetch_add(1, Ordering::Relaxed);
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
                CONVERTER_SKIPPED.fetch_add(1, Ordering::Relaxed);
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
                Some(fabrique::create_nv12_sample(&self.device, self.encode.0, self.encode.1)?)
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

pub(super) unsafe fn take_output_sample(buffer: &mut MFT_OUTPUT_DATA_BUFFER) -> Option<IMFSample> {
    // `pEvents` est presque toujours nul, mais quand un MFT y dépose une file
    // d'événements elle nous appartient exactement au même titre.
    drop(std::mem::ManuallyDrop::take(&mut buffer.pEvents));
    std::mem::ManuallyDrop::take(&mut buffer.pSample)
}
