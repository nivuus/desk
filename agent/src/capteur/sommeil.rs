//! The global sleep registry: a shared `Vivier`, one order channel per
//! window, and the wheel round that unblocks the hysteresis.
//!
//! **It decides nothing.** All the logic is in `vivier.rs`, which is pure and
//! tested; this module only plugs it into channels.
//!
//! **Why a process-global state rather than an object passed from hand to
//! hand.** Each sensor window lives on its own thread, created by
//! `serveur::ouvrir_les_commandes`, and arbitration is by nature cross-cutting:
//! one window's signal can put its neighbour to sleep. The registry of media
//! connection waits (`serveur.rs`) already uses exactly this pattern, for
//! the same reason. It is also what lets this sub-block **not
//! touch `serveur.rs`**, whose size margin is 10 lines.

// `parts` carries the computation and distribution of bitrate shares. Extracted for
// the same reason as `fenetre::transitions`: this file crossed the
// project's 500-line cap when the remedy for the broken channel
// detected through that path was added (see `parts::distribuer_les_parts`). It is not
// called `repartiteur`: that name is already taken by the module carrying
// the pure RULE; this one only carries its BRANCH on this registry.
// `file` carries the PURE coalescing RULE of a session's channel: no
// lock, no `cfg`, no Windows API — that is what makes it testable
// by `cargo test --workspace` on the Linux host, whereas this whole file
// is not. The lock and the wake-up stay with this caller (later
// task); this module ONLY decides whether a drop stacks, coalesces, or
// is refused.
pub(crate) mod file;
mod parts;
mod porteurs;
// `presse_papier` carries the DISTRIBUTION of the VM's clipboard, extracted
// at the same place as `parts` and `porteurs` — but NOT for the same reason, and
// it must be said: `parts` was extracted from a file that had CROSSED 500
// lines, whereas this one is 352 and keeps 148 of margin. What
// justifies it is that the inlined module, its three tests included, would have taken
// `sommeil.rs` beyond the cap. ⚠️ The initial wording said "this
// file is close to its cap": it was false at the time of writing (328
// lines then), and fixed by the cross-cutting review of 20 August 2026 — **the
// gesture was right, its written reason was not**.
// It is not named like the root module
// `crate::presse_papier` by confusion — that one carries the pure RULE, this one
// only its branch on this registry.
mod presse_papier;

// The registry itself — `Etat`, `etat`, the wheel round, `distribuer`,
// `oublier`, `inscrire`, `retirer` — extracted to stay under the project's
// 500-line cap (review of task 10, D9): this file had
// landed at exactly 500 with this block inline. See the header of
// `registre.rs`. The four re-exports below make the extraction
// invisible to `parts.rs`/`porteurs.rs`/`tests.rs`, which keep writing
// `super::{distribuer, oublier, Etat, Message}` without knowing it.
mod registre;
use registre::{distribuer, etat, oublier, Etat};
pub use registre::{inscrire, retirer};

use std::collections::HashMap;
// `ReceveurSession`, `Mutex` and `MutexGuard`: no longer used by the
// PRODUCTION code of this file since the extraction above — only
// `sommeil::tests` still uses them (`premier_ordre`, `VERROU_TESTS`), via
// `use super::*`. Gating on `cfg(test)` avoids an `unused_imports` outside
// the test build, without touching `tests.rs`.
//
// ⚠️ `Receiver` until 25 August 2026: the registry's channel is no longer an
// unbounded `std::sync::mpsc` (see `file.rs`).
#[cfg(test)]
use file::ReceveurSession;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::capteur::vivier::{Ordre, Raison};

/// Period of the wheel round. Neither a rendering cadence nor a clock: it is the
/// only way for a window blocked under hysteresis to be re-examined, and
/// 250 ms is well below the 2 s of hysteresis while staying negligible.
const PERIODE_REARBITRAGE: Duration = Duration::from_millis(250);

