//! A session's message queue, BOUNDED BY COALESCING.
//!
//! 🔴 WHY THIS MODULE EXISTS. The registry's channel was an
//! UNBOUNDED `std::sync::mpsc::channel()`, and four sub-blocks each added
//! a variant to it. Three of them are pushed "on change
//! only" and are only valuable in their LAST occurrence: under
//! coalescing, they stop making the queue grow in steady state,
//! whatever the arrival rate.
//!
//! 🔴 COALESCING KEEPS THE SLOT'S POSITION — IT DOES NOT KEEP
//! THE ARRIVAL ORDER OF THE VALUES, AND THE DISTINCTION IS THE WHOLE POINT.
//!
//! ❌ ~~Replacing in place preserves the delivery order; moving to the tail
//! would make a bitrate share cross a sleep order dropped in the meantime,
//! and would deliver the share AFTER the order that should have made it void.~~
//! **THIS ARGUMENT WAS FALSE, and fix round 1 refuted it**:
//! coalescing at the TAIL would always put the most recent value at the tail,
//! so `Part(asleep), Sommeil(Reveiller), Part(awake)` would yield
//! `[Reveiller, Part(awake)]` there — chronologically right AND carrying the
//! right value. The failure mode described does not exist. Struck out rather
//! than erased.
//!
//! **WHAT IS TRUE, AND WHAT HOLDS THE DECISION.** Replacing in place keeps
//! the slot's POSITION: the queue keeps exactly as many entries, in the
//! same places. It does NOT keep the arrival order of the values — the
//! awake value is delivered in the place the sleeping one occupied,
//! hence BEFORE the `Sommeil` that arrived between the two. **That is acceptable because
//! the sensor RELAYS these two variants without applying them**:
//! `fenetre/transitions.rs` writes "nothing to do locally" for `Part` and
//! for `Audio` — only `Sommeil` has a local effect. The two policies
//! therefore converge to the same final state, and it is that argument, not the
//! previous one, that justifies the choice.
//!
//! ⚠️ SPLITTING THE VARIANTS INTO DISTINCT CHANNELS REMAINS WRONG, for a reason
//! that has not moved: `Sommeil` has a local effect, and two
//! parallel channels would let a sleep order overtake an already delivered share or
//! the reverse, without any total order existing between them. It is the solution
//! that comes to mind first, and it is wrong.
//!
//! ⚠️ ~~THIS MODULE IS PURE: it knows neither lock, nor thread, nor Windows.~~
//! **BECAME FALSE when this module received the CHANNEL itself** (`Partage`,
//! `EmetteurSession`, `ReceveurSession`, below): it now knows a
//! `Mutex` and an `Arc`. Struck out rather than erased, as this repository does
//! everywhere. **What remains true, and which was the intent**: it still knows
//! neither Windows, nor any `#[cfg]` — `capteur/sommeil.rs` is not
//! gated, so this whole file compiles and is tested on the Linux host through
//! `cargo test --workspace`. The RULE (`deposer`, `coalescable`) stayed
//! pure: it takes a `VecDeque` and nothing else.
//!
//! 🔴 THIS MODULE REPLACES `std::sync::mpsc::channel()`, BUT ITS `envoyer` HAS
//! THREE OUTCOMES WHERE `send` HAD TWO — AND IT IS THE TRAP FIX ROUND 1
//! PAID FOR.
//!
//! Under `mpsc`, `send(...).is_ok()` meant **"delivered"**. Here it would
//! only mean "not disconnected": a queue-full refusal is a DELIVERY
//! FAILURE on a perfectly ALIVE session. The five production
//! call sites had kept the old reading, and two of them
//! MEMORISED the refusal as a send (`dernieres_parts`,
//! `derniers_audio`), which suppressed any future re-emission of that
//! value — a window stuck at the previous bitrate, or silent, **without
//! bound**.
//!
//! 🔴 THAT IS WHY `envoyer` DOES NOT RETURN A `Result` BUT A THREE-VARIANT
//! `Envoi`, `#[must_use]`, THAT EACH CALLER HANDLES WITH AN EXHAUSTIVE
//! `match`. The remedy is MECHANICAL: the compiler refuses a site that
//! forgets a case, and will likewise refuse any future variant.
//! ⚠️ **`#[must_use]` on `Depot` alone was NOT enough, and it was
//! measured**: `Result` is itself `#[must_use]`, and `.is_ok()`, `.is_err()`
//! or `let _ =` consume it — which switches off the `must_use` of the `Depot` it
//! contains. `cargo check` reported NONE of the five sites.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::Message;

