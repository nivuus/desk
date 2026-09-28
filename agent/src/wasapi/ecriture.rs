//! The WASAPI render client that **writes** the microphone to the virtual cable.
//!
//! Exact mirror of `LoopbackCapture::open` (`agent/src/wasapi.rs`): same
//! COM apartment check, same RAII guard on the `WAVEFORMATEX`, same
//! explicit refusal of an unexpected format. Only three things change, and
//! they are the only three that are new here:
//!
//! - `AUDCLNT_STREAMFLAGS_LOOPBACK` disappears;
//! - `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` + `SetEventHandle` appear —
//!   **attempted, not assumed** (see [`Reveil`]);
//! - `IAudioCaptureClient::GetBuffer` becomes `IAudioRenderClient::GetBuffer`
//!   followed by `ReleaseBuffer(trames, 0)`, and the available room is computed as
//!   `GetBufferSize() - GetCurrentPadding()`.
//!
//! ## This module is NOT `Send`, and that is deliberate
//!
//! `LoopbackCapture` carries an `unsafe impl Send` whose comment itself
//! names what would invalidate it: "if a future field adds an event
//! `HANDLE` […] this `unsafe impl` would stop being valid without anything
//! flagging it". `RenduWasapi` has exactly that field. Rather than writing
//! the argument that would save the `Send` — the event `HANDLE` is tied to the
//! process, not to the apartment —, we **open on the render thread
//! itself**: no COM object crosses a thread boundary, so there is
//! no promise to keep. It is the simplest approach and the only one without
//! debt (plan E2, task 7).
//!
//! The counterpart is that `windows_micro::ouvrir` cannot know the
//! opening verdict when returning from a `spawn`: it learns it through a channel.
//! See that module.
//!
//! ## This module is NOT tested on the host
//!
//! `#![cfg(windows)]`. Everything that can go wrong has been moved out of it: the
//! cable designation (`wasapi/peripherique.rs`), exclusivity
//! (`micro/exclusivite.rs`), the loop guard (`micro/boucle_locale.rs`) and
//! **the format check** (`wasapi/format.rs`), all pure and tested under
//! Linux. What remains here is COM plumbing only an acceptance run on
//! the VM exercises.

#![cfg(windows)]

use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, WAIT_FAILED};
use windows::Win32::Media::Audio::{
    IAudioClient, IAudioRenderClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::wasapi_format;

/// Duration of the render buffer requested from WASAPI, in 100 ns units. 40 ms.
///
/// ⚠️ **NOT CALIBRATED**, and it joins the list `CLAUDE.md` keeps
/// (`BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE`…).
/// It is not a neutral duration: in steady state the buffer stays **full**
/// — `remplir` never blocks and fills with silence, so all the room
/// returned by `attendre_place` is consumed at each round —, and this duration
/// is therefore **the latency floor the write adds**, on top of
/// that of the jitter buffer. 40 ms is a compromise written rather than measured:
/// enough to absorb four periods of the shared audio engine (10 ms is its
/// usual granularity on this VM, noted in workstream A), little enough not
/// to double end-to-end latency. **No listening judgement has been
/// made.**
const DUREE_TAMPON_100NS: i64 = 400_000;

/// `WAVE_FORMAT_EXTENSIBLE` tag of the `wFormatTag` field. Same constant
/// as in `wasapi.rs`; both halves of the sound read the same field.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// `WAVE_FORMAT_IEEE_FLOAT` tag.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// How the render thread is woken up. **MEASURED, never assumed.**
///
/// Spec §6 asserts that `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` "works
/// here, because it is an ordinary render stream and not a loopback".
/// It was a **prediction**: no code in this repository had ever opened a
/// WASAPI render stream before this block. So we attempt it, we fall back to a
/// deadline loop if `Initialize` refuses it, **and the mode actually obtained
/// is logged** — otherwise an acceptance run would not know what it is
/// measuring (Decision 8 of plan E2).
///
/// ✅ **IT IS NO LONGER A PREDICTION (acceptance E2, task 12, August 20th, 2026).**
/// The event wake mode was logged in all **FIVE** green runs, without exception: the
/// cable accepts `AUDCLNT_STREAMFLAGS_EVENTCALLBACK`, and spec §6 was
/// right. ⚠️ **Corollary not to lose: [`Reveil::Echeance`] has therefore
/// NEVER RUN on this path** — it is code shipped and never taken, just
/// like the `Local\` fallback of `windows_micro/verrou.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveil {
    /// `SetEventHandle` was accepted: WASAPI signals the event at each
    /// period.
    Evenement,
    /// Fallback: we poll `GetCurrentPadding` on a deadline. It is the mode that
    /// E1's measurement thread already uses (`demarrage/micro/mesure.rs`).
    ///
    /// ⚠️ **NEVER OBTAINED on the VM**: acceptance E2 records `Evenement` in the
    /// five green runs. This arm is reasoned and compiled, **not
    /// exercised**.
    Echeance,
}