/// Respite before a window whose audio capture has died becomes
/// eligible to carry sound again.
///
/// ❌ **THIS MECHANISM IS INERT FOR THE MAJORITY CASE, and an earlier
/// wording of this comment wrongly claimed the opposite** (found
/// by the VM acceptance run of task 15, sub-block D9, final review). It
/// said: "re-electing the same session builds a NEW *process
/// loopback* activation". **That is false, checked against the code**:
/// `WindowsAudioSource` is built ONLY ONCE, at the child's start-up
/// (`demarrage/audio.rs::brancher`, called only once, without a
/// loop); the capture thread (`windows_audio.rs`), once
/// `capture_morte` is set, executes a DEFINITIVE `return` and never re-reads
/// anything; and re-electing the SAME session only pushes
/// `Audio { actif: true }`, which ends in `AudioSource::set_actif(true)`
/// (`windows_audio.rs::set_actif`) — **which only writes an atomic boolean
/// this dead thread will never read again**. Nothing, anywhere, rebuilds
/// the source.
///
/// ✅ **The PROMOTION branch, for its part, remains valid**: a neighbour of the same
/// PID group has ITS OWN `WindowsAudioSource`, built at ITS OWN
/// start-up, on a capture thread that has never failed — electing it
/// really gives it the sound. It is the respite itself — the RE-ELECTION OF THE
/// SAME SESSION WITHOUT A NEIGHBOUR — that restores nothing.
///
/// **Accepted consequence**: the MAJORITY case — one application, one
/// window, hence no neighbour to promote — remains WITHOUT A REMEDY. The respite
/// silences then un-silences the right session at the REGISTRY level (the
/// sensor stops believing it unfit, sends it `Audio { actif: true }` again),
/// but no sound actually comes out on the child side as long as its capture has not
/// been rebuilt — which nothing does. **This remains a debt, not a
/// partial remedy**: the rebuild path does not exist, and should
/// live at the intersection of three files already named in this comment —
/// `demarrage/audio.rs` (which builds the source today, once),
/// `transport/piste_audio.rs` (which carries it via `appliquer_audio`), and
/// `windows_audio.rs` (which holds the thread and the `capture_morte` witness) —
/// handed over to the next sub-block.
///
/// ✅ **THIS PATH NOW EXISTS, wired end to end (sub-block D10,
/// tasks 11 and 12): the MAJORITY case IS NO LONGER WITHOUT A REMEDY.**
/// `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`) tries
/// FIRST to rebuild the source — exactly at the intersection named
/// above, `demarrage/audio.rs` providing the rebuilder — and
/// only calls `audio_mort` (hence this respite and this promotion) as a FALLBACK:
/// when its own attempt budget (`crate::audio::RECONSTRUCTIONS_MAX`)
/// is exhausted, or there is no rebuilder (`AUDIO=0`, or any
/// session whose initial audio opening failed: there, the behaviour
/// from before D10 — report immediately — is kept exactly).
///
/// ❌ **"Single-window path" was in this list, and it is false:
/// `demarrage/audio.rs::brancher` sets a rebuilder in BOTH
/// modes** (cross-cutting end-of-branch review). Single-window therefore rebuilds
/// too, `RECONSTRUCTIONS_MAX` times, before reporting. ⚠️ **The
/// remedy was INERT there for another reason**: the rebuilt source was
/// re-armed there on `audio_porteuse`, which no sensor order ever
/// set in the absence of a sensor. ✅ **"Handed over and not fixed" is
/// no longer so — D10's hand-over no. 4, FIXED in sub-block D11**: the
/// single-window wiring sets `audio_porteuse` itself (`demarrage/audio.rs`), and
/// acceptance run ① measures the sound back (441 Hz against the sentinel on red) —
/// see `Session::reconstruire_ou_signaler` (`transport/piste_audio.rs`). What
/// THIS mechanism (the respite and the
/// promotion) keeps doing, unchanged: giving its chance to a neighbour of the
/// same PID group, and preventing a definitively dead device from
/// making the cycle run forever. And the proof that the rebuild
/// actually produced sound — not merely managed to open — closes the
/// re-arm cycle: see `signaler_audio_vivant` below, and D9's hand-over 6
/// that it closes (`capteur/sommeil/porteurs.rs`).
///
/// ⚠️ **Clarification made in REVIEW of task 12: the role of
/// re-election does not stop at "giving its chance to a neighbour".** The
/// attempt budget (`crate::audio::RECONSTRUCTIONS_MAX`) is set
/// only ONCE when the `Session` is built, and without replenishment
/// the cycle described above could only run once PER
/// SESSION: after the first `AudioMort`, the `audio_mort_signale` latch (which
/// only drops on a re-attachment) prevented any new attempt,
/// whatever the session's remaining lifetime. It is
/// re-election ITSELF — the transition to `Audio { actif: true }`,
/// `Session::appliquer_audio`, `transport/piste_audio.rs` — that
/// replenishes this budget and lifts this latch. Without this second role,
/// `REARMEMENTS_MAX` just below would only ever have counted a single failure
/// per session, never several CONSECUTIVE failures — the opposite of its
/// intent.
///
/// ⚠️ **NOT CALIBRATED.** No measurement underpins it: it joins `BPP_MIN`,
/// `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`, `REPIT_APRES_ECHEC` and
/// `MAX_OUTPUT_SIZE`.
pub const REPIT_REARMEMENT_AUDIO: Duration = Duration::from_secs(5);

