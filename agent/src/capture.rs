//! Screen capture through DXGI Desktop Duplication, cropped to a window.
//!
//! `Windows.Graphics.Capture` would have allowed capturing the window directly,
//! but this API is unusable on Windows Server 2022: the system service
//! implementing it crashes on `CreateForWindow`. We therefore duplicate the screen
//! output and crop. Frames stay on the GPU: cropping is done
//! through `CopySubresourceRegion`, without a round trip through main memory.

#![cfg(windows)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET, D3D11_BOX,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};

use crate::geometry::Rect;

/// Diagnostic counters of DXGI acquisition (see `SOURCE_TRACE`).
///
/// `ACCUMULATED` is the sum of `DXGI_OUTDUPL_FRAME_INFO::AccumulatedFrames`,
/// that is the number of desktop updates DXGI merged into
/// the frames it returned to us. It is the only measurement that distinguishes the
/// two explanations of a low `captured_hz`: if `ACCUMULATED` is clearly
/// above `HITS`, the desktop does update fast and it is we who
/// query it too rarely; if it follows it closely, it is the source
/// (the captured window) that produces no more.
pub static ATTEMPTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static HITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ACCUMULATED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A captured and cropped frame, resident on the GPU.
pub struct CapturedFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
}

pub struct DesktopCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    /// Set to `None` by `rouvrir`, and MAY stay so if its reopening
    /// fails: no longer a defect, see `duplication()` and `types::lire`.
    duplication: Option<IDXGIOutputDuplication>,
    /// Last access-loss HRESULT, read by `duplication()` if the field
    /// above is `None`.
    dernier_code_perdu: i32,
    /// What must be reopened after an access loss. Kept at opening:
    /// at the moment access is lost, the topology has already changed and nothing
    /// in the DXGI objects still held says what we were capturing.
    cible: CibleCapture,
    /// Resumption window in progress (see `next_frame`): "opened at the
    /// first access loss and **closed by the first success**"
    /// (`capture::reprise`). `succes()` being called on `Ok(None)`, any
    /// non-refused acquisition closes it — the duration therefore runs from the
    /// LAST refusal. An UNINTERRUPTED loss beyond it is definitive.
    fenetre: crate::capture_reprise::FenetreDeReprise,
    desktop_width: u32,
    desktop_height: u32,
    /// Destination texture, reallocated only when the size changes.
    target: Option<(ID3D11Texture2D, u32, u32)>,
    /// True as long as an acquired frame has not been released.
    frame_held: bool,
    /// Current step published for the watchdog thread (see
    /// `encode::PHASE_CAPTURE_*`). Absent outside diagnostic mode.
    phase: Option<Arc<AtomicU64>>,
}

impl DesktopCapture {
    /// Opens at START-UP: the retry window is full, and the call can
    /// therefore block for up to `DUREE_FENETRE_OUVERTURE`.
    ///
    /// **Use only where blocking is legitimate** — see `ouvrir`. For
    /// rebuilding a capture during a session, it is
    /// `new_sans_attente` that is needed.
    pub fn new() -> Result<Self> {
        Self::ouvrir(
            CibleCapture::Bureau,
            crate::capture_reprise::DUREE_FENETRE_OUVERTURE,
        )
    }

    /// Opens WITHOUT waiting: a refusal is returned at the first attempt.
    ///
    /// For callers running on a thread whose immobilisation would
    /// be costly — `WindowsSource::resize`, which rebuilds its capture from
    /// the blocking thread of `Session::run`. Sleeping there would at the same time suspend
    /// keyframe requests, network adaptation and the
    /// following resizes: exactly the trade-off `next_frame`
    /// already refuses (see its comment "No internal loop").
    ///
    /// Behaviour **identical to the one before the retry was added**: nothing
    /// is retried, only the abandonment trace is new.
    pub fn new_sans_attente() -> Result<Self> {
        Self::ouvrir(CibleCapture::Bureau, std::time::Duration::ZERO)
    }

    /// Duplicates a specific DXGI output, designated by its name
    /// (`\\.\DISPLAYn`, as `enumerer_sorties` returns it).
    ///
    /// **By name and not by enumeration indices**: those are
    /// positional and change as soon as an output appears or disappears — which
    /// is the nominal case in multi-window mode, where the supervisor creates one
    /// output per window opening.
    ///
    /// Full retry window: this form is only called at the start-up
    /// of a supervisor child.
    pub fn sur_sortie(nom: &str) -> Result<Self> {
        Self::ouvrir(
            CibleCapture::Sortie(nom.to_string()),
            crate::capture_reprise::DUREE_FENETRE_OUVERTURE,
        )
    }