/// The depth beyond which a drop is REFUSED.
///
/// ⚠️ **NOT CALIBRATED.** No constant of this repository is. It is chosen
/// large enough for a normal regime never to reach it — the coalescable
/// variants do not contribute to it — and small enough for memory to stay
/// bounded if a child stops reading.
pub(crate) const PROFONDEUR_MAX: usize = 64;

/// What a drop did.
///
/// ⚠️ **`#[must_use]` is set here on principle — it does NOT guard the calls
/// to `envoyer`.** Measured: wrapped in a `Result` (itself `must_use`),
/// it is switched off as soon as one writes `.is_ok()` or `let _ =`. It is `Envoi`,
/// below, that carries the real guard. This `must_use` only covers
/// DIRECT calls to `deposer`.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Depot {
    /// Added at the tail.
    Empilee,
    /// Replaced, IN PLACE, a message of the same variant already pending.
    Coalescee,
    /// The queue was full. **Nothing was added, nothing was removed.**
    Refusee,
}

/// Can this variant replace a pending occurrence of itself?
///
/// 🔴 THE RULE IS "DOES LOSING IT COST ANYTHING?", NOT "IS IT
/// FREQUENT?". `Part` and `Audio` are pushed on change only and
/// are only valuable in their last occurrence. `Sommeil` carries an ORDER,
/// `PressePapier` carries the USER'S DATA: neither one nor the other can be
/// replaced.
///
/// ⚠️ ANY NEW VARIANT MUST GO THROUGH HERE, and the `match` is EXHAUSTIVE so
/// that the compiler requires it — never a `_ => false`, which would classify it
/// "to keep" silently and let the queue grow again.
pub(crate) fn coalescable(m: &Message) -> bool {
    match m {
        Message::Part { .. } | Message::Audio { .. } => true,
        Message::Sommeil(_) | Message::PressePapier { .. } => false,
    }
}

/// Are two messages of the same variant?
fn meme_variante(a: &Message, b: &Message) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Drops a message, applying its variant's policy.
pub(crate) fn deposer(file: &mut VecDeque<Message>, message: Message) -> Depot {
    if coalescable(&message) {
        if let Some(place) = file
            .iter()
            .position(|en_attente| meme_variante(en_attente, &message))
        {
            file[place] = message;
            return Depot::Coalescee;
        }
    }
    if file.len() >= PROFONDEUR_MAX {
        return Depot::Refusee;
    }
    file.push_back(message);
    Depot::Empilee
}

/// A session's shared state: its queue, and the count of its refusals.
///
/// **Exactly two holders, never more**: the sender and the receiver.
/// Neither `EmetteurSession` nor `ReceveurSession` is `Clone`, and **that is
/// not an oversight** — it is what gives meaning to the `Arc::strong_count`
/// in `envoyer` (see its doc). Making either of them cloneable would break the
/// detection of the dropped receiver **without any test flinching**.
struct Partage {
    file: Mutex<VecDeque<Message>>,
    /// CUMULATIVE count of this session's refused drops.
    refuses: AtomicU64,
    /// The session's name, carried HERE and for a single reason: **making the
    /// refusal trace ATTRIBUTABLE**.
    ///
    /// 🔴 ~~The session's name is not here, the caller's span
    /// attributes it.~~ **FALSE, and fix round 1 established it: THERE
    /// IS NO SPAN.** The sensor's only `info_span!` is set on the
    /// WINDOW thread (`capteur/fenetre.rs`); the five calls to `envoyer`
    /// run either on the wheel-round thread — no span —, or, via
    /// `signaler`, **under the span of ANOTHER session**, `distribuer_les_parts`
    /// pushing to *all*. The field would then have been WRONG, which is worse
    /// than missing. An operator reading `refuses=8` without knowing which
    /// session learns nothing.
    ///
    /// ⚠️ **The trace publishes it under the name `session_cible`, NOT `session`**
    /// (round 2): the span of another session may well wrap
    /// this line, and two `session=` with different values side by side
    /// would replay the confusion one step further. The distinct name says
    /// which of the two designates the overflowing window.
    session: String,
}

