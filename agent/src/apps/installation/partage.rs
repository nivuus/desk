//! The state the installation thread and the discovery loop share.
//!
//! 🔴 THIS MODULE HAS NO `cfg`, AND THAT IS THE POINT. The installation thread is
//! Windows; what it shares with discovery is not, and putting it
//! behind the `cfg` would force `apps::brancher` — which has a host variant — to
//! carry two signatures. It is also what makes these two mechanisms
//! observable on the host.
//!
//! 🔴 THERE ARE ONLY TWO POINTS OF CONTACT BETWEEN `apps` AND `installation`, and
//! here they both are. `apps::boucle` has nothing else to know about
//! installations — that is what lets decision D11 hold: two
//! families, two consumers, each with a single one.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::fenetre::Fenetres;

/// What the thread shares with the discovery loop.
#[derive(Clone)]
pub struct Partage {
    /// The open windows. The loop pours `diff.apparues.len()` into them at every
    /// reconciliation; the thread opens and closes them.
    pub fenetres: Arc<Mutex<Fenetres>>,
    /// 🔴 "RECONCILE NOW". When the installer exits, the agent
    /// **forces** a reconciliation rather than waiting for the next thirty
    /// seconds: without it, the verdict of a ten-second installation
    /// would arrive half a minute later.
    ///
    /// ⚠️ **THE FUTURE IS PAST** (sub-block G4): this sentence said "this is what
    /// G4 WILL MAKE immediate", and `apps::surveillance` exists. **G3 did not
    /// depend on it** — its window works at `PERIODE_RECONCILIATION` —, and
    /// that is exactly the guarantee the ordering of the sub-blocks existed to
    /// preserve.
    ///
    /// 🔴 **WHAT G4 FOUND HERE, AND IT WAS NOT A SPEED-UP**: the
    /// `reconcilier` / `reconciliee` handshake carried a RACE. The
    /// flag was read and lowered **after** the reconciliation, so that a
    /// periodic reconciliation ALREADY RUNNING when the installer exited
    /// declared `reconciliee` for a round started BEFORE the installer
    /// had finished writing — a FALSE `sans-effet`. The window was ≈ 0.2 % of
    /// installer exits, and **G4 would have widened it by an order of magnitude**
    /// since its whole purpose is to make reconciliations more frequent
    /// during an installation. Fixed in `apps/boucle.rs`, which carries the
    /// detail.
    ///
    /// ⚠️ **THE FIX HAS ONLY A CONTROL-FLOW ARGUMENT AS PROOF**:
    /// `boucle.rs` is `#[cfg(windows)]`, and the G4 acceptance run launches no
    /// installation. Measurement HANDED OVER, and declared missing.
    pub reconcilier: Arc<AtomicBool>,
    /// Set by the thread when a forced reconciliation has taken place.
    pub reconciliee: Arc<AtomicBool>,
}

impl Partage {
    pub fn neuf() -> Self {
        Self {
            fenetres: Arc::new(Mutex::new(Fenetres::nouvelles())),
            reconcilier: Arc::new(AtomicBool::new(false)),
            reconciliee: Arc::new(AtomicBool::new(false)),
        }
    }
}
