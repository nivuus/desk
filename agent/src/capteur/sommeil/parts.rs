//! Computing and distributing bitrate shares — the branch of
//! `capteur::repartiteur::repartir` on the registry of `capteur::sommeil`.
//!
//! **Extracted from `sommeil.rs` and not added into it**: the remedy for the broken
//! channel detected through this path (see `distribuer_les_parts` below) would have
//! taken it beyond the project's 500-line cap. Same reason and
//! same set-up as `fenetre.rs` / `fenetre/transitions.rs`.
//!
//! **No `pub` visibility outside the crate**: this module is a
//! DESCENDANT of `sommeil`, and so rightfully enjoys access to the
//! private items of `sommeil.rs` (`Etat`, `Message`, `distribuer`) — the same
//! Rust visibility rule that lets `fenetre::transitions` call the
//! private methods of `Fenetre`.

use std::sync::{MutexGuard, OnceLock};

use crate::capteur::repartiteur::{self, Fenetre};

use super::file::Envoi;
use super::{distribuer, oublier, Etat, Message};

/// Bitrate budget of the whole session, in bits per second.
///
/// **Per session, not per window** — that is the whole subject of sub-block D6.
/// Read only once: changing it during the process's life would make no sense as long
/// as the link does not change.
///
/// **12 Mb/s is a CHOICE, not a derivation.** Task 1 showed that the
/// link carries ≥ 1.45 Gb/s: the path's capacity bounds nothing here, and the
/// budget is not derived from it. What bounds is what the CLIENT decodes.
/// Task 1bis records, with eight windows: 18.03 % of frames dropped at the full
/// rung, 7.99 % at 1024×576, 1.47 % at 640×360 — and above all that cutting
/// bits **without** crossing a rung threshold saves nothing (23.08 % at
/// constant area). **The lever is resolution, bitrate is only
/// its control.**
///
/// 12 Mb/s keeps for the single-window case exactly what it has today, and
/// gives 1.33 Mb/s per window with eight — that is the 852×480 rung.
///
/// ⚖️ **MEASURED by the acceptance run of task 10 (3 August 2026), and the value is
/// KEPT — but the trade-off is NOT settled by the measurement.** The
/// 852×480 rung with eight windows, which nobody had measured, now is: **3.94 %
/// of frames dropped** by the browser, against a reception threshold set at
/// 7.99 %. The fallback point at 8 Mb/s was measured right after: **1.46 %
/// and 3.94 %** over two runs, but at the 640×360 rung.
///
/// **What the comparison yields is a TRADE-OFF, not a domination**:
/// 852×480 yields **196.4 MP/s** decoded against 95.5 to 135.6 at 640×360, and
/// 3.94 % of frames dropped against 1.46 to 3.94. More pixels delivered, more
/// dropped. **Neither value dominates the other on both quantities.**
/// What tips the balance towards 12 Mb/s comes down to a single reason which, for its part, is not
/// debatable: **it costs nothing to the single-window case**, where 8 Mb/s would
/// take away a third of its bitrate — and that case has never been measured at
/// 12 Mb/s (see §2.4 of the results). **The choice is therefore accepted, not
/// demonstrated.**
///
/// ⚠️ **The margin is a LABORATORY margin, and it is thin.** The 3.94 %
/// comes from **a single** run. **Only one of the five runs at 12 Mb/s
/// passes the threshold** — and **one in two** if we keep only those whose
/// encoding ladder had actually settled, the only honest population.
/// The others record 8.03 %, 16.70 %, 59.32 % and 75.47 %. The degradation
/// covaries with the load of the measuring host **and** with the ladder not
/// settling; **the two are not separated.** The quantity that
/// governs here is neither the link (`packetsLost` = 0 everywhere) nor the bitrate, but
/// **what the client manages to decode**: on a slower client machine,
/// 12 Mb/s would fall behind. **The product is not demonstrated robust** under the
/// host load actually encountered during the campaign, and the fallback to
/// 8 Mb/s has **never been tested under high load** — that it
/// holds up better there is **not established**.
///
/// ⚠️ **`FACTEUR_FOCUS` does NOT separate the two values.** A first
/// wording of this comment claimed it, on a subset of 8 of the
/// 14 recorded focus moves. Over the 14: **11 succeed**, and
/// **8 out of 10 at 12 Mb/s** against **3 out of 4 at 8 Mb/s** — the two failures at
/// 12 Mb/s happen with 51 % of margin on the rung threshold, hence
/// **without an arithmetic explanation**. See §3.4 ④ of the results.
fn budget_bps() -> u32 {
    static BUDGET: OnceLock<u32> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let budget = std::env::var("BUDGET_BPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(12_000_000);
        tracing::info!(budget_bps = budget, "budget de debit de la session");
        budget
    })
}

