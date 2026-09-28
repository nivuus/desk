//! The registry's tables: the `Etat` struct and nothing else.
//!
//! **Extracted from `registre.rs` in fix round 3 (25 August 2026),
//! BEFORE writing to it**: the file was at 470 lines for a project
//! cap of 500, and this round brings the trace pacing and two
//! wording fixes to it. Extract, never compress — and in a DEDICATED
//! task, before the one that adds. It is the third extraction of this
//! series (`file/tests.rs` in round 1, `parts/tests.rs` in round 2), and the
//! second of `registre.rs` after `registre/tour_de_roue.rs`.
//!
//! **What guided the cut**: `Etat` is an AGGREGATE OF TABLES, almost
//! entirely made of documentation — each of its fields carries the reason
//! for a table and the defects it cost. `registre.rs` keeps what
//! ACTS on them: `etat()`, `distribuer`, `oublier`, `inscrire`,
//! `retirer`.
//!
//! ⚠️ **The module is called `tables` and not `etat`, on purpose**:
//! `registre.rs` already carries a function `etat()`, and while Rust easily
//! tells a module from a function, a human reader stumbles. The name says what
//! the file contains.
//!
//! ⚠️ **Transposition, not rewrite**: the block is moved identically,
//! no field, no type, no comment changed. Only the VISIBILITY
//! is rewritten — `pub(super)` meant "visible in `sommeil`" from
//! `registre`; from `registre::tables` it would take two steps, and the scope
//! is therefore spelled out (`pub(in crate::capteur::sommeil)`)
//! rather than widened to `pub(crate)`, which would be BROADER than the original.

use std::collections::HashMap;
use std::time::Instant;

use crate::capteur::sommeil::file::EmetteurSession;
use crate::capteur::vivier::Vivier;

