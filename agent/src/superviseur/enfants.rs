//! Launch one agent process per window, and know which one died.
//!
//! Launching goes through a trait: it is what makes the bookkeeping — who
//! runs, who just died, who was already signalled — exercisable on the
//! host, although it carries the errors that would leak a virtual
//! output.

use std::collections::HashMap;

use anyhow::Result;

use super::table::IdSession;

/// What a child must know to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consigne {
    pub session: IdSession,
    /// The window's `HWND`, as a raw address — an `HWND` is not
    /// `Send` in windows-rs 0.62, and it is an opaque identifier, not a
    /// dereferenced pointer.
    pub fenetre: u64,
    /// DXGI name of the output (`\\.\DISPLAYn`), stable — unlike a
    /// positional pair of enumeration indexes.
    pub nom_sortie: String,
    /// The RETAINED size (`placement::retained_size`) at which the table
    /// put this window — not the output's size, which can be much
    /// larger. Set on the child through `TAILLE_FENETRE` (`lanceur.rs`), (policy: allow-fr, env var name)
    /// so that it repeats it to the capturer at attach time (task 9 of sub-block
    /// D10), which will need it to crop (task 8).
    pub size: (u32, u32),
}

pub trait Lanceur {
    /// Starts an agent process for this window.
    ///
    /// Returns the PID of the started process. The **contract is atomic**:
    /// `Err` means no process was started. Violating this contract
    /// — that is, returning `Err` *after* really launching the child,
    /// for example when a post-processing step or a wait for a readiness
    /// signal fails — leaves the process running untracked. It becomes
    /// **untraceable**: neither `morts()` nor `tuer()` will find it.
    /// The child occupies a virtual display output that will stay **captive**
    /// until supervisor shutdown, and the driver's pool limited to ten outputs
    /// empties for nothing.
    ///
    /// **If the implementation cannot hold this contract** — in particular if
    /// post-processing after launch can fail — it must
    /// **kill the process it just started itself before
    /// returning `Err`**, so that no child stays alive on
    /// error.
    fn lancer(&self, consigne: &Consigne) -> Result<u32>;
    fn est_vivant(&self, pid: u32) -> bool;
    fn tuer(&self, pid: u32) -> Result<()>;
}

pub struct Enfants<'l> {
    lanceur: &'l dyn Lanceur,
    vivants: HashMap<IdSession, u32>,
}

impl<'l> Enfants<'l> {
    pub fn nouveaux(lanceur: &'l dyn Lanceur) -> Self {
        Self {
            lanceur,
            vivants: HashMap::new(),
        }
    }

    pub fn lancer(&mut self, consigne: Consigne) -> Result<()> {
        let pid = self.lanceur.lancer(&consigne)?;
        tracing::info!(
            session = %consigne.session.0,
            pid,
            sortie = %consigne.nom_sortie,
            "child launched"
        );
        self.vivants.insert(consigne.session, pid);
        Ok(())
    }

    /// Removes the session from the bookkeeping **before** killing: whatever
    /// happens to the kill, this session must no longer come out as
    /// "dead" and get its output destroyed a second time.
    ///
    /// **The kill is immediate, without a prior graceful stop, and it is
    /// deliberate.** The spec speaks of a child "killed after a bounded delay": that
    /// delay would be that of a clean stop we would wait for. Yet the clean stop
    /// of an agent goes through destroying its encoder, where `IMFShutdown::
    /// Shutdown` is bounded by nothing and where a hang was observed (1 time in 6
    /// at N=4, cause not attributed). Waiting for that stop reintroduces into
    /// the supervisor the very risk multi-process avoids. The Windows
    /// window has already disappeared when we get here: the child has nothing left to
    /// save, and the system reclaims its resources.
    pub fn tuer(&mut self, session: &IdSession) {
        let Some(pid) = self.vivants.remove(session) else {
            return;
        };
        if let Err(error) = self.lanceur.tuer(pid) {
            tracing::warn!(session = %session.0, pid, %error, "killing the child failed");
        }
    }

