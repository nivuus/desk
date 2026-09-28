//! Recovery net: destroys the virtual outputs left by a previous
//! run that the RAII guard could not cover.
//!
//! **Why the guard is not enough.** `moniteurs_virtuels::Sorties`
//! destroys everything it created, including during a panic — but
//! `Drop::drop` never runs for a process killed outright (`Stop-Process
//! -Force`, a VM that freezes, a crash INSIDE the driver itself).
//! A virtual monitor is a global state of the system, not a state of the
//! process: it survives its disappearance. Without this purge, the only
//! recovery would be to reboot the VM.
//!
//! **How to find the GUIDs of a dead process — the question the
//! initial plan had not anticipated.** The driver removes through a GUID, not through an
//! identifier; `pilote.rs` only keeps its two tables (`apparies`,
//! `a_purger`) in the process's memory, lost with it. Two
//! angles exist:
//!
//! 1. **Regenerate the GUIDs.** Those this module assigns are
//!    DETERMINISTIC (`guid::guid_pour`: constant template | number restarted from
//!    zero at each run), and task 6 showed that the driver reuses
//!    its `TargetId`s from one run to the next — the driver itself is
//!    stable. A purge can therefore replay the same sequence of GUIDs
//!    (1..=`PLAFOND_NUMEROS`) and attempt a removal on each. **It is the
//!    strategy retained below**: no state to keep alive, no
//!    file to keep up to date between two runs — just replay a pure
//!    computation.
//! 2. **Log the GUIDs to disk** so that a separate process
//!    reads them back. Ruled out: such a log would itself be a state that can get
//!    corrupted or be missing (the process killed outright may not have had
//!    time to write it), only to gain one thing that angle 1 already has
//!    without it.
//!
//! **A refusal is the NORMAL outcome, not an error**: most of the
//! regenerated GUIDs designate NOTHING (a previous run created fewer than
//! `PLAFOND_NUMEROS`, or none), and a refusal on a free GUID is
//! indistinguishable from a refusal on a GUID held by a process still alive
//! — this purge therefore makes no difference between the two, by
//! construction.
//!
//! **What this purge does NOT recover**, to be said explicitly rather than
//! left to guesswork:
//! - any output created by software OTHER than this module (Apollo, for
//!   example, assigns its own GUIDs to its own outputs);
//! - any output created by an EARLIER version of `GABARIT_GUID_MONITEUR`
//!   — the template is an arbitrary constant chosen here; if it changes one
//!   day, the outputs of the old template become invisible to this
//!   purge, exactly as they would be to the old code itself;
//! - **nothing, anymore, on the side of a number too large** — and that is
//!   fix I1 of the final branch review. The previous wording
//!   justified the ceiling by "out of reach as long as it exceeds what the
//!   driver accepts (10, measured at task 6)", a reasoning that held for
//!   a bench creating at most ten outputs in its whole life. The supervisor, for its part,
//!   creates one output per window **opening**, without bound: with the
//!   monotonic counter of the time, the 17th opening assigned a GUID this
//!   purge would never have swept, hence a monitor unrecoverable without
//!   rebooting the VM. Two things close it now, and both are
//!   needed: `numeros::Numeros` **recycles** the number of an output actually
//!   removed (numbers in flight are therefore bounded by what is really
//!   due), and it **refuses** to create beyond `PLAFOND_NUMEROS` rather than
//!   assigning out of range. The range swept below therefore covers, by
//!   construction, everything this template could have assigned;
//! - two instances of `PiloteParIoctl` opened in parallel (two
//!   live processes at once, or the same process that would reopen the
//!   device): the allocator of `numeros` restarts from zero per INSTANCE,
//!   not per process nor per physical driver — both would therefore assign
//!   the same GUIDs, and this purge only removes once per number,
//!   not twice. A debt predating this task, but which bounds what the list
//!   above claims to cover.

use anyhow::Result;

use crate::moniteurs_virtuels::guid::guid_pour;
use crate::moniteurs_virtuels::numeros::PLAFOND_NUMEROS;
use crate::moniteurs_virtuels::pilote::{ouvrir_pilote, PiloteParIoctl};
use crate::moniteurs_virtuels::verdict_purge::{verdict, Verdict};
// This module remains a consumer of `diagnostics::multifenetre::montee` for
// two measurement items — the before/after DXGI topology survey and its settling
// delay. The ceiling, for its part, NO LONGER comes from there: it now lives
// in `moniteurs_virtuels::numeros`, next to the allocator that enforces
// it (fix I1) — it was the only one of the three borrowings whose value
// committed the recoverability of a system state, and it had nothing to do
// in a measurement module. See the caveat of task 4, rewritten to its real
// scope: `boucle.rs` borrows from this module too.
use crate::diagnostics::multifenetre::montee::{relever_topologie, DELAI_TOPOLOGIE};

