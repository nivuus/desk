//! Capturing the sound the machine plays, through WASAPI in loopback mode.
//!
//! The targeted device is **the one `AUDIO_PERIPHERIQUE` designates**, or the
//! session's default render device failing that: we capture what would come out of that
//! device, whatever application produces it.
//!
//! ⚠️ **This file long said "the default render device" unconditionally, and
//! it was an implicit dependency that turned around**: installing
//! VB-Cable on the VM (August 19th, 2026, preparation of workstream E) switched
//! that default to a virtual cable nothing feeds, and the product
//! started capturing silence without any line saying so. The
//! resolution now lives in `wasapi/rendu.rs`, and the rule that elects —
//! pure, tested on the host — in `wasapi/peripherique.rs`.
//!
//! **Polling, not events.** `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` is not
//! supported in combination with `AUDCLNT_STREAMFLAGS_LOOPBACK`: Microsoft
//! documents loopback capture as having to be timer-driven. An
//! idle render stream would moreover signal no event, which is
//! the frequent case here — nothing plays most of the time. Polling is therefore
//! the only correct form, and it directly serves the silence fill of
//! `frames.rs`.

#![cfg(windows)]

/// WRITING samples to a render endpoint: the Windows half
/// of the microphone (block E2). Mirror of `LoopbackCapture` below —
/// that one reads what the machine plays, this one makes the machine play what
/// the browser sends.
pub mod ecriture;
pub mod process_loopback;
/// Resolution of an audio **render** endpoint — the one the
/// loopback captures (fix "A-bis"), **and** that of the cable the
/// microphone writes to (block E2). Two consumers, two opposite fallback
/// policies — the first falls back, the second refuses: see the module
/// header, which carries the table and the reason.
pub mod rendu;

