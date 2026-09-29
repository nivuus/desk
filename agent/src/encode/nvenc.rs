//! The **PURE** part of the NVENC path: the choice of path, the translation of
//! our settings into NVENC's, and the version arithmetic of
//! structures. No `cfg`, no Windows call, **tested on the Linux host**.
//!
//! ⚠️ **`#[path]` in the parent, and it is checked against the convention**
//! (§ "Child module convention", head of `docs/claude/module-conventions.md`): this module is
//! extracted from `encode.rs`, which is `#![cfg(windows)]`, precisely so that its
//! pure logic compiles and is tested on the host. Its name, `encode_nvenc`,
//! carries the `encode_` prefix of an existing top-level module
//! (`mod encode;`, `main.rs`), so the rule places it **in its parent**:
//! file `encode/nvenc.rs`, declaration
//! `#[path = "encode/nvenc.rs"] mod encode_nvenc;` in `main.rs`.
//! No other top-level module is a prefix of it — the "longest prefix"
//! clause therefore changes nothing here. Same precedent as
//! `wasapi_format`, hoisted for exactly the same reason.
//!
//! 🔴 **WHY NVENC BEFORE THE MFT, AND WHY THE MFT STAYS.** It is
//! not a preference, it is a measurement, and the commands that establish it
//! are given so that it can be redone without believing anyone — the
//! precedent this repository is paying for right now is a wrong comment that
//! led to designing a defect (`placement.rs`).
//!
//! - On the target VM, on 30 August 2026, the `NVIDIA H.264 Encoder MFT`
//!   activates in **session 0** and returns `0x8000FFFF` in **session 1**, on
//!   the four arrangements Media Foundation allows trying. Two
//!   green controls set up in the same run — the **software** H.264 encoder
//!   and the **software** video processor do activate in
//!   both sessions — establish that the machinery is not at fault.
//! - Apollo, on the SAME machine, in the SAME session 1, builds six
//!   NVENC encoders through the **native** door: its live process carries
//!   **no** Media Foundation module.
//!
//! **Redo the measurement** (detail and raw surveys:
//! `docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md`): (policy: allow-fr, real file path)
//!
//! ```text
//! # Apollo's modules while it encodes, in session 1:
//! (Get-Process sunshine).Modules | ? { $_.ModuleName -match 'mfplat|nvEnc' }
//! # the encoders it built:
//! Select-String 'NvEnc: created encoder' 'C:\Program Files\Apollo\config\sunshine.log'
//! ```
//!
//! 🔴 **AND THE MFT MUST NOT BE REMOVED.** `MFTEnumEx` does not enumerate
//! "the NVIDIA encoder": it enumerates **the hardware H.264 encoders**,
//! Intel Quick Sync and AMD VCE included. A machine without NVIDIA has no
//! NVENC; taking the MFT away from it would deprive it of **any** hardware encoder.
//! The MFT is therefore the **generic** fallback, and it stays unchanged.

/// The transcription of the upstream ABI, isolated in its own file because
/// it carries a licence notice that only applies to it.
///
/// ⚠️ **The `#[path]` below is NOT the one of the repository's convention,
/// and confusing them would muddle the next reader.** The convention targets
/// modules extracted from a **non-portable** parent to compile them
/// on the host; `abi` has nothing to flee, its parent is already pure. This `#[path]`
/// is imposed by a **rustc** rule: when a module is itself
/// loaded through `#[path = "encode/nvenc.rs"]`, its children are looked up in
/// the directory of THIS file — `encode/` — and not in a same-named `encode/nvenc/`.
/// Without the explicit line, rustc asks for `encode/abi.rs`
/// (measured: `error[E0583]: file not found for module 'abi'`). It is the same
/// mechanism used for another reason, exactly as `table.rs` uses it
/// to split its tests.
#[path = "nvenc/abi.rs"]
pub mod abi;

/// The structure layouts, same attribution boundary as `abi`.
/// Same reason for the `#[path]` — it is rustc that imposes it, not the
/// repository's naming convention.
#[path = "nvenc/structures.rs"]
pub mod structures;

/// The layouts exchanged per image. Same attribution
/// boundary, same reason for the `#[path]`.
#[path = "nvenc/tampons.rs"]
pub mod tampons;

/// The driver's function table. Same boundary, same reason for the `#[path]`.
#[path = "nvenc/fonctions.rs"]
pub mod fonctions;

/// The encoding session itself. **The ONLY file of the NVENC subtree
/// that needs Windows**: everything else -- choice rule, ABI,
/// layouts -- is tested on the host. It lives here rather than under
/// `encode.rs` so that the attribution boundary of the licence notice
/// remains ONE single subtree.
#[cfg(windows)]
#[path = "nvenc/porte.rs"]
pub mod porte;