/// Sonde `MULTIFENETRE_VDD_PURGE`.
pub(crate) fn purger() -> Result<()> {
    // `relever_topologie` already logs the topology survey with `moment="avant
    // purge" nombre=…` — a second message here would be a duplicate.
    let avant = relever_topologie("avant purge")?;

    let pilote = ouvrir_pilote()?;
    let mut retirees = 0usize;
    for numero in 1..=PLAFOND_NUMEROS {
        let guid = guid_pour(numero);
        match pilote.retirer_par_guid(guid, "retrait déterministe (purge inter-processus)") {
            Ok(()) => {
                retirees += 1;
                tracing::info!(numero, guid = ?guid, "sortie virtuelle retirée par la purge");
            }
            Err(erreur) => {
                // At `debug`, not silent: out of `PLAFOND_NUMEROS`
                // attempts, the vast majority target a GUID no one has
                // ever assigned, and it is the expected outcome — an `info` or
                // `warn` per attempt would drown the useful signal. But the TOTAL
                // absence of trace is what I1 criticised: without it,
                // the assertion of the header comment ("indistinguishable
                // from a GUID held by a live process") was verifiable
                // by no one, including us. The error code stays here,
                // consultable afterwards.
                tracing::debug!(numero, guid = ?guid, %erreur, "retrait refusé (GUID jamais attribué, ou échec réel — indiscernable côté code de retour)");
            }
        }
    }

    std::thread::sleep(DELAI_TOPOLOGIE);
    let apres = relever_topologie("après purge")?;

    // Explicit verdict: without it, a broken handle or IOCTL would produce
    // silently `retirees=0` and an `Ok(())` — the probe whose job
    // IS to restore would then be the only one judging nothing, whereas
    // `monter_en_n` (montee.rs) emits one in the symmetric case.
    let attendu = avant.len().saturating_sub(retirees);
    match verdict(avant.len(), apres.len(), retirees) {
        Verdict::Conforme => {
            tracing::info!(
                retirees,
                avant = avant.len(),
                apres = apres.len(),
                "purge terminée"
            )
        }
        Verdict::UnTiersAAussiRetire => tracing::info!(
            retirees,
            avant = avant.len(),
            apres = apres.len(),
            attendu,
            "purge terminée — MOINS de sorties qu'attendu, ce que la purge ne \
             peut pas expliquer et n'a pas à dénoncer : un tiers en a retiré \
             une pendant ce temps (Apollo crée puis détruit une sortie \
             temporaire pour sonder ses encodeurs, mesuré au lot 32C)"
        ),
        Verdict::RetraitsSansEffet => tracing::error!(
            retirees,
            avant = avant.len(),
            apres = apres.len(),
            attendu,
            "purge terminée SANS retrouver le compte attendu — il reste PLUS de \
             sorties qu'attendu : des retraits déclarés réussis n'ont rien retiré"
        ),
    }
    Ok(())
}

/// Retries the removal of any output THIS `pilote` knows is due but did not
/// manage to remove earlier — the debt left by task 5: `a_purger`
/// was read by no one, a failed removal therefore stayed unrecoverable
/// for the rest of the run even though its GUID was known.
///
/// **Is NOT called by `purger()` above.** `a_purger` is only
/// fed by `creer` and `detruire` (`moniteurs.rs`); `purger()`
/// calls neither — it goes exclusively through
/// `retirer_par_guid`, which touches no table. `pilote.a_purger()` would
/// therefore ALWAYS be empty there: calling it there would have closed nothing, only
/// simulated a closure. The only real user is
/// `montee::monter_en_n`, where it replays, right after the `Sorties` guard
/// has finished destroying and BEFORE `pilote` goes away in turn, the
/// removals it left due. Distinct from `purger()` by what it
/// targets — a state still alive in memory, not a state regenerated by computation
/// — but close enough to share the same gesture (attempt, treat a refusal
/// as a possible outcome), hence its place here rather than in
/// `moniteurs.rs`, which no longer has room under the 500-line ceiling.
pub(crate) fn rejouer_purge_due(pilote: &PiloteParIoctl) -> usize {
    let mut reussis = 0usize;
    for guid_moniteur in pilote.a_purger() {
        match pilote.retirer_par_guid(guid_moniteur, "retrait rejoué d'un retrait dû") {
            Ok(()) => {
                pilote.oublier(guid_moniteur);
                reussis += 1;
                tracing::info!(guid = ?guid_moniteur, "retrait dû rejoué avec succès");
            }
            Err(erreur) => {
                tracing::warn!(guid = ?guid_moniteur, %erreur, "retrait dû toujours refusé");
            }
        }
    }
    reussis
}
