//! Binary frame of the file bridge (reliable, ordered channel).
//!
//! Format: `version: u8 | type: u8 | correlation: u32 | longueur_entete: u32 |
//! entete | charge`, integers in **little-endian** — the convention of [`input`]
//! (`proto/src/input.rs`), and that of `DataView.setUint32(…, true)` on the
//! browser side.
//!
//! **Why binary, and not JSON like [`control`]** — the reason is a
//! figure taken on the old bridge, not a preference: `src/file.js:264`
//! serializes the bytes of a write through `Array.from(buffer.slice(0, length))`,
//! i.e. ~4 bytes transmitted per useful byte, and `web/index.js:653-657` does worse
//! on the way back. A file protocol that encodes bytes in JSON pays this
//! order of magnitude **on every byte of every read**. Here the payload is
//! carried as is, never encoded; only the header, small and
//! structured, is JSON.
//!
//! From [`control`] we take the version doctrine **and its note**: no
//! default value on the version. A frame too short to carry its
//! version is **rejected**, never silently completed.
//!
//! [`input`]: crate::input
//! [`control`]: crate::control

use serde::{Deserialize, Serialize};

/// Version of the file protocol. Increment on any format change.
///
/// 🔴 **F2 ADDS FOUR MESSAGE TYPES AND DOES NOT INCREMENT IT, and it is a
/// decision, not an oversight.** F1 left it at 1 "precisely so that
/// the arrival of these verbs is a visible break" (its legacy 13) — but the
/// break is **ADDITIVE**: a read-only v1 bridge and a v1 client that
/// can write get along without reservation, the client simply ignoring
/// types it will never receive. Incrementing to 2 would break compatibility
/// in the only direction where it has no value — both ends are shipped
/// together — and would make a running session fail during a migration.
///
/// ⚠️ **What will increment it is a change of SHAPE, not of inventory**:
/// a renamed field, a different byte order, a header whose meaning
/// changes. Those, a peer of another version cannot ignore.
pub const FICHIERS_VERSION: u8 = 1;

/// Maximum size of the **payload** of a frame, in bytes.
///
/// ⚠️ **NOT CALIBRATED.** Set, not measured.
///
/// ✅ **F4 GAVE WHAT IS NEEDED TO JUDGE IT, AND IT ADDS A FACT THIS COMMENT
/// DID NOT SAY: IT IS NOT WHAT BOUNDS A LISTING.** An enumeration goes
/// into the HEADER of a single message, which **nothing bounds** — neither here (the check
/// is on `charge.len()`), nor on the browser side. It is SCTP's
/// `max-message-size = 256 KiB` that stops it, at **~3,150 entries**
/// measured for ~3,159 computed, and the refusal of `send()` produces NO
/// answer: the command dies at `DELAI_LISTER`.
///
/// 🔴 **What it does bound, on the other hand, bites**: at ~33 KiB/s measured on the
/// bridge channel, a 64 KiB chunk takes ~2 s, and four concurrent chunks
/// exceed `DELAI_LIRE`. It joins `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC`,
/// `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO` and `REARMEMENTS_MAX` in the
/// set of constants of this repository that no measurement has judged.
///
/// ⚠️ **The name says "frame", the value bounds the PAYLOAD** — divergence noted
/// in the plan, which names the constant `TAILLE_TRAME_MAX` and writes in the
/// same breath the check "the maximum payload of `TAILLE_TRAME_MAX` passes".
/// It is the second reading that is kept, because it is the one that makes the
/// module consistent with `pont::decoupe`, whose `max` is indeed a payload
/// size. A full frame therefore weighs `TAILLE_TRAME_MAX + TAILLE_ENTETE_FIXE +
/// the length of the JSON header`: the name is misleading, saying so costs three
/// lines, keeping quiet would cost an application MTU overrun one day.
pub const TAILLE_TRAME_MAX: usize = 64 * 1024;

/// Length of the fixed part of a frame: version, type, correlation,
/// header length.
pub const TAILLE_ENTETE_FIXE: usize = 1 + 1 + 4 + 4;

// Bridge → browser requests — **they AWAIT an answer**.
pub const TYPE_LISTER: u8 = 1;
pub const TYPE_ATTRIBUTS: u8 = 2;
pub const TYPE_LIRE: u8 = 3;
pub const TYPE_ECRIRE: u8 = 4; // F2 — `Ecrire` header, the payload carries the bytes
pub const TYPE_CREER: u8 = 5; // F2 — `Creer` header, empty payload
pub const TYPE_RENOMMER: u8 = 7; // F3 — `Renommer` header, empty payload
pub const TYPE_SUPPRIMER: u8 = 8; // F3 — `Supprimer` header, empty payload