impl Reveil {
    /// The label that goes to the log.
    pub fn libelle(self) -> &'static str {
        match self {
            Reveil::Evenement => "evenement",
            Reveil::Echeance => "echeance",
        }
    }
}

/// RAII guard for the pointer returned by `IAudioClient::GetMixFormat`.
///
/// Twin of `wasapi::FormatMixage`, duplicated rather than shared: that one
/// is private to its module, and hoisting it would require making `wasapi.rs`
/// depend on a module that depends on it. Seven lines against a cycle.
/// **The guard's reason is the same**: between `GetMixFormat` and `Initialize`,
/// opening can exit through several `?`s — exactly the paths
/// taken the day the machine changes audio configuration — and a
/// free placed at the end of the happy function would miss them.
struct FormatMixage(*mut WAVEFORMATEX);

impl std::ops::Deref for FormatMixage {
    type Target = WAVEFORMATEX;
    fn deref(&self) -> &WAVEFORMATEX {
        // SAFETY: the pointer comes from a successful `GetMixFormat` and is only
        // freed in `Drop`.
        unsafe { &*self.0 }
    }
}

impl Drop for FormatMixage {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

/// Rejoint l'appartement multi-thread, ou refuse.
///
/// To be called **on the render thread**, before any COM call. Copied from the check
/// of `LoopbackCapture::open` and for the same reason: `S_OK` (the thread has just
/// joined the MTA) and `S_FALSE` (it was already a member) are both
/// acceptable; only `RPC_E_CHANGED_MODE` — this thread already belongs to a
/// single-threaded apartment — must make the opening fail.
///
/// ⚠️ **No matching `CoUninitialize`**, as in `wasapi.rs`: the render
/// thread lives as long as the process, and it is never recycled.
pub fn rejoindre_mta() -> Result<()> {
    // SAFETY: COM call without a pointer, on the current thread.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr == RPC_E_CHANGED_MODE {
        bail!(
            "ecriture du micro refusee : le fil de rendu appartient deja a un appartement a \
             thread unique (STA), pas a l'appartement multi-thread (MTA) qu'exige WASAPI"
        );
    }
    Ok(())
}

/// The opened render stream, ready to receive samples.
pub struct RenduWasapi {
    client: IAudioClient,
    rendu: IAudioRenderClient,
    /// `None` en mode [`Reveil::Echeance`].
    evenement: Option<HANDLE>,
    /// `GetBufferSize()`, in frames per channel.
    taille_tampon: u32,
    canaux: usize,
    description: String,
    reveil: Reveil,
}

