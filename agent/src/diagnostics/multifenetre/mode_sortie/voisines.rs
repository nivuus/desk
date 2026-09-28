//! RAW DXGI duplications on neighbouring outputs, to count the access
//! losses that a mode change inflicts on them — D8's side unknown no. 2
//! ("how many `0x887a0026` access losses does a mode change
//! inflict on the neighbours?").
//!
//! Extracted from `mode_sortie.rs` at task 1 of sub-block D9, for the
//! 500-line ceiling (`CLAUDE.md`) — and separated from
//! `crate::capture::DesktopCapture` out of NECESSITY, not convenience: its
//! recovery window (`capture_reprise::FenetreDeReprise`) ABSORBS a transient
//! access loss by silently reopening it, without ever reporting it to
//! the caller as long as recovery succeeds within the 8 s of its window — see
//! `next_frame` in `capture.rs`. That is precisely what this side
//! measurement must observe and count, not what production has an interest in
//! masking. `crate::capture::ouverture` carries the same per-output opening
//! logic, but its functions are `pub(super)` of the `capture` module: out of
//! reach of a diagnostic module, and making them more visible would touch
//! production code for a need that is not its own.

use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_OUTDUPL_FRAME_INFO,
};

use super::super::capture_virtuelle::designer_sortie_neuve;
use super::super::montee::{attendre_en_pinguant, relever_topologie, DELAI_TOPOLOGIE};
use crate::capture::SortieDxgi;
use crate::moniteurs_virtuels::pilote::PiloteParIoctl;
use crate::moniteurs_virtuels::Sorties;

/// A raw duplication, held for the duration of the round, on an output that is
/// NOT the one under test.
pub(super) struct DuplicationVoisine {
    nom: String,
    device: ID3D11Device,
    output: IDXGIOutput1,
    duplication: IDXGIOutputDuplication,
    /// Set when a reopening after an access loss itself failed:
    /// nothing left to probe on this neighbour, see `sonder`.
    morte: bool,
}

