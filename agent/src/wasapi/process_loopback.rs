//! The *process loopback*: capturing the audio of a single process, and of its tree.
//!
//! **Extracted from `wasapi.rs` and not added to it.** That file is at
//! 543 lines, `#[cfg(windows)]`, without any test: it is frozen debt in
//! `CLAUDE.md`'s sense, and the repository's rule requires that a substantial
//! addition there come with an extraction. All the asynchronous COM
//! machinery of `ActivateAudioInterfaceAsync` therefore lives here, where
//! sub-block D7's code belongs.
//!
//! **What is peculiar about this API**: it is not synchronous. The
//! result does not arrive as a call return but through
//! `IActivateAudioInterfaceCompletionHandler::ActivateCompleted`, invoked
//! from a COM pool thread — hence the shared state and the `Condvar` below.

#![cfg(windows)]

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use windows::core::{implement, Interface, Ref};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX, WAVE_FORMAT_PCM,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, BLOB, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_BLOB;

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};

// ---------------------------------------------------------------------------
// Probe no. 4 of workstream A's spec (§11): the *process loopback*.
//
// Answers a question of workstream D, not of this one: nothing is built
// on this probe here, it only observes whether activation succeeds on
// this VM, and the result is recorded for workstream D (multi-window
// model, which needs to isolate audio per window/process).
// ---------------------------------------------------------------------------

/// State shared between the calling thread of `probe_process_loopback` and the
/// COM callback of `ActivateAudioInterfaceAsync`, which runs on a thread of the
/// COM thread pool — not necessarily the one that launched the call.
struct EtatActivation {
    resultat: Mutex<Option<ResultatActivation>>,
    signal: Condvar,
}

/// Result of the activation, as deposited by the callback.
///
/// SAFETY: `IAudioClient` is not `Send` by default (same reason as
/// `LoopbackCapture`, in the PARENT module `agent/src/wasapi.rs` — refer
/// to it for the full reasoning, this file being only an
/// extraction of that one). It is not a problem here: `activer_pour_processus`
/// (called by `probe_process_loopback` AND by `CaptureProcessus::ouvrir`)
/// joins the MTA before calling `ActivateAudioInterfaceAsync`, and its
/// completion callback necessarily runs on a thread that is itself
/// a member of that MTA (that is what Microsoft documents for this API).
/// Passing this value to the calling thread, once the callback has
/// signalled through the `Condvar` below, therefore violates no COM
/// apartment constraint.
struct ResultatActivation(Result<IAudioClient>);
unsafe impl Send for ResultatActivation {}