#[cfg(windows)]
#[path = "nvenc/session.rs"]
pub mod session;

/// NVIDIA's PCI vendor identifier.
///
/// Surveyed on the target VM rather than copied from a list: the batch 31 probe
/// read `vendeur=0x10DE peripherique=0x2786` on `NVIDIA GeForce RTX 4070`,
/// and `0x1414` (Microsoft) on the `Basic Render Driver` of the QEMU VGA.
pub const VENDEUR_NVIDIA: u32 = 0x10DE;

/// A graphics adapter, reduced to what the decision needs.
///
/// Deliberately **without a Windows type**: it is what lets the rule
/// below be tested on the host. The `#[cfg(windows)]` caller
/// fills these fields from `IDXGIAdapter1::GetDesc1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adaptateur {
    pub nom: String,
    pub vendeur: u32,
    /// The DXGI LUID, flattened. NVENC does not use it to choose — it is the
    /// D3D11 device that carries the choice — but tracing it makes it possible to say
    /// **which** of the same-named adapters was retained, and this VM
    /// presents two that carry the same name and the same device
    /// identifier.
    pub luid: (i32, u32),
}

/// La voie d'encodage retenue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voie {
    /// The native NVENC API (`nvEncodeAPI64.dll`), on the adapter of the given
    /// index.
    Nvenc(usize),
    /// The Media Foundation MFT — the **generic** fallback, for Intel, AMD, and
    /// for session 0 where the NVIDIA MFT works.
    Mft,
}

/// Chooses the path from the present adapters alone.
///
/// **The first NVIDIA adapter wins, and the DXGI enumeration order
/// is authoritative.** ⚠️ This is NOT the positional index trap paid for in D1:
/// this index is neither memorised nor carried from one run to another, it
/// is only a reference into the list just read, in the same call.
/// The name and the LUID are returned with it, so that the trace says **which one**.
///
/// **No NVIDIA adapter ⇒ `Mft`**, and it is the nominal case of an Intel
/// or AMD machine: see the module comment, the MFT is the generic
/// fallback and not a last resort.
pub fn choisir_voie(adaptateurs: &[Adaptateur]) -> Voie {
    match adaptateurs.iter().position(|a| a.vendeur == VENDEUR_NVIDIA) {
        Some(index) => Voie::Nvenc(index),
        None => Voie::Mft,
    }
}

