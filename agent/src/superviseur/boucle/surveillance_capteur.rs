//! Launching and supervising the single pooled-capture capturer.
//!
//! **`surveillance_capteur` and not `capteur`** (I7 of the branch's final
//! review of sub-block D4). `crate::capteur` already exists and designates SOMETHING
//! ELSE: the capturer itself, that is, the process holding the N
//! DXGI duplications and the N encoders. This module is only its
//! supervision, seen from the supervisor — it captures nothing. Two `capteur`s in
//! the same module graph, one of which does `use super::*`, were only waiting
//! for a hurried reader to be confused.
//!
//! Extracted from `boucle.rs` (task 7 of sub-block D4) to stay under the
//! project's 500-line ceiling — not for a design reason: this
//! logic is part of the loop like the others, in the same logical
//! module, just in a neighbouring file. Same scheme as
//! `placement_periodique.rs`.

use anyhow::Context;

use super::*;

/// Minimum spacing between two capturer restart attempts.
///
/// Without this bound, a capturer dying RIGHT AFTER being restarted
/// would make `lancer_capteur` be retried — hence `Command::spawn`, a real process
/// — at the loop's cadence: up to several times per second as long
/// as effects remain to be handled. The repository has already paid twice the cost
/// of a mere log LINE emitted at this rate on a CIFS share
/// (fix I2 of `lanceur.rs`, and the TURN work item before it); restarting a
/// process at this cadence would be worse. This constant only spaces
/// the attempts, it never prevents them: the capturer keeps being retried
/// indefinitely as long as it does not come back.
const PERIODE_RELANCE_CAPTEUR_MIN: std::time::Duration = std::time::Duration::from_millis(500);

/// What the loop keeps of the capturer from one turn to the next: the PID of the
/// last successful attempt (to log a restart with the dead PID
/// AND the new PID) and the timestamp of the last attempt (to space it out).
pub(super) struct EtatCapteur {
    pid: u32,
    derniere_tentative: std::time::Instant,
    /// True as soon as a restart cycle in progress has been signalled.
    ///
    /// **Fix I3 of the branch's final review.**
    /// `PERIODE_RELANCE_CAPTEUR_MIN` spaces the `spawn`s, not the LINES: a
    /// capturer dying again right after each restart produced two
    /// lines per second, indefinitely, on the CIFS share — i.e. about
    /// 170,000 per day. It is exactly the risk this constant's documentation
    /// invokes, and it only covered half of it.
    ///
    /// Same pattern as `Enfant::etat_illisible_signale` (`superviseur/lanceur.rs`,
    /// fix I2): signal the first time, keep quiet as long as the
    /// situation repeats identically, become noisy again as soon as it stops.
    /// What is lost is the COUNT of restarts; what is kept is the
    /// fact that a cycle started — and the first return to normal is
    /// logged, which bounds the silence.
    cycle_signale: bool,
}

impl EtatCapteur {
    /// Launches the capturer, BEFORE any window: it is the one serving the
    /// media to any child that attaches, and a child launched without a capturer
    /// facing it would capture into the void.
    ///
    /// No atomic contract to undo here, unlike `lancer_capteur`
    /// itself: a failure of THIS call is fatal to the supervisor, just
    /// like a driver or a hook that does not open — there is nothing
    /// else to clean up.
    pub(super) fn start(lanceur: &LanceurDeProcessus) -> Result<Self> {
        let pid = lanceur
            .lancer_capteur()
            .context("lancement initial du capteur")?;
        Ok(Self {
            pid,
            derniere_tentative: std::time::Instant::now(),
            cycle_signale: false,
        })
    }

    /// Restarts the capturer if it is dead, at most once per
    /// `PERIODE_RELANCE_CAPTEUR_MIN`, and logs each successful restart
    /// with the dead PID and the new PID.
    ///
    /// **Never closes any window.** Children hold on their
    /// resumption window (15 s, see `capteur::distante`) and reattach
    /// by themselves to the restarted capturer — it is acceptance criterion no. 2 of
    /// sub-block D4, and closing it on the supervisor side would make it fail by
    /// construction. This method therefore does nothing but restart and
    /// log: no call to `enfants.tuer` nor to a table effect
    /// has its place here.
    /// **Logs the first turn of a restart cycle, then keeps quiet**
    /// (fix I3, see `cycle_signale`): the attempts, for their part, never
    /// stop.
    pub(super) fn surveiller(&mut self, lanceur: &LanceurDeProcessus) {
        if lanceur.capteur_vivant() {
            // The capturer SURVIVED its restart period: the cycle is
            // broken, a later death will be new information.
            //
            // The duration condition is not decorative. The supervisor
            // loop runs at ~10 Hz while `PERIODE_RELANCE_CAPTEUR_MIN`
            // is 500 ms: a capturer living two or three loop
            // turns before dying would be seen alive at least once between
            // two restarts, which would re-arm the signalling at each cycle
            // and make the safeguard ineffective. Requiring it to hold at least
            // as long as the restart spacing is what
            // distinguishes "it is coming back" from "it is dying in a loop".
            if self.cycle_signale
                && self.derniere_tentative.elapsed() >= PERIODE_RELANCE_CAPTEUR_MIN
            {
                self.cycle_signale = false;
                tracing::info!(pid = self.pid, "capteur de nouveau stable");
            }
            return;
        }
        if self.derniere_tentative.elapsed() < PERIODE_RELANCE_CAPTEUR_MIN {
            return;
        }
        self.derniere_tentative = std::time::Instant::now();
        // Read BEFORE the attempt, and armed whatever happens: both outcomes
        // below log, and both must keep quiet at the next turn
        // if the cycle continues.
        let premier_du_cycle = !self.cycle_signale;
        self.cycle_signale = true;
        match lanceur.lancer_capteur() {
            Ok(new) => {
                if premier_du_cycle {
                    tracing::warn!(
                        pid_mort = self.pid,
                        pid_neuf = new,
                        "capteur mort, relancé (relances suivantes silencieuses \
                         tant que le cycle se répète)"
                    );
                }
                self.pid = new;
            }
            Err(error) => {
                // `self.pid` is NOT updated: it stays the last known
                // value, so that the next successful restart
                // logs an exact "dead" one — `lancer_capteur` only returns
                // `Ok` atomically, a failure never made a process
                // live (see its doc).
                if premier_du_cycle {
                    tracing::error!(
                        %error,
                        "relance du capteur échouée — retentée indéfiniment passé le délai \
                         minimal, et silencieusement tant que l'échec se répète"
                    );
                }
            }
        }
    }
}
