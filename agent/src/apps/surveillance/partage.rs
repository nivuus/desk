//! Ce que le fil de surveillance partage avec la boucle de découverte : **deux
//! compteurs monotones et un drapeau d'arrêt**, et rien d'autre.
//!
//! 🔴 CE MODULE N'A AUCUN `cfg`, ET C'EST LE POINT — c'est la forme exacte
//! qu'`apps/installation/partage.rs` a posée, et son en-tête en donne la
//! raison : *« le fil d'installation est Windows ; ce qu'il partage avec la
//! découverte ne l'est pas […]. C'est aussi ce qui rend ces deux mécanismes
//! observables sur l'hôte. »* Ici de même : le fil est
//! `ReadDirectoryChangesW`, ce qu'il publie est deux entiers.
//!
//! ⚠️ **DEUX ÉTATS PARTAGÉS DISTINCTS, ET C'EST VOULU.** `installation::partage`
//! porte la demande d'installation ; celui-ci porte la notification de fichier.
//! Les deux n'ont ni la même cause, ni la même sémantique, ni le même
//! consommateur d'écriture — l'un est écrit par une tâche `tokio` à la sortie
//! d'un installeur, l'autre par un fil Windows à chaque complétion d'E/S. Les
//! fusionner ferait un objet qui ment sur les deux.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Les compteurs de la surveillance, partagés par clonage d'`Arc`.
#[derive(Clone, Default)]
pub struct Veille {
    /// 🔴 **MONOTONE, JAMAIS REMIS À ZÉRO.** La boucle compare à la valeur
    /// qu'elle a retenue au tour précédent ; un incrément survenu **pendant**
    /// une réconciliation est donc vu au sondage suivant.
    ///
    /// **Un booléen échangé le perdrait**, et perdrait avec lui exactement la
    /// notification qui compte : celle qui arrive alors qu'on est déjà en train
    /// de lire le disque, c'est-à-dire pendant une installation.
    notifications: Arc<AtomicU64>,
    /// Monotone aussi. C'est lui qui rend le critère ② lisible **sur la ligne
    /// `catalogue reconcilie`**, en plus du `warn!` par occurrence : un compte
    /// cumulé porté par une ligne périodique se lit après coup, là où un `warn!`
    /// isolé se cherche.
    debordements: Arc<AtomicU64>,
    /// Posé une fois pour toutes à l'extinction. Le fil le relit à chaque tour
    /// d'attente, ce qui borne le délai d'arrêt par la durée du `Wait`.
    arret: Arc<AtomicBool>,
}

impl Veille {
    /// Le cumul des notifications reçues depuis le démarrage du fil.
    pub fn notifications(&self) -> u64 {
        self.notifications.load(Ordering::Relaxed)
    }

    /// Le cumul des débordements de tampon depuis le démarrage du fil.
    pub fn debordements(&self) -> u64 {
        self.debordements.load(Ordering::Relaxed)
    }

    /// Quelque chose a bougé sous l'une des racines.
    pub fn signaler(&self) {
        self.notifications.fetch_add(1, Ordering::Relaxed);
    }

    /// Le tampon a débordé : le contenu est perdu, mais **l'événement ne l'est
    /// pas**.
    ///
    /// 🔴 LES DEUX COMPTEURS MONTENT, ET C'EST DÉLIBÉRÉ. Un débordement **EST**
    /// un événement : le tampon déborde, `lpBytesReturned` vaut 0, et **la
    /// complétion se produit quand même**. Le compter comme une notification est
    /// ce qui déclenche la réconciliation qui répare — et une réconciliation
    /// relit le disque ENTIER, donc elle rattrape tout ce que le tampon a jeté.
    ///
    /// 🔴 NE L'INCRÉMENTER QUE DANS `debordements` **OUVRIRAIT** LE CHEMIN DE
    /// PERTE QUE CETTE CONCEPTION EXISTE POUR FERMER : le seul signal disant
    /// « quelque chose a changé » serait consommé par un compteur que personne
    /// ne sonde pour décider, et la réparation attendrait la période.
    pub fn signaler_debordement(&self) {
        self.debordements.fetch_add(1, Ordering::Relaxed);
        self.notifications.fetch_add(1, Ordering::Relaxed);
    }