/// Number of consecutive re-arms before giving up for good.
///
/// Without a bound, a definitively dead audio device would make the
/// "unfit → respite → re-elected → dead" cycle run forever, and each round costs
/// a COM activation.
///
/// ⚠️ **NOT CALIBRATED**, like the previous one. Giving up for good is
/// **logged**, never silent — that is the constraint F3 of D7 set.
pub const REARMEMENTS_MAX: u32 = 5;

/// What a window receives from the global registry.
///
/// **A single channel for both**, and not two parallel channels: what a
/// single channel guarantees is the DELIVERY order — two parallel channels
/// would let a sleeping window's share overtake the sleep order motivating it,
/// and the window would momentarily be described as asleep while it
/// is still encoding.
///
/// ⚠️ **It does NOT guarantee the COMPUTATION order, and the distinction is not
/// theoretical** (I2, final branch review of sub-block D6). Callers
/// do respect "`distribuer` then `distribuer_les_parts`", except one: the
/// `rompus` path of `distribuer_les_parts` sends the shares first, then
/// removes from the pool the sessions whose channel is broken, and only then
/// relays the orders that removal generates. A session woken by the
/// place thus freed receives its `Reveiller` AFTER an already stale sleeping
/// share, and only receives its awake share at the next wheel round.
/// **Bound: `PERIODE_REARBITRAGE`, 250 ms at the `PART_DORMANTE_BPS` floor.**
/// Accepted consequence: recomputing it on the spot would require a second pass
/// of shares under the same lock, for 250 ms of floor on a path that is only
/// taken on the unexpected death of a window thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Sommeil(Ordre),
    Part {
        bps: u32,
    },
    /// Order to carry the sound, or to go silent. Pushed **on change
    /// only**, like `Part`.
    ///
    /// On the same channel as the other two, and for the same reason: within
    /// ONE session's channel, a single channel guarantees the
    /// DELIVERY order between `Sommeil`, `Part` and `Audio`. **This does not extend
    /// across two sessions**: the old carrier and the new one each have
    /// their own channel, read by its own window thread. `porteurs::
    /// distribuer_l_audio` sends the order to go silent before the order to carry,
    /// which REDUCES the window where both windows of the same process
    /// would be audible together — without closing it: the real bound is
    /// the scheduling of the two threads, not this channel.
    Audio {
        actif: bool,
    },
    /// The VM's clipboard changed (sub-block P1). Pushed **on
    /// change only**, like `Part` and `Audio` — it is
    /// `crate::presse_papier::Sondeur` that carries the guards, not this registry.
    ///
    /// `texte` is `None` on a size REFUSAL, `octets` then carrying the
    /// refused size: the content is refused, never truncated, and the refusal is
    /// TOLD to the user (D4). A `None` therefore does not mean "nothing to announce".
    ///
    /// ⚠️ **It is by far the largest message on this channel**: up to
    /// `crate::presse_papier::PRESSE_PAPIER_MAX` (64 KiB), where `Sommeil`,
    /// `Part` and `Audio` weigh a few bytes — **four** orders of
    /// magnitude, and not "two" as this sentence first announced. The
    /// "two orders of magnitude" of `protocole.rs`, its neighbour, is accurate
    /// (8 MiB against 64 KiB): the two sentences did not use the same
    /// scale (cross-cutting review, 20 August 2026).
    ///
    /// 🔴 ~~The registry's channel is UNBOUNDED (`std::sync::mpsc::channel`),
    /// so `send` never blocks — but a blocked window thread
    /// would accumulate these messages. Bounded in practice by the fact that we only emit
    /// on change and that the content-equality guard suppresses
    /// repetitions; named here rather than discovered, and to watch if P3
    /// measures a slow window.~~ **FALSE SINCE 25 AUGUST 2026: the channel
    /// is BOUNDED**, by `capteur/sommeil/file.rs` — `PROFONDEUR_MAX` (64
    /// messages), and a drop beyond that is **REFUSED and COUNTED**, never
    /// blocking nor truncated. The "bounded in practice" above was a
    /// good-faith reasoning about the NORMAL regime: it is the
    /// ABNORMAL regime — a window thread that stops reading — that it did not bound,
    /// and that is the one that cost memory. ⚠️ **`PressePapier` is NOT
    /// coalescable** (it carries the user's data), so it is the
    /// variant through which the bound is actually hit: 64 messages ×
    /// 64 KiB, that is 4 MiB at worst for a blocked window.
    PressePapier {
        texte: Option<String>,
        octets: u32,
    },
}