/// The end through which the registry writes to a window.
pub(crate) struct EmetteurSession {
    partage: Arc<Partage>,
}

/// The end through which the window thread reads. **Returned by `inscrire`.**
pub(crate) struct ReceveurSession {
    partage: Arc<Partage>,
}

/// What became of an `envoyer`. **THREE outcomes, never two.**
///
/// 🔴 AN ENUM AND NOT A `Result<Depot, ()>`, AND IT IS THE MECHANICAL REMEDY FOR
/// ROUND 1'S CRITICAL. A `Result` invites `.is_ok()` / `.is_err()`, which
/// crush `Depose` and `Refuse` into a single value — that is exactly the
/// confusion that cost two faulty memorisations. Here the compiler
/// **requires** each site to name the three cases, and will still do so for
/// any variant added later. The repository's precedent: the exhaustive
/// `match` of `transport/controle.rs`, which "reports itself to the compiler",
/// as opposed to the catch-all of `capteur/pont_media.rs` that killed a thread
/// silently six times.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Envoi {
    /// The message is in the queue: it WILL REACH the window. Carries what the
    /// drop did (stacked, or coalesced onto a pending occurrence).
    Depose(Depot),
    /// The queue was full: **nothing was dropped**, and the message is
    /// lost. ⚠️ **The session is ALIVE** — purging it would kill the
    /// arbitration of the window in most trouble. And **it must not
    /// be counted as delivered either**: any caller that MEMORISES what it
    /// sent must refrain here, otherwise its overwrite guard suppresses
    /// the re-emission of that value forever.
    Refuse,
    /// The receiver has dropped: the session is DEAD, it must be PURGED. It is
    /// the only case that replaces the old `send(...).is_err()`.
    Rompu,
}

/// Why a receive returned nothing.
///
/// The distinction mirrors that of `std::sync::mpsc::TryRecvError`
/// (`Empty` / `Disconnected`) which this pair replaces: `transitions.rs`
/// treated the two cases differently in its comment, and confusing them
/// would erase that distinction.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VideOuFerme {
    /// The queue is EMPTY — the sender is still alive, it has dropped nothing.
    Vide,
    /// Nobody writes any more: the sender has dropped, and the queue is exhausted.
    Ferme,
}

/// A session's pair. **One sender, one receiver, and never more.**
///
/// `session` is kept only to make the refusal trace attributable —
/// see the `Partage::session` field, which says why no span can
/// do it in its place.
pub(crate) fn canal_de_session(session: &str) -> (EmetteurSession, ReceveurSession) {
    let partage = Arc::new(Partage {
        file: Mutex::new(VecDeque::new()),
        refuses: AtomicU64::new(0),
        session: session.to_string(),
    });
    (
        EmetteurSession {
            partage: Arc::clone(&partage),
        },
        ReceveurSession { partage },
    )
}

/// Takes a queue's lock, **without ever panicking**.
///
/// 🔴 WHAT HAPPENS TO A POISONED LOCK: **we take the state as is
/// and carry on**, never an `unwrap()`. Three reasons, in this order:
///
/// ① **The sensor holds ALL the windows.** A panic here, on the
/// wheel-round thread or on a window thread, would take down the channel of each of the
/// N sessions, not only that of the faulty session.
///
/// ② **The state stays consistent by construction.** Nothing running under this
/// lock can panic leaving the `VecDeque` half-written:
/// `deposer` only does a `position`, an indexed write and a
/// `push_back` there, and `essayer_recevoir` a `pop_front`. Poisoning could
/// only come from a panic of ANOTHER thread while it holds this
/// lock — at worst one message more or less in the queue.
///
/// ③ **The repository already has this precedent, and it is named**:
/// `registre.rs::etat()` does the same `unwrap_or_else(|e| e.into_inner())`,
/// for the same reason, written in the same place.
fn under_lock<T>(
    verrou: &Mutex<VecDeque<Message>>,
    action: impl FnOnce(&mut VecDeque<Message>) -> T,
) -> T {
    let mut file = verrou
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner());
    action(&mut file)
}

