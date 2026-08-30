//! **La porte** : charger `nvEncodeAPI64.dll`, vérifier que le pilote parle
//! notre version, et obtenir sa table de fonctions.
//!
//! Extrait de `session.rs` le 30 août 2026, **parce que l'ajout de
//! `regler_debit` y avait fait franchir le plafond de 500 lignes** (504
//! mesurées). La doctrine du dépôt est d'extraire, jamais de comprimer — et
//! la coupe suit une frontière réelle : ouvrir la porte n'est pas s'en
//! servir. `session.rs` retombe à ~440.
//!
//! 🟢 **CE MODULE A TOURNÉ** (30 août 2026, VM cible) : le pilote a annoncé
//! **`0xd1`** — soit **13.1**, plus récent que la **12.2** transcrite — et
//! `abi::pilote_compatible` l'a accepté. ⚠️ **La branche du REFUS, elle,
//! n'a jamais couru** : aucune machine ici ne porte un pilote antérieur à
//! 12.2, et ce chemin-là reste donc non éprouvé.
//!
//! ⚠️ **Notice de licence et provenance de l'ABI : `super::abi`.**

use anyhow::{anyhow, bail, Context, Result};

use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

use super::abi;
use super::fonctions::{self, ListeDeFonctions, Statut};

/// Traduit un `NVENCSTATUS` en erreur, en nommant l'appel.
pub(super) fn verifier(statut: Statut, quoi: &str) -> Result<()> {
    if statut == abi::SUCCESS {
        return Ok(());
    }
    // 🔴 Ce code-ci mérite d'être nommé : il veut dire « une version de
    // structure est fausse », et c'est le défaut le plus silencieux de cette
    // API — voir `super::abi`.
    if statut == abi::ERR_INVALID_VERSION {
        bail!(
            "{quoi} : NV_ENC_ERR_INVALID_VERSION ({statut}) — une version de \
             structure est fausse. Voir `encode_nvenc::abi` et sa commande de \
             relecture ; ce n'est PAS un défaut de pilote."
        );
    }
    bail!("{quoi} : NVENCSTATUS {statut}")
}

/// La porte : la DLL du pilote et sa table de fonctions.
///
/// ⚠️ **La DLL n'est jamais relâchée**, à dessein : plusieurs sessions
/// coexistent (une par fenêtre), et un `FreeLibrary` sous les pieds d'une
/// voisine serait un plantage. Le processus la rend en mourant.
pub struct Porte {
    pub(super) fonctions: ListeDeFonctions,
}

impl Porte {
    pub fn ouvrir() -> Result<Self> {
        let module = unsafe { LoadLibraryA(windows::core::s!("nvEncodeAPI64.dll")) }
            .context("chargement de nvEncodeAPI64.dll (la DLL vient du pilote NVIDIA)")?;

        // ① La version du pilote AVANT tout, pour que le refus soit lisible.
        let version_max = unsafe { GetProcAddress(module, windows::core::s!("NvEncodeAPIGetMaxSupportedVersion")) }
            .ok_or_else(|| anyhow!("NvEncodeAPIGetMaxSupportedVersion absente de la DLL"))?;
        let version_max: fonctions::VersionMaxSupportee = unsafe { std::mem::transmute(version_max) };
        let mut rendue = 0u32;
        verifier(unsafe { version_max(&mut rendue) }, "NvEncodeAPIGetMaxSupportedVersion")?;
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

        // ② La table de fonctions.
        let creer = unsafe { GetProcAddress(module, windows::core::s!("NvEncodeAPICreateInstance")) }
            .ok_or_else(|| anyhow!("NvEncodeAPICreateInstance absente de la DLL"))?;
        let creer: fonctions::CreerInstance = unsafe { std::mem::transmute(creer) };
        let mut table: ListeDeFonctions = unsafe { std::mem::zeroed() };
        table.version = abi::FUNCTION_LIST_VER;
        verifier(unsafe { creer(&mut table) }, "NvEncodeAPICreateInstance")?;

        Ok(Self { fonctions: table })
    }
}
