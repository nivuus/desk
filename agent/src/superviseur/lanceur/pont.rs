//! Launching and life control of the **files bridge** process.
//!
//! Written in a child module rather than inline in `lanceur.rs`
//! (367 lines, margin 133): the twins of `lancer_capteur` and
//! `capteur_vivant`, in this file's documentary style, would have weighed
//! 80 to 100 lines there and brought the margin under 45. This repository paid four times for the
//! lesson "the margin regained by an extraction is lost again the next round
//! if treated as acquired" — here the margin is not taken at all.
//! The extraction is placed **before** the addition that would make it necessary,
//! the only gesture that has worked in this repository (D9, `capteur/serveur/instances.rs`,
//! margin returned from 10 to 65; the two files handled after the fact in D9 were
//! **compressed**, a gesture `CLAUDE.md` forbids by name, then extracted
//! anyway).
//!
//! **Same ATOMIC contract as `lancer_capteur`**: `Err` means no
//! process is running. Attaching to the job is the only fallible
//! post-processing, and it therefore kills the bridge itself before returning `Err`.
//!
//! `#![cfg(windows)]` inherited from `lanceur.rs` (its `#![cfg(windows)]` covers
//! the whole module, children included): no `#[path]` is written here, and
//! `CLAUDE.md`'s "child module convention" does not apply — it only
//! targets modules extracted from a gated parent to make them compile
//! on the host, which this one has no reason to be.

use super::*;
use crate::relance_pont::EtatObserve;

impl LanceurDeProcessus {
    /// Launches the single files bridge (`agent/src/pont.rs`): same executable,
    /// `PONT=1`, **attached to the same job object** as the children and the capturer.
    ///
    /// **Why it carries `SESSION_ID`, `SIGNALING_URL` and `LOCAL_IP` where
    /// `lancer_capteur` sets none**: the capturer talks to no
    /// signaling — it serves the media through a named pipe —, whereas the bridge opens
    /// its own `PeerConnection` to the shell page.
    pub fn lancer_pont(&self) -> Result<u32> {
        // Composed HERE and not on the child side, exactly as a child's
        // `Consigne` is by `Table`: it is the supervisor that knows the
        // VM's prefix. The bridge, for its part, will enrol for its own TOKEN —
        // the prefix it will get from it does not have to recompose this name.
        let session = crate::superviseur::protocole::session_du_pont(&self.prefixe);
        let mut commande = std::process::Command::new(&self.executable);
        commande
            .env("PONT", "1")
            .env("SESSION_ID", &session)
            .env("SIGNALING_URL", &self.signaling_url)
            .env("LOCAL_IP", &self.local_ip)
            // A bridge inheriting `SUPERVISEUR` would take itself for a
            // supervisor and launch its own children, indefinitely.
            .env_remove("SUPERVISEUR")
            // A bridge inheriting `CAPTEUR` would take itself for a
            // CAPTURER, and not for a bridge: `main.rs` tests `CAPTEUR` BEFORE
            // everything else and would hand control to `capteur::executer`. The bridge
            // would never start, **without a single line saying so** —
            // and the `Mes Fichiers` drive would stay absent with no readable (policy: allow-fr, real Windows folder name)
            // cause.
            .env_remove("CAPTEUR")
            // Same reason as for a child and for the capturer: these two
            // variables change the MEANING of a source. The bridge has no
            // source at all, and `scripts/run-agent.sh` sets `TEST_FILE` as soon as
            // it is defined in the calling environment.
            .env_remove("TEST_FILE")
            .env_remove("WINDOW_TITLE");
        // 🔴 **AND THE IDENTITY — IT IS THE DEFECT MEASURED ON AUGUST 20TH, 2026, AND IT
        // WAS IN THIS VERY LIST.** `AGENT_VM` and `AGENT_SECRET` were not
        // in it: the bridge enrolled under the SAME identity as its
        // parent, the platform only admits one socket per VM, and the two
        // evicted each other endlessly — **95 enrolments, 94 evictions in 64 s**,
        // at ~1.5 Hz, at the price of lost incremental messages, lost
        // launch responses, and two application discovery loops
        // instead of one.
        //
        // ⚠️ **THE BRIDGE DOES NEED AN IDENTITY — but a TOKEN, NOT
        // A CHANNEL.** It opens its own `PeerConnection` to the shell page
        // (that is what `main.rs` says, and why it is placed AFTER
        // enrolment), so it presents a token like a child. What it
        // NEVER does with the channel, on the other hand: it does not beat the VM's
        // heart, pushes no catalogue, receives no launch order.
        // The question "own identity, or no enrolment at all?" is therefore
        // decided on this real use: **no enrolment**, and the parent's
        // token through `AGENT_JETON`.
        self.identite_heritee(&mut commande);
        let mut pont = commande.spawn().context("launching the file bridge")?;
        let pid = pont.id();
        let handle = HANDLE(pont.as_raw_handle());
        if let Err(error) = unsafe { AssignProcessToJobObject(self.job, handle) } {
            // Same atomic contract as `lancer` and `lancer_capteur`: a bridge
            // not attached to the job would outlive the supervisor WHILE HOLDING a
            // ProjFS virtualisation root, which nothing would unmount.
            if let Err(mise_a_mort) = pont.kill() {
                tracing::error!(pid, %mise_a_mort,
                    "file bridge NOT attached to the job AND NOT killed — it will outlive the supervisor");
            }
            let _ = pont.wait();
            return Err(anyhow::Error::new(error)
                .context(format!("attaching file bridge {pid} to the job object")));
        }
        tracing::info!(pid, session, "file bridge launched");
        *self.pont() = Some(Enfant {
            processus: pont,
            etat_illisible_signale: false,
        });
        Ok(pid)
    }

