//! Distribution of the VM's clipboard — the branch of
//! `crate::presse_papier::Sondeur` on the registry of `capteur::sommeil`.
//!
//! **Extracted from `sommeil.rs` and not added into it**, exactly like
//! `parts.rs` and `porteurs.rs`: that file is close to its cap, and
//! the repository's rule requires a substantial addition to come with an
//! extraction.
//!
//! It is not named like the root module `crate::presse_papier` by
//! chance, but it does not carry the same thing: that one carries the pure
//! RULE (normalise, denormalise, bound, compare with the last emitted, arm the
//! guards); this one only carries its BRANCH on this registry. Same distinction
//! as `repartiteur` / `parts` and `audio` / `porteurs`.
//!
//! **BOTH directions go through it since sub-block P2**: `distribuer` pushes to the
//! windows what the VM copied, `ecrire` writes into the VM what a window
//! pasted. The second is here and not elsewhere because **the owner of the
//! clipboard is the sensor, and it alone** (D1): a child writing
//! by itself would put N processes in competition over a resource Windows
//! only gives access to one at a time.

use std::sync::MutexGuard;

use anyhow::Result;

use crate::presse_papier::{Annonce, Sondeur};

use super::file::Envoi;
use super::{distribuer as distribuer_les_ordres, etat, oublier, Etat, Message};

/// Pushes a clipboard announcement to **all** registered windows.
///
/// **All, and not only the focused one**: the clipboard is a resource
/// GLOBAL to the Windows session, each browser window has its own
/// local clipboard to feed, and it is the client that decides whether it writes
/// (`PressePapierLocal::aEcrire`, which takes the focus as an argument). Deciding here
/// would deprive an unfocused window of content it will have to write as soon as
/// it gets the focus back — D3's deferred drop.
///
/// **No overwrite filter here**, unlike
/// `parts::distribuer_les_parts`: the `Sondeur` only calls this function on
/// CHANGE — it is the one carrying the content-equality guard (guard no. 2
/// of D5) and the repeated-refusal guard. **Since P2, it ALSO carries
/// guard no. 1** (`apres_notre_ecriture`, armed by `armer_les_gardes` just
/// below): a text we have just written ourselves therefore never
/// arrives here. Re-filtering here would duplicate a decision
/// already taken, and duplicate it *badly*: the registry does not know the text
/// previously emitted, and a second per-session guard would diverge from the first
/// as soon as a window registers or withdraws.
///
/// A broken channel goes through `oublier` — **the registry's single passage
/// point**, never a direct `remove`: that is the lesson of M1 (final branch
/// review of D6), where `focalisee` had been forgotten by two paths that
/// removed by hand.
pub(super) fn distribuer(garde: &mut MutexGuard<'static, Etat>, annonce: Annonce) {
    let (texte, octets) = match annonce {
        Annonce::Texte(texte) => {
            let octets = texte.len() as u32;
            (Some(texte), octets)
        }
        Annonce::Refus { octets } => (None, octets),
    };

    // ⚠️ **A SINGLE TRACE, AND NEVER THE TEXT** (D-P1-7). The clipboard's
    // content is a private resource, and a log committed to git is
    // public to the repository: we only log its SIZE and whether it
    // is a refusal. A single one, because two traces at the same instant are
    // counted as two events — a home-grown trap of D6, where each
    // rung change produced two lines with the same timestamp and
    // counted double in all acceptance counters.
    tracing::info!(octets, refus = texte.is_none(), "presse-papier de la VM");

    // The memory of the current state, for windows that will attach
    // LATER (D-P3-2, agent half). Set at EVERY announcement, refusals included —
    // see the `Etat::dernier_presse_papier` field, which carries the reason and says
    // why the symmetry with `dernieres_parts` is misleading.
    garde.dernier_presse_papier = Some(match &texte {
        Some(t) => Annonce::Texte(t.clone()),
        None => Annonce::Refus { octets },
    });

    let sessions: Vec<String> = garde.canaux.keys().cloned().collect();
    let mut rompus = Vec::new();
    for session in sessions {
        // ⚠️ **`None` IS NOT A BREAK** — third site of the same pattern,
        // and **this one had been shown by nobody**: round 3 named
        // `parts.rs` and `porteurs.rs`, and the repository's rule is to SEARCH for
        // the occurrences rather than fix where they are shown to us.
        // Unreachable here (the names come out of `canaux.keys()` under the same
        // lock, two lines above), hence without consequence — but
        // fabricating a break would purge a session on a fact that did not
        // happen if that invariance were to fall.
        let issue = garde.canaux.get(&session).map(|canal| {
            canal.envoyer(Message::PressePapier {
                texte: texte.clone(),
                octets,
            })
        });
        match issue {
            // No channel: nothing went out, and there is nothing to purge.
            None => {}
            Some(Envoi::Depose(_)) => {}
            // ⚠️ **REFUSED IS NOT BROKEN** (round 1 fix): this window's
            // queue is full, the session is ALIVE, and
            // purging it would kill its arbitration. Nothing is memorised on this
            // path — the emission is unconditional —, so the next
            // clipboard change will go out by itself. The refused
            // content, on the other hand, is LOST for this window: logging it a
            // second time would duplicate the trace of `journaliser_le_refus`, which
            // already names the session.
            Some(Envoi::Refuse) => {}
            Some(Envoi::Rompu) => rompus.push(session),
        }
    }

    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        distribuer_les_ordres(garde, ordres_du_retrait);
    }
}