impl EmetteurSession {
    /// Drops a message for the window, and says what became of it.
    ///
    /// 🔴 `Envoi::Rompu` MEANS "THE RECEIVER HAS DROPPED", and **it is the
    /// contract `std::sync::mpsc::Sender::send(...).is_err()` gave before
    /// this module**. `registre.rs::distribuer`,
    /// `parts::distribuer_les_parts`, `porteurs::distribuer_l_audio` and
    /// `presse_papier::distribuer` purge a dead session on this value,
    /// and **nothing else purges it on these paths**: never returning it would
    /// let dead sessions occupy a place in the pool for the life
    /// of the process.
    ///
    /// ⚠️ **HOW WE KNOW: `Arc::strong_count(&self.partage) == 1`.**
    /// It is the ONLY signal available — there is no `mpsc` any more to
    /// give it. It only holds because `canal_de_session` creates exactly
    /// two holders and neither end is `Clone`: `1` then
    /// means "I am alone", hence "the receiver was dropped".
    ///
    /// 🔴 **`Envoi::Refuse` IS NEITHER A DELIVERY NOR A BREAK**, and it is
    /// the third outcome `mpsc` did not have. Confusing it with the
    /// first memorises a message that never went out; with the second, it
    /// purges a perfectly alive session. The exhaustive `match` that `Envoi` imposes
    /// is what prevents both.
    pub(crate) fn envoyer(&self, message: Message) -> Envoi {
        if Arc::strong_count(&self.partage) == 1 {
            return Envoi::Rompu;
        }
        match under_lock(&self.partage.file, |file| deposer(file, message)) {
            Depot::Refusee => {
                let refuses = self.partage.refuses.fetch_add(1, Ordering::Relaxed) + 1;
                self.journaliser_le_refus(refuses);
                Envoi::Refuse
            }
            depose => Envoi::Depose(depose),
        }
    }

    /// The CUMULATIVE count of this session's refused drops.
    ///
    /// ⚠️ ~~**`#[cfg(test)]`**~~ **FALSE IN THE PRESENT — the gate fell in
    /// round 3, see just below; what follows describes the state AT THE TIME,
    /// in round 1.** It was a round 1 fix: it had NO
    /// production caller (`method 'refuses' is never used` on the
    /// Windows target), while its doc announced the benefit "telling WHICH one
    /// overflows". That benefit was delivered by the TRACE, which now carries the
    /// session name; this accessor existed only so the test could
    /// exercise the counter. Gating it was what prevented re-asserting an
    /// operational benefit that did not exist.
    ///
    /// ✅ **THE `#[cfg(test)]` GATE FELL IN ROUND 3**: the method now
    /// has a PRODUCTION caller — `registre::distribuer` uses it
    /// to pace its own trace on the SAME step as
    /// `journaliser_le_refus`, so that the two lines come out together.
    /// The reasoning that had gated it stays right: we do not ungate to
    /// make it pretty, we ungate because a caller appeared.
    pub(crate) fn refuses(&self) -> u64 {
        self.partage.refuses.load(Ordering::Relaxed)
    }