use anyhow::{bail, Context, Result};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::Audio::{
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use crate::opus::{CHANNELS, SAMPLE_RATE_HZ};

/// Duration of the buffer requested from WASAPI, in 100 ns units. 200 ms: a wide
/// margin to absorb a late loop round without ever losing a
/// sample.
const BUFFER_DURATION_100NS: i64 = 2_000_000;

/// `WAVE_FORMAT_EXTENSIBLE` tag of the `wFormatTag` field.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// `WAVE_FORMAT_IEEE_FLOAT` tag.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// RAII guard for the pointer returned by `IAudioClient::GetMixFormat`.
///
/// This pointer is allocated by COM through `CoTaskMemAlloc` (documentation of
/// `GetMixFormat`): the caller must free it with `CoTaskMemFree`, which
/// this guard does in its `Drop`. `IAudioClient::Initialize` copies the format
/// internally, so freeing *after* its call is always correct —
/// including at the normal exit of `open()`, where this guard is only released at
/// the very end of the function.
///
/// The point of a guard rather than an explicit free: between
/// `GetMixFormat` and `Initialize`, `open()` can exit with an error through two
/// `bail!`s (refused frequency or format width) — exactly the
/// paths taken the day the VM changes audio configuration.
/// A free placed only at the end of the happy function would
/// miss them; an RAII guard covers them by construction, whatever the
/// exit path (`return`, `?`, `bail!`).
struct FormatMixage(*mut WAVEFORMATEX);

impl std::ops::Deref for FormatMixage {
    type Target = WAVEFORMATEX;
    fn deref(&self) -> &WAVEFORMATEX {
        // SAFETY: the pointer comes from a successful `GetMixFormat` and is only
        // freed in `Drop`, hence valid for the whole lifetime of the
        // guard.
        unsafe { &*self.0 }
    }
}

impl Drop for FormatMixage {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

pub struct LoopbackCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    canaux: usize,
    flottant: bool,
    description: String,
}

// SAFETY: `LoopbackCapture` wraps COM interfaces (`IAudioClient`,
// `IAudioCaptureClient`) that `windows-core` does not mark `Send` by default —
// a generic COM object may be bound to a single-threaded apartment (STA), and
// moving it to another thread would then be undefined behaviour. That
// is not the case here: `open()` (below) *checks*, rather than
// assuming, that the calling thread joins the multi-threaded apartment (MTA) through
// `CoInitializeEx(None, COINIT_MULTITHREADED)`, and refuses to open if this thread
// already belongs to another apartment (`RPC_E_CHANGED_MODE`).
// `WindowsAudioSource::new` (agent/src/windows_audio.rs) then moves this
// object, by `move`, to a dedicated capture thread which in turn joins
// that same MTA before any COM call (see its comment). An object created
// in an MTA is by construction callable from any thread that
// is a member of it, without marshaling — it is this property, guaranteed by
// `open()`'s check, that makes the transfer safe.
//
// This promise covers the **whole struct, future fields included**: if
// a future field adds an event `HANDLE`, a raw pointer, or any
// other state tied to a specific thread rather than to the apartment, this `unsafe impl`
// would stop being valid without anything flagging it. Whoever adds a
// field to `LoopbackCapture` must check that it stays usable from
// any member thread of the MTA before doing so — otherwise this
// `Send` must be removed or restricted.
//
// Rejected alternative: `windows_core::AgileReference<T>`, the mechanism
// officially provided by `windows-core` 0.62 to carry a COM object
// between threads without assuming its threading model. Not chosen here: it requires
// a resolution (`resolve()`, an internal `QueryInterface`) at each
// retrieval, a cost and complexity that are useless when this process has,
// under this plan, no STA — `open()`'s check is enough and stays
// cheap.
unsafe impl Send for LoopbackCapture {}

impl LoopbackCapture {
    /// Opens the loopback on the device `rendu::resoudre` elects — the one
    /// `AUDIO_PERIPHERIQUE` designates, or Windows' default render device failing
    /// that — and starts capture.
    ///
    /// ⚠️ This line said "the default render device" until the
    /// closing of workstream E, whereas this module's header, twelve lines
    /// higher, already said the opposite since fix "A-bis".
    pub fn open() -> Result<Self> {
        unsafe {
            // `CoInitializeEx` must be **checked**, not ignored: it is the
            // precondition `unsafe impl Send for LoopbackCapture`
            // above depends on (read its comment first if not done yet).
            // `S_OK` (this thread has just joined the MTA) and `S_FALSE` (it
            // was already a member) are both acceptable: in both
            // cases, this thread is a member of the multi-threaded apartment — the same
            // one the capture thread of `windows_audio.rs` will join. Only
            // `RPC_E_CHANGED_MODE` — this thread already belongs to another
            // apartment, typically an STA bound by an earlier call to
            // `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` on this same thread
            // — must make the opening fail: without this refusal,
            // `LoopbackCapture` would migrate from an STA to the capture thread's MTA
            // without marshaling, undefined behaviour no test
            // reveals since the direct vtable call "works" most
            // of the time even when it is forbidden.
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr == RPC_E_CHANGED_MODE {
                bail!(
                    "ouverture du loopback audio refusée : le fil appelant appartient déjà \
                     à un appartement à thread unique (STA), pas à l'appartement \
                     multi-thread (MTA) qu'exige `LoopbackCapture`. `WindowsAudioSource::new` \
                     (agent/src/windows_audio.rs) déplace cet objet, par `move`, vers un fil \
                     de capture dédié qui rejoint la MTA : migrer un objet COM d'une STA vers \
                     un autre appartement sans marshaling est un comportement indéfini, pas \
                     seulement une erreur de type. Vérifiez qu'aucun \
                     `CoInitializeEx(..., COINIT_APARTMENTTHREADED)` (ni aucune autre \
                     initialisation qui lie ce fil à une STA, par exemple une init WinRT \
                     implicite) n'a précédé cet appel sur ce même fil."
                );
            }

            // No matching `CoUninitialize`, and that is deliberate: this thread
            // is not necessarily the one that will use nor the one that will release
            // the returned object. Task 6 (`windows_audio.rs`) calls `open()`
            // on the calling thread of `WindowsAudioSource::new()`, then
            // moves the obtained `LoopbackCapture` by `move` to a dedicated
            // capture thread — it is THAT thread that calls `read()` in a loop
            // and runs `Drop` when leaving. Calling `CoUninitialize` in
            // `Drop` would therefore run on a thread different from the one that
            // called `CoInitializeEx`, which COM explicitly forbids.
            // Accepted consequence: if the calling thread of `open()` is
            // recycled between sessions (pool thread, e.g. tokio's blocking
            // workers), its COM reference count grows by one
            // per session — never that of the capture thread, which for its part
            // is never recycled (created and destroyed once per session). A
            // future rebalancing will have to happen where the call is
            // really owned: around the thread of `WindowsAudioSource::new`,
            // not here.

            let enumerateur: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .context("création de l'énumérateur de périphériques audio")?;
            // Fix "A-bis": no more hardcoded `GetDefaultAudioEndpoint`
            // here. `rendu::resoudre` honours `AUDIO_PERIPHERIQUE` when it
            // is set, falls back to Windows' default otherwise (behaviour
            // from before, unchanged), and TRACES in all cases the device
            // actually retained — including when it is a fallback.
            let peripherique = rendu::resoudre(&enumerateur)?;
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio")?;

            let mix = FormatMixage(
                client
                    .GetMixFormat()
                    .context("lecture du format de mixage")?,
            );
            let canaux = mix.nChannels as usize;
            let frequence = mix.nSamplesPerSec;
            let bits = mix.wBitsPerSample;

            let flottant = if mix.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // WAVEFORMATEXTENSIBLE is repr(packed): taking a
                // reference on SubFormat — which `==` on a GUID does —
                // is an unaligned access, hence undefined behaviour.
                let ext = mix.0 as *const WAVEFORMATEXTENSIBLE;
                let sub_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                mix.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            };

            let description = format!(
                "{frequence} Hz, {canaux} canaux, {bits} bits, {}",
                if flottant { "flottant" } else { "entier" }
            );

            if frequence != SAMPLE_RATE_HZ {
                bail!(
                    "format de mixage à {frequence} Hz : seul {SAMPLE_RATE_HZ} Hz est supporté \
                     (aucun rééchantillonneur n'est embarqué)"
                );
            }
            if !flottant && bits != 16 {
                bail!("format de mixage entier {bits} bits non supporté ({description})");
            }
            if flottant && bits != 32 {
                bail!("format de mixage flottant {bits} bits non supporté ({description})");
            }

            client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    BUFFER_DURATION_100NS,
                    0,
                    mix.0,
                    None,
                )
                .context("initialisation du client audio en loopback")?;
            // `mix` (the `FormatMixage` guard) goes out of scope at the end of the
            // `unsafe` block and then frees the format through `CoTaskMemFree` — after
            // `Initialize`, which made its own internal copy of it, as
            // `GetMixFormat`'s documentation requires.

            let capture: IAudioCaptureClient = client
                .GetService()
                .context("obtention du service de capture")?;
            client.Start().context("démarrage de la capture")?;

            Ok(Self {
                client,
                capture,
                canaux,
                flottant,
                description,
            })
        }
    }

    /// Format actually obtained, for the log and the probe.
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// Reads the next available packet, converted to interleaved stereo
    /// 16-bit integers. Returns `None` when nothing is available — the
    /// common case when no application is playing.
    pub fn read(&mut self) -> Result<Option<Vec<i16>>> {
        unsafe {
            let dispo = self
                .capture
                .GetNextPacketSize()
                .context("interrogation du paquet suivant")?;
            if dispo == 0 {
                return Ok(None);
            }

            let mut data: *mut u8 = std::ptr::null_mut();
            let mut images: u32 = 0;
            let mut drapeaux: u32 = 0;
            self.capture
                .GetBuffer(&mut data, &mut images, &mut drapeaux, None, None)
                .context("lecture du tampon de capture")?;

            let muet = drapeaux & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            let sortie = if muet {
                vec![0i16; images as usize * CHANNELS]
            } else {
                let brut = images as usize * self.canaux;
                if self.flottant {
                    let source = std::slice::from_raw_parts(data as *const f32, brut);
                    convertir_flottant(source, self.canaux)
                } else {
                    let source = std::slice::from_raw_parts(data as *const i16, brut);
                    convertir_entier(source, self.canaux)
                }
            };

            self.capture
                .ReleaseBuffer(images)
                .context("libération du tampon de capture")?;
            Ok(Some(sortie))
        }
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

/// Converts float samples to interleaved stereo 16-bit integers.
///
/// Mono: the channel is duplicated. More than two channels: **truncation**, not
/// downmixing — only the first two channels (left and right of a
/// multichannel stream, by WAVE_FORMAT_EXTENSIBLE convention) are kept as
/// they are; the energy of the surround channels is mixed into neither,
/// it is simply ignored. Without consequence in practice: this VM's real
/// format is already stereo.
fn convertir_flottant(source: &[f32], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(vers_i16(g));
        sortie.push(vers_i16(d));
    }
    sortie
}

/// Same conversion, for a source already in 16-bit integers.
fn convertir_entier(source: &[i16], canaux: usize) -> Vec<i16> {
    let images = source.len() / canaux.max(1);
    let mut sortie = Vec::with_capacity(images * CHANNELS);
    for i in 0..images {
        let base = i * canaux;
        let g = source[base];
        let d = if canaux >= 2 { source[base + 1] } else { g };
        sortie.push(g);
        sortie.push(d);
    }
    sortie
}

/// Normalised float → 16-bit integer, with explicit clipping.
///
/// WASAPI does not guarantee that samples stay within [-1, 1]: a
/// mix of several streams can exceed it. The `as` cast already saturates
/// natively since Rust 1.45 (an out-of-bounds value is brought back to
/// `i16::MIN`/`i16::MAX`, never wrapped): the explicit `clamp` therefore
/// does not serve to avoid a silent overflow, but to set the upper bound
/// exactly at `i16::MAX` — the cast alone, without clamp, would saturate
/// downwards to `i16::MIN`, one LSB further than `-i16::MAX` — and to make
/// the intention explicit rather than relying on that semantic detail
/// of `as`.
fn vers_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}
