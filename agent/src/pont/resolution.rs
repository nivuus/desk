//! The **thirteen** entry points of `ProjectedFSLib.dll`: their name, their rank, and the
//! resolution itself. **PURE** — no `cfg`, no dependency on `windows`,
//! entirely tested on the host through an injected resolver.
//!
//! ⚠️ **Why this module exists separately from `pont/projfs/chargement.rs`.**
//! F1's plan (task 12, step 4a) has the guard "`charger()` fails
//! if **a single one** of the thirteen entry points is missing, and the error **names**
//! the entry point" carried by a `#[cfg(windows)]` smoke test, of which it writes itself
//! that it is only made red "by injecting a fourteenth bogus name into the
//! list, once, then removing it" — that is, by a manual
//! manipulation, on the VM, that no later run replays. This repository has
//! caught five checks unable to fail, three of them written by a plan.
//! The decision — which is a decision, and not a convenience — therefore lives here,
//! **pure**, and its test sweeps the thirteen entry points one by one, at each
//! `cargo test`. What remains in `chargement.rs` is what no host test
//! can reach: `LoadLibraryW`, `GetProcAddress`, and the thirteen
//! `transmute`s.
//!
//! ⚠️ **This module says NOTHING about the ABI.** It checks that thirteen names exist
//! and at which rank; it cannot check that a signature transcribed by
//! hand matches the DLL's. It is the spec's risk R7, and it
//! is not covered here — see the header of `pont/projfs/chargement.rs`.

/// The number of entry points. It is not a convenience: it types [`NOMS`] and the
/// array [`resoudre`] returns, so adding a fourteenth without
/// listing it in [`NOMS`] does not compile.
pub const NOMBRE: usize = 13;

/// The thirteen exported names, **in the order of the ranks below**.
///
/// The list is spec §4.3's, found present in
/// `windows-0.62.2/src/Windows/Win32/Storage/ProjectedFileSystem/mod.rs` on
/// August 19th, 2026. `PrjWritePlaceholderInfo2` and `PrjFillDirEntryBuffer2` are
/// **deliberately absent** from it: from a later generation, not checked
/// present on this machine (spec §2.2), and useless to v1 — they serve
/// symbolic links, out of scope (spec §3.5.2).
pub const NOMS: [&str; NOMBRE] = [
    "PrjAllocateAlignedBuffer",
    "PrjClearNegativePathCache",
    "PrjCompleteCommand",
    "PrjDeleteFile",
    "PrjFileNameCompare",
    "PrjFileNameMatch",
    "PrjFillDirEntryBuffer",
    "PrjFreeAlignedBuffer",
    "PrjMarkDirectoryAsPlaceholder",
    "PrjStartVirtualizing",
    "PrjStopVirtualizing",
    "PrjWriteFileData",
    "PrjWritePlaceholderInfo",
];

/// The thirteen addresses, **named**.
///
/// ⚠️ **Why a structure with named fields and not a `[usize; 13]` plus
/// thirteen rank constants.** The first draft did exactly that,
/// and a mutation played after green refuted it: swapping two ranks at the
/// `ProjFs` construction site (`adresses[COMPARER_NOMS]` against
/// `adresses[APPARIER_NOM]`) **survived** — the ranks test did pin
/// `NOMS[RANG]`, but nothing pinned the FIELD ↔ RANK pairing at the call
/// site, which is precisely where the mistake is made. Two swapped
/// entry points are two `transmute`s to the wrong signature.
///
/// With named fields, the only remaining positional pairing is
/// this one, in a **pure** module — and `chaque_champ_recoit_l_adresse_de_son_entree`
/// sweeps it, all thirteen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adresses {
    pub allouer_tampon_aligne: usize,
    pub vider_cache_negatif: usize,
    pub completer_commande: usize,
    pub supprimer_fichier: usize,
    pub comparer_noms: usize,
    pub apparier_nom: usize,
    pub remplir_tampon_entrees: usize,
    pub rendre_tampon_aligne: usize,
    pub marquer_racine: usize,
    pub demarrer_virtualisation: usize,
    pub arreter_virtualisation: usize,
    pub ecrire_donnees: usize,
    pub ecrire_info_marqueur: usize,
}

/// An entry point the library does not export.
///
/// **It carries the NAME**, not a rank nor a count: it is what distinguishes
/// "this VM has no ProjFS at all" from "this VM has a generation of
/// ProjectedFSLib earlier than the one v1 expects". A message that would say
/// only "an entry point is missing" would leave the two indistinguishable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntreeManquante {
    pub nom: &'static str,
}

impl std::fmt::Display for EntreeManquante {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProjectedFSLib.dll n'exporte pas « {} »", self.nom)
    }
}

impl std::error::Error for EntreeManquante {}

/// Resolves the thirteen entry points through the provided `resolveur`, and returns their addresses
/// **at their rank**.
///
/// The resolver returns `None` — or `Some(0)` — for an absent entry point: it is the
/// contract of `GetProcAddress`, whose `NULL` is not an address.
/// Confusing the two would `transmute` a null pointer into a function
/// pointer, and the first call would jump to address 0.
///
/// **Immediate failure at the first missing one**: the next would teach nothing
/// more, and it is the FIRST that the log must name — a DLL of an
/// earlier generation would otherwise name its last absent entry point, and the
/// diagnosis would start from the wrong end.
pub fn resoudre(
    mut resolveur: impl FnMut(&str) -> Option<usize>,
) -> Result<Adresses, EntreeManquante> {
    let mut a = [0usize; NOMBRE];
    for (rang, nom) in NOMS.iter().enumerate() {
        match resolveur(nom) {
            Some(adresse) if adresse != 0 => a[rang] = adresse,
            _ => return Err(EntreeManquante { nom }),
        }
    }
    // ⚠️ **The bridge's ONLY positional pairing**, and it is here, pure and
    // swept by a host test — rather than at the `transmute` site, where nothing
    // could exercise it. The order must follow that of [`NOMS`].
    Ok(Adresses {
        allouer_tampon_aligne: a[0],
        vider_cache_negatif: a[1],
        completer_commande: a[2],
        supprimer_fichier: a[3],
        comparer_noms: a[4],
        apparier_nom: a[5],
        remplir_tampon_entrees: a[6],
        rendre_tampon_aligne: a[7],
        marquer_racine: a[8],
        demarrer_virtualisation: a[9],
        arreter_virtualisation: a[10],
        ecrire_donnees: a[11],
        ecrire_info_marqueur: a[12],
    })
}

#[cfg(test)]
mod tests;