/// COM completion handler for `ActivateAudioInterfaceAsync`.
///
/// This API is **asynchronous with a callback**: the result does not arrive as a call
/// return but through `ActivateCompleted`, invoked from a COM pool thread.
/// We deposit the result in the shared state and wake the calling thread.
#[implement(IActivateAudioInterfaceCompletionHandler)]
struct GestionnaireCompletion {
    etat: Arc<EtatActivation>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for GestionnaireCompletion_Impl {
    fn ActivateCompleted(
        &self,
        activateoperation: Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let resultat: Result<IAudioClient> = (|| {
            let operation = activateoperation
                .ok()
                .context("le rappel d'activation n'a rendu aucune opération")?;
            let mut hr = windows::core::HRESULT::default();
            let mut interface: Option<windows::core::IUnknown> = None;
            unsafe { operation.GetActivateResult(&mut hr, &mut interface) }
                .context("GetActivateResult")?;
            hr.ok().context("activation du process loopback refusée")?;
            interface
                .context("GetActivateResult a réussi sans rendre d'interface")?
                .cast::<IAudioClient>()
                .context("l'interface activée n'est pas un IAudioClient")
        })();

        let mut verrou = self.etat.resultat.lock().unwrap_or_else(|e| e.into_inner());
        *verrou = Some(ResultatActivation(resultat));
        self.etat.signal.notify_one();
        Ok(())
    }
}

/// Maximum wait for the activation callback.
const DELAI_RAPPEL_ACTIVATION: Duration = Duration::from_secs(10);

/// Activates a *process loopback* `IAudioClient` for `pid` and waits for it.
///
/// `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK` (Windows 10 build 19041+)
/// isolates the audio of a single process (and its children) — exactly what
/// workstream D's multi-window model requires (one Windows window = one
/// browser window, hence potentially one audio track per window
/// rather than a single global loopback).
///
/// **Extracted from `probe_process_loopback`, which now calls it** (task 2
/// of sub-block D7): the probe and `CaptureProcessus::ouvrir` must activate
/// **exactly the same way**, otherwise the probe would not measure what
/// the product does.
fn activer_pour_processus(pid: u32) -> Result<IAudioClient> {
    unsafe {
        // Same safeguard as `LoopbackCapture::open` (PARENT module
        // `agent/src/wasapi.rs`): see its comment for the full reasoning
        // on `RPC_E_CHANGED_MODE`.
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr == RPC_E_CHANGED_MODE {
            bail!(
                "activation du process loopback refusée : le fil appelant appartient déjà à une \
                 STA, pas à la MTA qu'exige cette API (voir `LoopbackCapture::open` dans \
                 `agent/src/wasapi.rs` pour le même garde-fou)"
            );
        }

        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: pid,
                    // INCLUDE and not EXCLUDE: it is indeed the audio OF this
                    // process (and its children) we want to isolate, not
                    // that of the whole rest of the machine.
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };

        // `ActivateAudioInterfaceAsync` expects its parameters in the form
        // of a PROPVARIANT of type VT_BLOB carrying a raw pointer to
        // `params` — the Rust counterpart of `PropVariantInit` then manual
        // assignment of the `vt`/`blob` fields in C++. `params` must stay alive
        // until the end of the synchronous call (only the ACTIVATION itself
        // is asynchronous, reading the parameters is not): both
        // values stay in this same `unsafe` scope.
        // `Anonymous` (the first level) is a `ManuallyDrop<...>` field
        // of a COM union: `ManuallyDrop`'s implicit auto-dereference
        // is not applied on a union field by the compiler (it
        // would otherwise have to call the destructor of the old active value,
        // undetermined) — hence the explicit `*`.
        //
        // FIX (review): `PROPVARIANT` implements `Drop`
        // (`windows-0.62.2/src/extensions/Win32/System/StructuredStorage.rs`)
        // and calls `PropVariantClear` — which, for `VT_BLOB`, releases
        // `blob.pBlobData` through `CoTaskMemFree`. Yet `pBlobData` points here to
        // `params`, a STACK variable, not a `CoTaskMemAlloc` allocation:
        // letting this `Drop` run (on EVERY exit path, including the
        // `?` of `ActivateAudioInterfaceAsync` just below) calls
        // `CoTaskMemFree` on a stack address — outright undefined
        // behaviour, the only plausible cause of the corruption that made the probe
        // silent (no log line, no crash report
        // consistent with the failure point). `ManuallyDrop` prevents this `Drop`:
        // nothing needs to be freed, `blob` references no memory
        // this PROPVARIANT owns.
        let mut propriete = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        (*propriete.Anonymous.Anonymous).vt = VT_BLOB;
        (*propriete.Anonymous.Anonymous).Anonymous.blob = BLOB {
            cbSize: std::mem::size_of_val(&params) as u32,
            pBlobData: &mut params as *mut AUDIOCLIENT_ACTIVATION_PARAMS as *mut u8,
        };

        let etat = Arc::new(EtatActivation {
            resultat: Mutex::new(None),
            signal: Condvar::new(),
        });
        let gestionnaire: IActivateAudioInterfaceCompletionHandler =
            GestionnaireCompletion { etat: etat.clone() }.into();

        // The returned operation must stay alive until the end of the wait:
        // dropping it prematurely can cancel the ongoing activation.
        let _operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*propriete),
            &gestionnaire,
        )
        .context("appel à ActivateAudioInterfaceAsync")?;

        let verrou = etat.resultat.lock().unwrap_or_else(|e| e.into_inner());
        let (mut verrou, _attente) = etat
            .signal
            .wait_timeout_while(verrou, DELAI_RAPPEL_ACTIVATION, |r| r.is_none())
            .unwrap_or_else(|e| e.into_inner());

        match verrou.take() {
            Some(ResultatActivation(Ok(client))) => Ok(client),
            Some(ResultatActivation(Err(e))) => Err(e).context("activation refusée"),
            None => bail!(
                "aucun rappel d'activation reçu en {DELAI_RAPPEL_ACTIVATION:?} pour le PID {pid}"
            ),
        }
    }
}

/// ACTIVATION-only probe — kept as is so that workstream A's
/// reading (July 28th, 2026) stays reproducible identically. It
/// initialises nothing and reads no byte: it is `CaptureProcessus::ouvrir`
/// that goes further, and `PROCESS_LOOPBACK_CAPTURE` that measures it.
pub fn probe_process_loopback(pid: u32) -> Result<String> {
    let _client = activer_pour_processus(pid)?;
    Ok(format!(
        "activation réussie : IAudioClient obtenu pour le PID {pid} \
         (VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, INCLUDE_TARGET_PROCESS_TREE)"
    ))
}

/// Duration of the requested buffer, in 100 ns units. 200 ms, like
/// `LoopbackCapture::open`: a wide margin to absorb a late loop round
/// without losing a sample.
const DUREE_TAMPON_100NS: i64 = 2_000_000;