pub fn signaler(session: &str, visible: bool, focalisee: bool) {
    let mut garde = etat();
    if focalisee {
        garde.focalisee = Some(session.to_string());
        // The focus rank, and not a boolean: it is what makes
        // arbitration rule 3 hold — a group that loses all focus keeps its sound
        // on the LAST one to have had it.
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.derniers_focus.insert(session.to_string(), rang);
    } else if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    let ordres = garde
        .vivier
        .signaler(session, visible, focalisee, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Reports the failure to rebuild the `WindowsSource` during a wake-up.
///
/// Called by task 6 when rebuilding the `WindowsSource` fails
/// after an `Ordre::Reveiller`: without this return path, the pool would believe
/// the window awake forever and would never propose it again at the wheel
/// round.
pub fn echec_de_reveil(session: &str) {
    let mut garde = etat();
    let ordres = garde.vivier.echec_de_reveil(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
}

/// Writes `texte` into the VM's clipboard (browser → VM direction).
///
/// **The sensor is the sole owner of the clipboard** (D1): that is
/// why this door exists, and why no child writes by itself.
///
/// Returns `Err` when the write failed — refusal to open by another
/// application (a NORMAL case under Windows), or mechanism disarmed. **The caller
/// then does NOT inject `Ctrl+V`**: the key is lost, not postponed (D6).
pub fn write_clipboard(texte: &str) -> anyhow::Result<()> {
    presse_papier::write(texte)
}

/// A session reports that its audio capture has died.
///
/// **Replies nothing, and it is intended**: arbitration is global and the decision
/// may concern ANOTHER window. The effect comes back through
/// `DepuisCapteur::Audio`, pushed on the media connection of each window
/// concerned — exactly the pattern of `signaler`.
pub fn audio_mort(session: &str) {
    let mut garde = etat();
    // An already unfit session (respite in progress, or given up for good) that
    // reports a dead capture AGAIN has not failed a second time:
    // it is the SAME failure, said again — typically a channel reconnection on
    // a sensor that stayed alive (`SourceDistante::rattacher`, sub-block D9),
    // which structurally cannot distinguish this case from a real
    // sensor restart and therefore resets `Session::audio_mort_signale`
    // in both cases. Counting this signal as one more CONSECUTIVE failure
    // would bring giving up for good 24 h closer for a reason
    // foreign to the capture's real state. **Logged, never silenced**:
    // silence is precisely the defect F3 of D7 fixed.
    if garde.inaptes.contains_key(session) {
        // `info!`, not `debug!`: operations run at `RUST_LOG=info`
        // (see `encode/arret.rs`), and a silent signal here would be exactly
        // the defect this comment has just explained how to avoid.
        tracing::info!(
            %session,
            "audio capture dead reported again for a session already \
             unfit: redundant signal, re-arm not counted again"
        );
        return;
    }
    let tours = garde.rearmements.entry(session.to_string()).or_insert(0);
    *tours += 1;
    if *tours > REARMEMENTS_MAX {
        // Giving up for good, LOGGED. Silence is precisely the
        // defect F3 of D7 fixed; do not reintroduce it here.
        tracing::warn!(
            %session,
            rearmements = *tours - 1,
            "audio capture dead and given up for good: the PID group will stay silent"
        );
        garde.inaptes.insert(
            session.to_string(),
            Instant::now() + Duration::from_secs(86_400),
        );
    } else {
        tracing::info!(
            %session,
            rearmement = *tours,
            repit = ?REPIT_REARMEMENT_AUDIO,
            "audio capture dead, re-arm scheduled"
        );
        garde
            .inaptes
            .insert(session.to_string(), Instant::now() + REPIT_REARMEMENT_AUDIO);
    }
    porteurs::distribuer_l_audio(&mut garde);
}

/// A session brings the PROOF that its audio capture has restarted: a
/// real packet, not merely a rebuild that returned `Ok` (sub-block
/// D10, closes D9's hand-over 6).
///
/// **Replies nothing, and re-arbitrates nothing**: unlike `audio_mort`,
/// this proof only ever concerns a single session — its own — so
/// nothing to distribute to a neighbour. It only closes the re-arm
/// cycle on a PROOF rather than on the arbitration decision alone
/// (see `sommeil/porteurs.rs`, whose reset lived here until this
/// signal).
pub fn signaler_audio_vivant(session: &str) {
    etat().rearmements.remove(session);
}

/// Removes from the registry the unfitness entries whose respite has expired.
///
/// **Named and separated to be TESTABLE**: the wheel round (250 ms) is
/// what makes the respite effective, and without this purge an unfit window would stay so
/// until the next event, which may never come. An inline `retain`
/// in the wheel round would be covered by no test.
pub(super) fn purger_les_inaptitudes(inaptes: &mut HashMap<String, Instant>, maintenant: Instant) {
    inaptes.retain(|_, echeance| *echeance > maintenant);
}

/// Is this `retirer` stale, that is addressed to an instance already
/// replaced by a re-attachment?
///
/// **Named and separated to be TESTABLE**: the logic inline in
/// `retirer` would be covered by no test, `retirer` touching a global
/// state (`OnceLock<Mutex<Etat>>`) a host test cannot isolate.
pub(super) fn retirer_est_perime(
    generations: &HashMap<String, u64>,
    session: &str,
    generation: u64,
) -> bool {
    generations
        .get(session)
        .is_some_and(|courante| *courante > generation)
}

/// The text the client will receive. **Stable**: it crosses two protocols and
/// is shown to the user.
pub fn raison_en_texte(raison: Raison) -> &'static str {
    match raison {
        Raison::Masquee => "masquee",
        Raison::Evincee => "evincee",
    }
}

// Extracted into a sibling file: this suite took `sommeil.rs` to 500
// lines for a project cap of 500, zero margin from birth. See
// the header of `sommeil/tests.rs`.
#[cfg(test)]
mod tests;

// The two tests of the SIXTH memorisation site (fix round 2) in
// a DEDICATED sibling file, and not added to `tests.rs`: that one is at 473
// lines for a cap of 500, and they would have pushed it over. Avoid growing it
// rather than having to extract afterwards. Precedent:
// `superviseur/table.rs`, which likewise keeps its restart tests apart.
#[cfg(test)]
mod tests_refus;