impl RenduWasapi {
    /// Opens the render stream on `peripherique`, in **SHARED** mode, and
    /// starts it.
    ///
    /// Exclusive mode was weighed and rejected (Decision 7 of plan E2): nothing
    /// establishes that an exclusive stream crosses the cable — it short-circuits
    /// Windows' audio engine, and whether the VB-Cable driver still relays
    /// to CABLE Output is **unknown** —, and the only measured format is that
    /// of shared mode, `GetMixFormat` only describing that one.
    ///
    /// **Refuses** any format that is not 48 kHz stereo 32-bit float,
    /// naming what was encountered: the rule is pure and lives in
    /// `wasapi/format.rs`, where it is tested on the host.
    ///
    /// ⚠️ **Precondition: the current thread is a member of the MTA** — call
    /// [`rejoindre_mta`] first. This function exports no COM object
    /// to another thread, so it assumes being called on the one that will
    /// use it.
    pub fn ouvrir(peripherique: &IMMDevice) -> Result<Self> {
        // SAFETY: `peripherique` comes from an enumerator created on this thread (see
        // the precondition), and each call below immediately follows
        // a successful call.
        unsafe {
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio de rendu (cable)")?;

            let mix = FormatMixage(
                client
                    .GetMixFormat()
                    .context("lecture du format de mixage du cable")?,
            );
            let canaux = mix.nChannels as usize;
            let frequence = mix.nSamplesPerSec;
            let bits = mix.wBitsPerSample;

            let flottant = if mix.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // `WAVEFORMATEXTENSIBLE` is `repr(packed)`: taking a
                // reference on `SubFormat` — which `==` on a GUID does —
                // is an unaligned access, hence undefined behaviour.
                let ext = mix.0 as *const WAVEFORMATEXTENSIBLE;
                let sous_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sous_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                mix.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            };

            wasapi_format::verifier(frequence, canaux, bits, flottant)?;
            let description = wasapi_format::decrire(frequence, canaux, bits, flottant);

            // Decision 8: we ATTEMPT the event, we do not assume it.
            //
            // ⚠️ **An `IAudioClient` initialises only once.** A
            // refused `Initialize` leaves it in a state where a second call
            // would return `AUDCLNT_E_ALREADY_INITIALIZED` or worse: the fallback
            // therefore RE-ACTIVATES a new client rather than retrying on
            // this one. It is the same gesture as `set_encode_size` since D5 —
            // destroy before rebuilding —, and for the same reason.
            let (client, reveil) = match client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                DUREE_TAMPON_100NS,
                0,
                mix.0,
                None,
            ) {
                Ok(()) => (client, Reveil::Evenement),
                Err(e) => {
                    tracing::warn!(
                        erreur = %e,
                        "AUDCLNT_STREAMFLAGS_EVENTCALLBACK refuse par le cable : repli sur une \
                         boucle a echeance (spec §6 le predisait supporte, ce n'etait qu'une \
                         prediction)"
                    );
                    let neuf: IAudioClient = peripherique.Activate(CLSCTX_ALL, None).context(
                        "re-activation du client audio de rendu apres refus de \
                                  l'evenement",
                    )?;
                    neuf.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        0,
                        DUREE_TAMPON_100NS,
                        0,
                        mix.0,
                        None,
                    )
                    .context("initialisation du client audio de rendu (cable, sans evenement)")?;
                    (neuf, Reveil::Echeance)
                }
            };
            // `mix` goes out of scope at the end of the `unsafe` block and then frees the
            // format through `CoTaskMemFree` — AFTER `Initialize`, which made its
            // internal copy of it, as `GetMixFormat`'s documentation requires.

            let evenement = if reveil == Reveil::Evenement {
                let handle = CreateEventW(None, false, false, PCWSTR::null())
                    .context("creation de l'evenement de reveil du rendu")?;
                client
                    .SetEventHandle(handle)
                    .context("liaison de l'evenement au client de rendu")?;
                Some(handle)
            } else {
                None
            };

            let taille_tampon = client
                .GetBufferSize()
                .context("lecture de la taille du tampon de rendu")?;
            let rendu: IAudioRenderClient = client
                .GetService()
                .context("obtention du service de rendu")?;
            client.Start().context("demarrage du rendu")?;

            Ok(Self {
                client,
                rendu,
                evenement,
                taille_tampon,
                canaux,
                description,
                reveil,
            })
        }
    }

    /// Format actually obtained, for the log.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// How the thread is woken up — **MEASURED**, never assumed.
    pub fn reveil(&self) -> Reveil {
        self.reveil
    }

    /// Waits for room, at most `delai`. Returns the number of
    /// **frames per channel** to provide, or `0`.
    ///
    /// `0` is not an error: it is a round where the buffer was still
    /// full. The caller counts it as a missed deadline — it is this
    /// counter that will make decidable the question, conjectural today, of
    /// whether the reader's `Mutex` deserves to be replaced by a lock-free
    /// queue.
    pub fn attendre_place(&mut self, delai: Duration) -> Result<usize> {
        let echeance = Instant::now() + delai;
        loop {
            let place = self.place()?;
            if place > 0 {
                return Ok(place);
            }
            let reste = echeance.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return Ok(0);
            }
            match self.reveil {
                Reveil::Evenement => {
                    let handle = self
                        .evenement
                        .context("mode evenement sans handle : incoherence interne")?;
                    // `as u32` after `min`: a wait of more than 49 days
                    // makes no sense here, and `INFINITE` (0xFFFFFFFF) must
                    // never be reached by accident — a render thread that
                    // waits without bound would no longer wake up if the
                    // device disappeared.
                    let ms = reste.as_millis().min(u32::MAX as u128 - 1) as u32;
                    // SAFETY: `handle` comes from a successful `CreateEventW` and
                    // is only closed by `Drop`.
                    let issue = unsafe { WaitForSingleObject(handle, ms) };
                    if issue == WAIT_FAILED {
                        bail!("attente de l'evenement de rendu echouee (WAIT_FAILED)");
                    }
                }
                Reveil::Echeance => {
                    // Bounded polling: never more than what remains, and never
                    // more than a quarter of the audio engine's usual period —
                    // otherwise we would sleep past the next wake-up.
                    std::thread::sleep(reste.min(Duration::from_millis(2)));
                }
            }
        }
    }

    /// The free room in the buffer, in frames per channel.
    fn place(&self) -> Result<usize> {
        // SAFETY: `client` comes from a successful `Initialize`.
        let occupe = unsafe { self.client.GetCurrentPadding() }
            .context("lecture de l'occupation du tampon de rendu")?;
        Ok(self.taille_tampon.saturating_sub(occupe) as usize)
    }

    /// Writes `trames` per channel from `pcm` (interleaved stereo, `f32`).
    ///
    /// `pcm` must carry at least `trames * canaux` samples; the format
    /// having been refused if it was not stereo 32-bit float
    /// (`wasapi/format.rs`), the copy is a `memcpy` and nothing else.
    pub fn ecrire(&mut self, pcm: &[f32], trames: usize) -> Result<()> {
        if trames == 0 {
            return Ok(());
        }
        let besoin = trames * self.canaux;
        ensure!(
            pcm.len() >= besoin,
            "tampon PCM trop court : {} echantillons pour {trames} trames x {} canaux",
            pcm.len(),
            self.canaux
        );

        // ⚠️ **Injection is tested BEFORE `GetBuffer`, never between it and
        // `ReleaseBuffer`.** A buffer acquired and never released would block
        // rendering for good, and we would then be measuring the instrument.
        if faute_a_injecter() {
            bail!("faute injectee (MICRO_FAUTE_ECRITURE)");
        }

        // SAFETY: `GetBuffer(trames)` is only called after checking through
        // `attendre_place` that as many frames are free; it returns a
        // pointer to `trames * canaux` samples of the negotiated format, hence
        // `f32`s, and `ReleaseBuffer` gives it back to WASAPI in the same block.
        unsafe {
            let tampon = self
                .rendu
                .GetBuffer(trames as u32)
                .context("acquisition du tampon de rendu")?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), tampon as *mut f32, besoin);
            self.rendu
                .ReleaseBuffer(trames as u32, 0)
                .context("liberation du tampon de rendu")?;
        }
        Ok(())
    }
}

