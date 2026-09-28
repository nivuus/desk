//! The registry itself: ~~the global state (`Etat`)~~ **`Etat` left this
//! file in fix round 3, for `registre/tables.rs` — see
//! below** —, its access point (`etat`), and the four operations that
//! touch it (`distribuer`, `oublier`, `inscrire`, `retirer`).
//!
//! ⚠️ **~~and the wheel round~~: it left this file in fix round
//! 1** (25 August 2026), for `registre/tour_de_roue.rs` — the sentence above
//! named it, and it is fixed in the same move rather than left
//! to age.
//!
//! **Extracted from `sommeil.rs` to stay under the project's 500-line
//! cap** (review of task 10, D9) — same reason and same set-up as
//! `parts.rs` and `porteurs.rs`, its two neighbours already extracted for that
//! reason. `sommeil.rs` keeps *what we talk to the registry through* (`Message`,
//! the public façade `signaler`/`audio_mort`/`echec_de_reveil`); this
//! file is *the registry itself*.
//!
//! **Transposition, not rewrite**: this file is the identical move
//! of the `Etat`..`retirer` block of `sommeil.rs` — no value,
//! no order of operations, no signature changed, only the visibility
//! of `Etat`/`etat`/`distribuer`/`oublier` moved to `pub(super)` to
//! stay reachable from `sommeil.rs` and its other descendants
//! (`parts.rs`, `porteurs.rs`, `tests.rs`), exactly as before
//! the extraction. ⚠️ **WHAT PRECEDES DESCRIBES THE EXTRACTION FROM
//! `sommeil.rs`, AND IS NO LONGER THE CURRENT STATE OF THIS FILE**: `Etat`
//! left `registre.rs` a SECOND time, in fix round 3, for
//! `registre/tables.rs` (see the header above, and `mod tables;`
//! below). This paragraph stays accurate for the extraction it describes; it
//! must not be read as "`Etat` lives here today".

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use crate::capteur::vivier::{Ordre, Vivier, HYSTERESIS, PLAFOND_EVEIL};

use super::file::{canal_de_session, Envoi, ReceveurSession};
use super::{
    parts, porteurs, presse_papier, purger_les_inaptitudes, retirer_est_perime, Message,
    PERIODE_REARBITRAGE,
};

// The registry's TABLES live with the neighbour: `Etat` is an aggregate of
// tables made almost entirely of documentation, this file carries what
// ACTS on them. Extracted in fix round 3, BEFORE writing here — the
// file was at 470 for a cap of 500. The module is called `tables` and
// not `etat` because a function `etat()` lives just below.
mod tables;
pub(super) use tables::Etat;

// The wheel round's THREAD lives with the neighbour: it carries a clock and Win32
// I/O, this file carries the registry. Extracted in fix round 1,
// which had taken this file to 508 lines for a cap of 500 — extract,
// never compress.
mod tour_de_roue;
use tour_de_roue::demarrer_le_tour_de_roue;

static ETAT: OnceLock<Mutex<Etat>> = OnceLock::new();

pub(super) fn etat() -> MutexGuard<'static, Etat> {
    let mutex = ETAT.get_or_init(|| {
        demarrer_le_tour_de_roue();
        Mutex::new(Etat {
            vivier: Vivier::nouveau(PLAFOND_EVEIL, HYSTERESIS),
            canaux: HashMap::new(),
            focalisee: None,
            dernieres_parts: HashMap::new(),
            pids: HashMap::new(),
            arrivees: HashMap::new(),
            derniers_focus: HashMap::new(),
            horloge: 0,
            derniers_audio: HashMap::new(),
            inaptes: HashMap::new(),
            dernier_presse_papier: None,
            notre_ecriture: None,
            rearmements: HashMap::new(),
            generations: HashMap::new(),
            prochaine_generation: 0,
        })
    });
    // Poisoning must not kill the sensor: the pool's state stays
    // consistent (a `Vec` of orders lost at worst), and refusing to serve would be
    // worse than carrying on.
    mutex
        .lock()
        .unwrap_or_else(|empoisonne| empoisonne.into_inner())
}