impl DuplicationVoisine {
    /// Opens by INDEX (`sortie.index_adaptateur`/`index_sortie`) rather than
    /// by name, unlike `DesktopCapture::sur_sortie`: `sortie`
    /// comes from a topology survey the caller has just
    /// taken, and nothing comes between that survey and this opening that
    /// could shift these positional indices (see the name doctrine in
    /// `capture_virtuelle.rs` — it applies to an index kept ACROSS a
    /// mutation, not to an index read and consumed on the spot).
    ///
    /// **⚠️ This guarantee rests ENTIRELY on the caller: it must open
    /// THIS neighbour before creating the next output** — the review of
    /// task 1 (Critical 1) found `mode_sortie::executer` at fault on this
    /// exact point (neighbour 1 was opened after the creation of
    /// neighbour 2, hence on an already shifted index). That is why `ouvrir`
    /// stays private to this module: the only public construction path is
    /// now `create_two`, which enforces the strict order instead of
    /// depending on it.
    ///
    /// **`SetMultithreadProtected(TRUE)` is set, and the tension with
    /// `CLAUDE.md` is settled here rather than left implicit (Important 1
    /// of the review of task 1).** Two assertions coexist in this
    /// repository: `capture::ouverture::create_device_and_context` explains the mechanism
    /// — WITHOUT this call, a D3D11 device solicited from TWO THREADS AT
    /// ONCE (capture AND Media Foundation, each from its own threads)
    /// can block indefinitely INSIDE the driver, without an error — while
    /// `CLAUDE.md` (sub-block D3, the "minimal probe" test) states it
    /// WITHOUT that condition: "it inevitably carries an `ID3D11Device`
    /// with `SetMultithreadProtected(true)`, `DuplicateOutput` REQUIRING IT".
    /// This neighbour fulfils NEITHER of the two conditions of the mechanism of
    /// `create_device_and_context` (never handed to Media Foundation, only ever
    /// solicited from the probe's single thread) — a PLAUSIBLE
    /// reasoning for doing without it. But D3's measurement says "`DuplicateOutput`
    /// requiring it", with no sharing or concurrency caveat, and I have
    /// no measurement of my own contradicting this broader wording.
    /// **Unable to decide between the two readings, I make the call**:
    /// its cost is nil (a flag on the immediate context), and the doctrine
    /// already recorded, taken literally, requires it.
    fn ouvrir(sortie: &SortieDxgi) -> Result<Self> {
        let factory: IDXGIFactory1 =
            unsafe { CreateDXGIFactory1() }.context("fabrique DXGI (duplication d'une voisine)")?;
        let adapter =
            unsafe { factory.EnumAdapters1(sortie.index_adaptateur) }.with_context(|| {
                format!(
                    "adaptateur introuvable pour la voisine {}",
                    sortie.nom_sortie
                )
            })?;
        let output: IDXGIOutput1 = unsafe { adapter.EnumOutputs(sortie.index_sortie) }
            .with_context(|| format!("sortie introuvable pour la voisine {}", sortie.nom_sortie))?
            .cast()
            .with_context(|| format!("IDXGIOutput1 pour la voisine {}", sortie.nom_sortie))?;

        let mut device: Option<ID3D11Device> = None;
        let mut contexte = None;
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                Default::default(),
                // No flags: this neighbour never reads nor converts
                // any pixel, so `D3D11_CREATE_DEVICE_BGRA_SUPPORT` has nothing
                // to do here.
                Default::default(),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut contexte),
            )
        }
        .with_context(|| format!("périphérique D3D11 pour la voisine {}", sortie.nom_sortie))?;
        let device = device
            .ok_or_else(|| anyhow!("périphérique D3D11 absent (voisine {})", sortie.nom_sortie))?;
        let contexte: ID3D11DeviceContext = contexte
            .ok_or_else(|| anyhow!("contexte D3D11 absent (voisine {})", sortie.nom_sortie))?;

        // See this function's header comment: set by recorded
        // doctrine (`CLAUDE.md`, sub-block D3), not because a known
        // mechanism would require it for THIS device. The context itself
        // is only kept for the duration of this call -- nothing afterwards
        // uses it, the device carries the state set.
        let multithread: ID3D11Multithread = contexte
            .cast()
            .with_context(|| format!("ID3D11Multithread pour la voisine {}", sortie.nom_sortie))?;
        // Returns the PREVIOUS state (a `BOOL` that must be consumed): never read
        // elsewhere, but logged rather than ignored by a `let _`, same
        // discipline as `capture::ouverture::create_device_and_context`.
        let protection_precedente = unsafe { multithread.SetMultithreadProtected(true) };
        tracing::info!(
            voisine = %sortie.nom_sortie,
            protection_precedente = protection_precedente.as_bool(),
            "protection multi-fils activée sur le contexte immédiat D3D11 (voisine)"
        );

        let duplication = unsafe { output.DuplicateOutput(&device) }
            .with_context(|| format!("duplication de la voisine {}", sortie.nom_sortie))?;
        tracing::info!(voisine = %sortie.nom_sortie, "duplication brute ouverte sur une voisine");

        Ok(Self {
            nom: sortie.nom_sortie.clone(),
            device,
            output,
            duplication,
            morte: false,
        })
    }

    /// Probes once, without blocking (`AcquireNextFrame(0, ..)`, same
    /// convention as `capture.rs::tenter_acquisition`).
    ///
    /// Returns `true` if THIS solicitation detects an access loss —
    /// in which case the duplication is immediately reopened so that the
    /// NEXT solicitation can detect ANOTHER one. Without this
    /// reopening, `AcquireNextFrame` would indefinitely return the same refusal
    /// on the stale instance: a single cut would count for as many
    /// solicitations as remain in the round, which would skew
    /// `pertes_acces_voisines` in the dangerous direction (a silent overestimate).
    ///
    /// Therefore detects, by construction, AT MOST one loss per call —
    /// if several distinct cuts occurred between two solicitations,
    /// they would count as one. It is not a hidden defect:
    /// it is the granularity of the round, which only probes once per mode
    /// change attempt (see its caller).
    pub(super) fn sonder(&mut self) -> bool {
        if self.morte {
            return false;
        }
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        match unsafe {
            self.duplication
                .AcquireNextFrame(0, &mut info, &mut resource)
        } {
            Ok(()) => {
                let _ = unsafe { self.duplication.ReleaseFrame() };
                false
            }
            // Nothing new, or a failure unrelated to the question asked here:
            // in both cases, nothing to count.
            Err(e) if !crate::capture_reprise::est_acces_perdu(e.code().0) => false,
            Err(e) => {
                tracing::info!(
                    voisine = %self.nom,
                    hresult = format!("{:#010x}", e.code().0),
                    "perte d'acces detectee sur une voisine -- reouverture pour continuer a compter"
                );
                match unsafe { self.output.DuplicateOutput(&self.device) } {
                    Ok(fraiche) => self.duplication = fraiche,
                    Err(error) => {
                        // THIS cut still counts: it did
                        // take place. It is the FOLLOWING ones, on this neighbour,
                        // that will no longer be counted -- `morte` prevents it from
                        // indefinitely rereading the same stale instance.
                        self.morte = true;
                        tracing::warn!(
                            voisine = %self.nom,
                            %error,
                            "reouverture de la voisine apres perte d'acces : echouee -- les \
                             pertes suivantes ne seront plus comptees sur cette voisine"
                        );
                    }
                }
                true
            }
        }
    }
}