impl Drop for RenduWasapi {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
            if let Some(handle) = self.evenement.take() {
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// The write fault injection budget, **PROCESS-GLOBAL**.
///
/// 🔴 **Never reread per thread, and that is the lesson D10 paid for with a whole
/// pass** (`AUDIO_FAUTE_LECTURE`, task 14): a budget reread per thread
/// re-arms fully at each rebuild — `std::env::var` returning the
/// same value to the new thread —, so each thread dies in turn before any
/// real call, and **the judging figure cannot leave zero on a product
/// that is nevertheless fixed**. A `static` decremented by `fetch_update` makes the
/// budget run out **once** for the whole process.
///
/// ⚠️ **BENCH variable, never a shipped configuration.** Absent or zero:
/// no fault, and not a single log line.
fn faute_a_injecter() -> bool {
    use std::sync::atomic::{AtomicU32, Ordering};
    static BUDGET: std::sync::OnceLock<AtomicU32> = std::sync::OnceLock::new();
    let budget = BUDGET.get_or_init(|| {
        let n: u32 = std::env::var("MICRO_FAUTE_ECRITURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            // A SINGLE `warn!`: two traces at the same instant would count
            // as two events (sub-block D6's home-grown trap).
            tracing::warn!(
                fautes_a_injecter = n,
                "injection de fautes d'ecriture du micro ARMEE (banc, MICRO_FAUTE_ECRITURE)"
            );
        }
        AtomicU32::new(n)
    });
    budget
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
        .is_ok()
}