/// Sends each order to the window concerned. A broken channel signals an
/// already dead window: we remove its entry rather than logging it at
/// every wheel round.
///
/// **In a loop until exhaustion, not a single pass.** Removing an
/// awake session frees its place, and `arbitrer` may then elect ANOTHER
/// session in response — setting `eveillee = true` on it INTERNALLY, in
/// the same move that produces the corresponding `Reveiller` order. If this
/// new batch of orders were not distributed in turn, that election would
/// only be an artefact of the model: `arbitrer` is idempotent, it believes it
/// already served, and no future re-arbitration — not even the wheel round —
/// would re-emit that order. The place would stay occupied in the pool without
/// any real encoder occupying it, for the whole life of the process.
///
/// **Termination**: a round only generates a new batch if it detected at
/// least one broken channel, and each detected broken channel is removed from
/// `canaux` before the next round starts. `canaux` is finite and
/// strictly decreases at each removal; the number of rounds is therefore bounded
/// by the number of registered sessions.
pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, ordres: Vec<(String, Ordre)>) {
    let mut a_traiter = ordres;
    while !a_traiter.is_empty() {
        let mut suite = Vec::new();
        for (session, ordre) in a_traiter {
            // `refuses` is read AFTER the send, so it counts the refusal that
            // has just arrived: the step is exactly the one on which
            // `EmetteurSession::journaliser_le_refus` has just decided, which
            // makes the two lines come out together.
            let (issue, refuses) = match garde.canaux.get(&session) {
                Some(canal) => (
                    Some(canal.envoyer(Message::Sommeil(ordre))),
                    canal.refuses(),
                ),
                // 🔴 `None` IS NOT A DELIVERY, AND ROUND 2 FIXED
                // THIS WORDING. The behaviour is the one from before (the old
                // `None => false`: purge nothing, the session is already no longer
                // in `canaux`), but writing it as `Envoi::Depose(...)` fabricated
                // a delivery and made this loss INDISTINGUISHABLE from a real
                // drop. An `Option` names it for what it is: there was
                // no send.
                None => (None, 0),
            };
            let rompu = match issue {
                // No channel: nothing went out, and there is nothing to purge —
                // `oublier` has already removed this session. Tracing it would make one
                // line per order at every removal, on a nominal path.
                None => false,
                Some(Envoi::Depose(_)) => false,
                // 🔴 A SLEEP ORDER NOT DROPPED MUST NOT BE SO
                // SILENTLY (round 1 fix: it was). `Sommeil` is
                // the ONLY one of the four variants the window APPLIES instead
                // of relaying it — that is what earns it its own
                // handling here.
                //
                // ❌ ~~It is an IRREPARABLE loss of state: the pool has already
                // set `eveillee`, no future re-arbitration will re-emit this
                // order, and the place would stay occupied for the whole life of the
                // process.~~ **BECAME FALSE IN THE SAME BLOCK, AND ROUND 3
                // CAUGHT IT**: it was the reason for the `error!` level, and it
                // contradicted the message emitted twelve lines below
                // ("the order will go out again at the next arbitration"). The two
                // could not both be true; it is the first that fell
                // with the cancellation. **The level is therefore `warn!`** — a
                // delayed message, not a lost state.
                //
                // ⚠️ **NO purge**: the session is ALIVE, merely
                // late, and purging it would kill the arbitration of the window
                // in most trouble.
                //
                // 🔴 **WHAT IS DONE HERE IS "DO NOT LIE", NOT
                // "RETRY"**, and the distinction carries the whole decision.
                // The cancellation above gives the pool back the state from BEFORE
                // the order: nothing is re-emitted inside this loop,
                // it is the next arbitration that takes over, seeing the
                // window in its old state. **The loop's termination
                // is therefore not touched at all.**
                //
                // ⚠️ ~~Giving its place back to the pool cannot be applied here
                // without the loop DIVERGING.~~ **THIS SENTENCE WAS
                // STRONGER THAN WHAT WAS ESTABLISHED, and fix round 2
                // refuted it by measurement**: the third way — calling
                // `echec_de_reveil` from the loop — was played, and the
                // suite finishes in 0.02 s. `REPIT_APRES_ECHEC` excludes the
                // session from the candidates, so with a `maintenant` captured
                // only once, the set in respite strictly grows.
                // **What is true, and nothing more: the termination proof
                // WRITTEN ABOVE does not cover this case** — it
                // rests on the fact that a new batch is only generated by
                // a BROKEN channel, and that each broken one LEAVES `canaux` before the
                // next round. It is a proof to redo, not a
                // divergence. The chosen remedy does not raise the question, and it
                // moreover covers BOTH directions where `echec_de_reveil` only
                // covers wake-up.
                //
                // 🔴 THE TRACE IS PACED, AND IT IS THE REMEDY ITSELF THAT
                // MADE IT NECESSARY.
                //
                // ❌ ~~It is not paced because `distribuer` only emits an
                // order on an arbitration CHANGE — never at every
                // wheel round.~~ **THAT REASON WAS TRUE AS LONG AS THE ORDER WAS
                // LOST.** The cancellation below GIVES BACK the state: the
                // next re-arbitration therefore sees the same divergence again, re-emits
                // the same order, and it is refused again. **Measured: +1 per
                // round, strictly, without bound** — that is four lines per
                // second and per blocked window at `PERIODE_REARBITRAGE`
                // (250 ms), indefinitely. It is not an oversight: it is the
                // remedy that changed the nature of the phenomenon.
                //
                // **The pacing is that of `file.rs` — the powers of two
                // of THIS SESSION's cumulative refusal count**, hence at most
                // 64 lines for its whole life. It relies on the counter
                // the sender already keeps (`EmetteurSession::refuses`)
                // rather than on one more table in `Etat`: no new
                // state, nothing to purge in `oublier`, and **the two lines
                // come out at the SAME step** — the one from `file.rs` says the
                // session overflows, this one says which order suffered from it.
                //
                // ⚠️ **WHAT THIS COUPLING COSTS, and it must be said**: the
                // counter aggregates the four variants, so a sleep
                // order that does not land on a step is not traced
                // INDIVIDUALLY. The phenomenon itself stays visible — the
                // steps follow each other in log₂, so never more than one
                // doubling without a line. What is lost is the detail of which
                // order, at which round; what matters — this window overflows
                // and loses orders — always comes out.
                //
                // ⚠️ **NUANCE RAISED IN REVIEW AND LEFT UNWRITTEN: "stays
                // visible" is OPTIMISTIC, once translated into seconds.**
                // The `refuses` counter is CUMULATIVE and NEVER drops back
                // (see `file.rs`): a session that has already accumulated ~1,000
                // refusals will only emit its next line at step 2048, that is,
                // at one refusal per `PERIODE_REARBITRAGE` (250 ms), SEVERAL
                // MINUTES of silence on a late block. The bound "never
                // more than one doubling without a line" stays true in NUMBER of
                // refusals; its translation into TIME grows with the history already
                // accumulated by the session, and is therefore not a fixed bound.
                Some(Envoi::Refuse) => {
                    // 🔴 WE GIVE THE POOL BACK THE STATE FROM BEFORE THE ORDER. Without it,
                    // it would already have written `eveillee` for an order that never
                    // went out — the SIXTH memorisation site, found in fix round
                    // 2. See `Vivier::annuler_ordre_non_livre`
                    // for both directions and what each one costs.
                    garde.vivier.annuler_ordre_non_livre(&session, ordre);
                    if refuses.is_power_of_two() {
                        tracing::warn!(
                            session_cible = %session,
                            ?ordre,
                            refuses,
                            "ordre de sommeil NON DEPOSE : file de la fenêtre pleine, \
                             etat du vivier rendu, l'ordre repartira au prochain \
                             arbitrage (trace au palier, puissance de deux)"
                        );
                    }
                    false
                }
                Some(Envoi::Rompu) => true,
            };
            if rompu {
                suite.extend(oublier(garde, &session));
            }
        }
        a_traiter = suite;
    }
}