    /// `fenetre_ouverture`: duration during which a duplication refused
    /// for temporary unavailability is retried. **An ARGUMENT and not the
    /// constant read on the spot, because the right to block is decided by
    /// the caller** — `ouvrir` does not only run at a child's start-up. See
    /// `ouverture::dupliquer_avec_reprise` for the complete trade-off.
    fn ouvrir(cible: CibleCapture, fenetre_ouverture: std::time::Duration) -> Result<Self> {
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("création de la fabrique DXGI")?;

        // Without a target (`new()`, `CibleCapture::Bureau`), we keep the first
        // adapter that has an output attached to the desktop: it is the one that
        // composes the screen, and hence the only one that can be duplicated. On the target VM it is the
        // RTX 4070, which at the same time gives the right device for
        // the hardware encoder of task 10. With a target
        // (`CibleCapture::Sortie`), it is that one that is opened as is.
        let (adapter, output) = ouvrir_sortie(&factory, &cible)?;

        let (device, context) = creer_peripherique(&adapter)?;

        let (duplication, desktop_width, desktop_height) =
            dupliquer_avec_reprise(&device, &output, &cible, fenetre_ouverture)?;
        tracing::info!(
            desktop_width,
            desktop_height,
            "duplication de sortie établie"
        );

        Ok(Self {
            device,
            context,
            duplication: Some(duplication),
            dernier_code_perdu: 0,
            cible,
            fenetre: crate::capture_reprise::FenetreDeReprise::nouvelle(),
            desktop_width,
            desktop_height,
            target: None,
            frame_held: false,
            phase: None,
        })
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    /// Rebuilds the duplication after an access loss, **keeping the
    /// D3D11 device**.
    ///
    /// It is not a saving, it is a necessity. The H.264 encoder is bound
    /// to this device through the `IMFDXGIDeviceManager` (`encode::share_device`):
    /// creating a new one would force destroying the encoder, hence going through
    /// `Drop for H264Encoder`, whose worst case is bounded at 8 s and where a freeze has
    /// already been observed (`CLAUDE.md`). A resumption meant to go unnoticed
    /// cannot pay that price. The multithread protection set on the context at
    /// opening is not replayed: it concerns the immediate context,
    /// which we keep.
    ///
    /// **The old duplication is released BEFORE the new one is
    /// requested, and the order is the substance of this method.** DXGI only allows
    /// **one** duplication per output. The previous version called
    /// `dupliquer()` while `self.duplication` still held the stale
    /// object: the call succeeded — 378 times out of 378 in the reading of
    /// 1 August 2026 — and returned a **stillborn** duplication, which immediately
    /// refused any acquisition. Eight seconds of retries every 150 ms
    /// never got out of it.
    pub fn rouvrir(&mut self) -> Result<()> {
        // The held frame first: `release_frame` calls `ReleaseFrame` on
        // the duplication we are about to release.
        self.release_frame();

        // THEN the duplication itself, and it is this line that matters.
        // `None` makes it be released here, not at the following assignment.
        self.duplication = None;

        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
            .context("création de la fabrique DXGI (réouverture)")?;
        let (_adapter, output) =
            ouvrir_sortie(&factory, &self.cible).context("résolution de la sortie à rouvrir")?;
        let (duplication, largeur, hauteur) = dupliquer(&self.device, &output)?;

        // The dimensions may have changed: the destination texture is
        // sized on the REGION requested by the caller, not on these,
        // but `desktop_size()` is read elsewhere and must stay right.
        self.duplication = Some(duplication);
        self.desktop_width = largeur;
        self.desktop_height = hauteur;
        Ok(())
    }

    /// The current duplication, or the last lost HRESULT if absent — see `types::lire`.
    fn duplication(&self) -> std::result::Result<&IDXGIOutputDuplication, EchecAcquisition> {
        types::lire(&self.duplication, self.dernier_code_perdu)
    }

    /// Plugs in the step marker shared with the watchdog thread.
    ///
    /// Without it, a block in `next_frame` stays anonymous: the three Windows
    /// calls it chains merge into a single step.
    pub fn set_phase_marker(&mut self, phase: Arc<AtomicU64>) {
        self.phase = Some(phase);
    }

    fn set_phase(&self, value: u64) {
        if let Some(phase) = &self.phase {
            phase.store(value, Ordering::Relaxed);
        }
    }

    pub fn desktop_size(&self) -> (u32, u32) {
        (self.desktop_width, self.desktop_height)
    }

    /// Acquires the next frame and crops it to `region`, **reopening itself
    /// if DXGI revoked its access**.
    ///
    /// The resumption is here, and not with the caller, on purpose: the
    /// multi-window bench captures through this function without going through
    /// `WindowsSource` (`diagnostics/multifenetre/voies.rs`). A resumption placed
    /// higher up would leave the bench off the production path, and the measurement
    /// meant to validate this path would be worthless.
    pub fn next_frame(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        match self.tenter_acquisition(region) {
            Ok(issue) => {
                self.fenetre.succes();
                Ok(issue)
            }
            Err(EchecAcquisition::AccesPerdu(code_perdu)) => {
                // No internal loop, and that is the design point:
                // this function is called from the loop of
                // `Session::run`, and sleeping there for several seconds would at the same time suspend
                // keyframe requests, network adaptation rung
                // changes and resizes.
                // The resumption is therefore spread over several calls.
                match self.fenetre.tenter(std::time::Instant::now()) {
                    crate::capture_reprise::Tentative::Rouvrir => {
                        // `info!` and not `debug!`, for both traces of
                        // this branch: operations run at
                        // RUST_LOG=info, and a silent mitigation is not
                        // one (same rule as `encode/arret.rs`, `CLAUDE.md`
                        // — do not bring them back down).
                        //
                        // The bare HRESULT, and not only inferred from reading
                        // the code (as the report of the
                        // measurement of 1 August 2026 had to do): it is the only piece that
                        // says WHAT was lost.
                        tracing::info!(
                            tentative = self.fenetre.tentatives(),
                            cible = ?self.cible,
                            hresult = format!("{code_perdu:#010x}"),
                            "accès à la duplication perdu, réouverture"
                        );
                        if let Err(erreur) = self.rouvrir() {
                            // A reopening failure is NOT definitive: the
                            // output may not yet have reappeared in the
                            // topology. We say so and let the window
                            // run — it is what will decide. TRUE since
                            // the re-read fix of task 6 quater
                            // (`types::lire`) only: before it, the following
                            // call found `self.duplication` as `None` and
                            // broke the window down despite this text.
                            tracing::info!(
                                erreur = %crate::cause::chaine(&erreur),
                                cible = ?self.cible,
                                "réouverture de la duplication échouée, la fenêtre de reprise court toujours"
                            );
                        }
                        Ok(None)
                    }
                    crate::capture_reprise::Tentative::Patienter => Ok(None),
                    crate::capture_reprise::Tentative::Expiree => {
                        Err(EchecAcquisition::AccesPerdu(code_perdu))
                    }
                }
            }
            Err(panne) => Err(panne),
        }
    }

    /// Acquires the next frame and crops it to `region`, without resumption.
    ///
    /// Returns `Ok(None)` if no new frame is available — common
    /// and normal case: the desktop does not change at every call.
    fn tenter_acquisition(
        &mut self,
        region: Rect,
    ) -> std::result::Result<Option<CapturedFrame>, EchecAcquisition> {
        self.release_frame();

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        // `duplication()` FIRST, the phase NEXT — and not the reverse. This `?`
        // exits the function at every round of the resumption window (up to
        // `DUREE_FENETRE_REPRISE`, 8 s), without ever reaching the return to
        // `PHASE_CAPTURE` set after the acquisition: the watchdog thread
        // logged `étape = capture/AcquireNextFrame` during those 8 s while
        // the call was not made a single time. This repository has already lost
        // a campaign attributing a block to the wrong call — a published step
        // must only designate code actually running.
        let duplication = self.duplication()?;
        // Zero wait: the cadence is driven by the calling loop, not
        // by blocking here.
        self.set_phase(crate::encode::PHASE_CAPTURE_ACQUIRE);
        let acquired = unsafe { duplication.AcquireNextFrame(0, &mut info, &mut resource) };
        self.set_phase(crate::encode::PHASE_CAPTURE);

        if acquired.is_ok() {
            HITS.fetch_add(1, Ordering::Relaxed);
            ACCUMULATED.fetch_add(info.AccumulatedFrames as u64, Ordering::Relaxed);
        }

        if let Err(e) = acquired {
            if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return Ok(None);
            }
            // Classify on the CODE, never on the message text: `CLAUDE.md`
            // carries the precedent of a Windows label that got a
            // refusal attributed to the wrong call for a whole work stream.
            if crate::capture_reprise::est_acces_perdu(e.code().0) {
                self.dernier_code_perdu = e.code().0;
                return Err(EchecAcquisition::AccesPerdu(e.code().0));
            }
            return Err(EchecAcquisition::Panne(anyhow!(
                "acquisition d'image : {e}"
            )));
        }
        self.frame_held = true;

        // From here on the frame is held: every exit path must
        // release it, otherwise the duplication refuses any later
        // acquisition. `release_frame` at the head of the next iteration does not
        // cover the case of an error that bubbles up and stops the loop,
        // nor that of a caller that retries after swallowing the error.
        let cropped: Result<CapturedFrame> = (|| {
            let resource = resource.ok_or_else(|| anyhow!("ressource d'image absente"))?;
            let desktop: ID3D11Texture2D = resource.cast()?;
            self.crop(&desktop, region)
        })();
        match cropped {
            Ok(frame) => {
                // Release RIGHT AWAY, not at the next round.
                //
                // `crop` has already copied the pixels into our own texture
                // (`CopySubresourceRegion` to `self.target`): the desktop
                // frame is of no use past this line. Keeping it
                // until the next call, as the previous version
                // did via the sole `release_frame()` at the head of the
                // function, immobilised the duplication for the whole
                // interval between two rounds — that is ~16.7 ms out of 16.7 at the
                // cadence of `Session::run`.
                //
                // Measured: the same Firefox window, with the same content,
                // yielded 74 fps at `CAPTURE_TEST` (tight loop, hence
                // release every ~2 ms) against only 22 fps
                // in `Session::run` (release every ~16.7 ms). It
                // was therefore neither desktop composition, nor the driver, nor
                // GPU contention with the encoder (`ENCODE_TEST` holds
                // 66 fps encoder included): it was the holding duration.
                self.release_frame();
                Ok(Some(frame))
            }
            Err(e) => {
                self.release_frame();
                Err(EchecAcquisition::Panne(e))
            }
        }
    }

