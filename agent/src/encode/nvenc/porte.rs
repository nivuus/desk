//! **The door**: load `nvEncodeAPI64.dll`, check that the driver speaks
//! our version, and obtain its function table.
//!
//! Extracted from `session.rs` on 30 August 2026, **because the addition of
//! `regler_debit` had made it cross the 500-line ceiling** (504
//! measured). The repository's doctrine is to extract, never to compress — and
//! the cut follows a real boundary: opening the door is not
//! using it. `session.rs` falls back to ~440.
//!
//! 🟢 **THIS MODULE HAS RUN** (30 August 2026, target VM): the driver announced
//! **`0xd1`** — that is **13.1**, more recent than the transcribed **12.2** — and
//! `abi::pilote_compatible` accepted it. ⚠️ **The REFUSAL branch, for its part,
//! has never run**: no machine here carries a driver older than
//! 12.2, and that path therefore remains untested.
//!
//! ⚠️ **Licence notice and provenance of the ABI: `super::abi`.**

use anyhow::{anyhow, bail, Context, Result};

use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use super::abi;
use super::fonctions::{self, FunctionList, Statut};

/// Translates an `NVENCSTATUS` into an error, naming the call.
pub(super) fn verify(statut: Statut, quoi: &str) -> Result<()> {
    if statut == abi::SUCCESS {
        return Ok(());
    }
    // 🔴 This code deserves to be named: it means "a structure
    // version is wrong", and it is the most silent defect of this
    // API — see `super::abi`.
    if statut == abi::ERR_INVALID_VERSION {
        bail!(
            "{quoi} : NV_ENC_ERR_INVALID_VERSION ({statut}) — une version de \
             structure est fausse. Voir `encode_nvenc::abi` et sa commande de \
             relecture ; ce n'est PAS un défaut de pilote."
        );
    }
    bail!("{quoi} : NVENCSTATUS {statut}")
}

/// The door: the driver's DLL and its function table.
///
/// ⚠️ **The DLL is never released**, on purpose: several sessions
/// coexist (one per window), and a `FreeLibrary` pulled from under a
/// neighbour would be a crash. The process gives it back when dying.
pub struct Porte {
    pub(super) fonctions: FunctionList,
}

impl Porte {
    pub fn ouvrir() -> Result<Self> {
        let module = unsafe { LoadLibraryA(windows::core::s!("nvEncodeAPI64.dll")) }
            .context("chargement de nvEncodeAPI64.dll (la DLL vient du pilote NVIDIA)")?;

        // ① The driver version BEFORE anything, so that the refusal is readable.
        let version_max = unsafe {
            GetProcAddress(
                module,
                windows::core::s!("NvEncodeAPIGetMaxSupportedVersion"),
            )
        }
        .ok_or_else(|| anyhow!("NvEncodeAPIGetMaxSupportedVersion absente de la DLL"))?;
        let version_max: fonctions::VersionMaxSupportee =
            unsafe { std::mem::transmute(version_max) };
        let mut rendue = 0u32;
        verify(
            unsafe { version_max(&mut rendue) },
            "NvEncodeAPIGetMaxSupportedVersion",
        )?;
        if !abi::pilote_compatible(rendue) {
            bail!(
                "pilote NVIDIA trop ancien pour l'API transcrite : il annonce \
                 {rendue:#x}, il faut au moins {:#x} (soit {}.{}). \
                 ⚠️ Cet empaquetage est (majeure << 4) | mineure, PAS celui de \
                 NVENCAPI_VERSION.",
                abi::version_pilote_attendue(),
                abi::VERSION_MAJEURE,
                abi::VERSION_MINEURE
            );
        }
        tracing::info!(
            version_pilote = format!("{rendue:#x}"),
            version_attendue = format!("{:#x}", abi::version_pilote_attendue()),
            "porte NVENC : pilote compatible"
        );

        // ② The function table.
        let create =
            unsafe { GetProcAddress(module, windows::core::s!("NvEncodeAPICreateInstance")) }
                .ok_or_else(|| anyhow!("NvEncodeAPICreateInstance absente de la DLL"))?;
        let create: fonctions::CreateInstanceFn = unsafe { std::mem::transmute(create) };
        let mut table: FunctionList = unsafe { std::mem::zeroed() };
        table.version = abi::FUNCTION_LIST_VER;
        verify(unsafe { create(&mut table) }, "NvEncodeAPICreateInstance")?;

        Ok(Self { fonctions: table })
    }
}