/// Forgets EVERYTHING the registry keeps about a session, and returns the orders
/// its removal from the pool generates.
///
/// **The single passage point**, and that is its whole point: the registry
/// keeps NINE things about a session (its channel, its last share, its PID,
/// its arrival rank, its last received focus, its last audio order
/// sent, the current focus if it carries it, its audio unfitness and its
/// re-arm counter — these last two since D9), and separately returns
/// its entry to the pool. There are three removal paths —
/// the normal close (`retirer`), detection of a broken channel during the
/// distribution of ORDERS (`distribuer`), and the same detection during
/// that of SHARES (`parts::distribuer_les_parts`).
///
/// ⚠️ **`focalisee` was the field forgotten by the last two** (M1, final
/// branch review of sub-block D6). Only the normal close emptied it.
/// A focused session dying through a broken channel therefore left its name in
/// `focalisee`; since no live window carries that name any more, the
/// `FACTEUR_FOCUS` boost stopped applying to anyone — without an
/// observable difference, since it already applied to nobody
/// else. **The consequence that BITES is elsewhere, and it is reachable**:
/// a re-attachment re-registers the SAME session (see `inscrire` and D4's resumption
/// path), which then inherited the focus without the client ever having
/// re-emitted it — two shares instead of one, taken from its neighbours.
pub(super) fn oublier(
    garde: &mut MutexGuard<'static, Etat>,
    session: &str,
) -> Vec<(String, Ordre)> {
    garde.canaux.remove(session);
    garde.dernieres_parts.remove(session);
    // D7's four tables are forgotten HERE and nowhere else. The
    // registry has three removal paths (normal close, broken channel
    // detected by the orders, broken channel detected by the shares): a field
    // forgotten by two of them is exactly defect M1 of the final branch
    // review of sub-block D6.
    //
    // `derniers_audio` in particular: a re-attachment re-registers the SAME
    // session (see `inscrire`), and a `false` left in memory would make the
    // order be judged already delivered — on a channel gone with the break. The window
    // would stay silent without end.
    garde.pids.remove(session);
    garde.arrivees.remove(session);
    garde.derniers_focus.remove(session);
    garde.derniers_audio.remove(session);
    // `inaptes` and `rearmements` (D9): same reason as `derniers_audio`
    // above, and it is defect M1 of the final branch review of
    // sub-block D6 replayed a second time. A re-attachment re-registers the
    // SAME session (see `inscrire` and D4's resumption path): without
    // this purge, a session whose audio capture had just died
    // would inherit its stale unfitness or re-arm counter
    // through an otherwise healthy reconnection — up to
    // `REPIT_REARMEMENT_AUDIO` (5 s) of unjustified exclusion in the ordinary
    // case, up to 24 h of guaranteed silence after giving up for good.
    garde.inaptes.remove(session);
    garde.rearmements.remove(session);
    if garde.focalisee.as_deref() == Some(session) {
        garde.focalisee = None;
    }
    garde.vivier.retirer(session, Instant::now())
}