/// Creates, designates AND OPENS the two neighbours — in this STRICT order, for
/// EACH, without any creation coming between the survey of a
/// neighbour and the opening of ITS duplication (see the doctrine of `ouvrir`
/// above, and Critical 1 of the review of task 1). It is to make
/// this order STRUCTURAL, rather than a discipline the caller would have to
/// respect by itself, that `ouvrir` is private and that this is the only
/// public entry point of this module.
///
/// Updates `connues_a_ce_point` along the way: the caller still needs
/// it to designate the control output afterwards. Returns the NAMES of the two
/// neighbours and not their whole `SortieDxgi` — all that the caller
/// uses beyond this function.
pub(super) fn create_two(
    pilote: &PiloteParIoctl,
    sorties: &mut Sorties<'_>,
    connues_a_ce_point: &mut HashSet<String>,
    largeur: u32,
    hauteur: u32,
    hertz: u32,
) -> Result<(DuplicationVoisine, DuplicationVoisine, String, String)> {
    let id_v1 = sorties.create(largeur, hauteur, hertz)?;
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let apres_v1 = relever_topologie("après création (voisine 1)")?;
    let sortie_v1 = designer_sortie_neuve(&apres_v1, connues_a_ce_point, id_v1)?.clone();
    connues_a_ce_point.insert(sortie_v1.nom_sortie.clone());
    let voisine1 = DuplicationVoisine::ouvrir(&sortie_v1)?;

    let id_v2 = sorties.create(largeur, hauteur, hertz)?;
    attendre_en_pinguant(pilote, DELAI_TOPOLOGIE)?;
    let apres_v2 = relever_topologie("après création (voisine 2)")?;
    let sortie_v2 = designer_sortie_neuve(&apres_v2, connues_a_ce_point, id_v2)?.clone();
    connues_a_ce_point.insert(sortie_v2.nom_sortie.clone());
    let voisine2 = DuplicationVoisine::ouvrir(&sortie_v2)?;

    Ok((
        voisine1,
        voisine2,
        sortie_v1.nom_sortie,
        sortie_v2.nom_sortie,
    ))
}