/// Support factory shared by the two test modules of this file.
#[cfg(test)]
mod tests_appui {
    use super::Adaptateur;
    pub fn adaptateur(nom: &str, vendeur: u32) -> Adaptateur {
        Adaptateur {
            nom: nom.to_string(),
            vendeur,
            luid: (0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_appui::adaptateur;
    use super::*;

    #[test]
    fn an_nvidia_machine_takes_the_native_path() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        assert_eq!(choisir_voie(&vus), Voie::Nvenc(0));
    }

    /// 🔴 The case that forbids removing the MFT: a machine without NVIDIA.
    /// If this assertion fell to `Nvenc`, the machine would lose ANY
    /// hardware encoder — it is the regression this test pins.
    #[test]
    fn a_machine_without_nvidia_keeps_the_mft() {
        let vus = vec![
            adaptateur("Intel(R) UHD Graphics 770", 0x8086),
            adaptateur("Microsoft Basic Render Driver", 0x1414),
        ];
        assert_eq!(choisir_voie(&vus), Voie::Mft);
    }

    #[test]
    fn no_adapter_at_all_keeps_the_mft() {
        assert_eq!(choisir_voie(&[]), Voie::Mft);
    }

    /// The EXACT topology of the target VM, surveyed by the batch 31 probe in
    /// session 1: the QEMU VGA is adapter **0** and carries the only
    /// attached display; the two NVIDIA ones carry none. The path must
    /// nevertheless be NVENC, and target the first NVIDIA — that is
    /// index **1**, not index 0.
    #[test]
    fn the_measured_vm_topology_targets_the_first_nvidia() {
        let vus = vec![
            adaptateur("Microsoft Basic Render Driver", 0x1414),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
            adaptateur("Microsoft Basic Render Driver", 0x1414),
        ];
        assert_eq!(choisir_voie(&vus), Voie::Nvenc(1));
    }

    /// ⚠️ The name decides NOTHING: the vendor does. An adapter that would
    /// be named "NVIDIA …" without carrying `0x10DE` must not lead to
    /// a DLL its machine does not have.
    #[test]
    fn the_name_does_not_decide_the_vendor_does() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", 0x1414)];
        assert_eq!(choisir_voie(&vus), Voie::Mft);
    }
}

/// `E_UNEXPECTED` — "Catastrophic failure". The code the NVIDIA MFT returns
/// in session 1 on the target VM, measured on 30 August 2026.
pub const ECHEC_CATASTROPHIQUE: i32 = 0x8000_FFFFu32 as i32;

/// **Stage ③ of batch 31's three: make the failure READABLE.**
///
/// 🔴 **THIS DOES NOT MAKE THE PRODUCT WORK, and is not written as if it
/// did.** When the two useful stages have failed, what remains is not to
/// let a bare `0x8000FFFF` go up: batches 30 and 31 cost two
/// days to establish what this message says in a few lines, and without it
/// the next reader would pay for them again.
///
/// The message names three things, in this order: the returned code, what
/// adapters the machine carries (it is the deciding variable), and the
/// document that carries the measurement.
///
/// ⚠️ **The known-cause paragraph is only added if both
/// measured conditions are met** — the exact code AND an NVIDIA
/// adapter. On an Intel or AMD machine, the same code would mean something
/// else, and asserting our diagnosis there would be a false assertion
/// presented as a fact.
pub fn diagnostic_activation(code: i32, error: &str, adaptateurs: &[Adaptateur]) -> String {
    let noms: Vec<&str> = adaptateurs.iter().map(|a| a.nom.as_str()).collect();
    let mut message = format!(
        "activating the hardware H.264 encoder (ActivateObject): {error} \
         — adapters seen: [{}]",
        noms.join(" | ")
    );
    if code == ECHEC_CATASTROPHIQUE && matches!(choisir_voie(adaptateurs), Voie::Nvenc(_)) {
        message.push_str(
            " — KNOWN CAUSE, MEASURED ON 30 AUGUST 2026 (batch 31): the MFT \
             « NVIDIA H.264 Encoder MFT » returns 0x8000FFFF in SESSION 1 on this \
             machine, while it activates in session 0. It is neither a missing \
             driver nor a broken Media Foundation: in the same run, \
             the SOFTWARE H.264 encoder and the SOFTWARE video processor both \
             activate. Setting MFT_ENUM_ADAPTER_LUID, keeping an NVIDIA D3D11 \
             device alive, or binding the virtual display to the NVIDIA GPU are THREE \
             remedies already REFUTED BY MEASUREMENT — do not retry them. The path \
             that works here is the native NVENC API. Details, raw readings and \
             refuted remedies: ",
        );
        message.push_str("docs/superpowers/plans/2026-08-30-encodeur-porte-apollo-resultats.md");
    }
    message
}

#[cfg(test)]
mod tests_diagnostic {
    use super::tests_appui::adaptateur;
    use super::*;

    const AUTRE_CODE: i32 = 0x8007_0057u32 as i32; // E_INVALIDARG

    #[test]
    fn names_the_adapters_seen() {
        let vus = vec![
            adaptateur("Microsoft Basic Render Driver", 0x1414),
            adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA),
        ];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(
            m.contains("Microsoft Basic Render Driver | NVIDIA GeForce RTX 4070"),
            "{m}"
        );
    }

    #[test]
    fn adds_the_known_cause_when_both_conditions_are_met() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(m.contains("KNOWN CAUSE"), "{m}");
        assert!(
            m.contains("2026-08-30-encodeur-porte-apollo-resultats.md"),
            "{m}"
        );
    }

    /// 🔴 The arm that prevents asserting our diagnosis where it does not
    /// apply: same code, machine WITHOUT NVIDIA.
    #[test]
    fn stays_silent_on_the_cause_when_no_nvidia_is_present() {
        let vus = vec![adaptateur("Intel(R) UHD Graphics 770", 0x8086)];
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &vus);
        assert!(!m.contains("KNOWN CAUSE"), "{m}");
        assert!(m.contains("Intel(R) UHD Graphics 770"), "{m}");
    }

    /// The other arm: NVIDIA machine, but ANOTHER error code.
    #[test]
    fn stays_silent_on_the_cause_for_another_code() {
        let vus = vec![adaptateur("NVIDIA GeForce RTX 4070", VENDEUR_NVIDIA)];
        let m = diagnostic_activation(AUTRE_CODE, "The parameter is incorrect", &vus);
        assert!(!m.contains("KNOWN CAUSE"), "{m}");
    }

    #[test]
    fn without_any_adapter_the_message_stays_readable() {
        let m = diagnostic_activation(ECHEC_CATASTROPHIQUE, "Catastrophic failure", &[]);
        assert!(m.contains("adapters seen: []"), "{m}");
        assert!(!m.contains("KNOWN CAUSE"), "{m}");
    }
}