    /// Logs a refusal **WHEN A STEP IS CROSSED, never at every
    /// refusal** — and the chosen step is the **power of two** of the cumulative
    /// count (1, 2, 4, 8, 16…).
    ///
    /// 🔴 WHY NOT ONE TRACE PER REFUSAL. "Never trace per packet in
    /// the transport loop": 18,619 lines in a few seconds on a
    /// CIFS share have already prevented a session from establishing. A blocked
    /// window receives a message every `PERIODE_REARBITRAGE` (250 ms) at
    /// least, and much more on an active clipboard — a per-refusal
    /// trace would grow without bound with the duration of the block.
    ///
    /// 🔴 WHY THE STEP RATHER THAN A TRACE "ON ENTERING SATURATION".
    /// The alternative — tracing the "was not refusing → refuses" transition —
    /// is NOT bounded: a queue oscillating around `PROFONDEUR_MAX`
    /// crosses it at every wheel round, and we fall back to one line every
    /// 250 ms for the duration of the block. The cumulative count, on the other hand, is
    /// monotonic: **at most 64 lines for the whole life of a session**, whatever
    /// happens, and the line carries the count, so the magnitude stays
    /// readable without having to count the lines.
    ///
    /// 🔴 ~~The session's name is NOT here; the caller's span
    /// attributes it.~~ **FIXED IN ROUND 1: THERE IS NO SPAN**, and the
    /// trace was therefore attributable to nobody. The sender knows its
    /// session: it carries it, and the trace names it — under `session_cible`, so as
    /// not to collide with an enclosing span. See
    /// `Partage::session`.
    fn journaliser_le_refus(&self, refuses: u64) {
        if refuses.is_power_of_two() {
            tracing::warn!(
                // `session_cible` and not `session`: this field may be set
                // UNDER the `fenetre{session=…}` span of ANOTHER session — the
                // window thread calling `signaler` makes it push to
                // ALL. Two `session=` with different values on the same
                // line would replay, one step further, the confusion this
                // field has just removed.
                session_cible = %self.partage.session,
                refuses,
                profondeur_max = PROFONDEUR_MAX,
                "a session's queue is full: message REFUSED (traced at each step, power of two)"
            );
        }
    }
}

impl ReceveurSession {
    /// Removes the oldest pending message, without ever blocking.
    ///
    /// **The only PRODUCTION call** (`transitions.rs::appliquer_les_ordres`,
    /// in a loop until `Err`). ⚠️ **No blocking variant is shipped,
    /// and it is deliberate**: the surface survey found neither `recv()` nor
    /// `recv_timeout()` on this channel, neither in production nor in tests. A
    /// blocking method would require a `Condvar` nobody would call,
    /// hence a mechanism the product never exercises.
    ///
    /// ⚠️ **`Ferme` is only returned once the queue is EXHAUSTED**: what was
    /// dropped before the sender fell can still be read, as
    /// `mpsc` did. Throwing these messages away would lose an already decided sleep order.
    pub(crate) fn essayer_recevoir(&self) -> Result<Message, VideOuFerme> {
        match under_lock(&self.partage.file, |file| file.pop_front()) {
            Some(message) => Ok(message),
            None if Arc::strong_count(&self.partage) == 1 => Err(VideOuFerme::Ferme),
            None => Err(VideOuFerme::Vide),
        }
    }

    /// Removes and returns EVERYTHING waiting, in order.
    ///
    /// The replacement of `Receiver::try_iter().collect()`, which the suites of
    /// `parts`, `porteurs` and `presse_papier` use to read the LAST
    /// message of a variant. **In a single lock acquisition** rather than one
    /// per message.
    ///
    /// ⚠️ **`#[cfg(test)]`, and it is a round 1 fix**: its six
    /// callers are ALL test helpers (`method 'drain' is never used`
    /// on the Windows target). Production, for its part, only reads this channel through
    /// `essayer_recevoir`, in a loop.
    #[cfg(test)]
    pub(crate) fn drain(&self) -> Vec<Message> {
        under_lock(&self.partage.file, |file| file.drain(..).collect())
    }
}

// Test module extracted into a sibling file: this file was at 445
// lines for a project cap of 500, and fix round 1
// adds code and doc to it. Extract, never compress — and in a
// DEDICATED task, before the one that adds. Same set-up and same idiom as
// `superviseur/table.rs`; see the doc at the head of the extracted file for
// why this `#[path]` does NOT fall under the `<parent>_<child>` convention.
#[cfg(test)]
#[path = "file/tests.rs"]
mod tests;
