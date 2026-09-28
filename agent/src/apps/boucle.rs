//! The reconciliation loop: read, filter, compare, emit.
//!
//! 🔴 `#[cfg(windows)]`: it opens COM and walks four real
//! trees. Yet everything it DECIDES lives in `apps::raccourci` and
//! `apps::reconciliation`, which are pure and tested on the host — this module
//! only orchestrates.

use std::time::Instant;

use proto::plateforme::{IssueLancement, VersLaPlateforme};
use tokio::sync::{mpsc, watch};

use super::icone;
use super::{lancement, lecture};
use crate::plateforme::{Identite, Ordre};

/// What the loop keeps from one tick to the next, and the reconciliation that
/// produces it — **EXTRACTED VERBATIM from this file, BEFORE the addition that required it.**
///
/// ORDINARY `mod`, without `#[path]`: both are `#[cfg(windows)]`, and the
/// "Child module convention" of `CLAUDE.md` reserves `#[path]` for
/// modules that must cross a `#[cfg]` boundary to exist on the host.
mod memoire;

use memoire::{reconcilier, Memoire};

use super::surveillance::mode::Mode;
use super::surveillance::partage::Veille;
use super::surveillance::rebond::Rebond;

/// What triggered THIS reconciliation.
///
/// 🔴 IT IS CARRIED ON THE `catalogue reconcilie` LINE, AND THAT IS WHAT MAKES
/// CRITERION ① READABLE: without it, a reconciliation arriving within the second that
/// follows the creation of a shortcut is indistinguishable from a periodic
/// reconciliation that happened to fall there by chance. **A judging figure must say where
/// it comes from.**
///
/// ✅ No instrument shipped in this repository reads this trace — checked with
/// `grep -rn "catalogue reconcilie"` over the `.mjs`, `.js`, `.ts`, `.sh` and
/// `.ps1` files, which returns NO line. Adding fields to it therefore breaks nothing.
#[derive(Debug, Clone, Copy)]
pub(super) enum Declencheur {
    /// The very first tick. It is `complet` by construction, and it is the one
    /// that catches up on everything that changed **while the agent was stopped**.
    Demarrage,
    /// The `PERIODE_RECONCILIATION` deadline. **The source of truth.**
    Periode,
    /// The watcher saw something move. **An ACCELERATOR, and nothing
    /// else**: this tick does no less work than a periodic tick,
    /// it just arrives earlier.
    Notification,
    /// An installer exiting raised `partage.reconcilier` (sub-block G3).
    Installation,
}

impl Declencheur {
    fn mot(self) -> &'static str {
        match self {
            Declencheur::Demarrage => "demarrage",
            Declencheur::Periode => "periode",
            Declencheur::Notification => "notification",
            Declencheur::Installation => "installation",
        }
    }
}

/// What the `catalogue reconcilie` line carries on top of the catalogue itself.
pub(super) struct Contexte {
    pub(super) declencheur: Declencheur,
    /// Cumulative since the watcher thread started, never a delta:
    /// two successive lines can be subtracted, a delta already taken cannot be
    /// recomposed.
    pub(super) notifications: u64,
    pub(super) debordements: u64,
}

/// Honours a launch order, and returns its outcome.
fn honorer(memoire: &Memoire, demande: &str, cle: &str) -> IssueLancement {
    let Some((chemin, montrer)) = memoire.lancables.get(cle) else {
        // ⚠️ `Inconnue` IS RETURNED HERE AND NOWHERE ELSE: only this stage
        // knows the catalogue. `apps::lancement` knows nothing about keys.
        tracing::warn!(demande, cle, "clé absente du catalogue de l'agent");
        return IssueLancement::Inconnue;
    };
    let cible = memoire
        .catalogue
        .iter()
        .find(|a| a.cle == cle)
        .map(|a| a.cible.as_str())
        .unwrap_or_default();
    let issue = lancement::lancer(chemin, cible, *montrer);
    tracing::info!(demande, cle, ?issue, "lancement");
    issue
}

/// Where the loop talks, how often, and what it shares with the watch and
/// install threads.
pub struct Reglages {
    pub base_plateforme: String,
    pub periode: std::time::Duration,
    pub partage: crate::apps::installation::partage::Partage,
    pub veille: Veille,
    pub mode: Mode,
}

