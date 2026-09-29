//! Messages and framing of the channel between the sensor and a child.
//!
//! **No `#[cfg(windows)]`**: it is pure serialisation, and it is
//! precisely the kind of contract that must be tested on the host — a drifting
//! field name would otherwise only show in a real session on the VM.
//! Same reason and same set-up as `superviseur/protocole.rs`.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::h264::AccessUnit;

/// Name of the named pipe on which the sensor accepts its children.
pub const NOM_TUBE: &str = r"\\.\pipe\agent-capteur";

/// Size bound of a frame, checked BEFORE any allocation.
///
/// An access unit at 8 Mb/s weighs a few tens of kilobytes; a high-resolution
/// start-up key frame stays well below a megabyte. 8 MiB
/// leave three orders of magnitude of margin while making it impossible
/// for a corrupted length to reserve gigabytes.
pub const MAX_SIZE: usize = 8 * 1024 * 1024;

pub const ETIQUETTE_JSON: u8 = 1;
pub const ETIQUETTE_IMAGE: u8 = 2;

/// Binary header of a frame: 8 bytes of `pts_90k`, 1 key-frame byte.
const EN_TETE_IMAGE: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VersCapteur {
    /// A child's first message: it describes itself. There is
    /// **no direct supervisor→sensor channel**; what comes from the
    /// supervisor (for example `size`, below) goes through the child,
    /// which repeats it here.
    Attache {
        session: String,
        hwnd: u64,
        sortie: String,
        fps: u32,
        debit: u32,
        /// The KEPT size the supervisor set on this window
        /// (`TAILLE_FENETRE`), not the output size — which may be policy: allow-fr (env var name)
        /// much larger on a polluted registry. `(u32::MAX, u32::MAX)`
        /// when the child does not know it (single-window path, without a
        /// supervisor): `retained_size` then brings it back to the output
        /// size, which reproduces the behaviour before this sub-block.
        /// Not consumed before task 8 — see `Fenetre::ouvrir`.
        #[serde(rename = "taille")]
        size: (u32, u32),
        /// `QueryPerformanceCounter` read by the child at the very moment it creates
        /// its `clock_origin`. An `Instant` makes no sense in another
        /// process; QPC, on the other hand, is common to the whole machine. Without this
        /// rebasing, the video of the child carrying the sound would be shifted by
        /// the gap between the two origins.
        origine_qpc: i64,
    },
    /// First and **only** frame of the media connection: it pairs this
    /// second pipe with the session already attached on the command connection.
    /// After it, the child never writes on this connection again — that is
    /// what guarantees no read and write are concurrent there.
    Identite {
        session: String,
    },
    Redimensionner {
        largeur: u32,
        hauteur: u32,
    },
    #[serde(rename = "TailleEncodage")]
    EncodeSize {
        largeur: u32,
        hauteur: u32,
    },
    Debit {
        bps: u32,
    },
    ImageCle,
    /// Visibility announced by the client, relayed by the child.
    ///
    /// **Is not answered with `Fait`**: the sensor arbitrates globally, and the
    /// decision may concern ANOTHER window than the one that reported.
    /// The effect comes back through `DepuisCapteur::Sommeil`, pushed on the media
    /// connection of each window concerned.
    Visibilite {
        visible: bool,
        focalisee: bool,
    },
    /// This window's audio capture has stopped producing sound, and
    /// the child has **exhausted its means of restoring it**.
    ///
    /// ❌ **This field said "dead FOR GOOD, after
    /// `LECTURES_ECHOUEES_MAX` consecutive read errors", and BOTH
    /// halves have been false since sub-block D10** — found by the cross-cutting
    /// review, the task that added `AudioVivant` just below
    /// having not re-read the variant above.
    ///
    /// - **Not for good**: `Session::reconstruire_ou_signaler`
    ///   (`transport/piste_audio.rs`) rebuilds the capture, and
    ///   `VersCapteur::AudioVivant` exists precisely to prove the
    ///   resumption through a real packet.
    /// - **Not after `LECTURES_ECHOUEES_MAX`**: those ten errors set
    ///   `capture_morte`, nothing more. This message goes out **only as a
    ///   FALLBACK** — when `crate::audio::RECONSTRUCTIONS_MAX` rebuild
    ///   attempts have been exhausted, or there is no
    ///   rebuilder.
    ///
    /// **No payload**: the session is that of the channel, as for
    /// all commands — `capteur/fenetre/commandes.rs` draws it from its
    /// context.
    ///
    /// **Is not answered with `Fait` in the sense of the effect**: the sensor
    /// re-arbitrates globally, and the decision may concern ANOTHER window
    /// of the same PID group. The effect comes back through `DepuisCapteur::Audio`,
    /// pushed on the media connection. Exactly the same pattern as `Visibilite`.
    AudioMort,
    /// This window's audio capture has just brought the PROOF that it
    /// has restarted: a real packet was produced, not merely a
    /// rebuild that returned `Ok` (sub-block D10, closes D9's hand-over 6).
    ///
    /// **No payload**, like `AudioMort`. **Is not answered with
    /// `Fait` in the sense of the effect either**: it only resets
    /// the re-arm counter of THIS session
    /// (`capteur::sommeil::signaler_audio_vivant`). Unlike
    /// `AudioMort`, it re-arbitrates nothing: the proof only concerns the
    /// session bringing it, never ANOTHER window of the same PID group.
    AudioVivant,
    /// The user pasted into THEIR window: write this text into the
    /// VM's clipboard (sub-block P2 of the clipboard work stream).
    ///
    /// 🔴 **It IS answered with `Fait`, and it is the only point where this family
    /// of commands does so — the difference is not stylistic.**
    /// `Visibilite`, `AudioMort` and `AudioVivant` are not answered because
    /// their effect is a global ARBITRATION, which may concern another
    /// window and comes back through the media connection. Here the caller needs to
    /// know that the write **actually happened BEFORE** injecting
    /// `Ctrl+V`: that is D6's whole ordering, and nothing else carries it. On
    /// `DepuisCapteur::Error`, the child does not inject — the `V` key is
    /// **lost, not postponed**, because a `Ctrl+V` on an unchanged
    /// clipboard would paste the PREVIOUS content, without anything saying so.
    ///
    /// The text is already **normalised, bounded and denormalised** (`\r\n`) by
    /// the child when it arrives here: the owner decides nothing about its
    /// content, it writes it. This pipe's bound (`MAX_SIZE`, 8 MiB) is
    /// therefore **not** the constraining factor — `PRESSE_PAPIER_MAX` (64 KiB)
    /// bites a hundred and twenty-eight times earlier —, and a test checks it rather than
    /// assuming it.
    #[serde(rename = "PressePapierEcrire")]
    ClipboardWrite {
        texte: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DepuisCapteur {
    Attachee {
        largeur: u32,
        hauteur: u32,
    },
    Refus {
        motif: String,
    },
    #[serde(rename = "Taille")]
    Size {
        largeur: u32,
        hauteur: u32,
    },
    Fait,
    #[serde(rename = "Erreur")]
    Error {
        motif: String,
    },
    /// Emitted **on change only**, never periodically: it feeds
    /// the cache read by `is_alive`, `is_exhausted` and `dimensions`, which
    /// are queried at every round of the transport loop.
    Etat {
        vivante: bool,
        epuisee: bool,
        largeur: u32,
        hauteur: u32,
    },
    /// Pushed, unsolicited, when a window changes sleep state.
    ///
    /// Distinct from `Etat` on purpose: `Etat` feeds a cache read at every round
    /// of the transport loop (`is_alive`, `is_exhausted`, `dimensions`),
    /// and mixing sleep into it would route a one-off announcement through a
    /// path designed for a permanent state.
    Sommeil {
        endormie: bool,
        raison: String,
    },
    /// Share of the session's bitrate budget granted to this window, pushed
    /// unsolicited when it CHANGES.
    ///
    /// Distinct from `Etat` for the same reason as `Sommeil`: `Etat` feeds
    /// a cache read at every round of the transport loop, and mixing a
    /// one-off announcement into it would go through a path designed for a permanent state.
    ///
    /// The child applies it in TWO places (`transport/part.rs`), and it is the
    /// FIRST that acts: `Controleur::changer_plafond` bounds what the encoder
    /// produces, hence moves the ladder down one rung, hence reduces the
    /// RESOLUTION — the only lever D6's acceptance run measured as effective.
    /// `rtc.bwe().set_desired_bitrate` additionally stops upward probing,
    /// excessive in principle with N windows.
    ///
    /// ⚠️ **The second is not "the real cause of congestion with N
    /// windows", and the premise saying so was REFUTED by the branch
    /// itself**: the bridge carries ≥ 1.44 Gb/s, `packetsLost` is 0 on all eleven
    /// runs, there was never any link congestion. What saturates is
    /// the browser's decoder.
    ///
    /// **A SLEEPING window's share goes only to the second** — see
    /// `capteur::repartiteur::PART_DORMANTE_BPS` and `Session::appliquer_part`.
    Part {
        bps: u32,
    },
    /// Order to carry the sound, or to go silent. Pushed unsolicited, **on
    /// change only**.
    ///
    /// Distinct from `Etat` for the same reason as `Sommeil` and `Part`: `Etat`
    /// feeds a cache read at every round of the transport loop, and mixing
    /// a one-off announcement into it would go through a path designed for a permanent
    /// state.
    ///
    /// **The sensor captures NO sound.** It only arbitrates: it knows which
    /// windows share a process (it has their `hwnd`) and who has the focus,
    /// which the child does not know. The capture itself lives in the child — *process
    /// loopback* has none of the properties that forced the pooling of
    /// video in D4.
    Audio {
        actif: bool,
    },
    /// The Windows window went fullscreen, or left it. Pushed
    /// unsolicited, **on change only**.
    ///
    /// Distinct from `Etat` for the same reason as `Sommeil`, `Part` and `Audio`:
    /// `Etat` feeds a cache read at every round of the transport loop, and
    /// mixing a one-off announcement into it would go through a path designed for a
    /// permanent state.
    PleinEcran {
        actif: bool,
    },
    /// The VM's clipboard changed. Pushed unsolicited, **on
    /// change only**.
    ///
    /// Distinct from `Etat` for the same reason as `Sommeil`, `Part`, `Audio` and
    /// `PleinEcran`: `Etat` feeds a cache read at every round of the
    /// transport loop, and mixing a one-off announcement into it would go through a path
    /// designed for a permanent state.
    ///
    /// **The sensor owns the clipboard, and it alone.** The reason
    /// is not writing — P1 writes nothing — but LISTENING and the
    /// anti-echo guard: N children observing a resource GLOBAL to the Windows
    /// session would have N guards that do not see each other, and the oscillation would be
    /// **inter-process, hence irreparable locally**.
    ///
    /// `texte` is `None` on a size refusal: the content exceeded
    /// `presse_papier::PRESSE_PAPIER_MAX` and it is **refused, never truncated**.
    /// `octets` then carries the refused size, after normalising line
    /// endings.
    ///
    /// ⚠️ **It is NOT this channel that constrains the size**, and writing it here
    /// keeps a successor from believing the opposite: `MAX_SIZE` is **8 MiB**
    /// (see the head of the file) while `PRESSE_PAPIER_MAX` is **64 KiB** —
    /// two orders of magnitude apart. The bound is a product decision
    /// (D4), not a transport limit.
    ///
    /// ✅ **THIS VARIANT IS CONNECTED IN `capteur/pont_media.rs` since
    /// task 9 of sub-block P1** (`pont_media.rs:87`), with its test, and the
    /// RED was played before the arm: without it, the very first announcement
    /// returned `RecvError` at the end of the queue. The warning that lived here
    /// said "not yet connected" and "the check must return ONE line":
    /// both became false, and leaving them would have been precisely the
    /// stale-statement defect this repository's cross-cutting review hunts for.
    ///
    /// What the check returns TODAY, read from the command:
    /// `grep -n 'DepuisCapteur::PressePapier' agent/src/capteur/pont_media.rs`
    /// returns **four** lines — one for the arm, three for the test that
    /// guards it. **What matters is that it does not return ZERO**: the missing arm
    /// is reported by no compile error, it makes the
    /// message fall into the `Ok(autre)` catch-all, which **kills the `lire_le_media`
    /// thread without any visible failure** — the session falls into its resumption
    /// window, and nothing says why. The repository paid for this defect **four
    /// times** before this one — `Sommeil` (D5), `Part` (D6), `Audio` (D7),
    /// `PleinEcran` (D8) —, and any NEW variant of this enumeration
    /// pushed on the media connection will have to go the same way.
    PressePapier {
        texte: Option<String>,
        octets: u32,
    },
    /// The Windows window's accent colour — the dominant tint of its
    /// icon. Pushed unsolicited, **on change only**, and **its
    /// FIRST reading included** (sub-block A1).
    ///
    /// Distinct from `Etat` for the same reason as `Sommeil`, `Part`, `Audio`,
    /// `PleinEcran` and `PressePapier`: `Etat` feeds a cache read at every
    /// round of the transport loop, and mixing a one-off announcement into it
    /// would go through a path designed for a permanent state.
    ///
    /// `couleur` is **`#rrggbb`, six lowercase hexadecimal digits, and nothing
    /// else**. ⚠️ **The format is a constraint of the DESIGN SYSTEM, not of the
    /// protocol**: `client/src/design/contraste.ts::luminanceRelative`
    /// accepts only `#rgb`, `#rgba`, `#rrggbb` and `#rrggbbaa`, and **THROWS** on
    /// everything else. The client defends itself (`client/src/accent.ts` checks the
    /// shape BEFORE calling `rapportDeContraste`), but the agent has no
    /// reason to send it a shape it will have to throw away.
    ///
    /// ⚠️ **The field carries NEITHER the `hwnd`, NOR the PID, NOR the window's
    /// title**, and the second reason is a paid lesson: the session is
    /// already identified by the channel the message arrives on, and **P2
    /// found the clipboard IN CLEAR in `agent.log`**, at an earlier
    /// logging site, harmless as long as no variant
    /// carried private content. The remedy applies **TO THE TYPE, not to the site**:
    /// a window title or an executable path here would replay that defect
    /// identically.
    ///
    /// ⚠️ **THE READ DOES NOT LIVE ON THE WHEEL ROUND**, contrary to what
    /// the specification's decision D9 prescribed: the wheel round **does not have
    /// the `hwnd`** — none of the fifteen fields of `Etat`
    /// (`capteur/sommeil/registre.rs`) carries it, and `inscrire(session, pid)`
    /// does not take it. The clipboard lives there because it is **global to the
    /// window station**; the accent is **per window**, and so it lives on the
    /// window thread, with D8's fullscreen whose pattern it reuses.
    ///
    /// 🔴 **SIXTH time this passage point has to be connected in
    /// `capteur/pont_media.rs`**, after `Sommeil` (D5), `Part` (D6), `Audio`
    /// (D7), `PleinEcran` (D8) and `PressePapier` (P1). The missing arm is
    /// reported by NO compile error: it makes the message fall
    /// into the `Ok(autre)` catch-all, **which kills the `lire_le_media` thread without
    /// any visible failure**. The check, read from the command:
    /// `grep -n 'DepuisCapteur::Accent' agent/src/capteur/pont_media.rs` must
    /// return **four** lines — one for the arm, three for the test that
    /// guards it — and **above all not ZERO**.
    Accent {
        couleur: String,
    },
}

#[derive(Debug)]
pub enum Trame {
    Json(Vec<u8>),
    Image(AccessUnit),
}

fn write_frame<W: Write>(sortie: &mut W, etiquette: u8, corps: &[u8]) -> io::Result<()> {
    let length = corps.len() + 1;
    if length > MAX_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frame of {length} bytes above the bound {MAX_SIZE}"),
        ));
    }
    sortie.write_all(&(length as u32).to_le_bytes())?;
    sortie.write_all(&[etiquette])?;
    sortie.write_all(corps)
}