/// Registers a session in the registry and returns, with its channel, the generation
/// that has just been assigned to it.
///
/// **The generation is stamped HERE, by the sensor, and not transmitted by the
/// protocol** (review of the first version of this task, D9): stamping
/// it at a process LAUNCH does not cover the real race. The
/// only two paths that re-register a name are either a child restarted
/// by the supervisor — in which case `Table::prochaine_session`
/// (`superviseur/table.rs`, which increments `compteur` before composing
/// `w-<n>`) gives a NEW name, hence no race —,
/// or the SAME child re-attaching to the sensor (`CanalTube::rattacher`)
/// after a pipe break, in which case it deliberately says the same
/// `hwnd`/`sortie` again in an attach that never carried a generation. The
/// only boundary where two distinct registrations of the same name can
/// overlap is therefore an attach **on the sensor side**: it is there, and
/// only there, that a new generation must be born.
///
/// The caller (`Fenetre::servir`) keeps the returned value for the time of its
/// service and hands it back as is to `retirer`.
pub fn inscrire(session: &str, pid: u32) -> (ReceveurSession, u64) {
    let (emetteur, receveur) = canal_de_session(session);
    let mut garde = etat();
    // Stamped UNCONDITIONALLY, at EVERY call — including a
    // re-attachment under the same name: it is the only way to distinguish
    // the instance that has just registered from the previous one. See the
    // `prochaine_generation` field for why this counter is distinct from
    // `horloge`.
    garde.prochaine_generation += 1;
    let generation = garde.prochaine_generation;
    garde.generations.insert(session.to_string(), generation);
    if garde.canaux.insert(session.to_string(), emetteur).is_some() {
        tracing::warn!(%session, "canal d'ordres remplacé pour cette session");
        // Without this purge, a share identical to the one already sent on
        // the OLD channel (gone with the break) would be judged already delivered
        // by the overwrite filter of `distribuer_les_parts`, and the NEW
        // channel would never receive it if the topology did not change between
        // the two registrations — the bitrate ceiling would stay stale without
        // end. A first registration, for its part, has nothing to purge.
        garde.dernieres_parts.remove(session);
        // Same reason as the line above: the memorised audio order was memorised
        // on the OLD channel, gone with the break.
        garde.derniers_audio.remove(session);
    }
    garde.pids.insert(session.to_string(), pid);
    // The arrival rank is only set if the session does not already have one.
    //
    // ⚠️ **The real scope is narrower than the intent** (F6, final
    // branch review of sub-block D7). The intent is that a re-attachment
    // does not make the window lose its seniority within its PID
    // group — but `oublier` removes `arrivees` AND `derniers_focus`, and on an
    // ORDINARY re-attachment the `retirer` of the dead window thread runs BEFORE
    // the child reconnects: the two tables are then already empty,
    // and the guard below keeps nothing. It only bites in the race window
    // where the new registration precedes the removal of the old one.
    //
    // Keeping these tables for a grace period would be the remedy, but
    // it is a design change of the registry — to be framed, not
    // improvised: the same identity by NAME already carries a known race,
    // recorded for the next sub-block.
    if !garde.arrivees.contains_key(session) {
        garde.horloge += 1;
        let rang = garde.horloge;
        garde.arrivees.insert(session.to_string(), rang);
    }
    let ordres = garde.vivier.inscrire(session, Instant::now());
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
    presse_papier::emettre_l_etat_courant(&mut garde, session);
    (receveur, generation)
}

pub fn retirer(session: &str, generation: u64) {
    let mut garde = etat();
    // No effect if the recorded registration is MORE RECENT: this `retirer`
    // is that of an instance already replaced by a re-attachment (F5, D9).
    // NOT calling `oublier` here is deliberate: the nine tables it
    // purges all belong to the LIVE instance, not to the stale one
    // calling this `retirer`.
    if retirer_est_perime(&garde.generations, session, generation) {
        tracing::info!(%session, generation, "retirer périmé ignoré");
        return;
    }
    let ordres = oublier(&mut garde, session);
    distribuer(&mut garde, ordres);
    parts::distribuer_les_parts(&mut garde);
    porteurs::distribuer_l_audio(&mut garde);
    garde.generations.remove(session);
}