/// Recomputes the shares and sends only those that changed.
///
/// **Called AFTER `distribuer`**, never before: sleep orders
/// change wakefulness, and a share computed before them would describe the
/// previous state.
///
/// A channel broken here is removed from the POOL, exactly as in
/// `distribuer` — not only from `canaux` and `dernieres_parts`. Without this
/// removal, the entry would survive in `Vivier::entrees` for the whole life of the
/// process: once out of `canaux`, `distribuer` never detects it again
/// (its `None => false` arm only sees an already absent session), and
/// it would stay a candidate for an encoder place without any thread
/// occupying it. The orders this removal generates (for example waking a
/// session that was waiting for this place) are therefore relayed to `distribuer`, under
/// the same lock — no new acquisition, `distribuer` receives the
/// `MutexGuard` already held here.
pub(super) fn distribuer_les_parts(garde: &mut MutexGuard<'static, Etat>) {
    let eveillees = garde.vivier.eveillees();
    let focalisee = garde.focalisee.clone();
    let fenetres: Vec<Fenetre> = garde
        .canaux
        .keys()
        .map(|session| Fenetre {
            session: session.clone(),
            eveillee: eveillees.iter().any(|e| e == session),
            focalisee: focalisee.as_deref() == Some(session.as_str()),
        })
        .collect();

    let parts = repartiteur::repartir(budget_bps(), &fenetres);

    // Vanished sessions must not leave their share in memory.
    let vivantes: std::collections::HashSet<&String> =
        parts.iter().map(|(session, _)| session).collect();
    garde
        .dernieres_parts
        .retain(|session, _| vivantes.contains(session));

    let mut rompus = Vec::new();
    for (session, bps) in parts {
        if garde.dernieres_parts.get(&session) == Some(&bps) {
            continue;
        }
        // ⚠️ **`None` IS NOT A BREAK, and round 3 fixed this
        // wording** — same grievance as `registre::distribuer` in round 2.
        // It is unreachable today (the sessions come out of
        // `canaux.keys()` under the SAME lock, a few lines above),
        // hence without consequence; but this batch had set itself the rule of no
        // longer FABRICATING an outcome, and writing it as `Envoi::Rompu` would purge a
        // session on a fact that did not happen if that invariance were to
        // fall. An `Option` names the thing: there was no send.
        let issue = garde
            .canaux
            .get(&session)
            .map(|canal| canal.envoyer(Message::Part { bps }));
        match issue {
            // No channel: nothing went out, and there is nothing to purge — the
            // session is already no longer in `canaux`.
            None => {}
            // Delivered: we can memorise, and the overwrite guard at the head of the
            // loop will avoid re-emitting it as long as it does not change.
            Some(Envoi::Depose(_)) => {
                garde.dernieres_parts.insert(session, bps);
            }
            // 🔴 REFUSED: WE DO NOT MEMORISE, AND THAT IS ROUND 1'S WHOLE
            // FIX. This window's queue was full: the share never
            // went out. Writing it into `dernieres_parts` would make the overwrite guard above
            // judge the value "already delivered", which
            // would then suppress ANY future re-emission of that value — the
            // window would stay at its previous bitrate as long as its computed
            // share does not change, without bound. By memorising nothing, the
            // next wheel round proposes it again by itself.
            //
            // ⚠️ **And above all NOT `rompus.push`**: the session is ALIVE,
            // merely late. Purging it would amount to killing the arbitration of
            // the window in most trouble — exactly the wrong reaction.
            //
            // The refusal is already logged, at the step and with the name of the
            // session, by `EmetteurSession::journaliser_le_refus`:
            // tracing it again here would duplicate the line without adding anything.
            Some(Envoi::Refuse) => {}
            Some(Envoi::Rompu) => rompus.push(session),
        }
    }

    // The remedy: a channel broken HERE is not only a lost share,
    // it is the same signal as a channel broken in `distribuer` — a window
    // whose thread left without going through `retirer` (see `Fenetre::servir`,
    // the single passage point on the thread side, short-circuited by a panic). Without
    // this removal from the pool, the entry would survive there for the whole life of the
    // process.
    //
    // `oublier` and not three removals written here: it is the registry's single
    // passage point, and it carries the field this loop omitted —
    // `focalisee` (M1, final branch review). See its doc.
    let mut ordres_du_retrait = Vec::new();
    for session in rompus {
        ordres_du_retrait.extend(oublier(garde, &session));
    }
    if !ordres_du_retrait.is_empty() {
        distribuer(garde, ordres_du_retrait);
    }
}

// Test module extracted into a sibling file: this file was at 488
// lines for a project cap of 500, and fix round 2
// adds to it. Extract, never compress — and in a DEDICATED task, before the one
// that adds. Same idiom as `file/tests.rs` and `superviseur/table.rs`;
// see the doc at the head of the extracted file.
#[cfg(test)]
#[path = "parts/tests.rs"]
mod tests;