// Bridge → browser announcements — **they await NOTHING**.
//
// 🔴 **THIRD FAMILY, and it breaks the invariant the browser states
// in capitals** (`client/src/fichiers/protocole.ts`): "a request always
// receives an answer". An ANNOUNCEMENT receives none — no table
// entry corresponds to it on the bridge side, and not answering it therefore leaves nothing
// in flight. **The set of announcements is CLOSED**, and that is what keeps this
// family from becoming the silent catch-all arm this repository paid for four
// times on `capteur/pont_media.rs`.
pub const TYPE_DUES: u8 = 6; // F2 — `Dues` header, empty payload

// BROWSER → BRIDGE announcements — **they await NOTHING**.
//
// 🔴 **FOURTH FAMILY, and it is the first one that goes upstream.** The other three
// go from the bridge to the browser, or answer a bridge request; these
// two leave the browser **without having been asked for**, and their
// correlation is **IGNORED**.
//
// ⚠️ **THAT IS WHAT MAKES THEM DANGEROUS, and it must be said here rather than
// discovered.** The bridge decodes every incoming frame then looks up its correlation
// in `pont::table`; an unknown correlation is **thrown away** in a
// `tracing::debug!` (`agent/src/pont/service.rs`) — invisible under
// `RUST_LOG=info`, which is the setting of `scripts/run-agent.sh`. An upstream
// announcement that went through this path **would do nothing, and nothing would
// say so**. It is the catch-all arm this repository paid for **five times** on
// `capteur/pont_media.rs` (D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8
// `PleinEcran`, P1 clipboard) and a sixth on
// `superviseur/signalisation.rs`. **F5 routes them BEFORE `resoudre`**, and
// it is the only reason they work.
//
// **The set is CLOSED**, like that of the third family and for the same
// reason.
pub const TYPE_BONJOUR: u8 = 68; // F5 — `Bonjour` header, empty payload
pub const TYPE_RAFRAICHIR: u8 = 69; // F5 — EMPTY header `{}`, empty payload

// Browser → bridge answers.
pub const TYPE_ENTREES: u8 = 64;
pub const TYPE_META: u8 = 65;
pub const TYPE_DONNEES: u8 = 66;
pub const TYPE_FAIT: u8 = 67; // F2 — EMPTY header `{}`, empty payload
pub const TYPE_ECHEC: u8 = 127;

// ✅ **7 AND 8 ARE TAKEN, AND BY THE ONE THEY WERE RESERVED FOR.** *(These
// lines said "RESERVED for F3"; F3 took them, and the reservation has
// become a statement of fact rather than being left to the future.)* The reservation
// did its job: F2 took 4, 5 and 6 **skipping** 7 and 8, so
// that neither of the two sub-blocks had to renumber — and a late
// renumbering is exactly the move through which a reference outlives what it
// designates.
//
// ⚠️ **The numbering is therefore NOT contiguous: 6 is an ANNOUNCEMENT, 7 and 8 are
// REQUESTS.** The order of the values says nothing of the family; it is
// the named routing of `client/src/fichiers/protocole.ts` that says it, and it
// alone.

/// Cause of a failure sent back by the browser.
///
/// ⚠️ The wire representation is **kebab-case**, and the variants with
/// two words or more are the ones that break silently: this repository let
/// a `battement-recu` variant through green on fifty tests because
/// nothing pinned its bytes. `un_code_d_echec_a_une_forme_epinglee_sur_le_fil`
/// pins **all eleven**, literally — the ten of F2, plus
/// `RepertoireNonVide` which F3 adds, **with three words**, hence of the exact family
/// that breaks silently.
///
/// ⚠️ **The F2 plan announced NINE and called `CasseAmbigue` "the
/// ninth variant".** Its task 1 already adds two to the seven of F1
/// (`DisquePlein`, `DejaPresent`), which makes nine; its task 9 adds a
/// third. **`CasseAmbigue` is therefore the TENTH**, and the plan's count is
/// corrected here rather than copied.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CodeEchec {
    Introuvable,
    CheminIntrouvable,
    AccesRefuse,
    ProtegeEnEcriture,
    NonSupporte,
    TropGrand,
    Interne,
    /// The disk of the local machine is full — `QuotaExceededError` on the
    /// browser side (F2).
    ///
    /// 🔴 **THIS CODE REACHES NO WINDOWS APPLICATION, and saying so here is the
    /// only way a successor will not believe otherwise.** It is born from a
    /// write push, that is AFTER the application has closed its
    /// handle and believed it had saved: there is no longer any ProjFS command to
    /// complete. This code serves the LOG and the pending-writes counter of the
    /// shell page, never an `HRESULT` returned to anyone at all.
    DisquePlein,
    /// An entry with the same name already exists (F2).
    DejaPresent,
    /// 🔴 **The local machine holds a namesake that differs ONLY by case, and
    /// the writer REFUSED to write.**
    ///
    /// The case that reaches it: a file created in the VM with a
    /// case different from an existing local entry. The file system of the
    /// local machine is case-insensitive on Windows and on macOS;
    /// `getFileHandle('CASSE.TXT', { create: true })` would therefore open
    /// `Casse.txt` there and **overwrite it**. Refusing loudly is the only
    /// arbitration available between "refusing wrongly" and "overwriting the wrong
    /// file" — see `client/src/fichiers/ecriture.ts`.
    CasseAmbigue,
    /// 🔴 **The local machine refuses to delete a NON-EMPTY directory, and that
    /// means the MIRROR HAS DRIFTED.**
    ///
    /// F3 calls `removeEntry(nom)` **without `recursive`**: a gesture in the VM
    /// must not trigger a recursive destruction on the disk of the local
    /// machine, on the strength of a mirror that no proof says is up to date. Windows never
    /// deletes a non-empty directory in one gesture either —
    /// Explorer and `rd /s` erase the children one by one, and each child
    /// produces its own notification.
    ///
    /// 🔵 **This code is therefore DIAGNOSTIC, and that is what sets it apart from the
    /// three of F2**: receiving it means the local machine holds
    /// entries the VM does not know. It does reach a Windows
    /// application — the deletion is born from a POST notification, but the
    /// `PRE_DELETE` path precedes it, and it is `ERROR_DIR_NOT_EMPTY` that a
    /// successor would read there.
    RepertoireNonVide,
}

