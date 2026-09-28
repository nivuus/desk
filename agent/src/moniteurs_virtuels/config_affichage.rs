//! The correspondence between the target identifier the driver returns and the
//! GDI name (`\\.\DISPLAYn`) that DXGI enumerates.
//!
//! 🔴 **THIS MODULE EXISTS BECAUSE AN ASSERTION OF THIS REPOSITORY WAS FALSE.**
//! `superviseur/placement.rs` wrote since D1: "The first returns a
//! target identifier of its own, the second enumerates by
//! `(index_adaptateur, index_sortie)`. **No correspondence is
//! exposed**: pairing is therefore done by dimensions and by elimination."
//! A correspondence is exposed, through Win32's CCD API (*Connecting and
//! Configuring Displays*), and it is the whole design of the pairing
//! that rested on that sentence.
//!
//! **The chaining, in three steps:**
//!
//! 1. `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` returns the active paths,
//!    each linking a SOURCE (which the GDI name belongs to) to a TARGET (the
//!    monitor);
//! 2. the target of interest is the one whose `(adapterId, id)` is the
//!    pair `SortieAjoutee` returned to us at creation;
//! 3. `DisplayConfigGetDeviceInfo(GET_SOURCE_NAME)` on the source of this
//!    path returns `viewGdiDeviceName`, which is literally `\\.\DISPLAYn` —
//!    the same name as `DXGI_OUTPUT_DESC.DeviceName`, from which
//!    `SortieDxgi::nom_sortie` is populated (`capture/enumeration.rs`).
//!
//! 🔴 **WHAT THIS MODULE ASSUMES, AND WHICH IS NOT CONFIRMED.** That
//! `identifiant_cible` is the CCD target `id` on the returned adapter.
//! `sudovda.rs` itself says that the layout of `VIRTUAL_DISPLAY_ADD_OUT`
//! is "unconfirmed". **A versioned piece of evidence makes the hypothesis credible
//! without establishing it**: batch 22 counted ten ghost monitors
//! `DISPLAY\SMKD1CE\…UID256` to `UID265` noting that "the driver's
//! identifiers (256…265) go round in circles", and the `UIDnnnn` suffix of a
//! monitor instance path IS the CCD target `id`
//! (`docs/superpowers/plans/2026-08-30-lot22-hub-session-resultats.md`).
//! The numbers coincide; that the returned LUID is the one CCD uses is not
//! measured.
//!
//! 🔵 **And if the hypothesis is false, the cost is nil**: the search
//! finds no path, `nom_gdi_de_la_cible` returns `None`, and the caller
//! falls back on the fallback that is the product from before batch 32. It is this
//! property — and it alone — that made this module deliverable before being
//! measured on the VM.
//!
//! Outside `#[cfg(windows)]`, like the parent module and for the same reason: the
//! RULE must have tests, and they would not run under
//! `#[cfg(windows)]`. The Win32 half lives in `mod win`, below — same
//! pattern as `superviseur/placement.rs`.

use crate::moniteurs_virtuels::Adaptateur;

/// An active display path, reduced to what pairing needs.
///
/// A type OF OUR OWN, and not `DISPLAYCONFIG_PATH_INFO`: it is what lets the
/// rule below be pure, tested on the Linux host, and not bring
/// a Win32 type into a module the parent compiles everywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheminActif {
    /// The TARGET's adapter, never the source's: it is the one
    /// the driver returned to us.
    pub adaptateur_cible: Adaptateur,
    /// The target identifier, as the display system knows it.
    pub id_cible: u32,
    /// The GDI name of this path's SOURCE — `\\.\DISPLAYn`.
    pub nom_gdi: String,
}

/// The GDI name of the output the driver has just created, designated by what we
/// GAVE it rather than by a difference of sets.
///
/// 🔴 **AN AMBIGUITY REFUSES TO DECIDE, it does not take the first one.**
/// It is the precedent of `AUDIO_PERIPHERIQUE` (`wasapi/peripherique.rs`, where
/// `Choix::Ambigu` refuses rather than falling back on an enumeration rank through
/// the back door) and the lesson of D1's DXGI indices, paid for once.
/// Two active paths carrying the same `(adapter, id)` pair is a state
/// Windows should not produce; if it does, the caller falls back
/// on its fallback — that is on the product from before batch 32 — rather than
/// designating an output at random.
pub fn nom_gdi_de_la_cible(
    chemins: &[CheminActif],
    adaptateur: Adaptateur,
    id_cible: u32,
) -> Option<&str> {
    let mut trouves = chemins
        .iter()
        .filter(|c| c.adaptateur_cible == adaptateur && c.id_cible == id_cible);
    let premier = trouves.next()?;
    if trouves.next().is_some() {
        return None;
    }
    Some(&premier.nom_gdi)
}