    /// Demande l'arrêt du fil.
    ///
    /// 🔴 **CETTE MÉTHODE N'A AUCUN APPELANT DE PRODUCTION, ET C'EST DÉCLARÉ
    /// PLUTÔT QUE DISSIMULÉ** — elle est l'unique avertissement `dead_code` que
    /// G4 ajoute aux vingt-deux du dépôt, et le retirer par commodité
    /// masquerait un fait au lieu de le régler.
    ///
    /// **Le mécanisme, lui, est VIVANT** : `fil::boucler` relit `arretee()` à
    /// chaque tour d'attente, donc au plus une seconde après qu'elle serait
    /// posée. Ce qui manque est son DÉCLENCHEUR, et il manque pour une raison
    /// qui dépasse ce sous-bloc : **l'agent n'a aucun chemin d'extinction
    /// propre** — les fils de découverte et d'installation ne sont pas arrêtés
    /// davantage, et `CLAUDE.md` écrit depuis le sous-bloc D1 que ce chemin
    /// « n'a toujours jamais été exercé ».
    ///
    /// ⚠️ La construire quand même est un choix, et l'alternative était de
    /// laisser `fil::boucler` sans condition de sortie. Un fil qui ne PEUT pas
    /// s'arrêter est un fil qu'on ne saura pas arrêter le jour où le chemin
    /// existera ; celui-ci attend son appelant, et `Drop for Racine` ferme déjà
    /// les handles quel que soit le chemin de sortie.
    pub fn arreter(&self) {
        self.arret.store(true, Ordering::Relaxed);
    }

    /// L'arrêt a-t-il été demandé ?
    pub fn arretee(&self) -> bool {
        self.arret.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_veille_neuve_est_a_zero_et_n_est_pas_arretee() {
        let v = Veille::default();
        assert_eq!(v.notifications(), 0);
        assert_eq!(v.debordements(), 0);
        assert!(!v.arretee());
    }

    /// 🔴 MONOTONE : la boucle compare à la valeur qu'elle a RETENUE, jamais à
    /// zéro. Un compteur qui se remettrait à zéro à la lecture perdrait toute
    /// notification arrivée pendant une réconciliation — c'est-à-dire pendant
    /// une installation, le seul moment où elles arrivent en rafale.
    #[test]
    fn les_notifications_sont_monotones_et_la_lecture_ne_consomme_rien() {
        let v = Veille::default();
        v.signaler();
        v.signaler();
        assert_eq!(v.notifications(), 2);
        assert_eq!(v.notifications(), 2, "lire ne doit RIEN consommer");
        v.signaler();
        assert_eq!(v.notifications(), 3);
    }

    /// 🔴 UN DÉBORDEMENT EST UN ÉVÉNEMENT. Si seul `debordements` montait, rien
    /// ne déclencherait la réconciliation qui répare, et le tampon jeté serait
    /// une perte au lieu d'un retard.
    #[test]
    fn un_debordement_monte_les_deux_compteurs() {
        let v = Veille::default();
        v.signaler_debordement();
        assert_eq!(v.debordements(), 1);
        assert_eq!(
            v.notifications(),
            1,
            "un débordement DOIT aussi déclencher : c'est ce qui ferme le chemin de perte"
        );
        v.signaler();
        assert_eq!(v.notifications(), 2);
        assert_eq!(v.debordements(), 1, "une notification simple n'est pas un débordement");
    }

    #[test]
    fn l_arret_se_voit_de_tous_les_clones() {
        let v = Veille::default();
        let jumelle = v.clone();
        assert!(!jumelle.arretee());
        v.arreter();
        assert!(jumelle.arretee(), "les clones partagent l'Arc, pas une copie");
        // Et les compteurs aussi : c'est ce qui permet au fil d'écrire et à la
        // boucle de lire sans qu'aucun canal ne les relie.
        jumelle.signaler();
        assert_eq!(v.notifications(), 1);
    }
}