    /// Sessions whose process disappeared since the last call.
    ///
    /// They leave the bookkeeping along the way: a death is only signalled
    /// once.
    pub fn morts(&mut self) -> Vec<IdSession> {
        let morts: Vec<IdSession> = self
            .vivants
            .iter()
            .filter(|(_, pid)| !self.lanceur.est_vivant(**pid))
            .map(|(session, _)| session.clone())
            .collect();
        for session in &morts {
            let pid = self.vivants.remove(session);
            tracing::warn!(session = %session.0, ?pid, "child died by itself");
        }
        morts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Fake launcher: keeps what it is asked, without launching any
    /// process. It is what makes this machinery exercisable on the host.
    #[derive(Default)]
    struct LanceurFactice {
        lancees: RefCell<Vec<Consigne>>,
        tues: RefCell<Vec<u32>>,
        vivants: RefCell<Vec<u32>>,
        prochain_pid: RefCell<u32>,
    }

    impl Lanceur for LanceurFactice {
        fn lancer(&self, consigne: &Consigne) -> anyhow::Result<u32> {
            let mut pid = self.prochain_pid.borrow_mut();
            *pid += 1;
            self.lancees.borrow_mut().push(consigne.clone());
            self.vivants.borrow_mut().push(*pid);
            Ok(*pid)
        }
        fn est_vivant(&self, pid: u32) -> bool {
            self.vivants.borrow().contains(&pid)
        }
        fn tuer(&self, pid: u32) -> anyhow::Result<()> {
            self.tues.borrow_mut().push(pid);
            self.vivants.borrow_mut().retain(|p| *p != pid);
            Ok(())
        }
    }

    /// Launcher that fails immediately, without starting any process.
    /// Used to test that a launcher error leaves nothing in the
    /// bookkeeping.
    struct LanceurEchec;

    impl Lanceur for LanceurEchec {
        fn lancer(&self, _consigne: &Consigne) -> anyhow::Result<u32> {
            Err(anyhow::anyhow!("simulated launch failed"))
        }
        fn est_vivant(&self, _pid: u32) -> bool {
            false
        }
        fn tuer(&self, _pid: u32) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn consigne(session: &str) -> Consigne {
        Consigne {
            session: IdSession(session.into()),
            fenetre: 0x1234,
            nom_sortie: "\\\\.\\DISPLAY1".into(),
            size: (1280, 720),
        }
    }

    #[test]
    fn launching_passes_the_instruction_to_the_launcher() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1")).unwrap();
        assert_eq!(lanceur.lancees.borrow().len(), 1);
        assert_eq!(lanceur.lancees.borrow()[0].session, IdSession("w-1".into()));
    }

    #[test]
    fn killing_requests_the_death_of_the_right_process() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1")).unwrap();
        enfants.lancer(consigne("w-2")).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert_eq!(*lanceur.tues.borrow(), vec![1]);
    }

    #[test]
    fn dead_returns_the_sessions_whose_process_vanished() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1")).unwrap();
        enfants.lancer(consigne("w-2")).unwrap();
        lanceur.vivants.borrow_mut().retain(|p| *p != 1);

        assert_eq!(enfants.morts(), vec![IdSession("w-1".into())]);
    }

    #[test]
    fn a_dead_session_is_reported_only_once() {
        // Without this guarantee, the supervisor would destroy the output a
        // first time then request its destruction again at each loop
        // turn, and the log would fill with failures.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1")).unwrap();
        lanceur.vivants.borrow_mut().clear();

        assert_eq!(enfants.morts().len(), 1);
        assert!(enfants.morts().is_empty());
    }

    #[test]
    fn killing_an_unknown_session_does_nothing() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.tuer(&IdSession("w-jamais-lancee".into()));
        assert!(lanceur.tues.borrow().is_empty());
    }

    #[test]
    fn a_killed_session_does_not_show_up_among_the_dead() {
        // It was already handled by the `fenetre_disparue` path: signalling
        // it dead would get its output destroyed a second time.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1")).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert!(enfants.morts().is_empty());
    }

    #[test]
    fn failed_launch_leaves_nothing_in_the_accounting() {
        // Atomic contract of the Lanceur trait: Err ⇒ no process is running.
        // If this contract is violated — launching fails after having really
        // started the child — the child becomes untraceable, the virtual output
        // it occupies stays captive, and the driver's pool of ten empties
        // for nothing. This test fixes the contract: a launch error must
        // leave no trace.
        let lanceur = LanceurEchec;
        let mut enfants = Enfants::nouveaux(&lanceur);
        let err = enfants.lancer(consigne("w-1"));
        assert!(err.is_err());
        // No session registered.
        assert!(enfants.morts().is_empty());
    }
}