/// Writes `texte` into the VM's clipboard, and **arms the guards in the
/// same gesture**.
///
/// Called from the WINDOW thread serving `VersCapteur::PressePapierEcrire`.
/// The text arrives already normalised, bounded and denormalised by the child.
///
/// 🔴 **`PRESSE_PAPIER=0` ALSO forbids writing**, and not only
/// reading. The guard is here and not in `crate::presse_papier` because the
/// symmetry that matters is that of the OWNER: `Sondeur::tour` tests
/// `actif()` before any read, this path tests it before any write, and
/// "the whole mechanism is disarmed" stops being a half-truth.
pub(super) fn ecrire(texte: &str) -> Result<()> {
    ecrire_avec(texte, crate::presse_papier::ecrire_la_plateforme)
}

/// The heart of `ecrire`, with its **injected** writer — that is what makes it
/// testable on the host, exactly as `Sondeur::observer` receives its
/// read closure.
///
/// 🔴 **Nothing is set when the write FAILS**, and that is what matters
/// most here: arming before knowing would take out of observation a content
/// that never reached the clipboard, and that content would then become
/// invisible **forever** — the next round would not see it as a
/// change.
pub(super) fn ecrire_avec(texte: &str, ecrivain: impl FnOnce(&str) -> Result<u32>) -> Result<()> {
    if !crate::presse_papier::actif() {
        anyhow::bail!("presse-papier desarme (PRESSE_PAPIER=0)");
    }
    let seq = ecrivain(texte)?;
    // ⚠️ The lock is taken ONLY AFTER the Win32 I/O, never around it:
    // `OpenClipboard` is a contended resource of the window station,
    // and holding it under the registry's global lock would block the attach and
    // removal of ALL windows meanwhile. Same reason, and same
    // discipline, as the `outside the lock` polling of the wheel round.
    etat().notre_ecriture = Some((seq, texte.to_owned()));
    Ok(())
}

/// Consumes OUR pending write and arms the `Sondeur`'s guards.
///
/// 🔴 **To be called BEFORE `sondeur.tour()`, and the order IS the mechanism.**
/// After, the round would already have read the clipboard, found our
/// own text there, and sent it back to the windows: a round trip per
/// paste, exactly what D5's guards exist to suppress.
pub(super) fn armer_les_gardes(sondeur: &mut Sondeur) {
    // The lock is taken and released here, before the I/O of `tour()`: it only covers
    // reading a field.
    let notre = etat().notre_ecriture.take();
    if let Some((seq, texte)) = notre {
        sondeur.apres_notre_ecriture(seq, &texte);
    }
}

