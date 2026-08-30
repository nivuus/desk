//! La moitié **encodeur** de la pompe : la MFT H.264 *asynchrone*, pilotée
//! par événements (`METransformNeedInput` / `METransformHaveOutput`).
//!
//! Extrait d'`encode.rs` avec le reste du chemin MFT (lot 31). Son pendant est
//! `super::convertisseur`, dont il appelle `feed_converter` et
//! `take_output_sample` — les deux seuls, et c'est pourquoi ce sont les deux
//! seuls à être `pub(super)` là-bas.

use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};
use windows::Win32::Media::MediaFoundation::*;

// ⚠️ **Importation globale, et c'est un choix motivé.** Ce fichier est la
// CONTINUATION d'`encode.rs` : phases, compteurs publics et constantes de
// réglage y vivent, et les énumérer ici en donnerait une seconde liste à
// tenir à jour — celle qui se désynchronise. La règle du dépôt vise les
// affirmations recopiées ; une importation globale n'en recopie aucune.
use crate::encode::*;
// Les deux seuls elements que ce fichier emprunte a son pendant.
use super::convertisseur::take_output_sample;
impl H264Encoder {
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
                    NEED_INPUT_EVENTS.fetch_add(1, Ordering::Relaxed);
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

    /// Convertit l'image capturée et la remet à l'encodeur dès qu'il en
    /// réclame une.
    ///
    /// **Correction du plafond de débit à ~30 i/s (28/07).** La version
    /// précédente ne convertissait l'image que si `pending_nv12` ne suffisait
    /// pas déjà à satisfaire les demandes d'entrée en attente :
    ///
    /// ```ignore
    /// if (self.pending_nv12.len() as u32) < self.pending_input_requests {
    ///     self.feed_converter(frame, sample_time, duration)?;
    /// }
    /// ```
    ///
    /// L'intention (« ne pas convertir d'avance, ce ne serait que de la
    /// latence ») était juste pour une source qu'on peut réinterroger à
    /// volonté — elle est fausse pour celle-ci. `AcquireNextFrame` ne signale
    /// un contenu qu'**une fois** : l'image que ce garde écartait n'était pas
    /// remise à plus tard, elle était **perdue définitivement**. Au tour
    /// suivant, quand l'encodeur réclamait enfin une entrée, la capture
    /// n'avait plus rien à donner (le bureau n'avait pas rechangé), et la
    /// demande restait en souffrance jusqu'au tour d'après. D'où un
    /// verrouillage en antiphase à une image tous les deux tours de
    /// `Session::run` : 60 Hz / 2 = **exactement le plafond de ~30 i/s**
    /// mesuré de bout en bout, six fois, par les rondes précédentes.
    ///
    /// Ce garde explique aussi pourquoi les deux expériences qui auraient dû
    /// trancher n'ont rien montré : `ENCODER_THROUGHPUT_TEST`/`ENCODE_TEST`
    /// (`diagnostics/capture.rs`) resoumettent **la même texture** en boucle, si bien qu'y
    /// jeter une image ne coûte rien — d'où les ~80 i/s qui semblaient
    /// disculper le code et accuser le pilote NVENC ; et forcer la capture à
    /// 60 Hz (`remesure-debit.md`, étape 3a) n'a pas bougé le débit, les
    /// captures supplémentaires retombant toutes dans ce même garde.
    ///
    /// Le pilotage correct découple les deux rythmes : on convertit
    /// systématiquement ce que la capture a donné, on n'en garde d'avance que
    /// `MAX_PENDING_NV12` (la plus récente — une image plus ancienne est
    /// périmée pour un flux interactif), et on sert les demandes d'entrée
    /// avec ce qui est prêt.
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

        // Convertir sans condition : l'image ne sera jamais reproposée par la
        // capture (voir le commentaire de méthode).
        let t_convert = std::time::Instant::now();
        let converted = self.feed_converter(frame, sample_time, duration);
        CONVERT_NS.fetch_add(t_convert.elapsed().as_nanos() as u64, Ordering::Relaxed);
        converted?;

        // Ne garder que les plus récentes. `feed_converter` empile en queue,
        // donc les périmées sont en tête. Les retirer ici plutôt que de
        // laisser la file croître évite de servir à l'encodeur une image déjà
        // dépassée au moment où il la réclame — ce serait payer en latence le
        // débit qu'on vient de gagner.
        while self.pending_nv12.len() > MAX_PENDING_NV12 {
            self.pending_nv12.pop_front();
            self.telemetry.dropped_stale_nv12.fetch_add(1, Ordering::Relaxed);
            DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
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
            ENC_IN_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
            fed.context("soumission de l'image NV12 à l'encodeur")?;
            self.telemetry
                .encoder_inputs
                .fetch_add(1, Ordering::Relaxed);
            ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);
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
        ENC_OUT_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
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

    /// Remet à l'encodeur les échantillons NV12 déjà prêts, pour chaque
    /// demande d'entrée qu'il a émise depuis le dernier appel.
    ///
    /// Existe pour casser une sérialisation mesurée dans `Session::run` : le
    /// cycle « soumettre → l'encodeur produit → récupérer la sortie →
    /// l'encodeur libère un emplacement → il redemande une entrée » ne
    /// franchissait qu'UNE étape par tour de boucle, puisque `submit` et
    /// `poll_output` n'étaient appelés qu'une fois chacun par tour de 16,7 ms.
    /// Le débit s'en trouvait plafonné à la cadence de boucle divisée par le
    /// nombre d'étapes — mesuré à ~23 Hz pour 60 Hz de boucle, alors que le
    /// même encodeur soutient 66 Hz sollicité en boucle serrée
    /// (`ENCODE_TEST`), où ces étapes s'enchaînent en quelques microsecondes.
    ///
    /// Appelée après le drainage des sorties : c'est ce drainage qui libère
    /// les emplacements d'entrée, donc c'est juste après lui que la demande
    /// correspondante devient disponible.
    pub fn flush_pending_inputs(&mut self) -> Result<()> {
        self.drain_events()?;
        while self.pending_input_requests > 0 {
            let Some(nv12_sample) = self.pending_nv12.pop_front() else {
                break;
            };
            let t = std::time::Instant::now();
            let fed = unsafe { self.transform.ProcessInput(0, &nv12_sample, 0) };
            ENC_IN_NS.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
            fed.context("soumission différée de l'image NV12 à l'encodeur")?;
            self.telemetry.encoder_inputs.fetch_add(1, Ordering::Relaxed);
            ENCODER_INPUTS.fetch_add(1, Ordering::Relaxed);
            self.pending_input_requests -= 1;
        }
        self.publish_state();
        Ok(())
    }
}