pub fn write_json<W: Write, T: Serialize>(sortie: &mut W, message: &T) -> io::Result<()> {
    let corps = serde_json::to_vec(message).map_err(io::Error::other)?;
    write_frame(sortie, ETIQUETTE_JSON, &corps)
}

pub fn write_image<W: Write>(sortie: &mut W, unite: &AccessUnit) -> io::Result<()> {
    let mut corps = Vec::with_capacity(EN_TETE_IMAGE + unite.data.len());
    corps.extend_from_slice(&unite.pts_90k.to_le_bytes());
    corps.push(u8::from(unite.is_keyframe));
    corps.extend_from_slice(&unite.data);
    write_frame(sortie, ETIQUETTE_IMAGE, &corps)
}

pub fn lire_trame<R: Read>(entree: &mut R) -> io::Result<Trame> {
    let mut length = [0u8; 4];
    entree.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("absurd frame length: {length}"),
        ));
    }
    let mut corps = vec![0u8; length];
    entree.read_exact(&mut corps)?;
    let etiquette = corps[0];
    let corps = &corps[1..];
    match etiquette {
        ETIQUETTE_JSON => Ok(Trame::Json(corps.to_vec())),
        ETIQUETTE_IMAGE => {
            if corps.len() < EN_TETE_IMAGE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "image frame without a complete header",
                ));
            }
            let pts_90k = u64::from_le_bytes(corps[..8].try_into().expect("8 octets"));
            Ok(Trame::Image(AccessUnit {
                pts_90k,
                is_keyframe: corps[8] != 0,
                data: corps[EN_TETE_IMAGE..].to_vec(),
            }))
        }
        // REFUSED and not ignored: a misaligned stream must kill the channel
        // rather than make the read drift over arbitrary bytes.
        autre => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown frame tag: {autre}"),
        )),
    }
}

// This module's tests have lived apart since sub-block P2 of the clipboard
// work stream: the file was at 464 lines for a cap of 500, and the
// documentation of a `VersCapteur` variant is copious there — that
// of `AudioMort` is twenty-seven lines on its own. The extraction precedes
// the addition of `ClipboardWrite`, as the repository's rule requires.
//
// ⚠️ This use of `#[path]` is OUTSIDE the scope of the "Child module
// convention" of `docs/claude/module-conventions.md`: same Rust mechanism, different reason — the
// 500-line rule —, exactly like `superviseur/table.rs`. No hoisting.
#[cfg(test)]
#[path = "protocole/tests.rs"]
mod tests;