/// A decoded frame. Borrows the input bytes: neither the header nor the payload
/// is copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trame<'a> {
    pub version: u8,
    pub type_message: u8,
    pub correlation: u32,
    /// UTF-8 JSON. **Not validated here**: the decoder returns the bytes, the caller
    /// parses them. A malformed header is an error of the caller, not of
    /// the frame.
    pub entete: &'a [u8],
    /// Raw bytes, **never encoded**.
    pub charge: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErreurTrame {
    #[error("trame tronquée : {recu} octets reçus, {attendu} attendus")]
    TropCourte { recu: usize, attendu: usize },
    #[error("version de fichiers non supportée : {0}")]
    VersionNonSupportee(u8),
    #[error("en-tête de {longueur} octets annoncé, {disponible} disponibles")]
    EnteteDeborde { longueur: u64, disponible: usize },
    #[error("charge de {recu} octets, maximum {max}")]
    ChargeTropGrande { recu: usize, max: usize },
}

/// Encodes a frame. The header is UTF-8 JSON, the payload raw bytes.
pub fn encoder(type_message: u8, correlation: u32, entete: &str, charge: &[u8]) -> Vec<u8> {
    let entete = entete.as_bytes();
    let mut out = Vec::with_capacity(TAILLE_ENTETE_FIXE + entete.len() + charge.len());
    out.push(FICHIERS_VERSION);
    out.push(type_message);
    out.extend_from_slice(&correlation.to_le_bytes());
    out.extend_from_slice(&(entete.len() as u32).to_le_bytes());
    out.extend_from_slice(entete);
    out.extend_from_slice(charge);
    out
}

/// Decodes a frame, or says precisely why it is refused.
pub fn decoder(octets: &[u8]) -> Result<Trame<'_>, ErreurTrame> {
    if octets.len() < TAILLE_ENTETE_FIXE {
        return Err(ErreurTrame::TropCourte {
            recu: octets.len(),
            attendu: TAILLE_ENTETE_FIXE,
        });
    }
    let version = octets[0];
    if version != FICHIERS_VERSION {
        return Err(ErreurTrame::VersionNonSupportee(version));
    }
    let correlation = u32::from_le_bytes([octets[2], octets[3], octets[4], octets[5]]);
    let longueur_entete = u32::from_le_bytes([octets[6], octets[7], octets[8], octets[9]]);
    let reste = &octets[TAILLE_ENTETE_FIXE..];
    // Comparison in `u64`: `longueur_entete as usize` would overflow on a
    // 32-bit target, and would make this bound inoperative exactly where it is most
    // needed.
    if u64::from(longueur_entete) > reste.len() as u64 {
        return Err(ErreurTrame::EnteteDeborde {
            longueur: u64::from(longueur_entete),
            disponible: reste.len(),
        });
    }
    let (entete, charge) = reste.split_at(longueur_entete as usize);
    if charge.len() > TAILLE_TRAME_MAX {
        return Err(ErreurTrame::ChargeTropGrande {
            recu: charge.len(),
            max: TAILLE_TRAME_MAX,
        });
    }
    Ok(Trame {
        version,
        type_message: octets[1],
        correlation,
        entete,
        charge,
    })
}

pub mod entetes;

#[cfg(test)]
mod tests;