#[cfg(windows)]
mod win {
    use anyhow::Result;
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };
    use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};

    use super::CheminActif;

    /// The ACTIVE display paths, as the system sees them.
    ///
    /// `QDC_ONLY_ACTIVE_PATHS` and not all paths: an inactive target
    /// has no source, hence no GDI name, hence nothing to pair. A virtual
    /// output that has just been created but that Windows has not yet
    /// attached simply does not appear in it — the caller polls, it does not
    /// fail.
    ///
    /// ⚠️ **SILENT, and it is a constraint, not an oversight.** It is
    /// called in a polling loop at 10 Hz
    /// (`creation_sortie::attendre_notre_sortie`), and this repository has paid twice
    /// for a trace emitted at a loop's cadence (TURN work stream,
    /// fix I2 of D1; 18,619 lines in a few seconds on a CIFS
    /// share). The caller logs once, on its failure path.
    ///
    /// ⚠️ **The `ERROR_INSUFFICIENT_BUFFER` loop is deliberate**: the
    /// display configuration can change BETWEEN sizing and
    /// reading — it is precisely what a virtual output does that
    /// attaches while we poll. A single attempt would make a transient
    /// error indistinguishable from a failure.
    pub fn chemins_actifs() -> Result<Vec<CheminActif>> {
        const ATTEMPTS: u32 = 4;
        let mut derniere: Option<WIN32_ERROR> = None;
        for _ in 0..ATTEMPTS {
            let (mut n_chemins, mut n_modes) = (0u32, 0u32);
            let statut = unsafe {
                GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_chemins, &mut n_modes)
            };
            if statut != ERROR_SUCCESS {
                return Err(anyhow::anyhow!(
                    "GetDisplayConfigBufferSizes a rendu {:#010x}",
                    statut.0
                ));
            }
            let mut chemins = vec![DISPLAYCONFIG_PATH_INFO::default(); n_chemins as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
            let statut = unsafe {
                QueryDisplayConfig(
                    QDC_ONLY_ACTIVE_PATHS,
                    &mut n_chemins,
                    chemins.as_mut_ptr(),
                    &mut n_modes,
                    modes.as_mut_ptr(),
                    None,
                )
            };
            if statut == ERROR_SUCCESS {
                chemins.truncate(n_chemins as usize);
                return Ok(chemins.iter().filter_map(traduire).collect());
            }
            derniere = Some(statut);
        }
        Err(anyhow::anyhow!(
            "QueryDisplayConfig a rendu {:#010x} après {ATTEMPTS} essais — la \
             configuration d'affichage change plus vite qu'on ne la lit",
            derniere.map(|e| e.0).unwrap_or(0)
        ))
    }

    /// A Win32 path to our type, or `None` if its source has no name.
    ///
    /// A path without a source name is not an error: there is
    /// simply nothing to pair with it, and raising it as an `Err`
    /// would condemn all the other paths of the same survey.
    fn traduire(chemin: &DISPLAYCONFIG_PATH_INFO) -> Option<CheminActif> {
        let mut nom = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: windows::Win32::Devices::Display::DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: chemin.sourceInfo.adapterId,
                id: chemin.sourceInfo.id,
            },
            ..Default::default()
        };
        // Returns a raw `i32` (`ERROR_SUCCESS` is 0), and not a `WIN32_ERROR`
        // — the signature of `DisplayConfigGetDeviceInfo` differs from that of
        // its two neighbours. Checked in windows-0.62.2, not assumed.
        if unsafe { DisplayConfigGetDeviceInfo(&mut nom.header) } != 0 {
            return None;
        }
        let fin = nom
            .viewGdiDeviceName
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(nom.viewGdiDeviceName.len());
        let nom_gdi = String::from_utf16_lossy(&nom.viewGdiDeviceName[..fin]);
        if nom_gdi.is_empty() {
            return None;
        }
        Some(CheminActif {
            adaptateur_cible: (
                chemin.targetInfo.adapterId.LowPart,
                chemin.targetInfo.adapterId.HighPart,
            ),
            id_cible: chemin.targetInfo.id,
            nom_gdi,
        })
    }
}

#[cfg(windows)]
pub use win::chemins_actifs;

#[cfg(test)]
mod tests {
    use super::*;

    fn chemin(adaptateur: Adaptateur, id: u32, nom: &str) -> CheminActif {
        CheminActif {
            adaptateur_cible: adaptateur,
            id_cible: id,
            nom_gdi: nom.to_string(),
        }
    }

    #[test]
    fn la_cible_creee_donne_le_nom_gdi_de_sa_source() {
        let chemins = vec![
            chemin((7, 0), 4096, "\\\\.\\DISPLAY1"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
        ];
        assert_eq!(
            nom_gdi_de_la_cible(&chemins, (9, 0), 256),
            Some("\\\\.\\DISPLAY5")
        );
    }

    #[test]
    fn un_identifiant_de_cible_ne_suffit_pas_sans_son_adaptateur() {
        // The SAME target identifier on TWO adapters: it is the case
        // the pair exists to decide, and the reason why batch 32
        // stopped throwing away the LUID.
        let chemins = vec![
            chemin((7, 0), 256, "\\\\.\\DISPLAY1"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
        ];
        assert_eq!(
            nom_gdi_de_la_cible(&chemins, (7, 0), 256),
            Some("\\\\.\\DISPLAY1")
        );
        assert_eq!(
            nom_gdi_de_la_cible(&chemins, (9, 0), 256),
            Some("\\\\.\\DISPLAY5")
        );
    }

    #[test]
    fn une_cible_absente_rend_none_et_non_un_choix_au_hasard() {
        let chemins = vec![chemin((7, 0), 4096, "\\\\.\\DISPLAY1")];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), None);
    }

    #[test]
    fn une_paire_ambigue_refuse_de_trancher() {
        // Two paths for the same pair: Windows should not produce
        // this state. We return `None` — hence the caller's fallback — rather than
        // designating the first, which would be a disguised enumeration rank.
        let chemins = vec![
            chemin((9, 0), 256, "\\\\.\\DISPLAY5"),
            chemin((9, 0), 256, "\\\\.\\DISPLAY6"),
        ];
        assert_eq!(nom_gdi_de_la_cible(&chemins, (9, 0), 256), None);
    }
}