    fn release_frame(&mut self) {
        if self.frame_held {
            self.set_phase(crate::encode::PHASE_CAPTURE_RELEASE);
            // Defensive rather than `duplication()` (which returns a `Result`):
            // this method also runs in `Drop`, where nothing must bubble up.
            if let Some(duplication) = self.duplication.as_ref() {
                // A failure here is not recoverable and must not mask what follows.
                let _ = unsafe { duplication.ReleaseFrame() };
            }
            self.set_phase(crate::encode::PHASE_CAPTURE);
            self.frame_held = false;
        }
    }
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        self.release_frame();
    }
}

// `SortieDxgi` lives in `crate::sortie_dxgi` (portable, outside `#[cfg(windows)]`)
// so that `superviseur::placement::sortie_par_dimensions` (task 7) can
// consume it on the Linux host — `crate::capture` does not exist at all outside
// Windows. This re-export keeps `crate::capture::SortieDxgi` valid and identical
// to the portable type for all the code that only compiles under Windows.
pub use crate::sortie_dxgi::SortieDxgi;

// Cropping a duplicated frame into a dedicated texture, on the GPU.
mod recadrage;

// `enumerer_sorties`, `enumerer_sorties_silencieux` and the rest of
// DXGI enumeration live in this child module, extracted in task 2 of
// sub-block D2 to bring this file back under the 500-line cap
// (`CLAUDE.md`) — see the header of `capture/enumeration.rs`. Re-exported here
// so that `crate::capture::enumerer_sorties` stays valid without touching a
// single caller.
mod enumeration;
pub use enumeration::{enumerer_sorties, enumerer_sorties_silencieux};

// `ouvrir_sortie` and `dupliquer` live in this child module, extracted in
// task 3 of sub-block D2 to bring this file back under the 500-line
// cap (`CLAUDE.md`) after adding `EchecAcquisition` and the resumption
// in `next_frame` — see the header of `capture/ouverture.rs`.
// `creer_peripherique` and `dupliquer_avec_reprise` joined them in
// task 11 bis, the second carrying the opening retry and the first having
// moved down only to give the parent file back the margin that retry
// took from it.
pub mod ouverture;
use ouverture::{creer_peripherique, dupliquer, dupliquer_avec_reprise, ouvrir_sortie};

// `EchecAcquisition`, `CibleCapture` and the `lire` helper live in this child
// module, extracted in task 6 quater of sub-block D2 — see the header of
// `capture/types.rs`. `capture/ouverture.rs` keeps resolving
// `super::CibleCapture` without change.
mod types;
pub use types::{CibleCapture, EchecAcquisition};