/// Emits the current clipboard state **on the SINGLE channel of the session that
/// has just registered**.
///
/// 🔴 **It is the AGENT half of P1's hand-over no. 3**, and it is NOT enough
/// on its own: `client/src/main.ts` memorises the last `clipboard` received
/// before the attach, because this very message falls precisely in the interval
/// where `pressePapier` is not yet assigned. **Delivering one half without
/// the other would make the defect SEEM fixed while it would stay
/// intermittent — and that is worse than a known defect.**
///
/// ⚠️ **NEVER A FAN-OUT.** `distribuer` pushes to all windows; this one
/// only writes on the new channel. Replaying the content to all windows at
/// every attach would be a round trip per attach, and guard no. 3 on the
/// page side could do nothing about it: it only closes the send-back to the agent.
///
/// ⚠️ **A broken channel is NOT handled here**, unlike `distribuer`.
/// It cannot be: ~~this channel has just been inserted in the same
/// function~~ — **FALSE, SAME PREMISE AS THE ONE STRUCK OUT TEN LINES BELOW,
/// SURVIVING HERE UNDER ANOTHER VERB**: the channel is created by
/// `registre::inscrire`, NOT by this function (`emettre_l_etat_courant`
/// is called FROM `inscrire`, after the channel already exists — see the
/// ❌ correction below). What remains true, and actually carries the
/// conclusion: its receiver is still on `inscrire`'s stack, and
/// `envoyer` only returns `Err` if the receiver was dropped — which cannot
/// have happened yet. Calling `oublier` here would remove a session that
/// has just been born.
///
/// ⚠️ **The queue-full REFUSAL cannot happen there either** — but
/// **through a COUNT, not through the false premise first written.**
///
/// ❌ ~~This channel's queue has just been created, it is empty.~~ **FALSE**, and
/// fix round 2 measured it with a probe: this function is
/// called **last** in `inscrire`, hence AFTER `distribuer`,
/// `distribuer_les_parts` and `distribuer_l_audio`, which have already dropped into this
/// same channel — the queue contains at least one `Part`.
///
/// 🔵 **THE COUNT, which does hold.** Between the creation of the channel and this call,
/// `inscrire` can only drop three messages for THIS session: at most
/// one `Sommeil` (the `Reveiller` its registration generates, if there is a
/// place), at most one `Part` (`distribuer_les_parts` only emits if the value
/// changed), at most one `Audio` (same filter). **Three at most, against
/// `PROFONDEUR_MAX` = 64.** The refusal is therefore unreachable with a margin of
/// 61 messages — and if one day a fourth emitter slipped in here, the
/// `match` below would stay right, it would only lose an announcement.
pub(super) fn emettre_l_etat_courant(garde: &mut MutexGuard<'static, Etat>, session: &str) {
    let Some(annonce) = garde.dernier_presse_papier.clone() else {
        return;
    };
    let (texte, octets) = match annonce {
        Annonce::Texte(texte) => {
            let octets = texte.len() as u32;
            (Some(texte), octets)
        }
        Annonce::Refus { octets } => (None, octets),
    };
    // ⚠️ **NEVER THE TEXT IN THE LOG** (D-P1-7): the clipboard's content
    // is a private resource, and a log committed to git is public to the
    // repository. We only log its SIZE, and whether it is a
    // refusal — exactly like `distribuer`.
    tracing::info!(%session, octets, refus = texte.is_none(),
        "etat courant du presse-papier emis a l'inscription");
    if let Some(canal) = garde.canaux.get(session) {
        // 🔴 THE FIFTH SITE, AND THE ONLY ONE THE COMPILER DID NOT POINT AT:
        // `let _ =` absorbs even a `#[must_use]`. It is handled by hand, and
        // the exhaustive `match` replaces the `let _` so that the next
        // variant of `Envoi` is reported here as elsewhere.
        match canal.envoyer(Message::PressePapier { texte, octets }) {
            // The three outcomes are without consequence HERE, and the doc of this
            // function says why — THROUGH A COUNT, not through the false
            // premise that used to be read here.
            //
            // ❌ ~~This channel has just been created in the same function, its queue
            // is empty.~~ **TWICE FALSE, and round 3 caught it THIRTY-
            // EIGHT LINES BELOW ITS OWN REFUTATION**: the channel is created by
            // `registre::inscrire`, not here, and its queue is not empty —
            // `distribuer`, `distribuer_les_parts` and `distribuer_l_audio` have
            // already dropped there (measured: `[Part, Audio]`, and `[Part, Audio]`
            // again with twelve awake neighbours). **It is the 487 shipwreck
            // in its pure form: fixed where it was shown to us,
            // not searched for.**
            //
            // What holds, and what the doc establishes: **at most 3 messages
            // against `PROFONDEUR_MAX` = 64**, and the receiver is still on the
            // stack of `inscrire`. `Refuse` and `Rompu` are therefore indeed
            // unreachable.
            Envoi::Depose(_) | Envoi::Refuse | Envoi::Rompu => {}
        }
    }
}

/// The SECOND TAKE of D-P3-6: consumes the write that arrived AFTER
/// `armer_les_gardes`, arms the guards on it, and discards the announcement if it is
/// ours.
///
/// 🔴 **To be called BETWEEN `sondeur.tour()` and `presse_papier::distribuer`, and
/// the order IS the mechanism** — exactly as `armer_les_gardes` must
/// precede `tour()`. The demonstration of the race, its exact scope and the
/// residue that remains live next to `Sondeur::ecarter_notre_ecriture`, which
/// carries the rule; what is here is its BRANCH on this registry, and nothing
/// else — same distinction as `distribuer` versus the `Sondeur`, and as
/// `parts` versus `repartiteur`.
///
/// ⚠️ **The lock is taken and released here, and it covers no I/O**: same
/// discipline as `armer_les_gardes` just above. It only covers
/// reading a field.
pub(super) fn filtrer_nos_ecritures_tardives(
    sondeur: &mut Sondeur,
    annonce: Option<Annonce>,
) -> Option<Annonce> {
    let notre = etat().notre_ecriture.take();
    sondeur.ecarter_notre_ecriture(notre, annonce)
}

// This module's tests have lived apart since sub-block P3 of the clipboard
// work stream: the file was at 379 lines for a cap of 500, and P3
// adds the `dernier_presse_papier` memory (D-P3-2), the second take
// `filtrer_nos_ecritures_tardives` (D-P3-6) and their tests. The extraction
// precedes the addition, as the repository's rule requires.
//
// ⚠️ The declaration is placed AT THE END OF THE FILE, never where the
// block used to be — which was IN THE MIDDLE, the production code resuming right after.
// It is the place of all the repository's other test `#[path]`s, and a reader
// looking for the production code must not stumble on it.
//
// ⚠️ This use of `#[path]` is OUTSIDE the scope of the "Child module
// convention" of `CLAUDE.md`: same Rust mechanism, different reason — the
// 500-line rule. This module is NOT hoisted to the crate root.
#[cfg(test)]
#[path = "presse_papier/tests.rs"]
mod tests;