pub(in crate::capteur::sommeil) struct Etat {
    pub(in crate::capteur::sommeil) vivier: Vivier,
    /// The SENDING end of each session's channel.
    ///
    /// 🔴 **`EmetteurSession` and not `Sender<Message>` since 25 August
    /// 2026**: the `mpsc` channel was UNBOUNDED, and a window that stops
    /// reading made its queue grow without end. The contract that matters here is
    /// UNCHANGED — `envoyer` returns `Err` when the receiver has dropped, and it is
    /// on this value that `distribuer` and `parts::distribuer_les_parts`
    /// purge a dead session. See `file.rs` for the bound, the
    /// coalescing, the refusal count and its trace.
    pub(in crate::capteur::sommeil) canaux: HashMap<String, EmetteurSession>,
    /// The session the client declares focused, if it still exists.
    ///
    /// Held here and not in `Vivier`: the pool arbitrates encoder
    /// places, the distributor bitrate shares. The client emits `blur`
    /// as well as `focus` (`client/src/visibilite.ts`), so this field does
    /// empty when the window loses the focus.
    pub(in crate::capteur::sommeil) focalisee: Option<String>,
    /// Last share sent to each session. **The only rampart against a
    /// flood**: the wheel round re-arbitrates every 250 ms, and without
    /// this memory eight windows would receive 32 messages per second for life.
    pub(in crate::capteur::sommeil) dernieres_parts: HashMap<String, u32>,
    /// PID of the process owning each window. **Here and not in a
    /// second registry**: the sensor has only one truth to hold, and two
    /// tables to synchronise would make two of it.
    pub(in crate::capteur::sommeil) pids: HashMap<String, u32>,
    /// Arrival rank of each session, and rank of the last focus received. Two
    /// counters drawn from the same `horloge`, strictly increasing.
    pub(in crate::capteur::sommeil) arrivees: HashMap<String, u64>,
    pub(in crate::capteur::sommeil) derniers_focus: HashMap<String, u64>,
    /// Monotonic counter serving as a rank for the two tables above. An
    /// `Instant` would not do: a total, stable and comparable order is needed,
    /// not a duration.
    pub(in crate::capteur::sommeil) horloge: u64,
    /// Last audio order sent to each session. **The rampart against
    /// the flood**, exactly like `dernieres_parts`: the wheel round
    /// re-arbitrates every 250 ms.
    pub(in crate::capteur::sommeil) derniers_audio: HashMap<String, bool>,
    /// Instant after which a session whose audio capture has died
    /// becomes eligible to carry sound again. Absent = fit.
    ///
    /// **Here and not in `capteur::audio`**: this module has the clock, the other
    /// is pure and stays so.
    pub(in crate::capteur::sommeil) inaptes: HashMap<String, Instant>,
    /// The LAST clipboard announcement distributed, whatever it is.
    ///
    /// 🔴 **It is the AGENT half of P1's hand-over no. 3** — "a window attached
    /// after a copy never receives that content". Without this memory, a
    /// window that attaches waits for the NEXT copy, and the `Sondeur` says so
    /// itself: its first round takes the current state as reference and
    /// announces nothing.
    ///
    /// ⚠️ **It ALSO memorises `Annonce::Refus`, and it must**: a
    /// window attaching after a refusal must see the banner, otherwise
    /// it would wait for a content that will never arrive.
    ///
    /// 🔴 **NO PURGE ON RE-REGISTRATION, and the symmetry with
    /// `dernieres_parts` / `derniers_audio` is MISLEADING** (D-P3-3). Those
    /// two are purged because `distribuer_les_parts` and
    /// `distribuer_l_audio` FILTER on them: without a purge, a share identical
    /// to the one sent on the OLD channel would be judged already delivered on the
    /// NEW channel, which never received it. The clipboard emission at
    /// registration, on the other hand, is UNCONDITIONAL: there is nothing to filter,
    /// hence nothing to purge — and purging here would remove the memory at the precise
    /// moment we want to use it, the remedy then remedying nothing.
    pub(in crate::capteur::sommeil) dernier_presse_papier: Option<crate::presse_papier::Annonce>,
    /// The (sequence number, text) pair of OUR OWN clipboard write,
    /// waiting to be consumed by the wheel round to
    /// arm D5's guards no. 1 and no. 2 (sub-block P2).
    ///
    /// 🔴 **It lives HERE, under the lock, and not next to the `Sondeur`, because
    /// the two do not run on the same thread.** The `Sondeur` is local to the
    /// wheel-round thread; the write, for its part, comes from the WINDOW thread serving the
    /// `PressePapierEcrire` command. There is no way to arm the guard
    /// from there without a race — other than this registry, which is already the locked
    /// meeting point of the two.
    ///
    /// ⚠️ **This MOVES the race, it does not remove it, and it must be
    /// said**: up to `PERIODE_REARBITRAGE` (250 ms) may elapse between
    /// our `SetClipboardData` and the consumption below. If ANOTHER
    /// copy happens in that interval, setting `reference` on *our* `seq`
    /// does not mask it — the counter will have moved again, and that copy will be
    /// announced. **That is the intended behaviour**, exact in D5's sense, and a
    /// test checks it rather than assuming it.
    ///
    /// Last one overwrites: two writes in less than one wheel round
    /// leave only the second, which is the one the clipboard actually
    /// carries.
    pub(in crate::capteur::sommeil) notre_ecriture: Option<(u32, String)>,
    /// Number of consecutive re-arms already granted to each session.
    ///
    /// ❌ **"Reset as soon as it carries the sound without dying" describes the
    /// semantics that sub-block D10 precisely REMOVED** (found by the
    /// cross-cutting review: this file was not touched by the branch, hence
    /// the residue). The reset on the arbitration **decision** left
    /// `sommeil/porteurs.rs`; `signaler_audio_vivant` (`capteur/sommeil.rs`)
    /// is now its only point, and it only runs on a **PROOF**
    /// — a real packet, reported by `VersCapteur::AudioVivant`. It is
    /// D9's hand-over 6, and that was its purpose: the counter counted
    /// non-consecutive failures.
    ///
    /// **Accepted consequence, to be known**: a session may "carry the
    /// sound without dying" and never see its counter drop back, if no
    /// packet ever arrives. It is intended — it is exactly the state that
    /// D10's acceptance run ② found in production (a rebuilt source that
    /// was born silent) and that the counter must denounce, not absolve.
    pub(in crate::capteur::sommeil) rearmements: HashMap<String, u32>,
    /// Generation of the last known registration of each session (D9,
    /// F5 of D7 — race on `retirer` when a name re-registers). Set by
    /// `inscrire`, read and erased by `retirer` via `retirer_est_perime`.
    ///
    /// **Deliberately absent from `oublier`**: it is `retirer` alone that
    /// purges it, and only when it is not stale. Purging it from
    /// `oublier` would make it disappear on the broken-channel paths too,
    /// which have no generation to compare and must therefore never
    /// erase it in place of an already registered re-attachment. **Accepted
    /// consequence**: on those paths, a name's entry survives its session
    /// for the rest of the sensor process's life — without functional
    /// effect (it only sleeps in a `HashMap`), and session names
    /// never being reused (`Table::compteur`,
    /// `superviseur/table.rs`), this table only grows with the
    /// number of windows ever opened over the sensor's lifetime.
    pub(in crate::capteur::sommeil) generations: HashMap<String, u64>,
    /// Counter that stamps the generation of each `inscrire` — **DISTINCT
    /// from `horloge` above, deliberately**.
    ///
    /// `horloge` has an invariant that `generations` necessarily violates:
    /// `arrivees`/`derniers_focus` are only set at the FIRST
    /// registration of a name (`if !garde.arrivees.contains_key(session)`) and
    /// then stay STABLE as long as the name lives — that is what gives them
    /// their meaning of ARRIVAL rank. `generations` has the opposite requirement:
    /// **every** call to `inscrire`, including a re-attachment under the same
    /// name, must receive a NEW value — it is the only way to
    /// distinguish the live instance from the previous one. Making `horloge` carry this
    /// requirement would require stamping it unconditionally
    /// in `inscrire`, hence decoupling its progression from the guard that
    /// protects `arrivees`/`derniers_focus` — a coupling that would make this
    /// requirement depend on logic written for another need, and
    /// silently breakable by a future evolution of that guard.
    /// Two counters, two invariants, no risk of confusion.
    pub(in crate::capteur::sommeil) prochaine_generation: u64,
}