/// Loopback capture of a single process and its tree.
///
/// **The format is not requested, it is IMPOSED.** A *process
/// loopback* client is bound to no endpoint: `GetMixFormat` has no
/// obvious meaning there, and Microsoft's official sample also sets an
/// explicit format. We therefore set exactly the one `opus.rs` expects
/// (48 kHz, 2 channels, 16-bit integers), which at the same time removes any
/// conversion: `read` returns interleaved `i16`s directly usable by
/// `FrameAssembler`.
///
/// ⚠️ **That Windows accepts this format is not established before task 3's
/// measurement.** If it refuses, the exact `HRESULT` is the reading that counts — do not
/// guess a fallback.
pub struct CaptureProcessus {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    description: String,
    /// True when `Start()` has been called without `Stop()` since. **Necessary**:
    /// `IAudioClient::Start` on an already started stream returns
    /// `AUDCLNT_E_NOT_STOPPED`, and arbitration can re-emit an identical
    /// order after a channel reattachment.
    demarre: bool,
}

// SAFETY: same reasoning as `unsafe impl Send for LoopbackCapture`
// (PARENT module `agent/src/wasapi.rs`, whose comment must be read
// first). `activer_pour_processus` checks that the calling thread is a member
// of the MTA and refuses `RPC_E_CHANGED_MODE`; the capture thread of
// `windows_audio.rs` joins that same MTA before any COM call.
// **The promise covers the whole struct, future fields included.**
unsafe impl Send for CaptureProcessus {}

impl CaptureProcessus {
    pub fn ouvrir(pid: u32) -> Result<Self> {
        let client = activer_pour_processus(pid)?;
        unsafe {
            let mut format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: SAMPLE_RATE_HZ,
                wBitsPerSample: 16,
                nBlockAlign: (CHANNELS as u16) * 2,
                nAvgBytesPerSec: SAMPLE_RATE_HZ * (CHANNELS as u32) * 2,
                cbSize: 0,
            };
            format.nBlockAlign = format.nChannels * format.wBitsPerSample / 8;
            format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign as u32;

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    DUREE_TAMPON_100NS,
                    0,
                    &format,
                    None,
                )
                .context(
                    "Initialize du client de process loopback (format impose : 48 kHz, \
                     2 canaux, 16 bits)",
                )?;

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("GetService(IAudioCaptureClient) sur le client de process loopback")?;

            // `WAVEFORMATEX` is `repr(packed)` (same reason as
            // `WAVEFORMATEXTENSIBLE` in the parent module): taking a
            // reference into it — which `format!` does for any argument — is an
            // unaligned access, hence undefined behaviour. We first copy
            // the fields into ordinary stack variables.
            let frequence = format.nSamplesPerSec;
            let canaux = format.nChannels;
            let description = format!(
                "process loopback pid={pid} — {frequence} Hz, {canaux} canaux, \
                 16 bits entiers"
            );

            Ok(Self {
                client,
                capture,
                description,
                demarre: false,
            })
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// Starts the stream. Idempotent: a second call does nothing.
    pub fn demarrer(&mut self) -> Result<()> {
        if self.demarre {
            return Ok(());
        }
        unsafe { self.client.Start() }.context("Start du client de process loopback")?;
        self.demarre = true;
        Ok(())
    }

    /// Stops the stream. Idempotent.
    ///
    /// **It is not a destruction**: the client stays activated, so no
    /// COM reactivation — the only step that can refuse — happens at the
    /// next toggle. That is the whole point of the approach chosen in §4.4 of
    /// the spec.
    pub fn arreter(&mut self) -> Result<()> {
        if !self.demarre {
            return Ok(());
        }
        unsafe { self.client.Stop() }.context("Stop du client de process loopback")?;
        self.demarre = false;
        Ok(())
    }

    /// Reads a packet, or `None` if none is ready.
    ///
    /// Returns interleaved `i16`s, without conversion: the format is imposed at
    /// opening.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let disponibles = self
                .capture
                .GetNextPacketSize()
                .context("GetNextPacketSize sur le process loopback")?;
            if disponibles == 0 {
                return Ok(None);
            }

            let mut donnees: *mut u8 = std::ptr::null_mut();
            let mut images = 0u32;
            let mut drapeaux = 0u32;
            self.capture
                .GetBuffer(&mut donnees, &mut images, &mut drapeaux, None, None)
                .context("GetBuffer sur le process loopback")?;

            let echantillons = images as usize * CHANNELS;
            let sortie = if drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                // The SILENT flag allows the driver not to fill the
                // buffer: reading its bytes would return anything.
                vec![0i16; echantillons]
            } else {
                std::slice::from_raw_parts(donnees as *const i16, echantillons).to_vec()
            };

            self.capture
                .ReleaseBuffer(images)
                .context("ReleaseBuffer sur le process loopback")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for CaptureProcessus {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