    /// What the supervisor observes of the bridge launched by the last successful
    /// `lancer_pont`: alive, dead with an OUTCOME, or absent.
    ///
    /// 🔴 **IT IS THE OUTCOME, AND NO LONGER A SINGLE BOOLEAN, THAT CROSSES THIS
    /// BOUNDARY SINCE FIX ROUND 4.** The former `pont_vivant`
    /// already received the exit code in `Ok(Some(code))` — **to
    /// log it and drop it** —, and `surveillance_pont.rs`
    /// then had to GUESS, from the lifetime alone, whether a past failure
    /// was resolved. It could not: a healthy session that
    /// ends occupies the same interval as a refused bridge that slept
    /// up to `REPLI_MAX_MS` before dying. See the header doc of
    /// `crate::relance_pont` for the measurement that established it.
    ///
    /// 🔴 **`Mort` IS RETURNED ONLY ONCE PER DEATH, AND ALL OF `relance_pont`
    /// DEPENDS ON IT**: `*pont = None` right after reading the code makes
    /// the next turn return `Absent`. Returning `Mort(Propre)` at each turn
    /// would re-arm the fallback in a loop on an OLD clean exit, which
    /// is exactly the defect this round closes. **This property lives
    /// behind `#[cfg(windows)]` and is exercised by no test** — it
    /// is named in both files rather than assumed.
    ///
    /// Same logic as `capteur_vivant`: an unreadable state is **NOT**
    /// treated as a death — declaring it dead would restart a bridge that
    /// may still be running, and two bridges would contend for the same virtualisation
    /// root — and is only logged once as long as it persists.
    pub fn etat_du_pont(&self) -> EtatObserve {
        let mut pont = self.pont();
        let Some(en_cours) = pont.as_mut() else {
            return EtatObserve::Absent;
        };
        match en_cours.processus.try_wait() {
            Ok(None) => {
                en_cours.etat_illisible_signale = false;
                EtatObserve::Vivant
            }
            Ok(Some(code)) => {
                // ⚠️ `code.code()` and not `code.success()`: the pure module
                // distinguishes THREE outcomes, not two — see `IssueDeSortie`,
                // whose `Inconnue` variant covers the `None` POSIX returns
                // on a death by signal. On Windows the `None` never
                // runs, and `IssueDeSortie::depuis_le_code` says so.
                let issue = crate::relance_pont::IssueDeSortie::depuis_le_code(code.code());
                tracing::info!(
                    pid = en_cours.processus.id(),
                    ?code,
                    ?issue,
                    "file bridge finished"
                );
                *pont = None;
                EtatObserve::Mort(issue)
            }
            Err(error) => {
                if !en_cours.etat_illisible_signale {
                    en_cours.etat_illisible_signale = true;
                    tracing::warn!(
                        %error,
                        "file bridge state unreadable, taken as alive \
                         (reported only once as long as the state stays unreadable)"
                    );
                }
                EtatObserve::Vivant
            }
        }
    }
}