/// The body of the loop, on its dedicated thread.
///
/// 🔴 A DEDICATED BLOCKING THREAD, AND COM INITIALISED ONLY ONCE ON IT.
/// `IShellLinkW` and `ShellExecuteExW` both require an apartment, and an
/// apartment belongs to ITS thread: both must therefore run here, not
/// on a pool thread that tokio could change between two ticks.
pub fn tourner(
    canal_emission: impl Fn(VersLaPlateforme) + Send + 'static,
    mut ordres: mpsc::UnboundedReceiver<Ordre>,
    mut identite: watch::Receiver<Option<Identite>>,
    reglages: Reglages,
) {
    let Reglages {
        base_plateforme,
        periode,
        partage,
        veille,
        mode,
    } = reglages;
    if let Err(erreur) = lecture::initialiser_com() {
        tracing::error!(%erreur, "decouverte d'applications abandonnee : COM indisponible");
        return;
    }

    let mut memoire = Memoire::default();
    // 🔴 THE FIRST TICK IS COMPLETE, AND SO IS EVERY IDENTITY CHANGE. A
    // `Catalogue` lost during an outage would otherwise leave the platform
    // diverging WITH NO END — the channel is a `push` with no delivery
    // guarantee, and its queue drops what it cannot deliver.
    //
    // ⚠️ **IN PRACTICE, THE COMPLETE SEND IS PERIODIC — MEASURED, AND IT WAS
    // WRITTEN NOWHERE.** On the log of an idle agent, on 21 August 2026:
    // ELEVEN "le prochain catalogue sera COMPLET" lines for TWELVE
    // reconciliations, and **no re-enrolment took place**. It is the
    // heartbeat's token refresh that makes the `watch` move, and
    // `PERIODE_BATTEMENT` is exactly `PERIODE_RECONCILIATION`. **The
    // complete catalogue therefore goes on the wire every thirty seconds,
    // forever, over a disk that does not move.**
    //
    // 🔴 AND IT IS NOT FIXED, FOR A REASON THAT IS THE OPPOSITE OF WHAT ONE
    // WOULD THINK. This loop COULD tell the two apart — a re-enrolment
    // changes the `prefixe`, a heartbeat only changes the `jeton` — and only raise
    // `complet` on the former. But that would **remove a real
    // repair**: the promise written three lines above, "a lost `Catalogue`
    // does not leave the platform diverging with no end", has NO
    // other implementation than this periodic complete send. What looks
    // like a defect is the only thing that holds the guarantee this comment
    // announces.
    //
    // ⛔ **LEGACY, AND IT IS NAMED**: arbitrating between this safety net and its cost requires
    // MEASURING THE FRAME ON THE WIRE, which G4 does not do. The only figure
    // available is G1's — **56,145 bytes** for **154** applications,
    // and **WITHOUT** the `icone` and `source_max` fields G2 added. Today's
    // carries **156** applications **with** both: it is bigger,
    // and **it is measured by nothing**.
    let mut complet = true;
    // The current identity is marked as read: what follows only reacts to
    // CHANGES, and the first send is already complete through the line above.
    identite.borrow_and_update();

    let mut rebond = Rebond::default();
    // 🔴 THE REMEMBERED VALUE, NOT A FLAG. `Veille::notifications` is
    // MONOTONIC: the loop compares with the count it had last time.
    // A notification arriving DURING a reconciliation is therefore seen at the
    // next poll — which a swapped boolean would lose, and it is exactly
    // the notification that matters, the one that arrives while the disk is already being read.
    let mut notifications_vues = veille.notifications();
    let mut declencheur = Declencheur::Demarrage;

    loop {
        // 🔴 E5 — THE FLAG IS READ AND LOWERED **BEFORE** THE RECONCILIATION.
        //
        // ⚠️ THIS COMMENT REPLACES ONE THAT NAMED EXACTLY THE DEFECT THAT
        // ITS OWN CODE PRODUCED. It said: "the flag is lowered AFTER
        // the reconciliation, not before: the installation thread waits for
        // `reconciliee`, and raising it too early would make it read a count taken
        // before the installer had finished writing." The NOMINAL path was
        // correct — the flag is raised when the installer exits, the wait
        // is broken, and the reconciliation that follows did start AFTER.
        //
        // 🔴 THE RACE PATH WAS NOT. If a periodic
        // reconciliation was ALREADY RUNNING when the installer exited and raised
        // the flag, the `swap` placed after it saw it true and declared
        // `reconciliee` — **for a reconciliation started BEFORE the
        // installer had finished writing**. `forcer_une_reconciliation`
        // then returned immediately, and the verdict was read on a
        // window that had not seen the last files: a FALSE `sans-effet`,
        // that is precisely what the old comment claimed it
        // wanted to prevent.
        //
        // 🔴 WHY IT BELONGS TO G4: the race window was "duration of the
        // reconciliation / period", i.e. ≈ 0.2 % of installer exits.
        // G4 makes reconciliations far more frequent during an
        // installation — that is its whole purpose — and therefore WIDENS this window
        // by an order of magnitude.
        //
        // Read beforehand, the request SURVIVES the current tick: the NEXT
        // reconciliation honours it, one period later but CORRECTLY. None of the three
        // paths loses the request.
        //
        // ⚠️ ITS ONLY PROOF IS A CONTROL-FLOW ARGUMENT. This file is
        // `#[cfg(windows)]`, no host test can reach it, and the
        // G4 acceptance run launches no installation. It is the exact situation
        // that defect F1 of sub-block D7 paid for. The fix is applied
        // because it is STRICTLY SAFER; **the measurement is BEQUEATHED, and
        // declared missing.**
        let demandee = partage
            .reconcilier
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        let contexte = Contexte {
            declencheur,
            notifications: veille.notifications(),
            debordements: veille.debordements(),
        };
        let (diff, catalogue) = reconcilier(&mut memoire, contexte);
        // 🔴 THE COUNT GOES TO ALL OPEN WINDOWS, AND IT IS TAKEN HERE —
        // before `diff.apparues` is consumed by the delta branch.
        //
        // ⚠️ IT IS ADDED EVEN WHEN THE CATALOGUE GOES OUT COMPLETE: `complet` only
        // changes what is EMITTED, never what the diff contains. The
        // memory is not reset at a re-enrolment, so
        // `diff.apparues` remains the real novelty — counting it twice
        // would be the defect, not counting it would be another one.
        if !diff.apparues.is_empty() {
            if let Ok(mut f) = partage.fenetres.lock() {
                f.ajouter(diff.apparues.len());
            }
        }
        // The request read BEFORE the tick is honoured AFTER it: it really is
        // this reconciliation, started after the installer exited,
        // that `reconciliee` announces.
        if demandee {
            partage
                .reconciliee
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if complet {
            canal_emission(VersLaPlateforme::catalogue(true, catalogue, Vec::new()));
            complet = false;
        } else if !diff.est_vide() {
            // ⚠️ NOTHING IS EMITTED WHEN NOTHING MOVED, and that is the whole point
            // of the diff: on an idle disk, the loop is silent on the
            // channel as well as in the log of discarded entries.
            let mut applications = diff.apparues;
            applications.extend(diff.modifiees);
            canal_emission(VersLaPlateforme::catalogue(
                false,
                applications,
                diff.disparues,
            ));
        }

        // Wait for the period, while staying responsive to orders,
        // re-enrolments AND the watcher: sleeping blindly for thirty seconds
        // would make a user's click wait up to half a minute.
        //
        // 🔴 IN `Mode::Seule`, THE PERIOD DEADLINE DOES NOT EXIST. It is an
        // `Option`, not an absurdly long period: a period of
        // a thousand years would be a lie carried by the code, and the day
        // someone read it they would believe it was a setting. **BENCH variable, and
        // the only setup that makes criterion ③ discriminating** — a
        // purely event-driven agent loses everything a missed notification
        // carries away.
        let echeance = mode.periodique().then(|| Instant::now() + periode);
        loop {
            let maintenant = Instant::now();
            if let Some(echeance) = echeance {
                if echeance.saturating_duration_since(maintenant).is_zero() {
                    declencheur = Declencheur::Periode;
                    break;
                }
            }
            // 🔴 "RECONCILE NOW" SHORT-CIRCUITS THE WAIT. Without
            // it, the verdict of a ten-second installation would arrive
            // up to thirty seconds later, and the user would see a
            // finished bar next to an "in progress" state.
            if partage
                .reconcilier
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                declencheur = Declencheur::Installation;
                break;
            }
            // 🔴 POLLING THE WATCHER, INSIDE THE WAIT THAT ALREADY
            // EXISTED: the loop gains NO thread. That is what makes G4 an
            // acceleration and not one more architecture.
            let notifications = veille.notifications();
            if notifications != notifications_vues {
                notifications_vues = notifications;
                if mode.rebond() {
                    rebond.notifier(maintenant);
                } else {
                    // `sans-rebond`: any notification breaks the wait.
                    // ⚠️ IT IS NOT "DEBOUNCE VERSUS NOTHING": the
                    // polling has a 200 ms granularity, which is already a
                    // weak debounce. The RED of criterion ④ therefore measures
                    // "debounce versus 200 ms", and its statement must
                    // say so.
                    declencheur = Declencheur::Notification;
                    break;
                }
            }
            if rebond.du(maintenant) {
                rebond.consommer();
                declencheur = Declencheur::Notification;
                break;
            }
            match ordres.try_recv() {
                Ok(Ordre::Lancer { demande, cle }) => {
                    let issue = honorer(&memoire, &demande, &cle);
                    canal_emission(VersLaPlateforme::lancee(demande, issue));
                    continue;
                }
                Ok(Ordre::IconesManquantes { empreintes }) => {
                    icone::televersement::honorer(
                        &memoire.icones,
                        &empreintes,
                        &base_plateforme,
                        &identite,
                    );
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::warn!("canal /agent fermé : découverte d'applications arrêtée");
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            if identite.has_changed().unwrap_or(false) {
                identite.borrow_and_update();
                // 🔴 D7 — THIS TRACE LIED ABOUT ITS NAME, AND IT IS THE
                // THIRD TIME IN THIS SUB-PROJECT. It said
                // "reenrolement observe"; **no re-enrolment takes place**.
                // MEASURED on 21 August 2026 on the log of an idle agent:
                // ELEVEN lines for TWELVE reconciliations. What makes the
                // `watch` move is the heartbeat's TOKEN REFRESH — and
                // `PERIODE_BATTEMENT` is exactly `PERIODE_RECONCILIATION`,
                // **by coincidence and not by derivation**: they are two
                // independent constants, which would be recalibrated separately.
                //
                // After `retenus` (G1 legacy no. 7) and `icones_echouees` (G2
                // defect ②), it is the third counter of ④ to lie about its
                // name. It now says what it OBSERVES, and declares what
                // it cannot tell apart.
                tracing::info!(
                    "identité changée : le prochain catalogue sera COMPLET \
                     (rafraîchissement de jeton OU réenrôlement — cette boucle ne les distingue pas)"
                );
                complet = true;
            }
            // 🔴 THE POLLING GRANULARITY IS 200 ms, AND IT IS WHAT
            // BOUNDS ANY DEBOUNCE FROM BELOW — a shorter delay would not
            // be observable, it would be absorbed here. It is also the second
            // term of the worst case derived from `DELAI_ANTI_REBOND_MAX`.
            //
            // The wait is shortened to the nearest of the three deadlines:
            // the period, the debounce, and this step. Without the debounce in this `min`,
            // a debounce deadline falling just after a wake-up
            // would wait 200 ms more — measurable, and free to avoid.
            let mut attente = std::time::Duration::from_millis(200);
            if let Some(echeance) = echeance {
                attente = attente.min(echeance.saturating_duration_since(maintenant));
            }
            if let Some(du) = rebond.echeance() {
                attente = attente.min(du.saturating_duration_since(maintenant));
            }
            // ⚠️ NEVER ZERO: a zero wait would make this thread spin
            // on a whole core. Both deadlines above are tested at
            // the top of the loop, so a zero wait means "due this very
            // instant", and one millisecond is enough to hand control back.
            std::thread::sleep(attente.max(std::time::Duration::from_millis(1)));
        }
    }
}
