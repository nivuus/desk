//! Lancer un processus agent par fenêtre, et savoir lequel est mort.
//!
//! Le lancement passe par un trait : c'est ce qui rend la comptabilité — qui
//! tourne, qui vient de mourir, qui a déjà été signalé — éprouvable sur
//! l'hôte, alors qu'elle porte les erreurs qui feraient fuir une sortie
//! virtuelle.

use std::collections::HashMap;

use anyhow::Result;

use super::table::IdSession;

/// Ce qu'un enfant doit savoir pour démarrer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consigne {
    pub session: IdSession,
    /// `HWND` de la fenêtre, sous forme d'adresse brute — un `HWND` n'est pas
    /// `Send` en windows-rs 0.62, et c'est un identifiant opaque, pas un
    /// pointeur déréférencé.
    pub fenetre: u64,
    pub index_adaptateur: u32,
    pub index_sortie: u32,
    /// Vrai pour la seule fenêtre porteuse du son.
    pub audio: bool,
}

pub trait Lanceur {
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
        Self { lanceur, vivants: HashMap::new() }
    }

    pub fn lancer(&mut self, consigne: Consigne) -> Result<()> {
        let pid = self.lanceur.lancer(&consigne)?;
        tracing::info!(
            session = %consigne.session.0,
            pid,
            sortie = format!("{}:{}", consigne.index_adaptateur, consigne.index_sortie),
            audio = consigne.audio,
            "enfant lancé"
        );
        self.vivants.insert(consigne.session, pid);
        Ok(())
    }

    /// Retire la session de la comptabilité **avant** de tuer : quoi qu'il
    /// advienne de la mise à mort, cette session ne doit plus ressortir comme
    /// « morte » et faire détruire sa sortie une seconde fois.
    ///
    /// **La mise à mort est immédiate, sans arrêt gracieux préalable, et c'est
    /// délibéré.** La spec parle d'un enfant « tué après un délai borné » : ce
    /// délai serait celui d'un arrêt propre qu'on attendrait. Or l'arrêt propre
    /// d'un agent passe par la destruction de son encodeur, où `IMFShutdown::
    /// Shutdown` n'est borné par rien et où un gel a été observé (1 fois sur 6
    /// à N=4, cause non attribuée). Attendre cet arrêt, c'est réintroduire dans
    /// le superviseur le risque même que le multi-processus écarte. La fenêtre
    /// Windows a déjà disparu quand on arrive ici : l'enfant n'a plus rien à
    /// sauvegarder, et le système récupère ses ressources.
    pub fn tuer(&mut self, session: &IdSession) {
        let Some(pid) = self.vivants.remove(session) else {
            return;
        };
        if let Err(erreur) = self.lanceur.tuer(pid) {
            tracing::warn!(session = %session.0, pid, %erreur, "mise à mort de l'enfant échouée");
        }
    }

    /// Sessions dont le processus a disparu depuis le dernier appel.
    ///
    /// Elles quittent la comptabilité au passage : une mort ne se signale
    /// qu'une fois.
    pub fn morts(&mut self) -> Vec<IdSession> {
        let morts: Vec<IdSession> = self
            .vivants
            .iter()
            .filter(|(_, pid)| !self.lanceur.est_vivant(**pid))
            .map(|(session, _)| session.clone())
            .collect();
        for session in &morts {
            let pid = self.vivants.remove(session);
            tracing::warn!(session = %session.0, ?pid, "enfant mort de lui-même");
        }
        morts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Lanceur factice : retient ce qu'on lui demande, sans lancer aucun
    /// processus. C'est ce qui rend cette machinerie éprouvable sur l'hôte.
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

    fn consigne(session: &str, audio: bool) -> Consigne {
        Consigne {
            session: IdSession(session.into()),
            fenetre: 0x1234,
            index_adaptateur: 0,
            index_sortie: 1,
            audio,
        }
    }

    #[test]
    fn lancer_transmet_la_consigne_au_lanceur() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        assert_eq!(lanceur.lancees.borrow().len(), 1);
        assert!(lanceur.lancees.borrow()[0].audio);
    }

    #[test]
    fn tuer_demande_la_mise_a_mort_du_bon_processus() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.lancer(consigne("w-2", false)).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert_eq!(*lanceur.tues.borrow(), vec![1]);
    }

    #[test]
    fn morts_rend_les_sessions_dont_le_processus_a_disparu() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.lancer(consigne("w-2", false)).unwrap();
        lanceur.vivants.borrow_mut().retain(|p| *p != 1);

        assert_eq!(enfants.morts(), vec![IdSession("w-1".into())]);
    }

    #[test]
    fn une_session_morte_n_est_signalee_qu_une_fois() {
        // Sans cette garantie, le superviseur détruirait la sortie une
        // première fois puis en redemanderait la destruction à chaque tour
        // de boucle, et le journal se remplirait d'échecs.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        lanceur.vivants.borrow_mut().clear();

        assert_eq!(enfants.morts().len(), 1);
        assert!(enfants.morts().is_empty());
    }

    #[test]
    fn tuer_une_session_inconnue_ne_fait_rien() {
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.tuer(&IdSession("w-jamais-lancee".into()));
        assert!(lanceur.tues.borrow().is_empty());
    }

    #[test]
    fn une_session_tuee_ne_ressort_pas_dans_les_morts() {
        // Elle a déjà été traitée par le chemin `fenetre_disparue` : la
        // signaler morte ferait détruire sa sortie une seconde fois.
        let lanceur = LanceurFactice::default();
        let mut enfants = Enfants::nouveaux(&lanceur);
        enfants.lancer(consigne("w-1", true)).unwrap();
        enfants.tuer(&IdSession("w-1".into()));
        assert!(enfants.morts().is_empty());
    }
}
